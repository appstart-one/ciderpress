// VoiceMemoLiberator - Voice memo transcription and management tool
// Copyright (C) 2026 APPSTART LLC
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
// use rayon::prelude::*; // Disabled for now due to SQLite thread safety
use chrono::Utc;
use simple_whisper::{WhisperBuilder, Event};
use tokio_stream::StreamExt;
use std::env;

use super::config::Config;
use super::database::Database;
use super::logging;
use super::models::{Transcript, TranscriptionProgress};

// Global transcription progress state
lazy_static::lazy_static! {
    static ref TRANSCRIPTION_PROGRESS: Arc<Mutex<Option<TranscriptionProgress>>> = Arc::new(Mutex::new(None));
    static ref TRANSCRIPTION_START_TIME: Arc<Mutex<Option<std::time::Instant>>> = Arc::new(Mutex::new(None));
    static ref CURRENT_SLICE_START_TIME: Arc<Mutex<Option<std::time::Instant>>> = Arc::new(Mutex::new(None));
}

/// Get the current transcription progress
pub fn get_transcription_progress() -> Option<TranscriptionProgress> {
    let mut progress = TRANSCRIPTION_PROGRESS.lock().unwrap().clone();

    // Update elapsed time if transcription is active
    if let Some(ref mut p) = progress {
        if p.is_active {
            if let Some(start_time) = *TRANSCRIPTION_START_TIME.lock().unwrap() {
                p.elapsed_seconds = start_time.elapsed().as_secs() as u32;
            }
            // Update current slice elapsed time
            if let Some(slice_start_time) = *CURRENT_SLICE_START_TIME.lock().unwrap() {
                p.current_slice_elapsed_seconds = slice_start_time.elapsed().as_secs() as u32;
            }
        }
    }

    progress
}

/// Initialize transcription progress tracking
pub fn init_transcription_progress(total_slices: u32, estimated_total_seconds: u32, bytes_per_second_rate: f64) {
    let mut progress = TRANSCRIPTION_PROGRESS.lock().unwrap();
    *progress = Some(TranscriptionProgress {
        total_slices,
        completed_slices: 0,
        failed_slices: 0,
        current_slice_id: None,
        current_slice_name: None,
        current_step: "Initializing...".to_string(),
        estimated_total_seconds,
        elapsed_seconds: 0,
        is_active: true,
        current_slice_elapsed_seconds: 0,
        current_slice_estimated_seconds: 0,
        current_slice_file_size: 0,
        bytes_per_second_rate,
    });

    let mut start_time = TRANSCRIPTION_START_TIME.lock().unwrap();
    *start_time = Some(std::time::Instant::now());

    // Clear current slice start time
    let mut slice_start = CURRENT_SLICE_START_TIME.lock().unwrap();
    *slice_start = None;
}

/// Initialize transcription progress with logging
pub fn init_transcription_progress_with_logging(
    slice_ids: &[i64],
    total_slices: u32,
    estimated_total_seconds: u32,
    bytes_per_second_rate: f64,
    model_name: &str,
) {
    init_transcription_progress(total_slices, estimated_total_seconds, bytes_per_second_rate);

    // Log transcription start to JSON log
    logging::log_transcription_start(slice_ids, model_name, estimated_total_seconds);
}

/// Start tracking a new slice being transcribed
pub fn start_current_slice(slice_id: i64, slice_name: String, file_size: i64, audio_duration_seconds: Option<f64>) {
    // Calculate estimated time: 35 seconds of processing per 10 minutes of audio
    let estimated_seconds = if let Some(duration) = audio_duration_seconds {
        std::cmp::max(1, (duration / 600.0 * 35.0).ceil() as u32)
    } else {
        // Fallback: rough estimate from file size (~1MB per minute of audio)
        let audio_minutes = file_size as f64 / 1_048_576.0;
        std::cmp::max(1, (audio_minutes / 10.0 * 35.0).ceil() as u32)
    };

    let mut progress = TRANSCRIPTION_PROGRESS.lock().unwrap();
    if let Some(ref mut p) = *progress {
        p.current_slice_id = Some(slice_id);
        p.current_slice_name = Some(slice_name);
        p.current_slice_file_size = file_size;
        p.current_slice_estimated_seconds = estimated_seconds;
        p.current_slice_elapsed_seconds = 0;
        p.current_step = "Transcribing audio...".to_string();
    }

    // Start the current slice timer
    let mut slice_start = CURRENT_SLICE_START_TIME.lock().unwrap();
    *slice_start = Some(std::time::Instant::now());
}

/// Update the current progress state
fn update_transcription_progress(
    current_slice_id: Option<i64>,
    current_slice_name: Option<String>,
    current_step: &str,
) {
    let mut progress = TRANSCRIPTION_PROGRESS.lock().unwrap();
    if let Some(ref mut p) = *progress {
        p.current_slice_id = current_slice_id;
        p.current_slice_name = current_slice_name;
        p.current_step = current_step.to_string();

        // Update elapsed time
        if let Some(start_time) = *TRANSCRIPTION_START_TIME.lock().unwrap() {
            p.elapsed_seconds = start_time.elapsed().as_secs() as u32;
        }
    }
}

/// Mark a slice as completed
pub fn mark_slice_completed() {
    let mut progress = TRANSCRIPTION_PROGRESS.lock().unwrap();
    if let Some(ref mut p) = *progress {
        p.completed_slices += 1;
    }
}

/// Mark a slice as failed
pub fn mark_slice_failed() {
    let mut progress = TRANSCRIPTION_PROGRESS.lock().unwrap();
    if let Some(ref mut p) = *progress {
        p.failed_slices += 1;
    }
}

/// Clear the transcription progress (mark as complete)
pub fn clear_transcription_progress() {
    let progress_data = {
        let mut progress = TRANSCRIPTION_PROGRESS.lock().unwrap();
        if let Some(ref mut p) = *progress {
            p.is_active = false;
            p.current_step = "Complete".to_string();
            Some((p.total_slices, p.completed_slices, p.failed_slices))
        } else {
            None
        }
    };

    // Log transcription completion
    if let Some((total, completed, failed)) = progress_data {
        let elapsed = TRANSCRIPTION_START_TIME.lock().unwrap()
            .map(|start| start.elapsed().as_secs_f64())
            .unwrap_or(0.0);

        logging::log_transcription_complete(total, completed, failed, elapsed);
    }
    // Keep the final state for a moment so UI can show completion
    // It will be cleared on the next transcription start
}

pub struct TranscriptionEngine<'a> {
    config: &'a Config,
    db: &'a Database,
}

#[allow(dead_code)]
impl<'a> TranscriptionEngine<'a> {
    pub fn new(config: &'a Config, db: &'a Database) -> Self {
        Self { 
            config, 
            db,
        }
    }

    pub fn transcribe_recording(&self, recording_id: i64) -> Result<()> {
        self.transcribe_recordings(vec![recording_id])
    }

    pub async fn transcribe_slices(&self, slice_ids: Vec<i64>) -> Result<()> {
        // For now, process sequentially to avoid thread safety issues with SQLite
        for slice_id in slice_ids {
            if let Err(e) = self.transcribe_single_slice(slice_id).await {
                tracing::error!("Failed to transcribe slice {}: {}", slice_id, e);
            }
        }
        Ok(())
    }

    pub fn transcribe_recordings(&self, recording_ids: Vec<i64>) -> Result<()> {
        // For now, process sequentially to avoid thread safety issues with SQLite
        // TODO: Implement proper thread-safe database access or use a connection pool
        for recording_id in recording_ids {
            if let Err(e) = self.transcribe_single(recording_id) {
                tracing::error!("Failed to transcribe recording {}: {}", recording_id, e);
            }
        }
        Ok(())
    }

    fn transcribe_single(&self, recording_id: i64) -> Result<()> {
        // Get recording from database
        let recording = self.db.list_recordings(None, None)?
            .into_iter()
            .find(|r| r.recording.id == Some(recording_id))
            .context("Recording not found")?;

        let audio_path = recording.recording.copied_path
            .as_ref()
            .context("Recording has no copied path")?;

        if !PathBuf::from(audio_path).exists() {
            anyhow::bail!("Audio file does not exist: {}", audio_path);
        }

        // Create transcript record
        let started_at = Utc::now().timestamp();
        let mut transcript = Transcript {
            id: None,
            recording_id,
            model: self.config.model_name.clone(),
            started_at: Some(started_at),
            finished_at: None,
            word_count: None,
            text_path: None,
            success: false,
            error_message: None,
        };

        let transcript_id = self.db.insert_transcript(&transcript)?;

        // Prepare transcript file path
        let transcript_filename = format!("{}.txt", recording_id);
        let transcript_path = self.config.transcript_dir().join(&transcript_filename);
        fs::create_dir_all(self.config.transcript_dir())?;

        // TODO: Replace this with actual simple-whisper integration
        // For now, create a placeholder transcript
        let transcribed_text = self.mock_transcribe(audio_path)?;
        
        // Save transcript to file
        fs::write(&transcript_path, &transcribed_text)?;

        // Update transcript record
        let finished_at = Utc::now().timestamp();
        let word_count = transcribed_text.split_whitespace().count() as i32;
        
        transcript.finished_at = Some(finished_at);
        transcript.word_count = Some(word_count);
        transcript.text_path = Some(transcript_path.to_string_lossy().to_string());
        transcript.success = true;
        transcript.error_message = None;

        self.db.update_transcript(transcript_id, &transcript)?;

        tracing::info!("Successfully transcribed recording {} ({} words)", recording_id, word_count);
        Ok(())
    }

    async fn transcribe_single_slice(&self, slice_id: i64) -> Result<()> {
        // Get slice from database
        let slices = self.db.list_all_slices()?;
        let slice = slices
            .into_iter()
            .find(|s| s.id == Some(slice_id))
            .context("Slice not found")?;

        // Construct audio path from slice filename
        let audio_path = self.config.audio_dir().join(&slice.original_audio_file_name);
        
        if !audio_path.exists() {
            anyhow::bail!("Audio file does not exist: {}", audio_path.display());
        }

        tracing::info!("Starting transcription of slice {} ({})", slice_id, slice.original_audio_file_name);

        // Perform transcription
        let started_at = chrono::Utc::now();
        let transcribed_text = self.async_transcribe(audio_path.to_str().unwrap()).await?;
        let finished_at = chrono::Utc::now();
        
        let transcription_time_taken = (finished_at - started_at).num_seconds() as i32;
        let word_count = transcribed_text.split_whitespace().count() as i32;

        // Update slice record with transcription results
        self.db.update_slice_transcription(
            slice_id,
            &transcribed_text,
            transcription_time_taken,
            word_count,
            &self.config.model_name,
        )?;

        tracing::info!("Successfully transcribed slice {} ({} words in {}s)",
                      slice_id, word_count, transcription_time_taken);
        Ok(())
    }

    pub fn transcribe_slice_sync(&self, slice_id: i64) -> Result<()> {
        // Get slice from database
        let slices = self.db.list_all_slices()?;
        let slice = slices
            .into_iter()
            .find(|s| s.id == Some(slice_id))
            .context("Slice not found")?;

        // Construct audio path from slice filename
        let audio_path = self.config.audio_dir().join(&slice.original_audio_file_name);

        if !audio_path.exists() {
            anyhow::bail!("Audio file does not exist: {}", audio_path.display());
        }

        tracing::info!("Starting transcription of slice {} ({})", slice_id, slice.original_audio_file_name);

        // Start tracking this slice with its audio duration for progress calculation
        start_current_slice(
            slice_id,
            slice.original_audio_file_name.clone(),
            slice.audio_file_size,
            slice.audio_time_length_seconds,
        );

        // Perform transcription using the blocking version
        let started_at = chrono::Utc::now();
        let transcribed_text = self.sync_transcribe(audio_path.to_str().unwrap())?;
        let finished_at = chrono::Utc::now();

        let transcription_time_taken = (finished_at - started_at).num_seconds() as i32;
        let word_count = transcribed_text.split_whitespace().count() as i32;

        // Update progress: saving results
        update_transcription_progress(
            Some(slice_id),
            Some(slice.original_audio_file_name.clone()),
            "Saving transcription...",
        );

        // Update slice record with transcription results
        self.db.update_slice_transcription(
            slice_id,
            &transcribed_text,
            transcription_time_taken,
            word_count,
            &self.config.model_name,
        )?;

        // Log to JSON log
        logging::log_transcription_slice(
            slice_id,
            &slice.original_audio_file_name,
            "success",
            Some(transcription_time_taken as f64),
            Some(word_count as u32),
            None,
        );

        tracing::info!("Successfully transcribed slice {} ({} words in {}s)",
                      slice_id, word_count, transcription_time_taken);
        Ok(())
    }

    pub async fn transcribe_slice_async(&self, slice_id: i64) -> Result<()> {
        // Get slice from database
        let slices = self.db.list_all_slices()?;
        let slice = slices
            .into_iter()
            .find(|s| s.id == Some(slice_id))
            .context("Slice not found")?;

        // Construct audio path from slice filename
        let audio_path = self.config.audio_dir().join(&slice.original_audio_file_name);
        
        if !audio_path.exists() {
            anyhow::bail!("Audio file does not exist: {}", audio_path.display());
        }

        tracing::info!("Starting transcription of slice {} ({})", slice_id, slice.original_audio_file_name);

        // Perform transcription using the async version
        let started_at = chrono::Utc::now();
        let transcription = self.async_transcribe(audio_path.to_str().unwrap()).await?;
        let ended_at = chrono::Utc::now();
        
        let time_taken = (ended_at - started_at).num_seconds();
        let word_count = transcription.split_whitespace().count();
        
        tracing::info!("Transcription completed for slice {} in {} seconds with {} words", 
                      slice_id, time_taken, word_count);

        // Update the slice in the database
        self.db.update_slice_transcription(slice_id, &transcription, time_taken as i32, word_count as i32, &self.config.model_name)?;

        Ok(())
    }

    // Replace mock transcription with actual simple-whisper integration
    fn mock_transcribe(&self, audio_path: &str) -> Result<String> {
        // Convert M4A to WAV if needed
        let transcription_path = if audio_path.ends_with(".m4a") {
            self.convert_m4a_to_wav(audio_path)?
        } else {
            audio_path.to_string()
        };
        
        // Use tokio runtime to handle the async transcription
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.real_transcribe(&transcription_path))
    }

    async fn real_transcribe(&self, audio_path: &str) -> Result<String> {
        tracing::info!("Starting transcription of {} with model {}", audio_path, self.config.model_name);
        
        // Parse the model name to get the appropriate Model enum
        let model = self.parse_model_name(&self.config.model_name)?;
        
        // Create the Whisper instance using the builder
        let whisper = WhisperBuilder::default()
            .model(model)
            .language(simple_whisper::Language::English)  // Use the Language enum
            .build()
            .context("Failed to build Whisper instance")?;
        
        // Start transcription stream
        let mut stream = whisper.transcribe(audio_path);
        let mut transcription_segments = Vec::new();
        
        // Collect all transcription segments
        while let Some(event_result) = stream.next().await {
            match event_result {
                Ok(Event::Segment { transcription, .. }) => {
                    transcription_segments.push(transcription);
                }
                Ok(Event::DownloadStarted { file }) => {
                    tracing::info!("Downloading model file: {}", file);
                }
                Ok(Event::DownloadCompleted { file }) => {
                    tracing::info!("Downloaded model file: {}", file);
                }
                Ok(Event::DownloadProgress { file, percentage, .. }) => {
                    tracing::debug!("Download progress for {}: {:.1}%", file, percentage);
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("Transcription error: {}", e));
                }
            }
        }
        
        let full_transcription = transcription_segments.join(" ");
        tracing::info!("Transcription completed successfully");
        Ok(full_transcription)
    }

    fn parse_model_name(&self, model_name: &str) -> Result<simple_whisper::Model> {
        use simple_whisper::Model;
        
        match model_name {
            "tiny" => Ok(Model::Tiny),
            "tiny.en" => Ok(Model::TinyEn),
            "base" => Ok(Model::Base),
            "base.en" => Ok(Model::BaseEn),
            "small" => Ok(Model::Small),
            "small.en" => Ok(Model::SmallEn),
            "medium" => Ok(Model::Medium),
            "medium.en" => Ok(Model::MediumEn),
            "large" => Ok(Model::Large),
            "large-v1" => Ok(Model::Large),
            "large-v2" => Ok(Model::LargeV2),
            "large-v3" => Ok(Model::LargeV3),
            "large-v3-turbo" => Ok(Model::LargeV3Turbo),
            _ => Err(anyhow::anyhow!("Unsupported model: {}", model_name)),
        }
    }

    /// Convert M4A file to WAV format (16 kHz mono PCM S16LE) using ffmpeg-next library
    fn convert_m4a_to_wav(&self, m4a_path: &str) -> Result<String> {
        use ffmpeg_next::{format, codec, software, util::frame::audio::Audio, ChannelLayout};

        let m4a_pathbuf = PathBuf::from(m4a_path);
        let wav_path = m4a_pathbuf.with_extension("wav");
        let wav_path_str = wav_path.to_str().context("Invalid WAV path")?;

        tracing::info!("Converting {} to {}", m4a_path, wav_path.display());

        // Open input
        let mut ictx = format::input(m4a_path)
            .with_context(|| format!("Failed to open input: {}", m4a_path))?;

        let input_stream = ictx.streams().best(ffmpeg_next::media::Type::Audio)
            .context("No audio stream found in input")?;
        let input_stream_index = input_stream.index();
        let input_time_base = input_stream.time_base();

        // Create decoder
        let decoder_context = codec::context::Context::from_parameters(input_stream.parameters())
            .context("Failed to create decoder context")?;
        let mut decoder = decoder_context.decoder().audio()
            .context("Failed to open audio decoder")?;

        let src_rate = decoder.rate();
        let src_format = decoder.format();
        let src_channel_layout = if decoder.channel_layout().is_empty() {
            ChannelLayout::MONO
        } else {
            decoder.channel_layout()
        };

        // Set up resampler: convert to 16kHz mono S16
        let dst_rate = 16000u32;
        let dst_format = format::Sample::I16(format::sample::Type::Packed);
        let dst_channel_layout = ChannelLayout::MONO;

        let mut resampler = software::resampling::Context::get(
            src_format, src_channel_layout, src_rate,
            dst_format, dst_channel_layout, dst_rate,
        ).context("Failed to create resampler")?;

        // Open output
        let mut octx = format::output(wav_path_str)
            .with_context(|| format!("Failed to create output: {}", wav_path_str))?;

        let global_header = octx.format().flags().contains(format::Flags::GLOBAL_HEADER);

        // Add output stream with PCM S16LE encoder
        let codec = ffmpeg_next::encoder::find(codec::Id::PCM_S16LE)
            .context("PCM_S16LE encoder not found")?;
        let mut output_stream = octx.add_stream(codec)
            .context("Failed to add output stream")?;

        let encoder_context = codec::context::Context::from_parameters(output_stream.parameters())
            .context("Failed to create encoder context")?;

        let mut encoder = encoder_context.encoder().audio()
            .context("Failed to open audio encoder")?;

        encoder.set_rate(dst_rate as i32);
        encoder.set_channel_layout(dst_channel_layout);
        encoder.set_format(dst_format);
        encoder.set_time_base((1, dst_rate as i32));

        if global_header {
            encoder.set_flags(codec::Flags::GLOBAL_HEADER);
        }

        let mut encoder = encoder.open_as(codec)
            .context("Failed to open PCM encoder")?;

        output_stream.set_parameters(&encoder);

        octx.write_header().context("Failed to write output header")?;

        let output_time_base = octx.stream(0).unwrap().time_base();

        // Decode → resample → encode loop
        let mut decoded_frame = Audio::empty();

        for (stream, packet) in ictx.packets() {
            if stream.index() != input_stream_index {
                continue;
            }
            decoder.send_packet(&packet)?;
            while decoder.receive_frame(&mut decoded_frame).is_ok() {
                let mut resampled = Audio::empty();
                resampler.run(&decoded_frame, &mut resampled)?;
                if resampled.samples() > 0 {
                    Self::encode_and_write(&mut encoder, &resampled, &mut octx, input_time_base, output_time_base)?;
                }
            }
        }

        // Flush decoder
        decoder.send_eof()?;
        while decoder.receive_frame(&mut decoded_frame).is_ok() {
            let mut resampled = Audio::empty();
            resampler.run(&decoded_frame, &mut resampled)?;
            if resampled.samples() > 0 {
                Self::encode_and_write(&mut encoder, &resampled, &mut octx, input_time_base, output_time_base)?;
            }
        }

        // Flush resampler
        {
            let mut resampled = Audio::empty();
            if resampler.flush(&mut resampled).is_ok() && resampled.samples() > 0 {
                Self::encode_and_write(&mut encoder, &resampled, &mut octx, input_time_base, output_time_base)?;
            }
        }

        // Flush encoder
        encoder.send_eof()?;
        let mut encoded_packet = ffmpeg_next::Packet::empty();
        while encoder.receive_packet(&mut encoded_packet).is_ok() {
            encoded_packet.set_stream(0);
            encoded_packet.rescale_ts(input_time_base, output_time_base);
            encoded_packet.write_interleaved(&mut octx)?;
        }

        octx.write_trailer().context("Failed to write output trailer")?;

        if !wav_path.exists() {
            return Err(anyhow::anyhow!("WAV file was not created: {}", wav_path.display()));
        }

        tracing::info!("Successfully converted to WAV: {}", wav_path.display());
        Ok(wav_path.to_string_lossy().to_string())
    }

    /// Helper: encode an audio frame and write to output
    fn encode_and_write(
        encoder: &mut ffmpeg_next::encoder::Audio,
        frame: &ffmpeg_next::util::frame::audio::Audio,
        octx: &mut ffmpeg_next::format::context::Output,
        _input_tb: ffmpeg_next::Rational,
        output_tb: ffmpeg_next::Rational,
    ) -> Result<()> {
        encoder.send_frame(frame)?;
        let mut encoded_packet = ffmpeg_next::Packet::empty();
        while encoder.receive_packet(&mut encoded_packet).is_ok() {
            encoded_packet.set_stream(0);
            encoded_packet.rescale_ts((1, encoder.rate() as i32), output_tb);
            encoded_packet.write_interleaved(octx)?;
        }
        Ok(())
    }

    // Async transcription method that works with Tauri's runtime
    async fn async_transcribe(&self, audio_path: &str) -> Result<String> {
        // Convert M4A to WAV if needed
        let transcription_path = if audio_path.ends_with(".m4a") {
            self.convert_m4a_to_wav(audio_path)?
        } else {
            audio_path.to_string()
        };
        
        // Directly call the async transcription method
        self.real_transcribe(&transcription_path).await
    }

    // Synchronous transcription method for blocking contexts
    fn sync_transcribe(&self, audio_path: &str) -> Result<String> {
        // Convert M4A to WAV if needed
        let transcription_path = if audio_path.ends_with(".m4a") {
            self.convert_m4a_to_wav(audio_path)?
        } else {
            audio_path.to_string()
        };

        // Use the current runtime handle to run the async transcription
        // This works in spawn_blocking context
        let handle = tokio::runtime::Handle::current();
        handle.block_on(self.real_transcribe(&transcription_path))
    }

    /// Extract the first N seconds of audio file and return the path (stream copy, no re-encoding)
    fn extract_audio_segment(&self, audio_path: &str, duration_seconds: u32) -> Result<String> {
        use ffmpeg_next::format;

        let audio_pathbuf = PathBuf::from(audio_path);
        let temp_dir = env::temp_dir();
        let timestamp = chrono::Utc::now().timestamp_millis();
        let temp_filename = format!("temp_{}_{}.m4a",
            audio_pathbuf.file_stem().and_then(|s| s.to_str()).unwrap_or("audio"),
            timestamp
        );
        let temp_audio_path = temp_dir.join(&temp_filename);
        let temp_path_str = temp_audio_path.to_str().context("Invalid temp audio path")?;

        tracing::info!("Extracting first {} seconds from {} to {}",
                      duration_seconds, audio_path, temp_audio_path.display());

        // Open input
        let mut ictx = format::input(audio_path)
            .with_context(|| format!("Failed to open input: {}", audio_path))?;

        let input_stream = ictx.streams().best(ffmpeg_next::media::Type::Audio)
            .context("No audio stream found in input")?;
        let input_stream_index = input_stream.index();
        let input_time_base = input_stream.time_base();

        // Calculate the duration threshold in input stream time_base units
        let duration_limit_ts = (duration_seconds as i64) * input_time_base.1 as i64
            / input_time_base.0 as i64;

        // Open output
        let mut octx = format::output(temp_path_str)
            .with_context(|| format!("Failed to create output: {}", temp_path_str))?;

        {
            let mut output_stream = octx.add_stream(ffmpeg_next::encoder::find(
                ictx.stream(input_stream_index).unwrap().parameters().id(),
            ).context("Encoder not found for stream copy")?)?;
            output_stream.set_parameters(ictx.stream(input_stream_index).unwrap().parameters());
        }

        let output_time_base = octx.stream(0).unwrap().time_base();
        octx.write_header().context("Failed to write output header")?;

        // Copy packets up to the duration limit
        for (stream, mut packet) in ictx.packets() {
            if stream.index() != input_stream_index {
                continue;
            }
            // Check if PTS exceeds our duration limit
            if let Some(pts) = packet.pts() {
                if pts >= duration_limit_ts {
                    break;
                }
            }
            packet.set_stream(0);
            packet.rescale_ts(input_time_base, output_time_base);
            packet.write_interleaved(&mut octx)?;
        }

        octx.write_trailer().context("Failed to write output trailer")?;

        if !temp_audio_path.exists() {
            return Err(anyhow::anyhow!("Temp audio file was not created: {}", temp_audio_path.display()));
        }

        tracing::info!("Successfully extracted audio segment to: {}", temp_audio_path.display());
        Ok(temp_audio_path.to_string_lossy().to_string())
    }

    /// Transcribe the first N seconds of a slice's audio and return text suitable for a filename
    pub fn transcribe_for_name(&self, slice_id: i64, duration_seconds: u32) -> Result<String> {
        // Get slice from database
        let slices = self.db.list_all_slices()?;
        let slice = slices
            .into_iter()
            .find(|s| s.id == Some(slice_id))
            .context("Slice not found")?;

        // Construct audio path from slice filename
        let audio_path = self.config.audio_dir().join(&slice.original_audio_file_name);

        if !audio_path.exists() {
            anyhow::bail!("Audio file does not exist: {}", audio_path.display());
        }

        tracing::info!("Transcribing first {} seconds of slice {} for naming",
                      duration_seconds, slice_id);

        // Extract the first N seconds to a temporary file
        let temp_audio_path = self.extract_audio_segment(audio_path.to_str().unwrap(), duration_seconds)?;

        // Perform transcription
        let transcribed_text = self.sync_transcribe(&temp_audio_path)?;

        // Clean up the temporary file
        if let Err(e) = fs::remove_file(&temp_audio_path) {
            tracing::warn!("Failed to remove temporary audio file {}: {}", temp_audio_path, e);
        }

        // Sanitize the transcription for use as a filename:
        // - Take first 50 characters max
        // - Remove invalid filename characters
        // - Trim whitespace
        let sanitized_name = transcribed_text
            .chars()
            .take(50)
            .collect::<String>()
            .replace(|c: char| matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'), "")
            .trim()
            .to_string();

        // If sanitization resulted in empty string, provide a fallback
        let final_name = if sanitized_name.is_empty() {
            format!("Slice {}", slice_id)
        } else {
            sanitized_name
        };

        tracing::info!("Generated filename from transcription: '{}'", final_name);
        Ok(final_name)
    }
}

#[derive(serde::Serialize)]
pub struct TranscribeProgress {
    pub recording_id: i64,
    pub completed: bool,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;
    
    #[test]
    fn test_word_count() {
        let text = "Hello world, this is a test.";
        let count = text.split_whitespace().count();
        assert_eq!(count, 6);
    }

    #[test]
    fn test_parse_model_name() {
        let config = Config {
            voice_memo_root: "/tmp".to_string(),
            ciderpress_home: "/tmp".to_string(),
            model_name: "base.en".to_string(),
            first_run_complete: false,
        };
        
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = Database::new(&db_path).unwrap();
        
        let engine = TranscriptionEngine::new(&config, &db);
        
        // Test valid model names
        assert!(engine.parse_model_name("tiny").is_ok());
        assert!(engine.parse_model_name("base.en").is_ok());
        assert!(engine.parse_model_name("large-v3").is_ok());
        
        // Test invalid model name
        assert!(engine.parse_model_name("invalid-model").is_err());
    }

    #[test]
    fn test_slice_transcription_path_construction() {
        let temp_dir = TempDir::new().unwrap();
        let audio_dir = temp_dir.path().join("audio");
        fs::create_dir_all(&audio_dir).unwrap();
        
        let config = Config {
            voice_memo_root: "/tmp".to_string(),
            ciderpress_home: temp_dir.path().to_string_lossy().to_string(),
            model_name: "base.en".to_string(),
            first_run_complete: false,
        };
        
        let db_path = temp_dir.path().join("test.db");
        let db = Database::new(&db_path).unwrap();
        
        // Create a test audio file
        let test_filename = "test_audio.m4a";
        let test_audio_path = audio_dir.join(test_filename);
        fs::write(&test_audio_path, b"fake audio data").unwrap();
        
        // Insert a test slice
        let slice = super::super::models::Slice {
            id: None,
            original_audio_file_name: test_filename.to_string(),
            title: None,
            transcribed: false,
            audio_file_size: 100,
            audio_file_type: "m4a".to_string(),
            estimated_time_to_transcribe: 30,
            audio_time_length_seconds: None,
            transcription: None,
            transcription_time_taken: None,
            transcription_word_count: None,
        };

        let slice_id = db.insert_slice(&slice).unwrap();

        let engine = TranscriptionEngine::new(&config, &db);

        // Verify that the audio path construction works
        let expected_path = config.audio_dir().join(test_filename);
        assert!(expected_path.exists(), "Audio file should exist at constructed path");
        
        // Note: We can't test the actual transcription without a real audio file
        // and the simple-whisper model, but we can verify the path logic works
    }

    #[test]
    #[ignore] // This test downloads whisper models, so it's ignored by default
    fn test_m4a_to_wav_transcription_integration() -> Result<()> {
        use tempfile::TempDir;
        
        println!("=== M4A TO WAV TRANSCRIPTION TEST ===");
        
        // Get the current working directory and construct path to test file
        let current_dir = env::current_dir()?;
        println!("Current working directory: {:?}", current_dir);
        
        // Try multiple possible locations for the test file
        let possible_paths = vec![
            current_dir.join("test-audio").join("20250427 162429-5441EC7D.m4a"),
            current_dir.parent().unwrap_or(&current_dir).join("test-audio").join("20250427 162429-5441EC7D.m4a"),
        ];
        
        let mut test_audio_path = None;
        for path in possible_paths {
            println!("Checking for test audio file at: {:?}", path);
            if path.exists() {
                test_audio_path = Some(path);
                break;
            }
        }
        
        let test_audio_path = test_audio_path.ok_or_else(|| {
            anyhow::anyhow!("Test audio file not found. Please ensure '20250427 162429-5441EC7D.m4a' exists in the test-audio directory.")
        })?;
        
        println!("Found test audio file at: {:?}", test_audio_path);
        
        // Create temporary directories for the test
        let temp_dir = TempDir::new()?;
        let transcripts_dir = temp_dir.path().join("transcripts");
        fs::create_dir_all(&transcripts_dir)?;
        
        // Create test config with tiny model
        let config = Config {
            voice_memo_root: "/tmp".to_string(),
            ciderpress_home: temp_dir.path().to_string_lossy().to_string(),
            model_name: "tiny".to_string(),  // Use tiny model for faster testing
            first_run_complete: false,
        };
        
        // Create test database
        let db_path = temp_dir.path().join("test.db");
        let db = Database::new(&db_path)?;
        
        // Create transcription engine
        let engine = TranscriptionEngine::new(&config, &db);
        
        println!("1. Testing M4A to WAV conversion...");
        
        // Test the conversion
        let wav_path = engine.convert_m4a_to_wav(test_audio_path.to_str().unwrap())?;
        println!("   ✓ Converted to WAV: {}", wav_path);
        
        // Verify WAV file exists
        assert!(PathBuf::from(&wav_path).exists(), "WAV file should exist after conversion");
        
        println!("2. Testing transcription of WAV file...");
        
        // Test transcription
        let transcription = engine.mock_transcribe(test_audio_path.to_str().unwrap())?;
        println!("   ✓ Transcription completed");
        println!("   Transcription text: '{}'", transcription);
        
        // Count words
        let word_count = transcription.split_whitespace().count();
        println!("   Word count: {}", word_count);
        
        println!("3. Saving transcription to file...");
        
        // Save transcription to the specified filename
        let transcript_filename = "20250427 162429-5441EC7D-transciption.txt";
        let transcript_path = transcripts_dir.join(transcript_filename);
        fs::write(&transcript_path, &transcription)?;
        println!("   ✓ Saved transcription to: {:?}", transcript_path);
        
        println!("4. Verifying results...");
        
        // Verify the transcription file exists
        assert!(transcript_path.exists(), "Transcription file should exist");
        
        // Verify the transcription contains more than 10 words
        assert!(word_count > 10, "Transcription should contain more than 10 words, but got {}", word_count);
        println!("   ✓ Transcription contains {} words (> 10)", word_count);
        
        // Read back the file to verify content
        let saved_transcription = fs::read_to_string(&transcript_path)?;
        assert_eq!(saved_transcription, transcription, "Saved transcription should match original");
        println!("   ✓ File content verified");
        
        println!("=== TEST COMPLETED SUCCESSFULLY ===");
        println!("Summary:");
        println!("  - M4A file converted to WAV: ✓");
        println!("  - WAV file transcribed: ✓");
        println!("  - Transcription saved to {}: ✓", transcript_filename);
        println!("  - Word count ({}) > 10: ✓", word_count);
        println!("  - All verifications passed: ✓");
        
        Ok(())
    }

    #[test]
    fn test_transcribe_slice_sync_functionality() {
        use tempfile::TempDir;
        
        let temp_dir = TempDir::new().unwrap();
        let audio_dir = temp_dir.path().join("audio");
        fs::create_dir_all(&audio_dir).unwrap();
        
        let config = Config {
            voice_memo_root: "/tmp".to_string(),
            ciderpress_home: temp_dir.path().to_string_lossy().to_string(),
            model_name: "base.en".to_string(),
            first_run_complete: false,
        };
        
        let db_path = temp_dir.path().join("test.db");
        let db = Database::new(&db_path).unwrap();
        
        // Create a test audio file
        let test_filename = "test_sync_audio.m4a";
        let test_audio_path = audio_dir.join(test_filename);
        fs::write(&test_audio_path, b"fake audio data").unwrap();
        
        // Insert a test slice
        let slice = super::super::models::Slice {
            id: None,
            original_audio_file_name: test_filename.to_string(),
            title: None,
            transcribed: false,
            audio_file_size: 100,
            audio_file_type: "m4a".to_string(),
            estimated_time_to_transcribe: 30,
            audio_time_length_seconds: None,
            transcription: None,
            transcription_time_taken: None,
            transcription_word_count: None,
        };

        let slice_id = db.insert_slice(&slice).unwrap();

        let engine = TranscriptionEngine::new(&config, &db);

        // Verify that the transcribe_slice_sync method exists and can be called
        // (This won't actually transcribe without a real audio file, but tests the API)
        let result = engine.transcribe_slice_sync(slice_id);
        
        // It should fail because the audio file isn't real, but that's expected
        assert!(result.is_err());
        println!("transcribe_slice_sync method works correctly (failed as expected with fake audio)");
    }
} 
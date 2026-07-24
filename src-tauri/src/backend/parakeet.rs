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

//! NVIDIA Parakeet TDT transcription support.
//!
//! Parakeet TDT 0.6B (v2 English, v3 multilingual) are NeMo transducer models.
//! We run the k2-fsa/sherpa-onnx int8 ONNX exports through the official
//! `sherpa-onnx` Rust bindings' offline recognizer (`model_type =
//! "nemo_transducer"`).
//!
//! Model archives (`.tar.bz2`) are downloaded from the k2-fsa/sherpa-onnx
//! GitHub releases and extracted under `~/.ciderpress/models/`. Each archive
//! unpacks to a directory containing `encoder.int8.onnx`, `decoder.int8.onnx`,
//! `joiner.int8.onnx` and `tokens.txt`.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// A downloadable Parakeet model definition.
pub struct ParakeetModel {
    /// Config `model_name` string used throughout the app.
    pub name: &'static str,
    /// Release archive filename.
    pub archive: &'static str,
    /// Directory the archive unpacks to (also the on-disk model dir name).
    pub dir: &'static str,
    /// Full download URL.
    pub url: &'static str,
    /// Approximate archive size in bytes (used as a progress fallback).
    pub size_bytes: u64,
}

/// The Parakeet models CiderPress can download and use.
pub const MODELS: &[ParakeetModel] = &[
    ParakeetModel {
        name: "parakeet-tdt-0.6b-v2",
        archive: "sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8.tar.bz2",
        dir: "sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8",
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8.tar.bz2",
        size_bytes: 482_468_385,
    },
    ParakeetModel {
        name: "parakeet-tdt-0.6b-v3",
        archive: "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2",
        dir: "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8",
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2",
        size_bytes: 487_170_055,
    },
];

/// Returns true if `model_name` is a Parakeet model handled by this module.
pub fn is_parakeet(model_name: &str) -> bool {
    model_name.starts_with("parakeet")
}

/// Look up a model definition by config name.
pub fn lookup(model_name: &str) -> Option<&'static ParakeetModel> {
    MODELS.iter().find(|m| m.name == model_name)
}

/// Root directory where Parakeet models are stored: `~/.ciderpress/models`.
///
/// Kept independent of the (user-editable) `ciderpress_home` config so that
/// download, detection and transcription always agree on the location — the
/// same fixed-cache approach the Whisper path uses.
pub fn models_root() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    Ok(home.join(".ciderpress").join("models"))
}

/// On-disk directory for a specific model.
pub fn model_dir(model: &ParakeetModel) -> Result<PathBuf> {
    Ok(models_root()?.join(model.dir))
}

/// Find the first file in `dir` whose name contains `needle` and ends with `ext`.
fn find_file(dir: &Path, needle: &str, ext: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.contains(needle) && name.ends_with(ext) {
                return Some(path);
            }
        }
    }
    None
}

/// Resolve the (encoder, decoder, joiner, tokens) paths for an extracted model.
///
/// Robust to int8 vs non-int8 filenames (e.g. `encoder.int8.onnx`).
fn resolve_model_files(dir: &Path) -> Option<(PathBuf, PathBuf, PathBuf, PathBuf)> {
    let encoder = find_file(dir, "encoder", ".onnx")?;
    let decoder = find_file(dir, "decoder", ".onnx")?;
    let joiner = find_file(dir, "joiner", ".onnx")?;
    let tokens = dir.join("tokens.txt");
    if !tokens.exists() {
        return None;
    }
    Some((encoder, decoder, joiner, tokens))
}

/// Returns true if the model is fully downloaded and usable.
pub fn is_downloaded(model_name: &str) -> bool {
    let Some(model) = lookup(model_name) else {
        return false;
    };
    let Ok(dir) = model_dir(model) else {
        return false;
    };
    dir.is_dir() && resolve_model_files(&dir).is_some()
}

/// Names of all downloaded Parakeet models (for `get_downloaded_models`).
pub fn downloaded_models() -> Vec<String> {
    MODELS
        .iter()
        .filter(|m| is_downloaded(m.name))
        .map(|m| m.name.to_string())
        .collect()
}

/// Download and extract a Parakeet model archive, reporting download progress
/// (0.0..=100.0) via `on_progress`. No-op if already downloaded.
pub async fn download_model<F>(model_name: &str, on_progress: F) -> Result<()>
where
    F: Fn(f32),
{
    let model = lookup(model_name)
        .with_context(|| format!("Unknown Parakeet model: {}", model_name))?;

    if is_downloaded(model_name) {
        on_progress(100.0);
        return Ok(());
    }

    let root = models_root()?;
    std::fs::create_dir_all(&root)
        .with_context(|| format!("Failed to create models dir: {:?}", root))?;

    let archive_path = root.join(model.archive);

    tracing::info!("Downloading Parakeet model {} from {}", model.name, model.url);

    // Stream the archive to disk, emitting progress.
    let response = reqwest::get(model.url)
        .await
        .with_context(|| format!("Failed to GET {}", model.url))?
        .error_for_status()
        .with_context(|| format!("Bad status downloading {}", model.url))?;

    let total = response.content_length().unwrap_or(model.size_bytes);

    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;

    let mut file = tokio::fs::File::create(&archive_path)
        .await
        .with_context(|| format!("Failed to create {:?}", archive_path))?;

    let mut downloaded: u64 = 0;
    let mut last_emitted: f32 = -1.0;
    let mut stream = Box::pin(response.bytes_stream());

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Error while downloading model archive")?;
        file.write_all(&chunk)
            .await
            .context("Failed to write model archive chunk")?;
        downloaded += chunk.len() as u64;

        // Reserve the last 2% for extraction so the popup doesn't sit at 100%.
        let pct = if total > 0 {
            (downloaded as f32 / total as f32 * 98.0).min(98.0)
        } else {
            0.0
        };
        if pct - last_emitted >= 0.5 {
            on_progress(pct);
            last_emitted = pct;
        }
    }
    file.flush().await.context("Failed to flush model archive")?;
    drop(file);

    tracing::info!("Extracting Parakeet model archive {:?}", archive_path);

    // Extraction is blocking/CPU-bound — run it off the async runtime.
    let archive_path_clone = archive_path.clone();
    let root_clone = root.clone();
    tokio::task::spawn_blocking(move || extract_tar_bz2(&archive_path_clone, &root_clone))
        .await
        .context("Extraction task panicked")??;

    // Clean up the archive; ignore failure.
    let _ = std::fs::remove_file(&archive_path);

    if !is_downloaded(model_name) {
        anyhow::bail!(
            "Model {} did not contain expected onnx/tokens files after extraction",
            model.name
        );
    }

    on_progress(100.0);
    tracing::info!("Parakeet model {} ready", model.name);
    Ok(())
}

/// Extract a `.tar.bz2` archive into `dest_dir`.
fn extract_tar_bz2(archive: &Path, dest_dir: &Path) -> Result<()> {
    let file = std::fs::File::open(archive)
        .with_context(|| format!("Failed to open archive {:?}", archive))?;
    let decompressor = bzip2::read::BzDecoder::new(file);
    let mut tar = tar::Archive::new(decompressor);
    tar.unpack(dest_dir)
        .with_context(|| format!("Failed to unpack {:?} into {:?}", archive, dest_dir))?;
    Ok(())
}

/// Transcribe a 16 kHz mono WAV file using a Parakeet model.
///
/// Blocking/CPU-bound; call from a blocking context (e.g. `spawn_blocking`).
///
/// `on_progress`, if provided, is invoked with the exact fraction of the file
/// decoded so far (0.0..=1.0): once with `0.0` before the first chunk, then
/// with `range.end / samples.len()` after each chunk (reaching `1.0` at the end).
pub fn transcribe(
    model_name: &str,
    wav_path: &str,
    on_progress: Option<&(dyn Fn(f32) + Send + Sync)>,
) -> Result<String> {
    use sherpa_onnx::{
        OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig, Wave,
    };

    let model = lookup(model_name)
        .with_context(|| format!("Unknown Parakeet model: {}", model_name))?;
    let dir = model_dir(model)?;
    if !dir.is_dir() {
        anyhow::bail!(
            "Parakeet model {} is not downloaded (missing {:?})",
            model.name,
            dir
        );
    }

    let (encoder, decoder, joiner, tokens) = resolve_model_files(&dir)
        .with_context(|| format!("Model files missing in {:?}", dir))?;

    tracing::info!(
        "Transcribing {} with Parakeet model {}",
        wav_path,
        model.name
    );

    let wave = Wave::read(wav_path)
        .with_context(|| format!("Failed to read WAV file: {}", wav_path))?;

    let mut config = OfflineRecognizerConfig::default();
    config.model_config.transducer = OfflineTransducerModelConfig {
        encoder: Some(encoder.to_string_lossy().into_owned()),
        decoder: Some(decoder.to_string_lossy().into_owned()),
        joiner: Some(joiner.to_string_lossy().into_owned()),
    };
    config.model_config.tokens = Some(tokens.to_string_lossy().into_owned());
    config.model_config.model_type = Some("nemo_transducer".to_string());
    config.model_config.num_threads = 2;
    config.model_config.debug = false;

    let recognizer = OfflineRecognizer::create(&config)
        .context("Failed to create sherpa-onnx offline recognizer for Parakeet")?;

    // Feeding an entire long recording in one shot makes ONNX Runtime fail on
    // oversized encoder inputs, and its C++ exception aborts the process when
    // it crosses the FFI boundary — so long audio must be chunked.
    let sample_rate = wave.sample_rate();
    let samples = wave.samples();
    let chunks = chunk_boundaries(samples, sample_rate as u32);
    let total_samples = samples.len().max(1) as f32;

    // Emit an initial 0.0 so the UI resets to the start of this file.
    if let Some(cb) = on_progress {
        cb(0.0);
    }

    let mut texts: Vec<String> = Vec::with_capacity(chunks.len());
    for (i, range) in chunks.iter().enumerate() {
        tracing::info!(
            "Parakeet chunk {}/{}: {:.1}s–{:.1}s",
            i + 1,
            chunks.len(),
            range.start as f32 / sample_rate as f32,
            range.end as f32 / sample_rate as f32,
        );
        let stream = recognizer.create_stream();
        stream.accept_waveform(sample_rate, &samples[range.clone()]);
        recognizer.decode(&stream);
        if let Some(result) = stream.get_result() {
            let text = result.text.trim().to_string();
            if !text.is_empty() {
                texts.push(text);
            }
        }
        // Exact fraction of the file processed after this chunk.
        if let Some(cb) = on_progress {
            cb(range.end as f32 / total_samples);
        }
    }

    let full_text = texts.join(" ");
    tracing::info!("Parakeet transcription complete ({} chars)", full_text.len());
    Ok(full_text)
}

/// Max samples fed to the recognizer in one shot (60 s at 16 kHz).
const CHUNK_TARGET_SECS: usize = 60;
/// Earliest point a chunk is allowed to end, so the quiet-point search has a
/// window (CHUNK_MIN_SECS..CHUNK_TARGET_SECS) to cut in.
const CHUNK_MIN_SECS: usize = 45;
/// Energy-window size used when searching for the quietest cut point.
const ENERGY_WIN_MS: usize = 100;

/// Split audio into chunk ranges of at most CHUNK_TARGET_SECS, cutting each
/// chunk at the quietest 100 ms window in the last part of its span so cuts
/// land in pauses rather than mid-word. Audio at or under the target length
/// is returned as a single chunk.
fn chunk_boundaries(samples: &[f32], sample_rate: u32) -> Vec<std::ops::Range<usize>> {
    let target = CHUNK_TARGET_SECS * sample_rate as usize;
    let min = CHUNK_MIN_SECS * sample_rate as usize;
    let energy_win = (ENERGY_WIN_MS * sample_rate as usize) / 1000;

    let mut ranges = Vec::new();
    let mut start = 0usize;
    while samples.len() - start > target {
        let cut = quietest_point(&samples[start + min..start + target], energy_win) + start + min;
        ranges.push(start..cut);
        start = cut;
    }
    if start < samples.len() {
        ranges.push(start..samples.len());
    }
    ranges
}

/// Index (relative to `region`) of the center of the lowest-energy window.
fn quietest_point(region: &[f32], win: usize) -> usize {
    if region.len() <= win || win == 0 {
        return region.len() / 2;
    }
    let mut best_start = 0usize;
    let mut best_energy = f32::MAX;
    let mut idx = 0usize;
    while idx + win <= region.len() {
        let energy: f32 = region[idx..idx + win].iter().map(|s| s * s).sum();
        if energy < best_energy {
            best_energy = energy;
            best_start = idx;
        }
        idx += win / 2;
    }
    best_start + win / 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_parakeet() {
        assert!(is_parakeet("parakeet-tdt-0.6b-v2"));
        assert!(is_parakeet("parakeet-tdt-0.6b-v3"));
        assert!(!is_parakeet("large-v3-turbo"));
        assert!(!is_parakeet("base.en"));
    }

    #[test]
    fn test_lookup() {
        assert!(lookup("parakeet-tdt-0.6b-v2").is_some());
        assert!(lookup("parakeet-tdt-0.6b-v3").is_some());
        assert!(lookup("nonexistent").is_none());
    }

    /// End-to-end smoke test: downloads the Parakeet TDT v2 model (~460 MB) and
    /// transcribes a bundled 16 kHz mono sample. Ignored by default.
    ///
    /// Run with:
    ///   cargo test --release parakeet_e2e -- --ignored --nocapture
    #[test]
    #[ignore]
    fn parakeet_e2e_v2() -> Result<()> {
        let model_name = "parakeet-tdt-0.6b-v2";

        // Download (no-op if already present).
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            download_model(model_name, |pct| {
                if (pct as u32) % 10 == 0 {
                    println!("  download progress: {:.0}%", pct);
                }
            })
            .await
        })?;
        assert!(is_downloaded(model_name), "model should be downloaded");

        // Locate the sample WAV (16 kHz mono) in <repo>/test-audio.
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let wav = manifest_dir
            .parent()
            .unwrap()
            .join("test-audio")
            .join("20250427 162429-5441EC7D.wav");
        assert!(wav.exists(), "test wav not found at {:?}", wav);

        let start = std::time::Instant::now();
        let print_progress = |f: f32| println!("  progress: {:.3}", f);
        let text = transcribe(model_name, wav.to_str().unwrap(), Some(&print_progress))?;
        let elapsed = start.elapsed();

        println!("=== Parakeet TDT v2 transcript ===");
        println!("{}", text);
        println!("=== transcribed in {:.2}s ===", elapsed.as_secs_f64());

        assert!(!text.trim().is_empty(), "transcript should not be empty");
        Ok(())
    }

    #[test]
    fn test_chunk_boundaries_short_audio_single_chunk() {
        let sr = 16_000u32;
        let samples = vec![0.1f32; 30 * sr as usize];
        let ranges = chunk_boundaries(&samples, sr);
        assert_eq!(ranges, vec![0..samples.len()]);
    }

    #[test]
    fn test_chunk_boundaries_long_audio_contiguous_and_bounded() {
        let sr = 16_000u32;
        // ~7 minutes of "speech" with a silent gap every 55s for cuts to find.
        let total = 7 * 60 * sr as usize;
        let mut samples = vec![0.5f32; total];
        for gap_start in (55 * sr as usize..total).step_by(55 * sr as usize) {
            let gap_end = (gap_start + sr as usize / 2).min(total);
            samples[gap_start..gap_end].fill(0.0);
        }

        let ranges = chunk_boundaries(&samples, sr);
        assert!(ranges.len() > 1, "long audio must be split");
        assert_eq!(ranges.first().unwrap().start, 0);
        assert_eq!(ranges.last().unwrap().end, total);
        for pair in ranges.windows(2) {
            assert_eq!(pair[0].end, pair[1].start, "chunks must be contiguous");
        }
        for r in &ranges {
            assert!(
                r.end - r.start <= CHUNK_TARGET_SECS * sr as usize,
                "chunk exceeds target length"
            );
            assert!(!r.is_empty());
        }
    }
}

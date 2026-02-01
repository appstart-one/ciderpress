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

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recording {
    pub id: Option<i64>,
    pub apple_id: i64,
    pub created_at: i64,
    pub duration_sec: f64,
    pub title: Option<String>,
    pub original_path: String,
    pub copied_path: Option<String>,
    pub file_size: i64,
    pub mime_type: String,
    pub year: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcript {
    pub id: Option<i64>,
    pub recording_id: i64,
    pub model: String,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub word_count: Option<i32>,
    pub text_path: Option<String>,
    pub success: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingWithTranscript {
    #[serde(flatten)]
    pub recording: Recording,
    pub transcript_count: i32,
    pub has_successful_transcript: bool,
    pub latest_transcript_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stats {
    pub total_files: i64,
    pub total_transcribed: i64,
    pub avg_transcribe_sec_10m: Option<f64>,
    pub total_audio_bytes: i64,
    pub largest_file_bytes: i64,
    pub avg_file_bytes: f64,
    pub count_by_year: Vec<YearCount>,
    pub count_by_audio_length: Vec<AudioLengthBucket>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YearCount {
    pub year: i32,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioLengthBucket {
    pub label: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationSummary {
    pub copied: u32,
    pub skipped: u32,
    pub errors: u32,
    pub total_size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationProgress {
    pub total_recordings: u32,
    pub processed_recordings: u32,
    pub failed_recordings: u32,
    pub current_recording: Option<String>,
    pub current_step: String,
    pub total_size_bytes: u64,
    pub processed_size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Slice {
    pub id: Option<i64>,
    pub original_audio_file_name: String,
    pub title: Option<String>, // Title stored in slices table
    pub transcribed: bool,
    pub audio_file_size: i64,
    pub audio_file_type: String,
    pub estimated_time_to_transcribe: i32, // in seconds
    pub audio_time_length_seconds: Option<f64>, // actual audio duration in seconds
    pub transcription: Option<String>,
    pub transcription_time_taken: Option<i32>, // in seconds
    pub transcription_word_count: Option<i32>,
    pub transcription_model: Option<String>, // whisper model used for transcription
    pub recording_date: Option<i64>, // Unix timestamp of original recording from Apple's ZDATE
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionProgress {
    pub total_slices: u32,
    pub completed_slices: u32,
    pub failed_slices: u32,
    pub current_slice_id: Option<i64>,
    pub current_slice_name: Option<String>,
    pub current_step: String,
    pub estimated_total_seconds: u32,
    pub elapsed_seconds: u32,
    pub is_active: bool,
    // Per-slice progress tracking
    pub current_slice_elapsed_seconds: u32,
    pub current_slice_estimated_seconds: u32,
    pub current_slice_file_size: i64,
    pub bytes_per_second_rate: f64, // Historical transcription speed (bytes transcribed per second of processing time)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub id: Option<i64>,
    pub name: String,
    pub color: String,
    pub keywords: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreMigrationStats {
    // Origin (Apple Voice Memos) stats
    pub origin_total_files: u32,
    pub origin_total_size_bytes: u64,
    pub origin_most_recent_date: Option<String>,

    // Destination (CiderPress) stats
    pub destination_total_files: u32,
    pub destination_most_recent_date: Option<String>,
    pub files_to_migrate: u32,
    pub transcribed_count: u32,
    pub not_transcribed_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub message: String,
    pub kind: String,
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        ApiError {
            message: err.to_string(),
            kind: "AnyhowError".to_string(),
        }
    }
}

impl From<rusqlite::Error> for ApiError {
    fn from(err: rusqlite::Error) -> Self {
        ApiError {
            message: err.to_string(),
            kind: "DatabaseError".to_string(),
        }
    }
}

impl From<std::io::Error> for ApiError {
    fn from(err: std::io::Error) -> Self {
        ApiError {
            message: err.to_string(),
            kind: "IoError".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationLogEntry {
    pub timestamp: String,
    pub message: String,
    pub level: String, // "info", "warn", "error", "success"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDownloadProgress {
    pub model_name: String,
    pub percentage: f32,
    pub status: String, // "started", "progress", "completed", "error"
    pub error_message: Option<String>,
} 
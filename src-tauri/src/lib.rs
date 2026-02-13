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

use std::sync::{Mutex, OnceLock};
use std::path::PathBuf;
use tauri::{State, AppHandle, Emitter, Manager};
use tracing::{info, error};

mod backend;

use backend::{
    config::Config,
    database::Database,
    logging,
    migrate::{MigrationEngine, get_audio_duration},
    transcribe::{TranscriptionEngine, get_transcription_progress as get_transcription_progress_fn},
    stats,
    models::{ApiError, MigrationProgress, TranscriptionProgress, Stats, RecordingWithTranscript, Slice, PreMigrationStats, Label, MigrationLogEntry, ModelDownloadProgress},
};
use walkdir::WalkDir;

// Global app handle for emitting events from anywhere
static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

/// Initialize the global app handle
pub fn init_app_handle(handle: AppHandle) {
    let _ = APP_HANDLE.set(handle);
}

/// Emit a migration log entry to the frontend
pub fn emit_migration_log(message: &str, level: &str) {
    if let Some(handle) = APP_HANDLE.get() {
        let entry = MigrationLogEntry {
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
            message: message.to_string(),
            level: level.to_string(),
        };
        let _ = handle.emit("migration-log", entry);
    }
}

// Application state
pub struct AppState {
    config: Mutex<Config>,
    db: Mutex<Option<Database>>,
}

#[tauri::command]
async fn get_config(state: State<'_, AppState>) -> Result<Config, ApiError> {
    let config = state.config.lock().map_err(|e| ApiError {
        message: format!("Failed to lock config: {}", e),
        kind: "LockError".to_string(),
    })?;
    Ok(config.clone())
}

#[tauri::command]
async fn update_config(state: State<'_, AppState>, new_config: Config) -> Result<(), ApiError> {
    {
        let mut config = state.config.lock().map_err(|e| ApiError {
            message: format!("Failed to lock config: {}", e),
            kind: "LockError".to_string(),
        })?;
        *config = new_config.clone();
    }
    
    new_config.save()?;
    
    // Reinitialize database with new config
    let db_path = new_config.ciderpress_home_path().join("CiderPress-db.sqlite");
    let new_db = Database::new(&db_path)?;
    
    let mut db = state.db.lock().map_err(|e| ApiError {
        message: format!("Failed to lock database: {}", e),
        kind: "LockError".to_string(),
    })?;
    *db = Some(new_db);
    
    Ok(())
}

#[tauri::command]
async fn validate_paths(state: State<'_, AppState>) -> Result<bool, ApiError> {
    let config = state.config.lock().map_err(|e| ApiError {
        message: format!("Failed to lock config: {}", e),
        kind: "LockError".to_string(),
    })?;
    
    match config.validate_voice_memo_root() {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

#[tauri::command]
async fn start_migration(state: State<'_, AppState>) -> Result<(), ApiError> {
    let config = state.config.lock().map_err(|e| ApiError {
        message: format!("Failed to lock config: {}", e),
        kind: "LockError".to_string(),
    })?.clone();
    
    // Spawn the migration in a background task so it doesn't block the UI
    tokio::spawn(async move {
        let migration_engine = MigrationEngine::new(&config);
        if let Err(e) = migration_engine.start_migration() {
            error!("Migration failed: {}", e);
            // Clear progress state on error
            let mut progress = MigrationEngine::get_migration_progress_ref().lock().unwrap();
            *progress = None;
        }
    });
    
    Ok(())
}

#[tauri::command]
async fn get_migration_stats() -> Result<Option<MigrationProgress>, ApiError> {
    Ok(MigrationEngine::get_migration_progress())
}

#[tauri::command]
async fn get_pre_migration_stats(
    state: State<'_, AppState>,
) -> Result<PreMigrationStats, ApiError> {
    let config = state.config.lock().map_err(|e| ApiError {
        message: format!("Failed to lock config: {}", e),
        kind: "LockError".to_string(),
    })?.clone();

    let db_guard = state.db.lock().map_err(|e| ApiError {
        message: format!("Failed to lock database: {}", e),
        kind: "LockError".to_string(),
    })?;

    // --- Origin (Apple Voice Memos) stats ---
    // Count actual .m4a files on disk (consistent with how migration works)
    let voice_memo_root = config.voice_memo_root_path();
    let mut origin_total_files: u32 = 0;
    let mut origin_total_size_bytes: u64 = 0;
    let mut origin_most_recent_date: Option<String> = None;
    let mut most_recent_modified: Option<std::time::SystemTime> = None;

    if voice_memo_root.exists() {
        for entry in WalkDir::new(&voice_memo_root).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                if let Some(ext) = entry.path().extension() {
                    if ext == "m4a" {
                        origin_total_files += 1;
                        if let Ok(metadata) = std::fs::metadata(entry.path()) {
                            origin_total_size_bytes += metadata.len();
                            // Track most recent file
                            if let Ok(modified) = metadata.modified() {
                                if most_recent_modified.is_none() || modified > most_recent_modified.unwrap() {
                                    most_recent_modified = Some(modified);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Convert most recent modified time to string
    if let Some(time) = most_recent_modified {
        if let Ok(duration) = time.duration_since(std::time::UNIX_EPOCH) {
            if let Some(dt) = chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0) {
                origin_most_recent_date = Some(dt.format("%Y-%m-%d %H:%M:%S").to_string());
            }
        }
    }

    // --- Destination (CiderPress) stats ---
    let mut destination_total_files: u32 = 0;
    let mut destination_most_recent_date: Option<String> = None;
    let mut transcribed_count: u32 = 0;
    let mut not_transcribed_count: u32 = 0;
    let mut existing_slice_filenames: std::collections::HashSet<String> = std::collections::HashSet::new();

    if let Some(db) = db_guard.as_ref() {
        // Get slice statistics
        if let Ok(slices) = db.list_all_slices() {
            destination_total_files = slices.len() as u32;
            transcribed_count = slices.iter().filter(|s| s.transcribed).count() as u32;
            not_transcribed_count = destination_total_files - transcribed_count;

            // Collect existing filenames
            for slice in &slices {
                existing_slice_filenames.insert(slice.original_audio_file_name.clone());
            }
        }

        // Get most recent audio file date from the audio directory
        let audio_dir = config.audio_dir();
        if audio_dir.exists() {
            let mut most_recent: Option<std::time::SystemTime> = None;
            if let Ok(entries) = std::fs::read_dir(&audio_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        if let Some(ext) = path.extension() {
                            if ext == "m4a" {
                                if let Ok(metadata) = std::fs::metadata(&path) {
                                    if let Ok(modified) = metadata.modified() {
                                        if most_recent.is_none() || modified > most_recent.unwrap() {
                                            most_recent = Some(modified);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if let Some(time) = most_recent {
                if let Ok(duration) = time.duration_since(std::time::UNIX_EPOCH) {
                    if let Some(dt) = chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0) {
                        destination_most_recent_date = Some(dt.format("%Y-%m-%d %H:%M:%S").to_string());
                    }
                }
            }
        }
    }

    // --- Calculate files to migrate ---
    // Files to migrate = actual .m4a files on disk that are not yet in CiderPress slices table
    // This matches the actual migration logic which scans the filesystem
    let mut files_to_migrate: u32 = 0;
    let voice_memo_root = config.voice_memo_root_path();
    if voice_memo_root.exists() {
        // Use walkdir to recursively scan for .m4a files (same as migration does)
        for entry in WalkDir::new(&voice_memo_root).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                if let Some(ext) = entry.path().extension() {
                    if ext == "m4a" {
                        // Extract just the filename
                        if let Some(filename) = entry.path().file_name().and_then(|n| n.to_str()) {
                            if !existing_slice_filenames.contains(filename) {
                                files_to_migrate += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(PreMigrationStats {
        origin_total_files,
        origin_total_size_bytes,
        origin_most_recent_date,
        destination_total_files,
        destination_most_recent_date,
        files_to_migrate,
        transcribed_count,
        not_transcribed_count,
    })
}

#[tauri::command]
async fn clear_database(state: State<'_, AppState>) -> Result<(), ApiError> {
    let db_guard = state.db.lock().map_err(|e| ApiError {
        message: format!("Failed to lock database: {}", e),
        kind: "LockError".to_string(),
    })?;
    
    let db = db_guard.as_ref().ok_or_else(|| ApiError {
        message: "Database not initialized".to_string(),
        kind: "DatabaseError".to_string(),
    })?;
    
    db.clear_all_slices()?;
    info!("Database cleared successfully");
    Ok(())
}

#[tauri::command]
async fn get_slice_records(state: State<'_, AppState>) -> Result<Vec<Slice>, ApiError> {
    let db_guard = state.db.lock().map_err(|e| ApiError {
        message: format!("Failed to lock database: {}", e),
        kind: "LockError".to_string(),
    })?;
    
    let db = db_guard.as_ref().ok_or_else(|| ApiError {
        message: "Database not initialized".to_string(),
        kind: "DatabaseError".to_string(),
    })?;
    
    let slices = db.list_all_slices()?;
    Ok(slices)
}

#[tauri::command]
async fn get_stats(state: State<'_, AppState>) -> Result<Stats, ApiError> {
    let db_guard = state.db.lock().map_err(|e| ApiError {
        message: format!("Failed to lock database: {}", e),
        kind: "LockError".to_string(),
    })?;
    
    let db = db_guard.as_ref().ok_or_else(|| ApiError {
        message: "Database not initialized".to_string(),
        kind: "DatabaseError".to_string(),
    })?;
    
    let stats = stats::collect_stats(db)?;
    Ok(stats)
}

#[tauri::command]
async fn list_recordings(
    state: State<'_, AppState>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<RecordingWithTranscript>, ApiError> {
    let db_guard = state.db.lock().map_err(|e| ApiError {
        message: format!("Failed to lock database: {}", e),
        kind: "LockError".to_string(),
    })?;
    
    let db = db_guard.as_ref().ok_or_else(|| ApiError {
        message: "Database not initialized".to_string(),
        kind: "DatabaseError".to_string(),
    })?;
    
    let recordings = db.list_recordings(limit, offset)?;
    Ok(recordings)
}

#[tauri::command]
async fn search_recordings(
    state: State<'_, AppState>,
    query: String,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<RecordingWithTranscript>, ApiError> {
    let db_guard = state.db.lock().map_err(|e| ApiError {
        message: format!("Failed to lock database: {}", e),
        kind: "LockError".to_string(),
    })?;
    
    let db = db_guard.as_ref().ok_or_else(|| ApiError {
        message: "Database not initialized".to_string(),
        kind: "DatabaseError".to_string(),
    })?;
    
    let recordings = db.search_recordings(&query, limit, offset)?;
    Ok(recordings)
}

#[tauri::command]
async fn transcribe_many(
    state: State<'_, AppState>,
    recording_ids: Vec<i64>,
) -> Result<(), ApiError> {
    let config = state.config.lock().map_err(|e| ApiError {
        message: format!("Failed to lock config: {}", e),
        kind: "LockError".to_string(),
    })?;
    
    let db_guard = state.db.lock().map_err(|e| ApiError {
        message: format!("Failed to lock database: {}", e),
        kind: "LockError".to_string(),
    })?;
    
    let db = db_guard.as_ref().ok_or_else(|| ApiError {
        message: "Database not initialized".to_string(),
        kind: "DatabaseError".to_string(),
    })?;
    
    let transcription_engine = TranscriptionEngine::new(&config, db);
    transcription_engine.transcribe_recordings(recording_ids)?;
    
    Ok(())
}

#[tauri::command]
#[allow(non_snake_case)]
async fn transcribe_slices(
    state: State<'_, AppState>,
    sliceIds: Vec<i64>,
) -> Result<(), ApiError> {
    // Clone the data we need for the background task
    let config = state.config.lock().map_err(|e| ApiError {
        message: format!("Failed to lock config: {}", e),
        kind: "LockError".to_string(),
    })?.clone();

    let db_guard = state.db.lock().map_err(|e| ApiError {
        message: format!("Failed to lock database: {}", e),
        kind: "LockError".to_string(),
    })?;

    let db = db_guard.as_ref().ok_or_else(|| ApiError {
        message: "Database not initialized".to_string(),
        kind: "DatabaseError".to_string(),
    })?;

    // Get all slices and filter based on skip_already_transcribed setting
    let slices = db.list_all_slices()?;
    let skip_transcribed = config.skip_already_transcribed;

    // Filter slice IDs based on whether we should skip already transcribed
    let filtered_slice_ids: Vec<i64> = if skip_transcribed {
        sliceIds.iter()
            .filter(|id| {
                slices.iter()
                    .find(|s| s.id == Some(**id))
                    .map(|s| !s.transcribed) // Only include if not transcribed
                    .unwrap_or(false)
            })
            .cloned()
            .collect()
    } else {
        sliceIds
    };

    // If all slices were skipped, return early
    if filtered_slice_ids.is_empty() {
        info!("All selected slices are already transcribed, nothing to do");
        return Ok(());
    }

    // Calculate estimated total time for progress tracking
    let estimated_total_seconds: u32 = filtered_slice_ids.iter()
        .filter_map(|id| slices.iter().find(|s| s.id == Some(*id)))
        .map(|s| s.estimated_time_to_transcribe as u32)
        .sum();

    // Clone the database connection for the background task
    let db_path = config.ciderpress_home_path().join("CiderPress-db.sqlite");
    let total_slices = filtered_slice_ids.len() as u32;

    // Clone data for the closure
    let model_name = config.model_name.clone();
    let slice_ids_for_log = filtered_slice_ids.clone();

    // Spawn the transcription work in a blocking thread pool
    tokio::task::spawn_blocking(move || {
        // Create a new database connection for this task
        match Database::new(&db_path) {
            Ok(db) => {
                // Get transcription speed from historical data
                let bytes_per_second_rate = db.get_transcription_speed().unwrap_or(34000.0);

                // Initialize progress tracking with logging
                backend::transcribe::init_transcription_progress_with_logging(
                    &slice_ids_for_log,
                    total_slices,
                    estimated_total_seconds,
                    bytes_per_second_rate,
                    &model_name,
                );

                let transcription_engine = TranscriptionEngine::new(&config, &db);
                for slice_id in filtered_slice_ids {
                    // Use the sync version since we're in a blocking context
                    if let Err(e) = transcription_engine.transcribe_slice_sync(slice_id) {
                        tracing::error!("Failed to transcribe slice {}: {}", slice_id, e);
                        backend::transcribe::mark_slice_failed();
                    } else {
                        backend::transcribe::mark_slice_completed();
                    }
                }
                // Mark transcription as complete
                backend::transcribe::clear_transcription_progress();
            }
            Err(e) => {
                tracing::error!("Failed to create database connection for transcription: {}", e);
                backend::transcribe::clear_transcription_progress();
            }
        }
    });

    // Return immediately so the UI can update
    Ok(())
}

#[tauri::command]
async fn get_transcription_progress() -> Result<Option<TranscriptionProgress>, ApiError> {
    Ok(get_transcription_progress_fn())
}

#[tauri::command]
async fn export_transcribed_text(
    state: State<'_, AppState>,
    slice_ids: Vec<i64>,
) -> Result<String, ApiError> {
    let config = state.config.lock().map_err(|e| ApiError {
        message: format!("Failed to lock config: {}", e),
        kind: "LockError".to_string(),
    })?.clone();

    let db_guard = state.db.lock().map_err(|e| ApiError {
        message: format!("Failed to lock database: {}", e),
        kind: "LockError".to_string(),
    })?;

    let db = db_guard.as_ref().ok_or_else(|| ApiError {
        message: "Database not initialized".to_string(),
        kind: "DatabaseError".to_string(),
    })?;

    // Get all slices
    let all_slices = db.list_all_slices()?;

    // Filter to only the selected slices that have transcriptions, preserving order
    let slices_to_export: Vec<&Slice> = slice_ids
        .iter()
        .filter_map(|id| {
            all_slices.iter().find(|s| s.id == Some(*id) && s.transcription.is_some())
        })
        .collect();

    if slices_to_export.is_empty() {
        return Err(ApiError {
            message: "No transcribed slices found in selection".to_string(),
            kind: "NoDataError".to_string(),
        });
    }

    // Create exports directory
    let exports_dir = config.ciderpress_home_path().join("exports");
    std::fs::create_dir_all(&exports_dir)?;

    // Generate filename with timestamp
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("transcripts_export_{}.txt", timestamp);
    let export_path = exports_dir.join(&filename);

    // Build the export content
    let mut content = String::new();
    let export_date = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    for (i, slice) in slices_to_export.iter().enumerate() {
        if i > 0 {
            content.push_str("\n-------\n\n");
        }

        // Header
        let title = slice.title.as_deref().unwrap_or("Untitled");
        let word_count = slice.transcription_word_count.unwrap_or(0);

        content.push_str(&format!("Title: {}\n", title));
        content.push_str(&format!("Export Date: {}\n", export_date));
        content.push_str(&format!("Word Count: {}\n", word_count));
        content.push_str("\n");

        // Transcription text (strip HTML tags if present)
        if let Some(transcription) = &slice.transcription {
            // Simple HTML tag stripping
            let plain_text = strip_html_tags(transcription);
            content.push_str(&plain_text);
            content.push_str("\n");
        }
    }

    // Write to file
    std::fs::write(&export_path, &content)?;

    // Log export to JSON log
    logging::log_export(
        "transcripts",
        &slice_ids,
        Some(export_path.to_string_lossy().as_ref()),
    );

    info!("Exported {} transcriptions to {:?}", slices_to_export.len(), export_path);

    Ok(export_path.to_string_lossy().to_string())
}

/// Simple HTML tag stripping helper
fn strip_html_tags(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;

    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                // Add space after closing tags that typically end blocks
            }
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }

    // Clean up multiple whitespace and trim
    result
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[tauri::command]
async fn export_audio(
    state: State<'_, AppState>,
    recording_ids: Vec<i64>,
    dest_dir: String,
    _reencode: Option<bool>,
) -> Result<u32, ApiError> {
    let db_guard = state.db.lock().map_err(|e| ApiError {
        message: format!("Failed to lock database: {}", e),
        kind: "LockError".to_string(),
    })?;
    
    let db = db_guard.as_ref().ok_or_else(|| ApiError {
        message: "Database not initialized".to_string(),
        kind: "DatabaseError".to_string(),
    })?;
    
    let recordings = db.list_recordings(None, None)?;
    let dest_path = PathBuf::from(&dest_dir);
    
    std::fs::create_dir_all(&dest_path)?;
    
    let mut exported_count = 0u32;
    
    for recording in recordings {
        if recording_ids.contains(&recording.recording.id.unwrap_or(-1)) {
            if let Some(copied_path) = &recording.recording.copied_path {
                let source = PathBuf::from(copied_path);
                let default_name = format!("{}.m4a", recording.recording.apple_id);
                let filename = source.file_name().unwrap_or_else(|| {
                    std::ffi::OsStr::new(&default_name)
                });
                let dest = dest_path.join(filename);
                
                std::fs::copy(&source, &dest)?;
                exported_count += 1;
            }
        }
    }
    
    info!("Exported {} audio files to {:?}", exported_count, dest_path);
    Ok(exported_count)
}

#[tauri::command]
#[allow(non_snake_case)]
async fn update_slice_name(
    state: State<'_, AppState>,
    sliceId: i64,
    newName: String,
) -> Result<(), ApiError> {
    let db_guard = state.db.lock().map_err(|e| ApiError {
        message: format!("Failed to lock database: {}", e),
        kind: "LockError".to_string(),
    })?;
    
    let db = db_guard.as_ref().ok_or_else(|| ApiError {
        message: "Database not initialized".to_string(),
        kind: "DatabaseError".to_string(),
    })?;
    
    db.update_slice_name(sliceId, &newName).map_err(ApiError::from)
}

#[tauri::command]
async fn update_slice(
    state: State<'_, AppState>,
    slice: Slice,
) -> Result<(), ApiError> {
    let db_guard = state.db.lock().map_err(|e| ApiError {
        message: format!("Failed to lock database: {}", e),
        kind: "LockError".to_string(),
    })?;
    
    let db = db_guard.as_ref().ok_or_else(|| ApiError {
        message: "Database not initialized".to_string(),
        kind: "DatabaseError".to_string(),
    })?;
    
    let slice_id = slice.id.ok_or_else(|| ApiError {
        message: "Slice ID is required for update".to_string(),
        kind: "ValidationError".to_string(),
    })?;
    
    db.update_slice(slice_id, &slice).map_err(ApiError::from)
}

#[tauri::command]
#[allow(non_snake_case)]
async fn update_transcription_model(
    state: State<'_, AppState>,
    modelName: String,
) -> Result<(), ApiError> {
    let mut config = state.config.lock().map_err(|e| ApiError {
        message: format!("Failed to lock config: {}", e),
        kind: "LockError".to_string(),
    })?;
    
    // Validate model name
    let valid_models = [
        "tiny", "tiny.en", "base", "base.en", "small", "small.en",
        "medium", "medium.en", "large", "large-v1", "large-v2", "large-v3"
    ];
    
    if !valid_models.contains(&modelName.as_str()) {
        return Err(ApiError {
            message: format!("Invalid model name: {}", modelName),
            kind: "ValidationError".to_string(),
        });
    }
    
    config.model_name = modelName;
    config.save().map_err(ApiError::from)?;
    
    Ok(())
}

#[tauri::command]
async fn get_available_models() -> Result<Vec<String>, ApiError> {
    let models = vec![
        "tiny".to_string(),
        "tiny.en".to_string(),
        "base".to_string(),
        "base.en".to_string(),
        "small".to_string(),
        "small.en".to_string(),
        "medium".to_string(),
        "medium.en".to_string(),
        "large".to_string(),
        "large-v1".to_string(),
        "large-v2".to_string(),
        "large-v3".to_string(),
        "large-v3-turbo".to_string(),
    ];
    Ok(models)
}

#[tauri::command]
async fn get_downloaded_models() -> Result<Vec<String>, ApiError> {
    let mut downloaded = Vec::new();

    // Get user home directory
    let home = dirs::home_dir().ok_or_else(|| ApiError {
        message: "Could not determine home directory".to_string(),
        kind: "IoError".to_string(),
    })?;

    // Huggingface cache path for whisper.cpp models
    let hf_cache = home.join(".cache/huggingface/hub/models--ggerganov--whisper.cpp");

    // Model name to filename mapping
    let model_files = [
        ("tiny", "ggml-tiny.bin"),
        ("tiny.en", "ggml-tiny.en.bin"),
        ("base", "ggml-base.bin"),
        ("base.en", "ggml-base.en.bin"),
        ("small", "ggml-small.bin"),
        ("small.en", "ggml-small.en.bin"),
        ("medium", "ggml-medium.bin"),
        ("medium.en", "ggml-medium.en.bin"),
        ("large", "ggml-large.bin"),
        ("large-v1", "ggml-large-v1.bin"),
        ("large-v2", "ggml-large-v2.bin"),
        ("large-v3", "ggml-large-v3.bin"),
        ("large-v3-turbo", "ggml-large-v3-turbo.bin"),
    ];

    // Search in snapshots directory for each model file
    if let Ok(snapshots) = std::fs::read_dir(hf_cache.join("snapshots")) {
        for snapshot in snapshots.flatten() {
            let snapshot_path = snapshot.path();
            if snapshot_path.is_dir() {
                for (model_name, filename) in &model_files {
                    let model_path = snapshot_path.join(filename);
                    if model_path.exists() && !downloaded.contains(&model_name.to_string()) {
                        downloaded.push(model_name.to_string());
                    }
                }
            }
        }
    }

    Ok(downloaded)
}

#[tauri::command]
async fn download_whisper_model(model_name: String) -> Result<(), ApiError> {
    use simple_whisper::Model;
    use tokio::sync::mpsc::unbounded_channel;

    // Parse model name to simple_whisper::Model enum
    let model = match model_name.as_str() {
        "tiny" => Model::Tiny,
        "tiny.en" => Model::TinyEn,
        "base" => Model::Base,
        "base.en" => Model::BaseEn,
        "small" => Model::Small,
        "small.en" => Model::SmallEn,
        "medium" => Model::Medium,
        "medium.en" => Model::MediumEn,
        "large" => Model::Large,
        "large-v1" => Model::Large,
        "large-v2" => Model::LargeV2,
        "large-v3" => Model::LargeV3,
        "large-v3-turbo" => Model::LargeV3Turbo,
        _ => {
            return Err(ApiError {
                message: format!("Invalid model name: {}", model_name),
                kind: "ValidationError".to_string(),
            });
        }
    };

    // Check if already downloaded
    if model.cached() {
        // Emit completed event immediately
        if let Some(handle) = APP_HANDLE.get() {
            let progress = ModelDownloadProgress {
                model_name: model_name.clone(),
                percentage: 100.0,
                status: "completed".to_string(),
                error_message: None,
            };
            let _ = handle.emit("model-download-progress", progress);
        }
        return Ok(());
    }

    // Emit started event
    if let Some(handle) = APP_HANDLE.get() {
        let progress = ModelDownloadProgress {
            model_name: model_name.clone(),
            percentage: 0.0,
            status: "started".to_string(),
            error_message: None,
        };
        let _ = handle.emit("model-download-progress", progress);
    }

    // Create channel for progress events
    let (tx, mut rx) = unbounded_channel();
    let model_name_clone = model_name.clone();

    // Spawn task to handle progress events
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            if let Some(handle) = APP_HANDLE.get() {
                let progress = match event {
                    simple_whisper::Event::DownloadStarted { .. } => ModelDownloadProgress {
                        model_name: model_name_clone.clone(),
                        percentage: 0.0,
                        status: "started".to_string(),
                        error_message: None,
                    },
                    simple_whisper::Event::DownloadProgress { percentage, .. } => ModelDownloadProgress {
                        model_name: model_name_clone.clone(),
                        percentage,
                        status: "progress".to_string(),
                        error_message: None,
                    },
                    simple_whisper::Event::DownloadCompleted { .. } => ModelDownloadProgress {
                        model_name: model_name_clone.clone(),
                        percentage: 100.0,
                        status: "completed".to_string(),
                        error_message: None,
                    },
                    _ => continue,
                };
                let _ = handle.emit("model-download-progress", progress);
            }
        }
    });

    // Start download
    match model.download_model_listener(false, tx).await {
        Ok(_) => {
            // Emit final completed event
            if let Some(handle) = APP_HANDLE.get() {
                let progress = ModelDownloadProgress {
                    model_name: model_name.clone(),
                    percentage: 100.0,
                    status: "completed".to_string(),
                    error_message: None,
                };
                let _ = handle.emit("model-download-progress", progress);
            }
            Ok(())
        }
        Err(e) => {
            // Emit error event
            if let Some(handle) = APP_HANDLE.get() {
                let progress = ModelDownloadProgress {
                    model_name: model_name.clone(),
                    percentage: 0.0,
                    status: "error".to_string(),
                    error_message: Some(e.to_string()),
                };
                let _ = handle.emit("model-download-progress", progress);
            }
            Err(ApiError {
                message: format!("Failed to download model: {}", e),
                kind: "DownloadError".to_string(),
            })
        }
    }
}

#[tauri::command]
async fn pick_directory() -> Result<Option<String>, ApiError> {
    // For now, return None - this will be implemented with the dialog plugin
    // TODO: Implement with tauri-plugin-dialog
    Ok(None)
}

#[tauri::command]
async fn get_slice_audio_bytes(
    state: State<'_, AppState>,
    slice_id: i64,
) -> Result<Vec<u8>, ApiError> {
    let config = state.config.lock().map_err(|e| ApiError {
        message: format!("Failed to lock config: {}", e),
        kind: "LockError".to_string(),
    })?;

    let db_guard = state.db.lock().map_err(|e| ApiError {
        message: format!("Failed to lock database: {}", e),
        kind: "LockError".to_string(),
    })?;

    let db = db_guard.as_ref().ok_or_else(|| ApiError {
        message: "Database not initialized".to_string(),
        kind: "DatabaseError".to_string(),
    })?;

    // Get all slices and find the one with matching ID
    let slices = db.list_all_slices()?;
    let slice = slices.iter().find(|s| s.id == Some(slice_id))
        .ok_or_else(|| ApiError {
            message: format!("Slice with ID {} not found", slice_id),
            kind: "NotFoundError".to_string(),
        })?;

    // Construct the full path to the audio file
    let audio_path = config.audio_dir().join(&slice.original_audio_file_name);

    // Verify the file exists
    if !audio_path.exists() {
        return Err(ApiError {
            message: format!("Audio file not found: {}", audio_path.display()),
            kind: "FileNotFoundError".to_string(),
        });
    }

    // Read the file as bytes
    let bytes = std::fs::read(&audio_path).map_err(|e| ApiError {
        message: format!("Failed to read audio file: {}", e),
        kind: "IoError".to_string(),
    })?;

    Ok(bytes)
}

#[tauri::command]
async fn update_slice_names_from_audio(
    state: State<'_, AppState>,
    slice_ids: Vec<i64>,
) -> Result<(), ApiError> {
    // Clone the data we need for the background task
    let config = state.config.lock().map_err(|e| ApiError {
        message: format!("Failed to lock config: {}", e),
        kind: "LockError".to_string(),
    })?.clone();

    let db_guard = state.db.lock().map_err(|e| ApiError {
        message: format!("Failed to lock database: {}", e),
        kind: "LockError".to_string(),
    })?;

    // Verify database is initialized
    db_guard.as_ref().ok_or_else(|| ApiError {
        message: "Database not initialized".to_string(),
        kind: "DatabaseError".to_string(),
    })?;

    // Clone the database connection for the background task
    let db_path = config.ciderpress_home_path().join("CiderPress-db.sqlite");

    // Spawn the work in a blocking thread pool
    tokio::task::spawn_blocking(move || {
        // Create a new database connection for this task
        match Database::new(&db_path) {
            Ok(db) => {
                let transcription_engine = TranscriptionEngine::new(&config, &db);
                for slice_id in slice_ids {
                    match transcription_engine.transcribe_for_name(slice_id, 15) {
                        Ok(new_name) => {
                            // Update the slice name in the database
                            if let Err(e) = db.update_slice_name(slice_id, &new_name) {
                                tracing::error!("Failed to update slice name for slice {}: {}", slice_id, e);
                            } else {
                                tracing::info!("Updated slice {} name to: {}", slice_id, new_name);
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to transcribe slice {} for naming: {}", slice_id, e);
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to create database connection for name update: {}", e);
            }
        }
    });

    // Return immediately so the UI can update
    Ok(())
}

#[tauri::command]
async fn update_recording_title(
    state: State<'_, AppState>,
    slice_id: i64,
    new_title: String,
) -> Result<(), ApiError> {
    let db_guard = state.db.lock().map_err(|e| ApiError {
        message: format!("Failed to lock database: {}", e),
        kind: "LockError".to_string(),
    })?;

    let db = db_guard.as_ref().ok_or_else(|| ApiError {
        message: "Database not initialized".to_string(),
        kind: "DatabaseError".to_string(),
    })?;

    db.update_recording_title_by_slice(slice_id, &new_title)
        .map_err(ApiError::from)
}

#[tauri::command]
async fn auto_populate_titles(state: State<'_, AppState>) -> Result<u32, ApiError> {
    let db_guard = state.db.lock().map_err(|e| ApiError {
        message: format!("Failed to lock database: {}", e),
        kind: "LockError".to_string(),
    })?;

    let db = db_guard.as_ref().ok_or_else(|| ApiError {
        message: "Database not initialized".to_string(),
        kind: "DatabaseError".to_string(),
    })?;

    let count = db.auto_populate_titles().map_err(ApiError::from)?;
    Ok(count)
}

#[tauri::command]
async fn populate_audio_durations(state: State<'_, AppState>) -> Result<u32, ApiError> {
    let config = state.config.lock().map_err(|e| ApiError {
        message: format!("Failed to lock config: {}", e),
        kind: "LockError".to_string(),
    })?.clone();

    let db_guard = state.db.lock().map_err(|e| ApiError {
        message: format!("Failed to lock database: {}", e),
        kind: "LockError".to_string(),
    })?;

    let db = db_guard.as_ref().ok_or_else(|| ApiError {
        message: "Database not initialized".to_string(),
        kind: "DatabaseError".to_string(),
    })?;

    // Clear any corrupted durations from a prior unit-conversion bug
    match db.clear_corrupt_audio_durations() {
        Ok(cleared) if cleared > 0 => {
            info!("Cleared {} corrupted audio durations for recalculation", cleared);
        }
        Err(e) => {
            error!("Failed to clear corrupt audio durations: {}", e);
        }
        _ => {}
    }

    // Get slices without duration
    let slices_without_duration = db.get_slices_without_duration().map_err(ApiError::from)?;
    let mut updated_count = 0u32;

    for slice in slices_without_duration {
        if let Some(slice_id) = slice.id {
            // Construct the full path to the audio file
            let audio_path = config.audio_dir().join(&slice.original_audio_file_name);

            if audio_path.exists() {
                if let Some(duration) = get_audio_duration(&audio_path) {
                    if let Err(e) = db.update_slice_audio_duration(slice_id, duration) {
                        error!("Failed to update audio duration for slice {}: {}", slice_id, e);
                    } else {
                        updated_count += 1;
                        info!("Updated audio duration for slice {}: {:.2}s", slice_id, duration);
                    }
                }
            }
        }
    }

    Ok(updated_count)
}

#[tauri::command]
async fn backfill_recording_dates(state: State<'_, AppState>) -> Result<u32, ApiError> {
    let db_guard = state.db.lock().map_err(|e| ApiError {
        message: format!("Failed to lock database: {}", e),
        kind: "LockError".to_string(),
    })?;

    let db = db_guard.as_ref().ok_or_else(|| ApiError {
        message: "Database not initialized".to_string(),
        kind: "DatabaseError".to_string(),
    })?;

    let count = db.backfill_recording_dates().map_err(ApiError::from)?;
    if count > 0 {
        info!("Backfilled recording dates for {} slices", count);
    }
    Ok(count)
}

// ==================== NLM (NotebookLM) commands ====================

#[tauri::command]
async fn nlm_get_status() -> Result<backend::nlm::NlmStatus, ApiError> {
    // This is fast (only reads local files, never spawns NLM binary)
    Ok(backend::nlm::get_nlm_status())
}

#[tauri::command]
async fn nlm_authenticate() -> Result<String, ApiError> {
    // Run in blocking thread to avoid freezing async runtime
    tokio::task::spawn_blocking(|| {
        backend::nlm::start_auth()
    }).await.map_err(|e| ApiError {
        message: format!("Task failed: {}", e),
        kind: "TaskError".to_string(),
    })?.map_err(|e| ApiError {
        message: e.to_string(),
        kind: "NlmError".to_string(),
    })
}

#[tauri::command]
async fn nlm_list_notebooks() -> Result<Vec<backend::nlm::NlmNotebook>, ApiError> {
    tokio::task::spawn_blocking(|| {
        backend::nlm::list_notebooks()
    }).await.map_err(|e| ApiError {
        message: format!("Task failed: {}", e),
        kind: "TaskError".to_string(),
    })?.map_err(|e| ApiError {
        message: e.to_string(),
        kind: "NlmError".to_string(),
    })
}

#[tauri::command]
async fn nlm_add_text(
    notebook_id: String,
    text: String,
    title: Option<String>,
) -> Result<String, ApiError> {
    tokio::task::spawn_blocking(move || {
        backend::nlm::add_text_to_notebook(
            &notebook_id,
            &text,
            title.as_deref(),
        )
    }).await.map_err(|e| ApiError {
        message: format!("Task failed: {}", e),
        kind: "TaskError".to_string(),
    })?.map_err(|e| ApiError {
        message: e.to_string(),
        kind: "NlmError".to_string(),
    })
}

#[tauri::command]
async fn nlm_add_audio(
    state: State<'_, AppState>,
    notebook_id: String,
    slice_id: i64,
) -> Result<String, ApiError> {
    // Resolve the audio path while holding locks, then drop them before await
    let audio_path_str = {
        let config = state.config.lock().map_err(|e| ApiError {
            message: format!("Failed to lock config: {}", e),
            kind: "LockError".to_string(),
        })?.clone();

        let db_guard = state.db.lock().map_err(|e| ApiError {
            message: format!("Failed to lock database: {}", e),
            kind: "LockError".to_string(),
        })?;

        let db = db_guard.as_ref().ok_or_else(|| ApiError {
            message: "Database not initialized".to_string(),
            kind: "DatabaseError".to_string(),
        })?;

        let slices = db.list_all_slices()?;
        let slice = slices.iter().find(|s| s.id == Some(slice_id))
            .ok_or_else(|| ApiError {
                message: format!("Slice with ID {} not found", slice_id),
                kind: "NotFoundError".to_string(),
            })?;

        let audio_path = config.audio_dir().join(&slice.original_audio_file_name);
        if !audio_path.exists() {
            return Err(ApiError {
                message: format!("Audio file not found: {}", audio_path.display()),
                kind: "FileNotFoundError".to_string(),
            });
        }
        audio_path.to_string_lossy().to_string()
    };

    tokio::task::spawn_blocking(move || {
        backend::nlm::add_audio_to_notebook(&notebook_id, &audio_path_str)
    }).await.map_err(|e| ApiError {
        message: format!("Task failed: {}", e),
        kind: "TaskError".to_string(),
    })?.map_err(|e| ApiError {
        message: e.to_string(),
        kind: "NlmError".to_string(),
    })
}

#[tauri::command]
async fn nlm_list_profiles() -> Result<Vec<backend::nlm::NlmBrowserProfile>, ApiError> {
    // Reads potentially large Chrome Preferences files, run off the async runtime
    tokio::task::spawn_blocking(|| {
        backend::nlm::list_browser_profiles()
    }).await.map_err(|e| ApiError {
        message: format!("Task failed: {}", e),
        kind: "TaskError".to_string(),
    })
}

#[tauri::command]
async fn nlm_auth_with_profile(profile_name: String) -> Result<String, ApiError> {
    tokio::task::spawn_blocking(move || {
        backend::nlm::auth_with_profile(&profile_name)
    }).await.map_err(|e| ApiError {
        message: format!("Task failed: {}", e),
        kind: "TaskError".to_string(),
    })?.map_err(|e| ApiError {
        message: e.to_string(),
        kind: "NlmError".to_string(),
    })
}

#[tauri::command]
async fn nlm_create_notebook(title: String) -> Result<String, ApiError> {
    tokio::task::spawn_blocking(move || {
        backend::nlm::create_notebook(&title)
    }).await.map_err(|e| ApiError {
        message: format!("Task failed: {}", e),
        kind: "TaskError".to_string(),
    })?.map_err(|e| ApiError {
        message: e.to_string(),
        kind: "NlmError".to_string(),
    })
}

#[tauri::command]
async fn nlm_get_notebook_details(notebook_id: String, title: String) -> Result<backend::nlm::NlmNotebookDetails, ApiError> {
    tokio::task::spawn_blocking(move || {
        backend::nlm::get_notebook_details(&notebook_id, &title)
    }).await.map_err(|e| ApiError {
        message: format!("Task failed: {}", e),
        kind: "TaskError".to_string(),
    })?.map_err(|e| ApiError {
        message: e.to_string(),
        kind: "NlmError".to_string(),
    })
}

// ==================== Label management commands ====================

#[tauri::command]
async fn list_labels(state: State<'_, AppState>) -> Result<Vec<Label>, ApiError> {
    let db_guard = state.db.lock().map_err(|e| ApiError {
        message: format!("Failed to lock database: {}", e),
        kind: "LockError".to_string(),
    })?;

    let db = db_guard.as_ref().ok_or_else(|| ApiError {
        message: "Database not initialized".to_string(),
        kind: "DatabaseError".to_string(),
    })?;

    db.list_labels().map_err(ApiError::from)
}

#[tauri::command]
async fn create_label(state: State<'_, AppState>, label: Label) -> Result<i64, ApiError> {
    let db_guard = state.db.lock().map_err(|e| ApiError {
        message: format!("Failed to lock database: {}", e),
        kind: "LockError".to_string(),
    })?;

    let db = db_guard.as_ref().ok_or_else(|| ApiError {
        message: "Database not initialized".to_string(),
        kind: "DatabaseError".to_string(),
    })?;

    db.create_label(&label).map_err(ApiError::from)
}

#[tauri::command]
async fn update_label(state: State<'_, AppState>, id: i64, label: Label) -> Result<(), ApiError> {
    let db_guard = state.db.lock().map_err(|e| ApiError {
        message: format!("Failed to lock database: {}", e),
        kind: "LockError".to_string(),
    })?;

    let db = db_guard.as_ref().ok_or_else(|| ApiError {
        message: "Database not initialized".to_string(),
        kind: "DatabaseError".to_string(),
    })?;

    db.update_label(id, &label).map_err(ApiError::from)
}

#[tauri::command]
async fn delete_label(state: State<'_, AppState>, id: i64) -> Result<(), ApiError> {
    let db_guard = state.db.lock().map_err(|e| ApiError {
        message: format!("Failed to lock database: {}", e),
        kind: "LockError".to_string(),
    })?;

    let db = db_guard.as_ref().ok_or_else(|| ApiError {
        message: "Database not initialized".to_string(),
        kind: "DatabaseError".to_string(),
    })?;

    db.delete_label(id).map_err(ApiError::from)
}

// ==================== Logging commands ====================

#[derive(serde::Deserialize)]
pub struct LogUserActionRequest {
    pub action_type: String,
    pub screen: Option<String>,
    pub button: Option<String>,
    pub context: Option<String>,
    pub details: Option<serde_json::Value>,
}

#[tauri::command]
async fn log_user_action(request: LogUserActionRequest) -> Result<(), ApiError> {
    match request.action_type.as_str() {
        "navigate" => {
            if let Some(screen) = request.screen {
                logging::log_navigation(&screen);
            }
        }
        "click" => {
            if let Some(button) = request.button {
                logging::log_button_click(&button, request.context.as_deref());
            }
        }
        "settings_change" => {
            if let Some(details) = request.details {
                if let (Some(setting), Some(new_value)) = (
                    details.get("setting").and_then(|s| s.as_str()),
                    details.get("new_value").and_then(|s| s.as_str()),
                ) {
                    let old_value = details.get("old_value").and_then(|s| s.as_str());
                    logging::log_settings_change(setting, old_value, new_value);
                }
            }
        }
        "select_slices" => {
            if let Some(details) = request.details {
                let entry = logging::LogEntry::new(
                    logging::LogEventType::SelectSlices,
                    "user_action",
                    "Slices selected",
                ).with_details(details);
                let _ = logging::log_event(entry);
            }
        }
        _ => {
            // Log as generic info
            logging::log_info(
                "user_action",
                &format!("User action: {}", request.action_type),
                request.details,
            );
        }
    }
    Ok(())
}

#[tauri::command]
async fn get_system_info() -> Result<serde_json::Value, ApiError> {
    let app_version = env!("CARGO_PKG_VERSION").to_string();

    let macos_version = std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "Unknown".to_string());

    Ok(serde_json::json!({
        "app_version": app_version,
        "macos_version": macos_version
    }))
}

// ==================== Slice creation commands ====================

#[tauri::command]
async fn create_text_slice(
    state: State<'_, AppState>,
    title: String,
    content: String,
) -> Result<i64, ApiError> {
    let db_guard = state.db.lock().map_err(|e| ApiError {
        message: format!("Failed to lock database: {}", e),
        kind: "LockError".to_string(),
    })?;

    let db = db_guard.as_ref().ok_or_else(|| ApiError {
        message: "Database not initialized".to_string(),
        kind: "DatabaseError".to_string(),
    })?;

    // Generate a unique filename for this text-based slice
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let unique_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let filename = format!("text_entry_{}_{}.txt", timestamp, unique_id);

    let word_count = content.split_whitespace().count() as i32;

    let slice = Slice {
        id: None,
        original_audio_file_name: filename,
        title: Some(title),
        transcribed: true,
        audio_file_size: content.len() as i64,
        audio_file_type: "text".to_string(),
        estimated_time_to_transcribe: 0,
        audio_time_length_seconds: None,
        transcription: Some(content),
        transcription_time_taken: Some(0),
        transcription_word_count: Some(word_count),
        transcription_model: Some("manual".to_string()),
        recording_date: Some(chrono::Utc::now().timestamp()),
    };

    let id = db.insert_slice(&slice)?;
    info!("Created text slice with ID {}", id);
    Ok(id)
}

#[tauri::command]
async fn import_audio_slice(
    state: State<'_, AppState>,
    file_path: String,
    title: Option<String>,
) -> Result<i64, ApiError> {
    let config = state.config.lock().map_err(|e| ApiError {
        message: format!("Failed to lock config: {}", e),
        kind: "LockError".to_string(),
    })?.clone();

    let db_guard = state.db.lock().map_err(|e| ApiError {
        message: format!("Failed to lock database: {}", e),
        kind: "LockError".to_string(),
    })?;

    let db = db_guard.as_ref().ok_or_else(|| ApiError {
        message: "Database not initialized".to_string(),
        kind: "DatabaseError".to_string(),
    })?;

    let source_path = PathBuf::from(&file_path);
    if !source_path.exists() {
        return Err(ApiError {
            message: format!("File not found: {}", file_path),
            kind: "FileNotFoundError".to_string(),
        });
    }

    let filename = source_path.file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| ApiError {
            message: "Invalid filename".to_string(),
            kind: "ValidationError".to_string(),
        })?
        .to_string();

    // Check if a slice with this filename already exists
    if db.slice_exists(&filename)? {
        return Err(ApiError {
            message: format!("A slice with filename '{}' already exists", filename),
            kind: "DuplicateError".to_string(),
        });
    }

    // Copy audio file to CiderPress audio directory
    let dest_path = config.audio_dir().join(&filename);
    std::fs::copy(&source_path, &dest_path).map_err(|e| ApiError {
        message: format!("Failed to copy audio file: {}", e),
        kind: "IoError".to_string(),
    })?;

    // Get file metadata
    let metadata = std::fs::metadata(&dest_path).map_err(|e| ApiError {
        message: format!("Failed to read file metadata: {}", e),
        kind: "IoError".to_string(),
    })?;

    let file_size = metadata.len() as i64;
    let ext = source_path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("unknown")
        .to_lowercase();

    // Try to get audio duration
    let duration = get_audio_duration(&dest_path);

    // Estimate transcription time (roughly 1 second per 34KB)
    let estimated_time = (file_size / 34000).max(1) as i32;

    let slice_title = title.unwrap_or_else(|| {
        source_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Imported Audio")
            .to_string()
    });

    let slice = Slice {
        id: None,
        original_audio_file_name: filename,
        title: Some(slice_title),
        transcribed: false,
        audio_file_size: file_size,
        audio_file_type: ext,
        estimated_time_to_transcribe: estimated_time,
        audio_time_length_seconds: duration,
        transcription: None,
        transcription_time_taken: None,
        transcription_word_count: None,
        transcription_model: None,
        recording_date: Some(chrono::Utc::now().timestamp()),
    };

    let id = db.insert_slice(&slice)?;
    info!("Imported audio slice with ID {} from {}", id, file_path);
    Ok(id)
}

#[tauri::command]
async fn import_text_file_slice(
    state: State<'_, AppState>,
    file_path: String,
    title: Option<String>,
) -> Result<i64, ApiError> {
    let db_guard = state.db.lock().map_err(|e| ApiError {
        message: format!("Failed to lock database: {}", e),
        kind: "LockError".to_string(),
    })?;

    let db = db_guard.as_ref().ok_or_else(|| ApiError {
        message: "Database not initialized".to_string(),
        kind: "DatabaseError".to_string(),
    })?;

    let source_path = PathBuf::from(&file_path);
    if !source_path.exists() {
        return Err(ApiError {
            message: format!("File not found: {}", file_path),
            kind: "FileNotFoundError".to_string(),
        });
    }

    let content = std::fs::read_to_string(&source_path).map_err(|e| ApiError {
        message: format!("Failed to read text file: {}", e),
        kind: "IoError".to_string(),
    })?;

    let filename = source_path.file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| ApiError {
            message: "Invalid filename".to_string(),
            kind: "ValidationError".to_string(),
        })?
        .to_string();

    let slice_title = title.unwrap_or_else(|| {
        source_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Imported Text")
            .to_string()
    });

    let word_count = content.split_whitespace().count() as i32;

    let slice = Slice {
        id: None,
        original_audio_file_name: filename,
        title: Some(slice_title),
        transcribed: true,
        audio_file_size: content.len() as i64,
        audio_file_type: "text".to_string(),
        estimated_time_to_transcribe: 0,
        audio_time_length_seconds: None,
        transcription: Some(content),
        transcription_time_taken: Some(0),
        transcription_word_count: Some(word_count),
        transcription_model: Some("imported".to_string()),
        recording_date: Some(chrono::Utc::now().timestamp()),
    };

    let id = db.insert_slice(&slice)?;
    info!("Imported text file slice with ID {} from {}", id, file_path);
    Ok(id)
}

#[tauri::command]
async fn open_url(url: String) -> Result<(), ApiError> {
    std::process::Command::new("open")
        .arg(&url)
        .spawn()
        .map_err(|e| ApiError {
            message: format!("Failed to open URL: {}", e),
            kind: "IoError".to_string(),
        })?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Load initial config
    let config = Config::load().expect("Failed to load config");
    println!("Loaded config: {:?}", config);
    
    // Ensure CiderPress home exists
    if let Err(e) = config.ensure_ciderpress_home() {
        eprintln!("Failed to create CiderPress home: {}", e);
    }

    // Initialize logging
    if let Err(e) = logging::init_logging(&config) {
        eprintln!("Failed to initialize logging: {}", e);
    }

    // Initialize FFmpeg library (statically linked)
    ffmpeg_next::init().expect("Failed to initialize FFmpeg library");
    // Suppress FFmpeg's internal diagnostic logging (our code handles errors via Result/Option)
    ffmpeg_next::log::set_level(ffmpeg_next::log::Level::Fatal);

    // Initialize database
    let db_path = config.ciderpress_home_path().join("CiderPress-db.sqlite");
    let db = match Database::new(&db_path) {
        Ok(db) => Some(db),
        Err(e) => {
            eprintln!("Failed to initialize database: {}", e);
            None
        }
    };

    let app_state = AppState {
        config: Mutex::new(config),
        db: Mutex::new(db),
    };

    tauri::Builder::default()
        .manage(app_state)
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            get_config,
            update_config,
            validate_paths,
            start_migration,
            get_migration_stats,
            get_pre_migration_stats,
            clear_database,
            get_slice_records,
            get_stats,
            list_recordings,
            search_recordings,
            transcribe_many,
            transcribe_slices,
            get_transcription_progress,
            export_transcribed_text,
            export_audio,
            update_slice_name,
            update_slice,
            update_transcription_model,
            get_available_models,
            get_downloaded_models,
            download_whisper_model,
            pick_directory,
            get_slice_audio_bytes,
            update_slice_names_from_audio,
            update_recording_title,
            auto_populate_titles,
            populate_audio_durations,
            backfill_recording_dates,
            list_labels,
            create_label,
            update_label,
            delete_label,
            log_user_action,
            nlm_get_status,
            nlm_authenticate,
            nlm_list_notebooks,
            nlm_add_text,
            nlm_add_audio,
            nlm_list_profiles,
            nlm_auth_with_profile,
            nlm_create_notebook,
            nlm_get_notebook_details,
            get_system_info,
            open_url,
            create_text_slice,
            import_audio_slice,
            import_text_file_slice
        ])
        .setup(|app| {
            // Initialize global app handle for event emission
            init_app_handle(app.handle().clone());

            // Set window title with app version
            if let Some(window) = app.get_webview_window("main") {
                let version = env!("CARGO_PKG_VERSION");
                let _ = window.set_title(&format!("CiderPress v{} - Voice Memo Liberator", version));
            }

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
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

use anyhow::Result;
use chrono::{Local, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use super::config::Config;

lazy_static::lazy_static! {
    static ref LOG_FILE_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);
}

/// Types of log events
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogEventType {
    // User actions
    NavigateTo,
    ButtonClick,
    SettingsChange,
    SelectSlices,
    ExportRequest,

    // Migration events
    MigrationStart,
    MigrationProgress,
    MigrationFileProcessed,
    MigrationComplete,
    MigrationError,

    // Transcription events
    TranscriptionStart,
    TranscriptionProgress,
    TranscriptionComplete,
    TranscriptionError,

    // System events
    AppStart,
    AppShutdown,
    ConfigLoad,
    ConfigSave,
    DatabaseInit,

    // General
    Info,
    Warning,
    Error,
}

/// A single log entry in JSON format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub timestamp_utc: String,
    pub event_type: LogEventType,
    pub category: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl LogEntry {
    pub fn new(event_type: LogEventType, category: &str, message: &str) -> Self {
        let now = Local::now();
        Self {
            timestamp: now.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
            timestamp_utc: Utc::now().to_rfc3339(),
            event_type,
            category: category.to_string(),
            message: message.to_string(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

/// Initialize the logging system with the given config
pub fn init_logging(config: &Config) -> Result<()> {
    let logs_dir = config.logs_dir();
    fs::create_dir_all(&logs_dir)?;

    // Create log file with date in the name
    let today = Local::now().format("%Y-%m-%d").to_string();
    let log_file_path = logs_dir.join(format!("ciderpress_{}.jsonl", today));

    // Set the log file path in a separate scope to release the lock before calling log_event
    {
        let mut path = LOG_FILE_PATH.lock().unwrap();
        *path = Some(log_file_path);
    }

    // Log that the logging system was initialized
    log_event(LogEntry::new(
        LogEventType::AppStart,
        "system",
        "CiderPress logging initialized",
    ))?;

    Ok(())
}

/// Write a log entry to the log file
pub fn log_event(entry: LogEntry) -> Result<()> {
    let path = LOG_FILE_PATH.lock().unwrap();

    if let Some(log_path) = path.as_ref() {
        // Ensure parent directory exists
        if let Some(parent) = log_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Serialize the entry to JSON
        let json = serde_json::to_string(&entry)?;

        // Append to the log file
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)?;

        writeln!(file, "{}", json)?;
    }

    Ok(())
}

// Convenience functions for common log operations

/// Log a user navigation event
pub fn log_navigation(screen: &str) {
    let entry = LogEntry::new(
        LogEventType::NavigateTo,
        "user_action",
        &format!("User navigated to {}", screen),
    ).with_details(serde_json::json!({
        "screen": screen
    }));
    let _ = log_event(entry);
}

/// Log a button click event
pub fn log_button_click(button_name: &str, context: Option<&str>) {
    let mut entry = LogEntry::new(
        LogEventType::ButtonClick,
        "user_action",
        &format!("User clicked {}", button_name),
    );

    let mut details = serde_json::json!({ "button": button_name });
    if let Some(ctx) = context {
        details["context"] = serde_json::Value::String(ctx.to_string());
    }
    entry = entry.with_details(details);

    let _ = log_event(entry);
}

/// Log a settings change
pub fn log_settings_change(setting_name: &str, old_value: Option<&str>, new_value: &str) {
    let entry = LogEntry::new(
        LogEventType::SettingsChange,
        "user_action",
        &format!("Setting '{}' changed to '{}'", setting_name, new_value),
    ).with_details(serde_json::json!({
        "setting": setting_name,
        "old_value": old_value,
        "new_value": new_value
    }));
    let _ = log_event(entry);
}

/// Log migration start
pub fn log_migration_start(source_dir: &str, file_count: u32, total_size_bytes: u64) {
    let entry = LogEntry::new(
        LogEventType::MigrationStart,
        "migration",
        &format!("Starting migration of {} files ({} bytes)", file_count, total_size_bytes),
    ).with_details(serde_json::json!({
        "source_directory": source_dir,
        "file_count": file_count,
        "total_size_bytes": total_size_bytes,
        "total_size_mb": format!("{:.2}", total_size_bytes as f64 / 1024.0 / 1024.0)
    }));
    let _ = log_event(entry);
}

/// Log a file being processed during migration
pub fn log_migration_file(filename: &str, result: &str, size_bytes: Option<u64>, error: Option<&str>) {
    let event_type = if error.is_some() {
        LogEventType::MigrationError
    } else {
        LogEventType::MigrationFileProcessed
    };

    let entry = LogEntry::new(
        event_type,
        "migration",
        &format!("File '{}': {}", filename, result),
    ).with_details(serde_json::json!({
        "filename": filename,
        "result": result,
        "size_bytes": size_bytes,
        "error": error
    }));
    let _ = log_event(entry);
}

/// Log migration completion
pub fn log_migration_complete(copied: u32, skipped: u32, errors: u32, total_size_bytes: u64) {
    let entry = LogEntry::new(
        LogEventType::MigrationComplete,
        "migration",
        &format!("Migration complete: {} copied, {} skipped, {} errors", copied, skipped, errors),
    ).with_details(serde_json::json!({
        "files_copied": copied,
        "files_skipped": skipped,
        "files_with_errors": errors,
        "total_size_bytes": total_size_bytes,
        "total_size_mb": format!("{:.2}", total_size_bytes as f64 / 1024.0 / 1024.0)
    }));
    let _ = log_event(entry);
}

/// Log transcription start
pub fn log_transcription_start(slice_ids: &[i64], model_name: &str, total_seconds: u32) {
    let entry = LogEntry::new(
        LogEventType::TranscriptionStart,
        "transcription",
        &format!("Starting transcription of {} slices with model '{}'", slice_ids.len(), model_name),
    ).with_details(serde_json::json!({
        "slice_ids": slice_ids,
        "slice_count": slice_ids.len(),
        "model_name": model_name,
        "estimated_total_seconds": total_seconds
    }));
    let _ = log_event(entry);
}

/// Log transcription of a single slice
pub fn log_transcription_slice(slice_id: i64, filename: &str, result: &str, duration_seconds: Option<f64>, word_count: Option<u32>, error: Option<&str>) {
    let event_type = if error.is_some() {
        LogEventType::TranscriptionError
    } else {
        LogEventType::TranscriptionProgress
    };

    let entry = LogEntry::new(
        event_type,
        "transcription",
        &format!("Slice {} ({}): {}", slice_id, filename, result),
    ).with_details(serde_json::json!({
        "slice_id": slice_id,
        "filename": filename,
        "result": result,
        "duration_seconds": duration_seconds,
        "word_count": word_count,
        "error": error
    }));
    let _ = log_event(entry);
}

/// Log transcription completion
pub fn log_transcription_complete(total_slices: u32, successful: u32, failed: u32, total_duration: f64) {
    let entry = LogEntry::new(
        LogEventType::TranscriptionComplete,
        "transcription",
        &format!("Transcription complete: {}/{} successful, {} failed", successful, total_slices, failed),
    ).with_details(serde_json::json!({
        "total_slices": total_slices,
        "successful": successful,
        "failed": failed,
        "total_duration_seconds": total_duration
    }));
    let _ = log_event(entry);
}

/// Log export request
pub fn log_export(export_type: &str, slice_ids: &[i64], destination: Option<&str>) {
    let entry = LogEntry::new(
        LogEventType::ExportRequest,
        "user_action",
        &format!("Export requested: {} ({} slices)", export_type, slice_ids.len()),
    ).with_details(serde_json::json!({
        "export_type": export_type,
        "slice_ids": slice_ids,
        "slice_count": slice_ids.len(),
        "destination": destination
    }));
    let _ = log_event(entry);
}

/// Log a general info message
pub fn log_info(category: &str, message: &str, details: Option<serde_json::Value>) {
    let mut entry = LogEntry::new(LogEventType::Info, category, message);
    if let Some(d) = details {
        entry = entry.with_details(d);
    }
    let _ = log_event(entry);
}

/// Log a warning message
#[allow(dead_code)]
pub fn log_warning(category: &str, message: &str, details: Option<serde_json::Value>) {
    let mut entry = LogEntry::new(LogEventType::Warning, category, message);
    if let Some(d) = details {
        entry = entry.with_details(d);
    }
    let _ = log_event(entry);
}

/// Log an error message
#[allow(dead_code)]
pub fn log_error(category: &str, message: &str, details: Option<serde_json::Value>) {
    let mut entry = LogEntry::new(LogEventType::Error, category, message);
    if let Some(d) = details {
        entry = entry.with_details(d);
    }
    let _ = log_event(entry);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_log_entry_serialization() {
        let entry = LogEntry::new(
            LogEventType::NavigateTo,
            "user_action",
            "User navigated to Settings",
        ).with_details(serde_json::json!({
            "screen": "Settings"
        }));

        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("navigate_to"));
        assert!(json.contains("user_action"));
        assert!(json.contains("Settings"));
    }

    #[test]
    fn test_init_logging() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config = Config {
            voice_memo_root: "/tmp".to_string(),
            ciderpress_home: temp_dir.path().to_string_lossy().to_string(),
            model_name: "base.en".to_string(),
            first_run_complete: false,
            skip_already_transcribed: true,
        };

        init_logging(&config)?;

        // Check that logs directory was created
        assert!(config.logs_dir().exists());

        Ok(())
    }
}
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
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::{info, error, warn};
use walkdir::WalkDir;

use super::config::Config;
use super::database::Database;
use super::logging;
use super::models::{MigrationSummary, MigrationProgress, Slice};

/// Helper to emit migration log events
fn log_migration(message: &str, level: &str) {
    // Log to tracing as well
    match level {
        "error" => error!("{}", message),
        "warn" => warn!("{}", message),
        "success" => info!("{}", message),
        _ => info!("{}", message),
    }
    // Emit to frontend
    crate::emit_migration_log(message, level);
}

// Global migration progress state
lazy_static::lazy_static! {
    static ref MIGRATION_PROGRESS: Arc<Mutex<Option<MigrationProgress>>> = Arc::new(Mutex::new(None));
}

pub struct MigrationEngine<'a> {
    config: &'a Config,
}

impl<'a> MigrationEngine<'a> {
    pub fn new(config: &'a Config) -> Self {
        Self { config }
    }

    pub fn start_migration(&self) -> Result<()> {
        log_migration("Starting migration process", "info");

        // Reset progress
        {
            let mut progress = MIGRATION_PROGRESS.lock().unwrap();
            *progress = Some(MigrationProgress {
                total_recordings: 0,
                processed_recordings: 0,
                failed_recordings: 0,
                current_recording: None,
                current_step: "Initializing...".to_string(),
                total_size_bytes: 0,
                processed_size_bytes: 0,
            });
        }

        // Create CiderPress database if it doesn't exist
        let ciderpress_db_path = self.config.ciderpress_home_path().join("CiderPress-db.sqlite");
        log_migration(&format!("Opening CiderPress database at: {:?}", ciderpress_db_path), "info");

        // Ensure directory exists
        if let Some(parent) = ciderpress_db_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let db = Database::new(&ciderpress_db_path)?;

        self.update_progress("Connecting to Apple Voice Memo database...", None, None)?;
        log_migration("Connecting to Apple Voice Memo database...", "info");

        // Open Apple's database
        let apple_db_path = self.config.voice_memo_root_path().join("CloudRecordings.db");
        log_migration(&format!("Looking for Apple database at: {:?}", apple_db_path), "info");

        if !apple_db_path.exists() {
            let error_message = format!("Apple Voice Memo database not found at: {:?}. Please check your configuration.", apple_db_path);
            log_migration(&error_message, "error");
            self.update_progress(&error_message, Some(0), Some(0))?;
            std::thread::sleep(std::time::Duration::from_secs(5));
            *MIGRATION_PROGRESS.lock().unwrap() = None;
            return Err(anyhow::anyhow!(error_message));
        }

        // 1. Copy ZCLOUDRECORDING table
        self.update_progress("Copying Apple Voice Memo database records...", None, None)?;
        log_migration("Copying Apple Voice Memo database records...", "info");
        match db.copy_zcloudrecording_table(apple_db_path.to_str().unwrap()) {
            Ok(rows_copied) => log_migration(&format!("Copied {} new rows from ZCLOUDRECORDING", rows_copied), "success"),
            Err(e) => {
                let error_message = format!("Failed to copy ZCLOUDRECORDING table: {}", e);
                log_migration(&error_message, "error");
                self.update_progress(&error_message, None, None)?;
                return Err(e);
            }
        }

        // 2. Find all .m4a files to process
        self.update_progress("Scanning for .m4a audio files...", None, None)?;
        log_migration("Scanning for .m4a audio files...", "info");
        let voice_memo_dir = self.config.voice_memo_root_path();
        
        // Enhanced directory access logging
        info!("=== DIRECTORY ACCESS CHECK ===");
        info!("Target directory: {:?}", voice_memo_dir);
        info!("Directory exists: {}", voice_memo_dir.exists());
        
        if voice_memo_dir.exists() {
            info!("✓ Can see the directory");
            
            // Test basic directory listing
            match fs::read_dir(&voice_memo_dir) {
                Ok(entries) => {
                    info!("✓ Can list directory contents");
                    let mut file_count = 0;
                    let mut m4a_count = 0;
                    
                    for entry in entries {
                        match entry {
                            Ok(entry) => {
                                file_count += 1;
                                let path = entry.path();
                                let filename = path.file_name().unwrap_or_default().to_string_lossy();
                                let is_m4a = path.extension().map_or(false, |ext| ext == "m4a");
                                
                                if is_m4a {
                                    m4a_count += 1;
                                    info!("  Found .m4a file: {}", filename);
                                } else if file_count <= 10 { // Limit non-m4a logging
                                    info!("  Found other file: {} (type: {:?})", filename, path.extension());
                                }
                            }
                            Err(e) => {
                                error!("  Error reading directory entry: {}", e);
                            }
                        }
                    }
                    
                    info!("Directory listing summary: {} total files, {} .m4a files", file_count, m4a_count);
                }
                Err(e) => {
                    error!("✗ Cannot list directory contents: {}", e);
                    let error_message = format!("Permission denied accessing voice memo directory: {}", e);
                    self.update_progress(&error_message, Some(0), Some(0))?;
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    *MIGRATION_PROGRESS.lock().unwrap() = None;
                    return Err(anyhow::anyhow!(error_message));
                }
            }
        } else {
            error!("✗ Cannot see the directory");
            let error_message = "Voice memo directory does not exist or is not accessible".to_string();
            self.update_progress(&error_message, Some(0), Some(0))?;
            std::thread::sleep(std::time::Duration::from_secs(5));
            *MIGRATION_PROGRESS.lock().unwrap() = None;
            return Err(anyhow::anyhow!(error_message));
        }
        
        let m4a_files = self.scan_m4a_files(&voice_memo_dir)?;
        log_migration(&format!("Found {} .m4a files to process", m4a_files.len()), "success");

        if m4a_files.is_empty() {
            log_migration("No files to migrate. All files have already been migrated.", "success");
            self.update_progress("No files to migrate.", Some(0), Some(0))?;
            *MIGRATION_PROGRESS.lock().unwrap() = None;
            return Ok(());
        }

        // Log first few files found
        info!("=== FILES TO PROCESS ===");
        for (i, file) in m4a_files.iter().take(10).enumerate() {
            info!("  [{}] {:?}", i + 1, file);
        }
        if m4a_files.len() > 10 {
            info!("  ... and {} more files", m4a_files.len() - 10);
        }

        // 3. Calculate total size and update progress
        let total_size_bytes: u64 = m4a_files.iter().map(|f| {
            fs::metadata(f).map(|m| m.len()).unwrap_or(0)
        }).sum();

        log_migration(&format!("Starting file migration ({} bytes total)...", total_size_bytes), "info");

        // Log migration start to JSON log
        logging::log_migration_start(
            &self.config.voice_memo_root,
            m4a_files.len() as u32,
            total_size_bytes,
        );

        self.update_progress(
            "Starting file migration...",
            Some(m4a_files.len() as u32),
            Some(total_size_bytes)
        )?;

        let mut summary = MigrationSummary {
            copied: 0,
            skipped: 0,
            errors: 0,
            total_size_bytes,
        };

        // Ensure destination directory exists
        let dest_audio_dir = self.config.audio_dir();

        match fs::create_dir_all(&dest_audio_dir) {
            Ok(()) => log_migration(&format!("Destination directory ready: {:?}", dest_audio_dir), "info"),
            Err(e) => {
                log_migration(&format!("Failed to create destination directory: {}", e), "error");
                return Err(e.into());
            }
        }

        // 4. Process each .m4a file
        for (index, m4a_file) in m4a_files.iter().enumerate() {
            let filename = m4a_file.file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("unknown.m4a");

            log_migration(&format!("Processing ({}/{}): {}", index + 1, m4a_files.len(), filename), "info");

            self.update_progress(
                &format!("Processing ({}/{}): {}", index + 1, m4a_files.len(), filename),
                None,
                None,
            )?;

            match self.process_m4a_file(&m4a_file, &db) {
                Ok(ProcessResult::Copied(size)) => {
                    summary.copied += 1;

                    // Log to JSON log
                    logging::log_migration_file(filename, "copied", Some(size), None);

                    let mut progress = MIGRATION_PROGRESS.lock().unwrap();
                    if let Some(ref mut p) = *progress {
                        p.processed_recordings = (index + 1) as u32;
                        p.processed_size_bytes += size;
                    }
                }
                Ok(ProcessResult::Skipped) => {
                    summary.skipped += 1;
                    log_migration(&format!("  Skipped (already migrated): {}", filename), "warn");

                    // Log to JSON log
                    logging::log_migration_file(filename, "skipped", None, None);

                    let mut progress = MIGRATION_PROGRESS.lock().unwrap();
                    if let Some(ref mut p) = *progress {
                        p.processed_recordings = (index + 1) as u32;
                    }
                }
                Err(e) => {
                    log_migration(&format!("  Error: {} - {}", filename, e), "error");
                    summary.errors += 1;

                    // Log to JSON log
                    logging::log_migration_file(filename, "error", None, Some(&e.to_string()));

                    let mut progress = MIGRATION_PROGRESS.lock().unwrap();
                    if let Some(ref mut p) = *progress {
                        p.failed_recordings += 1;
                        p.processed_recordings = (index + 1) as u32; // Also count as processed
                    }
                }
            }
        }

        self.update_progress("Migration completed!", None, None)?;

        // Final summary
        log_migration("", "info");
        log_migration("=== MIGRATION SUMMARY ===", "info");

        if summary.copied == 0 && summary.errors == 0 {
            // All files were already migrated
            log_migration("No files to migrate. All files have already been migrated.", "success");
            log_migration(&format!("Files already in database: {}", summary.skipped), "info");
        } else {
            if summary.copied > 0 {
                log_migration(&format!("Files copied: {}", summary.copied), "success");
            }
            if summary.skipped > 0 {
                log_migration(&format!("Files skipped (already migrated): {}", summary.skipped), "warn");
            }
            if summary.errors > 0 {
                log_migration(&format!("Files with errors: {}", summary.errors), "error");
            }
            log_migration(&format!("Total size processed: {}", format_file_size(summary.total_size_bytes)), "info");
        }

        if summary.errors == 0 {
            log_migration("", "info");
            log_migration("SUCCESS - Migration completed with no errors.", "success");
        } else {
            log_migration("", "info");
            log_migration(&format!("Migration completed with {} error(s). Review the log above for details.", summary.errors), "warn");
        }

        // Log migration complete to JSON log
        logging::log_migration_complete(
            summary.copied,
            summary.skipped,
            summary.errors,
            summary.total_size_bytes,
        );

        // Clear the progress state to indicate completion
        {
            let mut progress = MIGRATION_PROGRESS.lock().unwrap();
            *progress = None;
        }

        Ok(())
    }

    pub fn get_migration_progress() -> Option<MigrationProgress> {
        MIGRATION_PROGRESS.lock().unwrap().clone()
    }

    pub fn get_migration_progress_ref() -> &'static Arc<Mutex<Option<MigrationProgress>>> {
        &MIGRATION_PROGRESS
    }

    fn update_progress(&self, step: &str, total: Option<u32>, total_size: Option<u64>) -> Result<()> {
        let mut progress = MIGRATION_PROGRESS.lock().unwrap();
        if let Some(ref mut p) = *progress {
            p.current_step = step.to_string();
            if let Some(t) = total {
                p.total_recordings = t;
            }
            if let Some(s) = total_size {
                p.total_size_bytes = s;
            }
        }
        Ok(())
    }

    fn scan_m4a_files(&self, voice_memo_dir: &Path) -> Result<Vec<PathBuf>> {
        log_migration(&format!("Scanning directory: {:?}", voice_memo_dir), "info");

        if !voice_memo_dir.exists() {
            log_migration(&format!("Voice memo directory does not exist: {:?}", voice_memo_dir), "error");
            return Ok(Vec::new());
        }

        let mut m4a_files = Vec::new();
        let mut directories_scanned = 0;
        let mut access_errors = 0;

        for entry in WalkDir::new(voice_memo_dir).into_iter() {

            match entry {
                Ok(entry) => {
                    let path = entry.path();

                    if entry.file_type().is_dir() {
                        directories_scanned += 1;
                    } else if entry.file_type().is_file() {
                        if let Some(ext) = path.extension() {
                            if ext.to_str() == Some("m4a") {
                                m4a_files.push(path.to_path_buf());
                            }
                        }
                    }
                }
                Err(e) => {
                    access_errors += 1;
                    if access_errors <= 3 {
                        log_migration(&format!("Access error during scan: {}", e), "error");
                    }
                }
            }
        }

        if access_errors > 0 {
            log_migration(&format!("Scan had {} access errors (may need Full Disk Access permission)", access_errors), "warn");
        }

        log_migration(&format!("Scan complete: {} directories scanned, {} .m4a files found", directories_scanned, m4a_files.len()), "info");

        Ok(m4a_files)
    }

    fn process_m4a_file(&self, m4a_file_path: &Path, db: &Database) -> Result<ProcessResult> {
        let filename = m4a_file_path.file_name()
            .and_then(|f| f.to_str())
            .context("Invalid file name")?;

        // 1. Check if the slice already exists in the database
        if db.slice_exists(filename)? {
            info!("Skipping (already in DB): {}", filename);
            return Ok(ProcessResult::Skipped);
        }

        // 2. Determine destination path
        let dest_dir = self.config.audio_dir();
        fs::create_dir_all(&dest_dir).with_context(|| format!("Failed to create destination directory at {:?}", dest_dir))?;
        let dest_path = dest_dir.join(filename);

        // 3. Copy the file
        info!("Attempting to copy from '{}' to '{}'", m4a_file_path.display(), dest_path.display());

        match fs::copy(m4a_file_path, &dest_path) {
            Ok(size) => {
                info!("✅ SUCCESSFULLY COPIED FILE: {} ({} bytes)", filename, size);

                // Verify the file actually exists at destination
                if dest_path.exists() {
                    let actual_size = fs::metadata(&dest_path)?.len();
                    info!("✅ VERIFIED: File exists at destination with size {} bytes", actual_size);
                } else {
                    error!("❌ CRITICAL: File copy reported success but file not found at destination!");
                    return Err(anyhow::anyhow!("File copy verification failed"));
                }

                // 4. Create and insert a slice record
                let file_type = m4a_file_path.extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("m4a")
                    .to_string();

                // Extract audio duration from the file
                let audio_duration = get_audio_duration(&dest_path);

                // Get the recording date from Apple's ZCLOUDRECORDING table
                let recording_date = db.get_recording_date_for_filename(filename).ok().flatten();

                let slice = Slice {
                    id: None,
                    original_audio_file_name: filename.to_string(),
                    title: None,
                    transcribed: false,
                    audio_file_size: size as i64,
                    audio_file_type: file_type.clone(),
                    estimated_time_to_transcribe: estimate_transcription_time(size, audio_duration),
                    audio_time_length_seconds: audio_duration,
                    transcription: None,
                    transcription_time_taken: None,
                    transcription_word_count: None,
                    transcription_model: None,
                    recording_date,
                };

                db.insert_slice(&slice)?;
                info!(slice = ?&slice, "Inserted slice record");

                // Log file details and metadata to the migration log window
                log_migration(&format!("  Copied: {} ({})", filename, format_file_size(size)), "success");
                let mut meta_parts: Vec<String> = Vec::new();
                meta_parts.push(format!("type: {}", file_type));
                if let Some(duration) = audio_duration {
                    meta_parts.push(format!("duration: {}", format_audio_duration(duration)));
                }
                if let Some(date) = recording_date {
                    meta_parts.push(format!("recorded: {}", format_recording_date(date)));
                }
                log_migration(&format!("  Metadata: {}", meta_parts.join(", ")), "info");

                Ok(ProcessResult::Copied(size))
            },
            Err(e) => {
                error!("Failed to copy file from '{}' to '{}'. Error: {}", m4a_file_path.display(), dest_path.display(), e);
                Err(e.into())
            }
        }
    }
}

fn estimate_transcription_time(file_size_bytes: u64, audio_duration_seconds: Option<f64>) -> i32 {
    // If audio duration is known, use 35 seconds of processing per 10 minutes of audio
    if let Some(duration) = audio_duration_seconds {
        let seconds = (duration / 600.0 * 35.0).ceil() as i32;
        return std::cmp::max(1, seconds);
    }
    // Fallback when duration is unknown: rough heuristic based on file size
    // ~1 minute of audio is ~1MB for .m4a, processing at ~35s per 10min
    let audio_minutes = file_size_bytes as f64 / 1_048_576.0;
    let seconds = (audio_minutes / 10.0 * 35.0).round() as i32;
    std::cmp::max(1, seconds)
}

fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1_048_576 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1_073_741_824 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    }
}

fn format_audio_duration(seconds: f64) -> String {
    let total = seconds.round() as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{}h {}m {}s", h, m, s)
    } else if m > 0 {
        format!("{}m {}s", m, s)
    } else {
        format!("{}s", s)
    }
}

fn format_recording_date(unix_timestamp: i64) -> String {
    chrono::DateTime::from_timestamp(unix_timestamp, 0)
        .map(|dt| dt.format("%b %d, %Y").to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Get the duration of an audio file in seconds using ffmpeg-next library API
pub fn get_audio_duration(audio_path: &Path) -> Option<f64> {
    let path_str = audio_path.to_str()?;
    match ffmpeg_next::format::input(path_str) {
        Ok(ictx) => {
            let duration = ictx.duration();
            if duration > 0 {
                Some(duration as f64 * f64::from(ffmpeg_next::rescale::TIME_BASE))
            } else {
                // Fallback: try stream-level duration
                ictx.streams()
                    .best(ffmpeg_next::media::Type::Audio)
                    .and_then(|s| {
                        let dur = s.duration();
                        let tb = s.time_base();
                        if dur > 0 {
                            Some(dur as f64 * tb.0 as f64 / tb.1 as f64)
                        } else {
                            None
                        }
                    })
            }
        }
        Err(e) => {
            warn!("Failed to open '{}' for duration probe: {}", audio_path.display(), e);
            None
        }
    }
}

enum ProcessResult {
    Copied(u64), // Size in bytes
    Skipped,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_estimate_transcription_time() {
        // With audio duration: 35s per 10 minutes (600s) of audio
        assert_eq!(super::estimate_transcription_time(10_000, Some(600.0)), 35); // 10 min -> 35s
        assert_eq!(super::estimate_transcription_time(10_000, Some(60.0)), 4); // 1 min -> ceil(3.5) = 4s
        assert_eq!(super::estimate_transcription_time(10_000, Some(6000.0)), 350); // 100 min -> 350s
        assert_eq!(super::estimate_transcription_time(10_000, Some(5.0)), 1); // tiny -> at least 1s

        // Without audio duration: fallback to file size heuristic
        assert_eq!(super::estimate_transcription_time(10_000, None), 1); // small files
        assert_eq!(super::estimate_transcription_time(1_048_576, None), 4); // 1MB (~1min audio) -> ceil(0.1*35)=4s
        assert_eq!(super::estimate_transcription_time(50_000_000, None), 167); // ~50min audio -> ~167s
    }

    #[test]
    fn test_scan_m4a_files() -> Result<()> {
        // Create a temporary directory with test files
        let temp_dir = TempDir::new()?;
        let temp_path = temp_dir.path();

        // Create test files
        fs::write(temp_path.join("recording1.m4a"), b"fake audio data 1")?;
        fs::write(temp_path.join("recording2.m4a"), b"fake audio data 2")?;
        fs::write(temp_path.join("document.txt"), b"not an audio file")?;
        let sub_dir = temp_path.join("subdir");
        fs::create_dir_all(&sub_dir)?;
        fs::write(sub_dir.join("recording3.m4a"), b"fake audio data 3")?;

        // Create a test config
        let config = Config {
            voice_memo_root: temp_path.to_string_lossy().to_string(),
            ciderpress_home: temp_path.join("ciderpress").to_string_lossy().to_string(),
            model_name: "base.en".to_string(),
            first_run_complete: false,
        };

        let migration_engine = MigrationEngine::new(&config);
        let m4a_files = migration_engine.scan_m4a_files(temp_path)?;

        // Should find exactly 3 .m4a files
        assert_eq!(m4a_files.len(), 3);
        
        let filenames: Vec<String> = m4a_files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        
        assert!(filenames.contains(&"recording1.m4a".to_string()));
        assert!(filenames.contains(&"recording2.m4a".to_string()));
        assert!(filenames.contains(&"recording3.m4a".to_string()));

        Ok(())
    }

    #[test]
    fn test_process_m4a_file_integration() -> Result<()> {
        // Create temporary directories
        let temp_dir = TempDir::new()?;
        let source_dir = temp_dir.path().join("source");
        let dest_dir = temp_dir.path().join("ciderpress");
        
        fs::create_dir_all(&source_dir)?;
        fs::create_dir_all(&dest_dir)?;

        // Create a test .m4a file
        let test_content = b"fake m4a audio data for testing";
        let source_file = source_dir.join("test_recording.m4a");
        fs::write(&source_file, test_content)?;

        // Create test config
        let config = Config {
            voice_memo_root: source_dir.to_string_lossy().to_string(),
            ciderpress_home: dest_dir.to_string_lossy().to_string(),
            model_name: "base.en".to_string(),
            first_run_complete: false,
        };
        config.ensure_ciderpress_home()?;

        // Create test database
        let db_path = dest_dir.join("test.db");
        let db = Database::new(&db_path)?;

        // Test the migration engine
        let migration_engine = MigrationEngine::new(&config);
        let result = migration_engine.process_m4a_file(&source_file, &db)?;

        // Verify the result
        match result {
            ProcessResult::Copied(size) => {
                assert_eq!(size, test_content.len() as u64);
            }
            ProcessResult::Skipped => {
                panic!("File should have been copied, not skipped");
            }
        }

        // Verify file was copied
        let dest_file = config.audio_dir().join("test_recording.m4a");
        assert!(dest_file.exists(), "Destination file should exist");

        let copied_content = fs::read(&dest_file)?;
        assert_eq!(copied_content, test_content, "Copied content should match original");

        // Verify slice record was created
        let slices = db.list_all_slices()?;
        assert_eq!(slices.len(), 1, "Should have exactly one slice record");

        let slice = &slices[0];
        assert_eq!(slice.original_audio_file_name, "test_recording.m4a");
        assert_eq!(slice.audio_file_size, test_content.len() as i64);
        assert!(!slice.transcribed);

        Ok(())
    }

    #[test]
    fn test_full_migration_with_multiple_files() -> Result<()> {
        // Create temporary directories
        let temp_dir = TempDir::new()?;
        let source_dir = temp_dir.path().join("source");
        let dest_dir = temp_dir.path().join("ciderpress");
        
        fs::create_dir_all(&source_dir)?;
        fs::create_dir_all(&dest_dir)?;

        // Create test files
        let test_files = [("rec1.m4a", "rec1"), ("rec2.m4a", "rec2"), ("rec3.m4a", "rec3")];
        fs::write(source_dir.join(test_files[0].0), test_files[0].1)?;
        fs::write(source_dir.join(test_files[1].0), test_files[1].1)?;
        let sub_dir = source_dir.join("subdir");
        fs::create_dir_all(&sub_dir)?;
        fs::write(sub_dir.join(test_files[2].0), test_files[2].1)?;

        // Create test config and db
        let config = Config {
            voice_memo_root: source_dir.to_string_lossy().to_string(),
            ciderpress_home: dest_dir.to_string_lossy().to_string(),
            model_name: "base.en".to_string(),
            first_run_complete: false,
        };
        config.ensure_ciderpress_home()?;

        let db_path = dest_dir.join("CiderPress-db.sqlite");
        let db = Database::new(&db_path)?;
        
        // Mock Apple database
        let apple_db_path = source_dir.join("CloudRecordings.db");
        let conn = Connection::open(&apple_db_path)?;
        conn.execute("CREATE TABLE ZCLOUDRECORDING (Z_PK INTEGER PRIMARY KEY, ZPATH TEXT)", [])?;
        conn.execute("INSERT INTO ZCLOUDRECORDING (ZPATH) VALUES (?)", [test_files[0].0])?;
        conn.execute("INSERT INTO ZCLOUDRECORDING (ZPATH) VALUES (?)", [test_files[1].0])?;
        conn.execute("INSERT INTO ZCLOUDRECORDING (ZPATH) VALUES (?)", [&format!("subdir/{}", test_files[2].0)])?;
        drop(conn);

        // Run migration
        let engine = MigrationEngine::new(&config);
        engine.start_migration()?;

        // Verify files were copied
        for (filename, content) in &test_files {
            let dest_file = config.audio_dir().join(filename);
            let read_content = fs::read_to_string(dest_file)?;
            assert_eq!(content, &read_content);
        }

        // Verify slices were created
        let slices = db.list_all_slices()?;
        assert_eq!(slices.len(), 3);

        // Run again and check for skips
        let engine = MigrationEngine::new(&config);
        engine.start_migration()?;
        let slices = db.list_all_slices()?;
        assert_eq!(slices.len(), 3, "Should not create duplicate slices");


        Ok(())
    }

    #[test]
    #[ignore] // This test interacts with the live file system and user config. Run with `cargo test -- --ignored`.
    fn test_live_migration_file_copy() -> Result<()> {
        // This test uses the actual user configuration and file system locations.
        // It's meant to be a direct verification of the file copy mechanism.
        
        // 1. Load the live configuration
        let config = Config::load()?;
        info!("Loaded live config: {:?}", config);

        let voice_memo_dir = config.voice_memo_root_path();
        let audio_dest_dir = config.audio_dir();

        // 2. Ensure the destination directory exists
        fs::create_dir_all(&audio_dest_dir)?;
        info!("Ensured destination audio directory exists: {:?}", audio_dest_dir);

        // 3. Find the first .m4a file in the source directory
        let source_file_path = WalkDir::new(voice_memo_dir)
            .into_iter()
            .filter_map(Result::ok)
            .find(|e| e.path().extension().map_or(false, |ext| ext == "m4a"))
            .map(|e| e.path().to_path_buf())
            .context("Could not find any .m4a files in the source directory.")?;
            
        info!("Found source file to test copy: {:?}", source_file_path);

        // 4. Attempt to copy the file
        let dest_file_path = audio_dest_dir.join(source_file_path.file_name().unwrap());
        info!("Attempting to copy from: {:?} to: {:?}", source_file_path, dest_file_path);

        fs::copy(&source_file_path, &dest_file_path)
            .with_context(|| format!("Failed to copy test file from {:?} to {:?}", source_file_path, dest_file_path))?;

        // 5. Verify the file was copied
        assert!(dest_file_path.exists(), "The test file was not copied successfully to the destination.");
        
        info!("SUCCESS: Verified that test file was copied to {:?}", dest_file_path);

        // Clean up the copied file
        fs::remove_file(&dest_file_path)?;
        info!("Cleaned up test file: {:?}", dest_file_path);

        Ok(())
    }

    #[test]
    #[ignore] // Run with `cargo test test_migration_diagnostics -- --ignored --nocapture`
    fn test_migration_diagnostics() -> Result<()> {
        println!("=== MIGRATION DIAGNOSTICS TEST ===");
        
        // 1. Load and display configuration
        let config = Config::load()?;
        println!("1. Configuration loaded:");
        println!("   voice_memo_root: {}", config.voice_memo_root);
        println!("   ciderpress_home: {}", config.ciderpress_home);
        
        let voice_memo_dir = config.voice_memo_root_path();
        let audio_dest_dir = config.audio_dir();
        
        // 2. Check if source directory exists
        println!("2. Source directory check:");
        println!("   Path: {:?}", voice_memo_dir);
        println!("   Exists: {}", voice_memo_dir.exists());
        
        if !voice_memo_dir.exists() {
            println!("   ERROR: Source directory does not exist!");
            return Err(anyhow::anyhow!("Source directory does not exist"));
        }
        
        // 3. List contents of source directory
        println!("3. Source directory contents:");
        match std::fs::read_dir(&voice_memo_dir) {
            Ok(entries) => {
                let mut count = 0;
                for entry in entries {
                    if let Ok(entry) = entry {
                        count += 1;
                        let path = entry.path();
                        let is_m4a = path.extension().map_or(false, |ext| ext == "m4a");
                        println!("   [{}] {:?} (m4a: {})", count, path.file_name().unwrap_or_default(), is_m4a);
                        if count >= 10 { // Limit output
                            println!("   ... (showing first 10 entries)");
                            break;
                        }
                    }
                }
                println!("   Total entries checked: {}", count);
            }
            Err(e) => {
                println!("   ERROR reading directory: {}", e);
                return Err(e.into());
            }
        }
        
        // 4. Use WalkDir to find .m4a files
        println!("4. Scanning for .m4a files with WalkDir:");
        let m4a_files: Vec<PathBuf> = WalkDir::new(&voice_memo_dir)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| {
                e.file_type().is_file() &&
                e.path().extension().map_or(false, |ext| ext.to_str() == Some("m4a"))
            })
            .map(|e| e.path().to_path_buf())
            .collect();
            
        println!("   Found {} .m4a files", m4a_files.len());
        for (i, file) in m4a_files.iter().take(5).enumerate() {
            println!("   [{}] {:?}", i + 1, file);
        }
        if m4a_files.len() > 5 {
            println!("   ... (showing first 5 of {} files)", m4a_files.len());
        }
        
        if m4a_files.is_empty() {
            println!("   ERROR: No .m4a files found!");
            return Err(anyhow::anyhow!("No .m4a files found"));
        }
        
        // 5. Check destination directory
        println!("5. Destination directory check:");
        println!("   Path: {:?}", audio_dest_dir);
        println!("   Exists: {}", audio_dest_dir.exists());
        
        // 6. Create destination directory
        println!("6. Creating destination directory:");
        match std::fs::create_dir_all(&audio_dest_dir) {
            Ok(()) => println!("   SUCCESS: Directory created/verified"),
            Err(e) => {
                println!("   ERROR creating directory: {}", e);
                return Err(e.into());
            }
        }
        
        // 7. Test copying the first file
        println!("7. Testing file copy:");
        let test_file = &m4a_files[0];
        let filename = test_file.file_name().unwrap().to_str().unwrap();
        let dest_path = audio_dest_dir.join(filename);
        
        println!("   Source: {:?}", test_file);
        println!("   Destination: {:?}", dest_path);
        
        // Check source file properties
        match std::fs::metadata(test_file) {
            Ok(metadata) => {
                println!("   Source file size: {} bytes", metadata.len());
                println!("   Source file permissions: {:?}", metadata.permissions());
            }
            Err(e) => {
                println!("   ERROR reading source file metadata: {}", e);
                return Err(e.into());
            }
        }
        
        // Attempt the copy
        match std::fs::copy(test_file, &dest_path) {
            Ok(bytes_copied) => {
                println!("   SUCCESS: Copied {} bytes", bytes_copied);
                
                // Verify the copy
                if dest_path.exists() {
                    let dest_size = std::fs::metadata(&dest_path)?.len();
                    println!("   VERIFICATION: Destination file exists with {} bytes", dest_size);
                    
                    // Clean up
                    std::fs::remove_file(&dest_path)?;
                    println!("   CLEANUP: Test file removed");
                } else {
                    println!("   ERROR: Destination file does not exist after copy!");
                    return Err(anyhow::anyhow!("Copy operation reported success but file doesn't exist"));
                }
            }
            Err(e) => {
                println!("   ERROR copying file: {}", e);
                return Err(e.into());
            }
        }
        
        println!("8. CONCLUSION: File copy mechanism is working correctly!");
        println!("   The issue may be in the migration logic or database checks.");
        
        Ok(())
    }

    #[test]
    #[ignore] // Run with `cargo test test_full_migration_debug -- --ignored --nocapture`
    fn test_full_migration_debug() -> Result<()> {
        println!("=== FULL MIGRATION DEBUG TEST ===");
        
        // Load config and create migration engine
        let config = Config::load()?;
        println!("Config loaded: voice_memo_root = {}", config.voice_memo_root);
        
        let migration_engine = MigrationEngine::new(&config);
        
        // Create database
        let ciderpress_db_path = config.ciderpress_home_path().join("CiderPress-db.sqlite");
        println!("Database path: {:?}", ciderpress_db_path);
        
        // Ensure directory exists
        if let Some(parent) = ciderpress_db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let db = Database::new(&ciderpress_db_path)?;
        println!("Database created/opened successfully");
        
        // Scan for files
        let voice_memo_dir = config.voice_memo_root_path();
        let m4a_files = migration_engine.scan_m4a_files(&voice_memo_dir)?;
        println!("Found {} .m4a files", m4a_files.len());
        
        if m4a_files.is_empty() {
            return Err(anyhow::anyhow!("No files to process"));
        }
        
        // Process first file with detailed logging
        let test_file = &m4a_files[0];
        println!("Processing test file: {:?}", test_file);
        
        match migration_engine.process_m4a_file(test_file, &db) {
            Ok(ProcessResult::Copied(size)) => {
                println!("SUCCESS: File processed and copied ({} bytes)", size);
                
                // Verify file exists in destination
                let filename = test_file.file_name().unwrap().to_str().unwrap();
                let dest_path = config.audio_dir().join(filename);
                if dest_path.exists() {
                    println!("VERIFICATION: File exists at destination: {:?}", dest_path);
                } else {
                    println!("ERROR: File not found at destination: {:?}", dest_path);
                }
                
                // Check database
                let slices = db.list_all_slices()?;
                println!("Database contains {} slice records", slices.len());
            }
            Ok(ProcessResult::Skipped) => {
                println!("File was skipped (already exists in database)");
            }
            Err(e) => {
                println!("ERROR processing file: {}", e);
                return Err(e);
            }
        }
        
        Ok(())
    }

    #[test]
    #[ignore] // Run with `cargo test test_migration_with_accessible_files -- --ignored --nocapture`
    fn test_migration_with_accessible_files() -> Result<()> {
        println!("=== MIGRATION TEST WITH ACCESSIBLE FILES ===");
        
        // Create a temporary directory structure that mimics the real setup
        let temp_dir = TempDir::new()?;
        let temp_root = temp_dir.path();
        
        // Create mock voice memo directory with real-sized files
        let mock_voice_memo_dir = temp_root.join("voice_memos");
        fs::create_dir_all(&mock_voice_memo_dir)?;
        
        // Create some realistic .m4a files
        let test_files = [
            ("Recording 001.m4a", vec![0u8; 1024 * 1024]), // 1MB file
            ("Recording 002.m4a", vec![1u8; 2 * 1024 * 1024]), // 2MB file
            ("Recording 003.m4a", vec![2u8; 512 * 1024]), // 512KB file
        ];
        
        for (filename, content) in &test_files {
            fs::write(mock_voice_memo_dir.join(filename), content)?;
            println!("Created test file: {} ({} bytes)", filename, content.len());
        }
        
        // Create subdirectory with more files
        let subdir = mock_voice_memo_dir.join("Archive");
        fs::create_dir_all(&subdir)?;
        fs::write(subdir.join("Old Recording.m4a"), vec![3u8; 256 * 1024])?; // 256KB
        println!("Created subdirectory file: Old Recording.m4a");
        
        // Create mock CloudRecordings.db
        let mock_db_path = mock_voice_memo_dir.join("CloudRecordings.db");
        let conn = Connection::open(&mock_db_path)?;
        conn.execute(
            "CREATE TABLE ZCLOUDRECORDING (
                Z_PK INTEGER PRIMARY KEY,
                ZDATE REAL,
                ZDURATION REAL,
                ZTITLE TEXT,
                ZPATH TEXT
            )",
            [],
        )?;
        
        // Insert some mock records
        for (i, (filename, _)) in test_files.iter().enumerate() {
            conn.execute(
                "INSERT INTO ZCLOUDRECORDING (Z_PK, ZDATE, ZDURATION, ZTITLE, ZPATH) VALUES (?, ?, ?, ?, ?)",
                params![i + 1, 694224000.0 + (i as f64 * 3600.0), 60.0 + (i as f64 * 30.0), format!("Test Recording {}", i + 1), filename],
            )?;
        }
        drop(conn);
        println!("Created mock CloudRecordings.db with {} records", test_files.len());
        
        // Create destination directory
        let dest_dir = temp_root.join("ciderpress");
        fs::create_dir_all(&dest_dir)?;
        
        // Create test config pointing to our mock directories
        let config = Config {
            voice_memo_root: mock_voice_memo_dir.to_string_lossy().to_string(),
            ciderpress_home: dest_dir.to_string_lossy().to_string(),
            model_name: "base.en".to_string(),
            first_run_complete: false,
        };
        
        println!("Test config created:");
        println!("  voice_memo_root: {}", config.voice_memo_root);
        println!("  ciderpress_home: {}", config.ciderpress_home);
        
        // Ensure ciderpress directories exist
        config.ensure_ciderpress_home()?;
        
        // Run the migration
        println!("Starting migration...");
        let migration_engine = MigrationEngine::new(&config);
        migration_engine.start_migration()?;
        
        // Verify results
        println!("Migration completed. Verifying results...");
        
        // Check that files were copied to audio directory
        let audio_dir = config.audio_dir();
        println!("Checking audio directory: {:?}", audio_dir);
        
        let mut copied_files = 0;
        for (filename, original_content) in &test_files {
            let dest_path = audio_dir.join(filename);
            if dest_path.exists() {
                let copied_content = fs::read(&dest_path)?;
                if copied_content == *original_content {
                    println!("  ✓ {} copied successfully ({} bytes)", filename, copied_content.len());
                    copied_files += 1;
                } else {
                    println!("  ✗ {} copied but content differs", filename);
                }
            } else {
                println!("  ✗ {} not found in destination", filename);
            }
        }
        
        // Check subdirectory file
        let subdir_file = audio_dir.join("Old Recording.m4a");
        if subdir_file.exists() {
            println!("  ✓ Old Recording.m4a copied successfully");
            copied_files += 1;
        } else {
            println!("  ✗ Old Recording.m4a not found");
        }
        
        // Check database records
        let db_path = dest_dir.join("CiderPress-db.sqlite");
        let db = Database::new(&db_path)?;
        let slices = db.list_all_slices()?;
        
        println!("Database verification:");
        println!("  Slice records created: {}", slices.len());
        for slice in &slices {
            println!("    - {} ({} bytes, {}s est.)", 
                slice.original_audio_file_name, 
                slice.audio_file_size,
                slice.estimated_time_to_transcribe
            );
        }
        
        // Final verification
        let expected_files = test_files.len() + 1; // +1 for subdirectory file
        if copied_files == expected_files && slices.len() == expected_files {
            println!("🎉 SUCCESS: All {} files copied and {} database records created!", copied_files, slices.len());
        } else {
            println!("❌ FAILURE: Expected {} files and records, got {} files and {} records", 
                expected_files, copied_files, slices.len());
            return Err(anyhow::anyhow!("Migration verification failed"));
        }
        
        Ok(())
    }
} 
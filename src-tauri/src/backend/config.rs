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
use dirs::home_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Result of validating the Voice Memos directory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", content = "message")]
pub enum VoiceMemoValidation {
    /// Directory exists, contains DB and recordings
    Valid,
    /// macOS denied access — FDA not granted or signing identity mismatch
    PermissionDenied,
    /// Directory path does not exist
    NotFound,
    /// Directory exists but CloudRecordings.db is missing
    NoDatabaseFound,
    /// Directory exists with DB but no .m4a files
    NoRecordings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub voice_memo_root: String,
    pub ciderpress_home: String,
    pub model_name: String,
    pub first_run_complete: bool,
    #[serde(default = "default_skip_already_transcribed")]
    pub skip_already_transcribed: bool,
    #[serde(default)]
    pub password_enabled: bool,
    #[serde(default)]
    pub password_hash: Option<String>,
    #[serde(default = "default_lock_timeout_minutes")]
    pub lock_timeout_minutes: u32,
}

fn default_lock_timeout_minutes() -> u32 {
    5
}

fn default_skip_already_transcribed() -> bool {
    true // Default to skipping already transcribed slices
}

impl Default for Config {
    fn default() -> Self {
        let home = home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let default_voice_memo_root = home
            .join("Library/Group Containers/group.com.apple.VoiceMemos.shared/Recordings")
            .to_string_lossy()
            .to_string();
        let ciderpress_home = home.join(".ciderpress").to_string_lossy().to_string();

        Config {
            voice_memo_root: default_voice_memo_root,
            ciderpress_home,
            model_name: "base.en".to_string(),
            first_run_complete: false,
            skip_already_transcribed: true,
            password_enabled: false,
            password_hash: None,
            lock_timeout_minutes: 5,
        }
    }
}

impl Config {
    pub fn load() -> Result<Config> {
        let config_path = Self::config_path()?;
        
        if !config_path.exists() {
            let config = Config::default();
            config.save()?;
            return Ok(config);
        }

        let contents = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {:?}", config_path))?;
        
        let config: Config = toml::from_str(&contents)
            .with_context(|| "Failed to parse config file")?;
        
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;
        
        // Ensure parent directory exists
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory: {:?}", parent))?;
        }

        let contents = toml::to_string_pretty(self)
            .with_context(|| "Failed to serialize config")?;
        
        fs::write(&config_path, contents)
            .with_context(|| format!("Failed to write config file: {:?}", config_path))?;
        
        Ok(())
    }

    pub fn config_path() -> Result<PathBuf> {
        let home = home_dir().context("Failed to get home directory")?;
        Ok(home.join(".ciderpress").join("ciderpress-settings.toml"))
    }

    pub fn ciderpress_home_path(&self) -> PathBuf {
        PathBuf::from(&self.ciderpress_home)
    }

    pub fn voice_memo_root_path(&self) -> PathBuf {
        PathBuf::from(&self.voice_memo_root)
    }

    pub fn audio_dir(&self) -> PathBuf {
        self.ciderpress_home_path().join("audio")
    }

    pub fn transcript_dir(&self) -> PathBuf {
        self.ciderpress_home_path().join("transcripts")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.ciderpress_home_path().join("logs")
    }

    /// Validate that the voice memo root contains the expected files.
    /// Returns a structured result distinguishing permission errors from missing dirs.
    pub fn validate_voice_memo_root(&self) -> VoiceMemoValidation {
        let root = self.voice_memo_root_path();

        // On macOS, protected directories (like Voice Memos) return false for
        // .exists() when FDA is not granted — the OS reports EPERM, which
        // std::fs::metadata() turns into an error, making .exists() false.
        // We distinguish "permission denied" from "truly missing" by checking
        // whether the parent directory exists and is accessible.
        if !root.exists() {
            // Check if parent is accessible to distinguish permission denied from missing
            if let Some(parent) = root.parent() {
                match fs::read_dir(parent) {
                    Ok(_) => {
                        // Parent is accessible but the target dir doesn't exist
                        return VoiceMemoValidation::NotFound;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                        return VoiceMemoValidation::PermissionDenied;
                    }
                    Err(_) => {
                        // Parent doesn't exist or other error — treat as not found
                        return VoiceMemoValidation::NotFound;
                    }
                }
            }
            return VoiceMemoValidation::NotFound;
        }

        // Directory exists — try to read it
        match fs::read_dir(&root) {
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                return VoiceMemoValidation::PermissionDenied;
            }
            Err(_) => {
                return VoiceMemoValidation::NotFound;
            }
            Ok(entries) => {
                let mut has_db = false;
                let mut has_m4a = false;

                // Check for CloudRecordings.db separately (it may be in the dir root)
                if root.join("CloudRecordings.db").exists() {
                    has_db = true;
                }

                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        if ext.eq_ignore_ascii_case("m4a") {
                            has_m4a = true;
                        }
                    }
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if name == "CloudRecordings.db" {
                            has_db = true;
                        }
                    }
                    if has_db && has_m4a {
                        break;
                    }
                }

                if !has_db {
                    return VoiceMemoValidation::NoDatabaseFound;
                }
                if !has_m4a {
                    return VoiceMemoValidation::NoRecordings;
                }

                VoiceMemoValidation::Valid
            }
        }
    }

    /// Ensure CiderPress home directory and subdirectories exist
    pub fn ensure_ciderpress_home(&self) -> Result<()> {
        let home = self.ciderpress_home_path();
        let audio_dir = self.audio_dir();
        let transcript_dir = self.transcript_dir();
        let logs_dir = self.logs_dir();

        fs::create_dir_all(&home)
            .with_context(|| format!("Failed to create CiderPress home: {:?}", home))?;

        fs::create_dir_all(&audio_dir)
            .with_context(|| format!("Failed to create audio directory: {:?}", audio_dir))?;

        fs::create_dir_all(&transcript_dir)
            .with_context(|| format!("Failed to create transcript directory: {:?}", transcript_dir))?;

        fs::create_dir_all(&logs_dir)
            .with_context(|| format!("Failed to create logs directory: {:?}", logs_dir))?;

        Ok(())
    }
} 
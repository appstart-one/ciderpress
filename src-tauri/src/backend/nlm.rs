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

use std::process::Command;
use std::path::PathBuf;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tracing::debug;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NlmNotebook {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NlmStatus {
    pub authenticated: bool,
    pub binary_available: bool,
    pub binary_path: Option<String>,
    pub current_profile: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NlmAccountInfo {
    pub profile_name: String,
    pub has_credentials: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NlmBrowserProfile {
    pub name: String,
    pub display_name: String,
}

/// Resolve the path to the NLM sidecar binary.
/// In development, it's in src-tauri/binaries/nlm-{target_triple}
/// In production, it's next to the application binary.
pub fn resolve_nlm_path() -> Result<PathBuf> {
    // First try the sidecar path (next to our binary)
    let current_exe = std::env::current_exe()?;
    let exe_dir = current_exe.parent()
        .ok_or_else(|| anyhow!("Cannot determine executable directory"))?;

    // In a Tauri bundle, sidecars are placed next to the main binary
    let sidecar_path = exe_dir.join("nlm");
    if sidecar_path.exists() {
        return Ok(sidecar_path);
    }

    // In development, check the binaries directory
    let target_triple = get_target_triple();
    let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("binaries")
        .join(format!("nlm-{}", target_triple));
    if dev_path.exists() {
        return Ok(dev_path);
    }

    Err(anyhow!("NLM binary not found. Run scripts/build-nlm.sh to build it."))
}

fn get_target_triple() -> &'static str {
    if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "aarch64-apple-darwin"
        } else {
            "x86_64-apple-darwin"
        }
    } else if cfg!(target_os = "linux") {
        if cfg!(target_arch = "aarch64") {
            "aarch64-unknown-linux-gnu"
        } else {
            "x86_64-unknown-linux-gnu"
        }
    } else {
        "x86_64-unknown-linux-gnu"
    }
}

/// Run an NLM command and return its output (with a 30-second timeout).
pub fn run_nlm(args: &[&str]) -> Result<String> {
    let nlm_path = resolve_nlm_path()?;
    debug!("Running NLM: {} {:?}", nlm_path.display(), args);

    let mut child = Command::new(&nlm_path)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| anyhow!("Failed to execute NLM: {}", e))?;

    // Wait with a 30-second timeout to prevent hanging the app
    let timeout = std::time::Duration::from_secs(30);
    let start = std::time::Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = child.stdout.take()
                    .map(|mut s| { let mut buf = String::new(); std::io::Read::read_to_string(&mut s, &mut buf).ok(); buf })
                    .unwrap_or_default();
                let stderr = child.stderr.take()
                    .map(|mut s| { let mut buf = String::new(); std::io::Read::read_to_string(&mut s, &mut buf).ok(); buf })
                    .unwrap_or_default();

                if status.success() {
                    return Ok(stdout);
                } else {
                    return Err(anyhow!("NLM command failed: {}{}", stderr, stdout));
                }
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    return Err(anyhow!("NLM command timed out after 30 seconds"));
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => {
                return Err(anyhow!("Failed to wait for NLM: {}", e));
            }
        }
    }
}

/// Get the NLM env file path (~/.nlm/env).
fn nlm_env_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".nlm")
        .join("env")
}

/// Read the current browser profile from ~/.nlm/env.
pub fn get_current_profile() -> Option<String> {
    let env_path = nlm_env_path();
    if !env_path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&env_path).ok()?;
    for line in content.lines() {
        if let Some(value) = line.strip_prefix("NLM_BROWSER_PROFILE=") {
            let profile = value.trim().trim_matches('"');
            if profile.is_empty() {
                return Some("Default".to_string());
            }
            return Some(profile.to_string());
        }
    }
    None
}

/// List available Chromium browser profiles on macOS.
pub fn list_browser_profiles() -> Vec<NlmBrowserProfile> {
    let mut profiles = Vec::new();

    // Check common Chromium browser locations on macOS
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return profiles,
    };

    let browser_paths = [
        (home.join("Library/Application Support/Google/Chrome"), "Chrome"),
        (home.join("Library/Application Support/Google/Chrome Canary"), "Chrome Canary"),
        (home.join("Library/Application Support/BraveSoftware/Brave-Browser"), "Brave"),
        (home.join("Library/Application Support/Microsoft Edge"), "Edge"),
        (home.join("Library/Application Support/Chromium"), "Chromium"),
    ];

    for (browser_path, browser_name) in &browser_paths {
        if !browser_path.exists() {
            continue;
        }

        // Check "Default" profile
        let default_prefs = browser_path.join("Default/Preferences");
        if default_prefs.exists() {
            let display = extract_profile_display_name(&default_prefs, browser_name, "Default");
            profiles.push(NlmBrowserProfile {
                name: format!("{}:Default", browser_name),
                display_name: display,
            });
        }

        // Check numbered profiles (Profile 1, Profile 2, etc.)
        if let Ok(entries) = std::fs::read_dir(browser_path) {
            for entry in entries.flatten() {
                let dir_name = entry.file_name().to_string_lossy().to_string();
                if dir_name.starts_with("Profile ") {
                    let prefs_path = entry.path().join("Preferences");
                    if prefs_path.exists() {
                        let display = extract_profile_display_name(&prefs_path, browser_name, &dir_name);
                        profiles.push(NlmBrowserProfile {
                            name: format!("{}:{}", browser_name, dir_name),
                            display_name: display,
                        });
                    }
                }
            }
        }
    }

    profiles
}

/// Extract a human-readable profile display name from Chrome Preferences JSON.
fn extract_profile_display_name(prefs_path: &PathBuf, browser_name: &str, profile_dir: &str) -> String {
    if let Ok(content) = std::fs::read_to_string(prefs_path) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            // Try to get the profile name from preferences
            if let Some(name) = json.pointer("/profile/name").and_then(|v| v.as_str()) {
                if !name.is_empty() {
                    // Also try to get the Google account email
                    if let Some(email) = json.pointer("/account_info/0/email").and_then(|v| v.as_str()) {
                        return format!("{} ({}) [{}]", name, email, browser_name);
                    }
                    return format!("{} [{}]", name, browser_name);
                }
            }
            // Try account_info for email
            if let Some(email) = json.pointer("/account_info/0/email").and_then(|v| v.as_str()) {
                return format!("{} [{}]", email, browser_name);
            }
        }
    }
    format!("{} [{}]", profile_dir, browser_name)
}

/// Check if NLM credentials exist in ~/.nlm/env (non-empty auth token).
fn has_credentials() -> bool {
    let env_path = nlm_env_path();
    if !env_path.exists() {
        return false;
    }
    if let Ok(content) = std::fs::read_to_string(&env_path) {
        for line in content.lines() {
            if let Some(value) = line.strip_prefix("NLM_AUTH_TOKEN=") {
                let token = value.trim().trim_matches('"');
                if !token.is_empty() {
                    return true;
                }
            }
        }
    }
    false
}

/// Check if NLM is available and authenticated.
/// This is a fast, non-blocking check (reads local files only, never spawns NLM).
pub fn get_nlm_status() -> NlmStatus {
    let binary_path = resolve_nlm_path().ok();
    let binary_available = binary_path.is_some();
    let current_profile = get_current_profile();
    let authenticated = binary_available && has_credentials();

    NlmStatus {
        authenticated,
        binary_available,
        binary_path: binary_path.map(|p| p.to_string_lossy().to_string()),
        current_profile,
    }
}

/// List notebooks from NotebookLM.
pub fn list_notebooks() -> Result<Vec<NlmNotebook>> {
    let output = run_nlm(&["list"])?;
    parse_notebook_list(&output)
}

/// Parse the output of `nlm list` into notebook structs.
/// Output format:
///   Total notebooks: N (showing first 10)
///
///   ID                                   TITLE                                    SOURCES LAST UPDATED
///   905d5947-137a-49ba-9c68-3c7fd86d800e 📙 Testing1                              0       2026-01-23T19:12:24Z
fn parse_notebook_list(output: &str) -> Result<Vec<NlmNotebook>> {
    let mut notebooks = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || line.len() < 36 {
            continue;
        }

        // Lines must start with a UUID (36 chars: 8-4-4-4-12 hex + dashes)
        let potential_id = &line[..36];
        let bytes = potential_id.as_bytes();
        if !bytes.iter().enumerate().all(|(i, &b)| {
            if i == 8 || i == 13 || i == 18 || i == 23 {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        }) {
            continue;
        }

        let id = potential_id.to_string();
        // Strip control characters (nlm outputs 0x08 backspace for column alignment)
        let rest: String = line[36..].chars()
            .filter(|c| !c.is_control())
            .collect();
        let rest = rest.trim();

        // rest contains: TITLE  SOURCES_COUNT  TIMESTAMP
        // Parse from right: last token is ISO timestamp, second-to-last is sources count
        let words: Vec<&str> = rest.split_whitespace().collect();

        let title = if words.len() >= 2 {
            let last = words[words.len() - 1];
            let second_last = words[words.len() - 2];
            if last.contains('T') && last.ends_with('Z')
                && second_last.chars().all(|c| c.is_ascii_digit())
            {
                words[..words.len() - 2].join(" ")
            } else {
                rest.to_string()
            }
        } else {
            rest.to_string()
        };

        notebooks.push(NlmNotebook {
            id,
            title: if title.is_empty() { "(untitled)".to_string() } else { title },
        });
    }

    Ok(notebooks)
}

/// Add a text source to a notebook.
pub fn add_text_to_notebook(notebook_id: &str, text: &str, title: Option<&str>) -> Result<String> {
    // Write text to a temp file and add it as a source
    let temp_dir = std::env::temp_dir();
    let filename = title.unwrap_or("ciderpress-upload.txt");
    let temp_path = temp_dir.join(filename);
    std::fs::write(&temp_path, text)?;

    let result = run_nlm(&["add", notebook_id, temp_path.to_str().unwrap_or("")]);

    // Clean up temp file
    let _ = std::fs::remove_file(&temp_path);

    result
}

/// Add an audio file as a source to a notebook.
pub fn add_audio_to_notebook(notebook_id: &str, audio_path: &str) -> Result<String> {
    run_nlm(&["add", notebook_id, audio_path])
}

/// Initiate NLM authentication with the default profile.
pub fn start_auth() -> Result<String> {
    run_nlm(&["auth", "login"])
}

/// Authenticate with a specific browser profile.
/// The profile_name may be prefixed with "Browser:" (e.g. "Chrome:Default").
/// We strip the prefix and pass just the profile directory name to NLM.
pub fn auth_with_profile(profile_name: &str) -> Result<String> {
    let dir_name = profile_name.split_once(':')
        .map(|(_, dir)| dir)
        .unwrap_or(profile_name);
    run_nlm(&["auth", "login", "-profile", dir_name])
}

/// Create a new notebook with the given title.
pub fn create_notebook(title: &str) -> Result<String> {
    run_nlm(&["create", title])
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NlmNotebookDetails {
    pub id: String,
    pub title: String,
    pub sources: String,
    pub notes: String,
    pub analytics: String,
}

/// Get detailed information about a notebook (sources, notes, analytics).
pub fn get_notebook_details(notebook_id: &str, title: &str) -> Result<NlmNotebookDetails> {
    let sources = run_nlm(&["sources", notebook_id]).unwrap_or_else(|e| format!("Error: {}", e));
    let notes = run_nlm(&["notes", notebook_id]).unwrap_or_else(|e| format!("Error: {}", e));
    let analytics = run_nlm(&["analytics", notebook_id]).unwrap_or_else(|e| format!("Error: {}", e));

    Ok(NlmNotebookDetails {
        id: notebook_id.to_string(),
        title: title.to_string(),
        sources,
        notes,
        analytics,
    })
}
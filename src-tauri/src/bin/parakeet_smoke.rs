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

//! Standalone end-to-end smoke test for the Parakeet transcription path.
//!
//! Downloads the Parakeet TDT v2 model (~460 MB, once) and transcribes a
//! bundled 16 kHz mono sample, printing the transcript and timing.
//!
//!   cargo run --release --bin parakeet_smoke [-- <model-name> <wav-path>]

// Reuse the exact production module (self-contained, no crate:: refs).
// The bin only calls a subset of its API; silence dead-code for the rest.
#[path = "../backend/parakeet.rs"]
#[allow(dead_code)]
mod parakeet;

use std::path::PathBuf;
use std::time::Instant;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let model_name = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "parakeet-tdt-0.6b-v2".to_string());

    let wav_path = args.get(2).cloned().unwrap_or_else(|| {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest_dir
            .parent()
            .unwrap()
            .join("test-audio")
            .join("20250427 162429-5441EC7D.wav")
            .to_string_lossy()
            .into_owned()
    });

    println!("Model: {}", model_name);
    println!("WAV:   {}", wav_path);

    let rt = tokio::runtime::Runtime::new()?;
    let dl_start = Instant::now();
    rt.block_on(async {
        parakeet::download_model(&model_name, |pct| {
            print!("\r  download: {:.0}%   ", pct);
            use std::io::Write;
            let _ = std::io::stdout().flush();
        })
        .await
    })?;
    println!("\n  model ready in {:.1}s", dl_start.elapsed().as_secs_f64());

    let t0 = Instant::now();
    let text = parakeet::transcribe(&model_name, &wav_path)?;
    let elapsed = t0.elapsed();

    println!("\n=== TRANSCRIPT ===");
    println!("{}", text);
    println!("=== transcribed in {:.2}s ===", elapsed.as_secs_f64());

    if text.trim().is_empty() {
        anyhow::bail!("transcript was empty");
    }
    Ok(())
}

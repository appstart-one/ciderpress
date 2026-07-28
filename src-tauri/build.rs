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

/// Stamp the build with git provenance, exposed to the crate via `env!`.
///
/// The build number is the commit count rather than a stored counter: it is
/// monotonic by construction, identical for anyone building the same commit, has
/// no state file to drift or merge-conflict, and cannot be forgotten during a
/// release — which matters because releases are built from a developer machine,
/// so there is no build server to own the number.
fn emit_build_metadata() {
    // Without these, cargo caches this build script's output and the reported
    // hash silently goes stale — a confidently wrong hash sends bug reports at
    // the wrong source, which is worse than showing nothing.
    let git_dir = std::path::Path::new("../.git");
    println!("cargo:rerun-if-changed=../.git/HEAD");
    if let Ok(head) = std::fs::read_to_string(git_dir.join("HEAD")) {
        // Detached HEAD has no ref to watch; on a branch, the ref file is what
        // actually changes when a commit lands.
        if let Some(reference) = head.strip_prefix("ref: ").map(str::trim) {
            println!("cargo:rerun-if-changed=../.git/{}", reference);
        }
    }
    println!("cargo:rerun-if-changed=../.git/index");
    // Also refresh on source edits, so the -dirty marker tracks Rust changes.
    // It still cannot be exact: the marker reflects the tree at the last
    // build-script run, and "the tree became dirty" is not expressible as a
    // rerun trigger without watching every file in the repo. Release builds are
    // the case that matters and they build from a clean, committed tree.
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=Cargo.toml");

    let git = |args: &[&str]| -> Option<String> {
        let out = std::process::Command::new("git").args(args).output().ok()?;
        if !out.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if s.is_empty() { None } else { Some(s) }
    };

    // A source tarball has no .git at all; degrade rather than fail the build.
    let count = git(&["rev-list", "--count", "HEAD"]).unwrap_or_else(|| "0".into());
    let short = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let full = git(&["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".into());

    // An unqualified hash asserts "this binary is exactly that commit", which is
    // false for any build with uncommitted changes.
    let dirty = git(&["status", "--porcelain"]).is_some();
    let short = if dirty { format!("{}-dirty", short) } else { short };

    println!("cargo:rustc-env=CIDERPRESS_BUILD_NUMBER={}", count);
    println!("cargo:rustc-env=CIDERPRESS_COMMIT_SHORT={}", short);
    println!("cargo:rustc-env=CIDERPRESS_COMMIT_FULL={}", full);
}

fn main() {
  emit_build_metadata();

  // Set macOS deployment target to 11.0 for C++17 std::filesystem and Metal GPU support
  std::env::set_var("MACOSX_DEPLOYMENT_TARGET", "11.0");
  std::env::set_var("CMAKE_OSX_DEPLOYMENT_TARGET", "11.0");
  std::env::set_var("CXXFLAGS", "-std=c++17 -mmacosx-version-min=11.0");

  // Link clang compiler-rt builtins for ___isPlatformVersionAtLeast
  // (needed by Metal's @available() checks in whisper.cpp)
  if let Ok(output) = std::process::Command::new("xcrun")
      .args(["clang", "--print-resource-dir"])
      .output()
  {
      if output.status.success() {
          let resource_dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
          let lib_dir = format!("{}/lib/darwin", resource_dir);
          println!("cargo:rustc-link-search=native={}", lib_dir);
          println!("cargo:rustc-link-lib=static=clang_rt.osx");
      }
  }

  tauri_build::build()
}
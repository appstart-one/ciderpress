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

fn main() {
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
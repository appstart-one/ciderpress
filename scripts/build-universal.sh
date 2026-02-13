#!/bin/bash
# VoiceMemoLiberator - Voice memo transcription and management tool
# Copyright (C) 2026 APPSTART LLC
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program.  If not, see <https://www.gnu.org/licenses/>.

# Build a macOS universal binary (.app) containing both arm64 and x86_64.
#
# Prerequisites: Go, nasm (brew install nasm), Rust targets for both architectures
#
# This script:
#   1. Builds the NLM sidecar for both architectures + universal fat binary
#   2. Patches ffmpeg-sys-next upstream bug (macos vs darwin target-os)
#   3. Runs tauri build with --target universal-apple-darwin

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "=== Building macOS Universal Binary ==="

# Check prerequisites
if ! command -v nasm &> /dev/null; then
    echo "Error: nasm is required for x86_64 FFmpeg build. Install with: brew install nasm"
    exit 1
fi

echo ""
echo "--- Step 1: Build NLM sidecar (universal) ---"
"$SCRIPT_DIR/build-nlm.sh" --universal

echo ""
echo "--- Step 2: Patch ffmpeg-sys-next build.rs (upstream bug workaround) ---"
# ffmpeg-sys-next passes --target-os=macos to FFmpeg's configure, but FFmpeg
# expects --target-os=darwin. Fix by adding the "macos" => "darwin" mapping.
# See: https://github.com/zmwangx/rust-ffmpeg-sys/blob/master/build.rs
PATCHED=false
for build_rs in "$HOME"/.cargo/registry/src/*/ffmpeg-sys-next-*/build.rs; do
    if [ -f "$build_rs" ] && grep -q '"ios" => "darwin"' "$build_rs" && ! grep -q '"macos" => "darwin"' "$build_rs"; then
        echo "  Patching: $build_rs"
        sed -i '' 's/"ios" => "darwin"/"ios" | "macos" => "darwin"/' "$build_rs"
        PATCHED=true
    fi
done
# If we patched, clean cached ffmpeg build script binaries so Cargo recompiles
if [ "$PATCHED" = true ]; then
    echo "  Cleaning cached ffmpeg-sys-next build artifacts..."
    rm -rf "$PROJECT_ROOT/src-tauri/target/release/build/ffmpeg-sys-next-"*
    rm -rf "$PROJECT_ROOT/src-tauri/target/x86_64-apple-darwin/release/build/ffmpeg-sys-next-"*
    rm -rf "$PROJECT_ROOT/src-tauri/target/aarch64-apple-darwin/release/build/ffmpeg-sys-next-"*
fi

echo ""
echo "--- Step 3: Build Tauri app (universal) ---"
cd "$PROJECT_ROOT"
npm run tauri build -- --target universal-apple-darwin

echo ""
echo "=== Universal build complete ==="
echo "Check output in src-tauri/target/universal-apple-darwin/release/bundle/"

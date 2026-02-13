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
# This script:
#   1. Builds the NLM sidecar for both architectures
#   2. Runs tauri build with --target universal-apple-darwin

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "=== Building macOS Universal Binary ==="

echo ""
echo "--- Step 1: Build NLM sidecar (universal) ---"
"$SCRIPT_DIR/build-nlm.sh" --universal

echo ""
echo "--- Step 2: Build Tauri app (universal) ---"
cd "$PROJECT_ROOT"
npm run tauri build -- --target universal-apple-darwin

echo ""
echo "=== Universal build complete ==="
echo "Check output in src-tauri/target/universal-apple-darwin/release/bundle/"

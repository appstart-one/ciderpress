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

# Build the NLM (NotebookLM CLI) binary for bundling with the Tauri app.
# This script compiles NLM from source using Go and places it in src-tauri/binaries/
# with the appropriate target triple suffix for Tauri sidecar support.
#
# Usage:
#   ./build-nlm.sh              # Build for host architecture only (dev)
#   ./build-nlm.sh --universal  # Build for both arm64 and x86_64 (release)

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BINARIES_DIR="$PROJECT_ROOT/src-tauri/binaries"

# Check if Go is installed
if ! command -v go &> /dev/null; then
    echo "Error: Go is not installed. Install it from https://go.dev/dl/"
    exit 1
fi

mkdir -p "$BINARIES_DIR"

build_nlm() {
    local goarch="$1"
    local rust_target="$2"
    local output_path="$BINARIES_DIR/nlm-${rust_target}"

    echo "Building NLM for target: $rust_target (GOARCH=$goarch)"

    TMPDIR=$(mktemp -d)
    GOOS=darwin GOARCH="$goarch" GOBIN="$TMPDIR" go install github.com/tmc/nlm/cmd/nlm@latest
    mv "$TMPDIR/nlm" "$output_path"
    rm -rf "$TMPDIR"
    chmod +x "$output_path"

    echo "  Output: $output_path ($(du -h "$output_path" | cut -f1))"
}

if [ "$1" = "--universal" ]; then
    echo "Building NLM universal (arm64 + x86_64)..."
    build_nlm "arm64" "aarch64-apple-darwin"
    build_nlm "amd64" "x86_64-apple-darwin"
    echo "NLM universal build complete."
else
    # Build for host architecture only
    ARCH=$(uname -m)
    case "$ARCH" in
        x86_64)
            build_nlm "amd64" "x86_64-apple-darwin"
            ;;
        arm64|aarch64)
            build_nlm "arm64" "aarch64-apple-darwin"
            ;;
        *)
            echo "Unsupported architecture: $ARCH"
            exit 1
            ;;
    esac
    echo "NLM build complete."
fi

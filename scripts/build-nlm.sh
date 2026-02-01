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

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BINARIES_DIR="$PROJECT_ROOT/src-tauri/binaries"

# Determine target triple
ARCH=$(uname -m)
OS=$(uname -s)

case "$ARCH" in
    x86_64) RUST_ARCH="x86_64" ;;
    arm64|aarch64) RUST_ARCH="aarch64" ;;
    *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

case "$OS" in
    Darwin) RUST_TARGET="${RUST_ARCH}-apple-darwin" ;;
    Linux) RUST_TARGET="${RUST_ARCH}-unknown-linux-gnu" ;;
    *) echo "Unsupported OS: $OS"; exit 1 ;;
esac

OUTPUT_PATH="$BINARIES_DIR/nlm-${RUST_TARGET}"

# Check if Go is installed
if ! command -v go &> /dev/null; then
    echo "Error: Go is not installed. Install it from https://go.dev/dl/"
    exit 1
fi

echo "Building NLM for target: $RUST_TARGET"
echo "Output: $OUTPUT_PATH"

# Build NLM from source
mkdir -p "$BINARIES_DIR"

# Set GOBIN to a temp location and build
TMPDIR=$(mktemp -d)
GOBIN="$TMPDIR" go install github.com/tmc/nlm/cmd/nlm@latest

# Move to the correct location with target triple suffix
mv "$TMPDIR/nlm" "$OUTPUT_PATH"
rm -rf "$TMPDIR"

chmod +x "$OUTPUT_PATH"

echo "NLM built successfully: $OUTPUT_PATH"
echo "Binary size: $(du -h "$OUTPUT_PATH" | cut -f1)"
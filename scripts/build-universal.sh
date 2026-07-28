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
#   4. Verifies the app signature, re-signing only if tauri's has broken
#   5. Notarizes and staples the DMG (tauri only does the .app, not the DMG)
#   6. Verifies the finished DMG and fails the build if it is not publishable
#
# Notarization needs APPLE_ID, APPLE_PASSWORD (an app-specific password) and
# APPLE_TEAM_ID in the environment. Without them the build still succeeds but
# produces a DMG that Gatekeeper will reject; release.sh will not publish it.

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
echo "--- Step 4: Verify app signature, re-sign only if broken ---"
# The lipo step in universal builds can strip/invalidate signatures, which is
# why this step exists. It is now conditional: tauri signs the bundle itself
# using tauri.conf.json's signingIdentity, so when that has worked there is
# nothing to fix and re-signing is pure risk for no benefit.
#
# The risk is to the notarization ticket tauri staples to the .app: a ticket is
# bound to the signature's CDHash, so any re-sign that changes the CDHash
# orphans it. Measured on the 0.3.0 build, an identical re-sign (same identity,
# same options, same bits) left the ticket valid — so this was redundant rather
# than destructive in practice. Not a reason to keep doing it: "usually
# harmless" is a bad property for the step standing between a notarized build
# and a shipped one.
#
# Any re-sign must use the same stable Developer ID identity: the macOS
# folder-access grant (com.apple.macl) binds to the signing identity, so an
# ad-hoc re-sign would make users re-grant access after every update.
# Override with SIGNING_IDENTITY=... if needed.
SIGNING_IDENTITY="${SIGNING_IDENTITY:-$(python3 -c "import json; print(json.load(open('$PROJECT_ROOT/src-tauri/tauri.conf.json'))['bundle']['macOS']['signingIdentity'])")}"
BUNDLE_DIR="$PROJECT_ROOT/src-tauri/target/universal-apple-darwin/release/bundle"
APP_BUNDLE="$BUNDLE_DIR/macos/CiderPress.app"
if [ -d "$APP_BUNDLE" ]; then
    if codesign --verify --deep --strict "$APP_BUNDLE" 2>/dev/null; then
        echo "  Signature already valid; leaving it alone (a re-sign here would"
        echo "  strip the stapled notarization ticket)."
    else
        echo "  Signature invalid or missing — re-signing with $SIGNING_IDENTITY"
        codesign --force --options runtime --sign "$SIGNING_IDENTITY" --deep "$APP_BUNDLE"
        # A re-sign invalidates any stapled ticket, so re-staple if we can.
        if xcrun stapler staple "$APP_BUNDLE" 2>/dev/null; then
            echo "  Re-stapled notarization ticket."
        else
            echo "  Warning: could not re-staple. The .app is signed but its"
            echo "  ticket is missing; the DMG step below is what ships."
        fi
    fi
else
    echo "  Warning: App bundle not found at $APP_BUNDLE, skipping"
fi

echo ""
echo "--- Step 5: Notarize and staple the DMG ---"
# Tauri notarizes and staples the .app, then builds the DMG around it — but it
# never submits the DMG itself. An unnotarized DMG is rejected by Gatekeeper
# when the user mounts it, which is the very first thing they do, even though
# the app inside is perfectly notarized.
DMG_PATH="$(ls "$BUNDLE_DIR"/dmg/*.dmg 2>/dev/null | head -1 || true)"
if [ -z "$DMG_PATH" ]; then
    echo "  Warning: no DMG found under $BUNDLE_DIR/dmg, skipping"
elif [ -z "${APPLE_ID:-}" ] || [ -z "${APPLE_PASSWORD:-}" ] || [ -z "${APPLE_TEAM_ID:-}" ]; then
    echo "  APPLE_ID / APPLE_PASSWORD / APPLE_TEAM_ID not set — SKIPPING"
    echo "  notarization. This DMG will trip Gatekeeper on the user's machine."
    echo "  Fine for a local build; scripts/release.sh refuses to publish it."
    # Let the verify step report the gap without failing a credential-less build.
    export ALLOW_UNNOTARIZED=1
else
    echo "  Submitting $(basename "$DMG_PATH") to the Apple notary service..."
    xcrun notarytool submit "$DMG_PATH" \
        --apple-id "$APPLE_ID" \
        --password "$APPLE_PASSWORD" \
        --team-id "$APPLE_TEAM_ID" \
        --wait
    xcrun stapler staple "$DMG_PATH"
    echo "  Notarized and stapled: $(basename "$DMG_PATH")"
fi

echo ""
echo "--- Step 6: Verify the shipped artifact ---"
# Assert against the DMG itself rather than trusting the steps above. This is
# the check whose absence let 0.3.0 publish an unnotarized DMG.
if [ -n "$DMG_PATH" ]; then
    "$SCRIPT_DIR/verify-dmg.sh" "$DMG_PATH" || {
        echo ""
        echo "  DMG verification FAILED. Do not publish this artifact."
        exit 1
    }
fi

echo ""
echo "=== Universal build complete ==="
echo "Check output in $BUNDLE_DIR"

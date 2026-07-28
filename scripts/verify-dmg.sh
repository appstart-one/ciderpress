#!/bin/bash
# CiderPress - Voice Memo Liberator
# Copyright (C) 2026 APPSTART LLC
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.

# Assert that a DMG is fit to publish: signed, notarized, stapled, and
# containing a universal app that is itself signed, notarized and stapled.
#
# Exists because 0.3.0 shipped a DMG that was signed but never submitted to the
# notary service. The app inside was fine, so every "is it signed?" check
# passed while Gatekeeper still rejected the DMG on mount — the first thing a
# user does with it. Trust the artifact, not the build steps that produced it.
#
#   ./scripts/verify-dmg.sh path/to/CiderPress_x.y.z_universal.dmg
#
# Exit 0 only if every check passes. Set ALLOW_UNNOTARIZED=1 to downgrade the
# notarization failures to warnings, for local builds made without Apple
# credentials. Never set it in a release path.

set -uo pipefail

DMG="${1:-}"
if [ -z "$DMG" ] || [ ! -f "$DMG" ]; then
    echo "Usage: $0 <path-to-dmg>"
    exit 2
fi

ALLOW_UNNOTARIZED="${ALLOW_UNNOTARIZED:-0}"
FAILED=0
MOUNTPOINT=""

cleanup() {
    if [ -n "$MOUNTPOINT" ] && [ -d "$MOUNTPOINT" ]; then
        hdiutil detach "$MOUNTPOINT" -quiet 2>/dev/null || true
    fi
}
trap cleanup EXIT

pass() { echo "  PASS  $1"; }
fail() {
    echo "  FAIL  $1"
    FAILED=1
}
# A notarization problem is fatal for a release but tolerable for a local build.
soft_fail() {
    if [ "$ALLOW_UNNOTARIZED" = "1" ]; then
        echo "  WARN  $1 (tolerated: ALLOW_UNNOTARIZED=1)"
    else
        fail "$1"
    fi
}

echo "Verifying $(basename "$DMG")"
echo ""
echo "The DMG itself:"

if codesign --verify --verbose=1 "$DMG" 2>/dev/null; then
    pass "code signature is valid"
else
    fail "code signature is invalid or absent"
fi

# Capture command output into a variable before matching on it, never
# `cmd | grep -q`. Under `pipefail`, grep -q exits on first match, the producer
# takes SIGPIPE, and the pipeline reports failure even though the check passed.
# That bug made this script fail a DMG that was in fact perfectly notarized.
DMG_SIGINFO=$(codesign -dv --verbose=2 "$DMG" 2>&1 || true)
IDENTITY=$(printf '%s\n' "$DMG_SIGINFO" | sed -n 's/^Authority=//p' | sed -n 1p)
if [[ "$IDENTITY" == Developer\ ID\ Application:* ]]; then
    pass "signed by $IDENTITY"
else
    fail "not signed by a Developer ID Application identity (got: ${IDENTITY:-none})"
fi

if xcrun stapler validate "$DMG" >/dev/null 2>&1; then
    pass "notarization ticket is stapled"
else
    soft_fail "no stapled notarization ticket"
fi

# The check that actually predicts the user's experience on mount.
SPCTL=$(spctl -a -t open --context context:primary-signature -v "$DMG" 2>&1 || true)
if [[ "$SPCTL" == *accepted* ]]; then
    pass "Gatekeeper accepts it ($(printf '%s\n' "$SPCTL" | sed -n 's/.*\(source=.*\)/\1/p' | sed -n 1p))"
else
    soft_fail "Gatekeeper rejects it ($(printf '%s' "$SPCTL" | tr '\n' ' '))"
fi

echo ""
echo "The app inside:"

ATTACH_OUT=$(hdiutil attach -nobrowse -readonly "$DMG" 2>/dev/null || true)
MOUNTPOINT=$(printf '%s\n' "$ATTACH_OUT" | sed -n 's|.*\(/Volumes/.*\)|\1|p' | sed -n 1p)
if [ -z "$MOUNTPOINT" ]; then
    fail "could not mount the DMG"
    echo ""
    echo "RESULT: FAILED"
    exit 1
fi

APP=$(find "$MOUNTPOINT" -maxdepth 1 -name '*.app' 2>/dev/null | sed -n 1p)
if [ -z "$APP" ]; then
    fail "no .app found inside"
else
    if codesign --verify --deep --strict "$APP" 2>/dev/null; then
        pass "app signature is valid (deep, strict)"
    else
        fail "app signature is invalid"
    fi

    APP_SIGINFO=$(codesign -dv --verbose=2 "$APP" 2>&1 || true)
    if [[ "$APP_SIGINFO" == *"flags="*"runtime"* ]]; then
        pass "hardened runtime enabled"
    else
        fail "hardened runtime NOT enabled (notarization requires it)"
    fi

    if xcrun stapler validate "$APP" >/dev/null 2>&1; then
        pass "app notarization ticket is stapled"
    else
        soft_fail "app has no stapled ticket"
    fi

    APP_SPCTL=$(spctl -a -t exec -vv "$APP" 2>&1 || true)
    if [[ "$APP_SPCTL" == *accepted* ]]; then
        pass "Gatekeeper accepts the app for execution"
    else
        soft_fail "Gatekeeper rejects the app for execution"
    fi

    MAIN_BIN=$(find "$APP/Contents/MacOS" -maxdepth 1 -type f -perm +111 2>/dev/null | sed -n 1p)
    if [ -n "$MAIN_BIN" ]; then
        ARCHS=$(lipo -archs "$MAIN_BIN" 2>/dev/null)
        if [[ "$ARCHS" == *x86_64* && "$ARCHS" == *arm64* ]]; then
            pass "universal binary ($ARCHS)"
        else
            fail "not universal (got: ${ARCHS:-unknown})"
        fi
    else
        fail "no main executable found in Contents/MacOS"
    fi
fi

echo ""
if [ "$FAILED" -eq 0 ]; then
    echo "RESULT: OK — safe to publish"
    exit 0
fi
echo "RESULT: FAILED — do not publish"
exit 1

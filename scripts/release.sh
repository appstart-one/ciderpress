#!/bin/bash
# CiderPress - Voice Memo Liberator
# Copyright (C) 2026 APPSTART LLC
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.

# Release script: bump version (optional), build, and publish a GitHub release.
#
# Usage:
#   ./scripts/release.sh                    # Release current version
#   ./scripts/release.sh --bump patch       # 0.1.1 -> 0.1.2
#   ./scripts/release.sh --bump minor       # 0.1.1 -> 0.2.0
#   ./scripts/release.sh --bump major       # 0.1.1 -> 1.0.0
#   ./scripts/release.sh --bump 0.3.0       # Set explicit version
#   ./scripts/release.sh --skip-build       # Skip build, use existing DMG
#   ./scripts/release.sh --dry-run          # Show what would happen, don't execute
#
# The script will:
#   1. Optionally bump the version in package.json, tauri.conf.json, Cargo.toml
#   2. Commit and push the version bump
#   3. Build the universal macOS binary (unless --skip-build)
#   4. Generate release notes from commits since the last tag
#   5. Create a git tag and GitHub release with the DMG attached

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# --- Defaults ---
BUMP=""
SKIP_BUILD=false
DRY_RUN=false

# --- Parse arguments ---
while [[ $# -gt 0 ]]; do
    case "$1" in
        --bump)
            BUMP="$2"
            shift 2
            ;;
        --skip-build)
            SKIP_BUILD=true
            shift
            ;;
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        -h|--help)
            sed -n '12,22p' "$0"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Run with --help for usage."
            exit 1
            ;;
    esac
done

cd "$PROJECT_ROOT"

# --- Preflight checks ---
echo "=== CiderPress Release ==="
echo ""

# Check for clean working tree (staged or unstaged changes; untracked files are OK)
if ! git diff --quiet || ! git diff --cached --quiet; then
    echo "Error: Working tree has uncommitted changes. Commit or stash them first."
    git status --short
    exit 1
fi

# Check gh CLI
if ! command -v gh &> /dev/null; then
    echo "Error: gh CLI is required. Install with: brew install gh"
    exit 1
fi
if ! gh auth status &> /dev/null 2>&1; then
    echo "Error: gh CLI is not authenticated. Run: gh auth login"
    exit 1
fi

# --- Read current version ---
CURRENT_VERSION=$(grep '"version"' package.json | head -1 | sed 's/.*"version": *"\([^"]*\)".*/\1/')
echo "Current version: $CURRENT_VERSION"

# --- Compute new version ---
if [ -n "$BUMP" ]; then
    IFS='.' read -r V_MAJOR V_MINOR V_PATCH <<< "$CURRENT_VERSION"

    case "$BUMP" in
        patch)
            NEW_VERSION="$V_MAJOR.$V_MINOR.$((V_PATCH + 1))"
            ;;
        minor)
            NEW_VERSION="$V_MAJOR.$((V_MINOR + 1)).0"
            ;;
        major)
            NEW_VERSION="$((V_MAJOR + 1)).0.0"
            ;;
        *)
            # Treat as explicit version string — validate format
            if [[ ! "$BUMP" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
                echo "Error: Version must be in X.Y.Z format (got: $BUMP)"
                exit 1
            fi
            NEW_VERSION="$BUMP"
            ;;
    esac
    echo "New version:     $NEW_VERSION"
else
    NEW_VERSION="$CURRENT_VERSION"
    echo "No version bump requested."
fi

# --- Check that the tag doesn't already exist ---
TAG="$NEW_VERSION"
if git rev-parse "refs/tags/$TAG" &> /dev/null; then
    echo "Error: Tag '$TAG' already exists. Pick a different version or delete the tag."
    exit 1
fi

# --- Determine the last tag for release notes ---
LAST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "")
if [ -z "$LAST_TAG" ]; then
    echo "No previous tags found — release notes will cover the entire history."
    LOG_RANGE="HEAD"
else
    echo "Previous tag:    $LAST_TAG"
    LOG_RANGE="$LAST_TAG..HEAD"
fi

# --- Generate release notes ---
echo ""
echo "--- Generating release notes ---"

generate_release_notes() {
    local range="$1"
    local features="" fixes="" other=""

    while IFS= read -r line; do
        subject=$(echo "$line" | sed 's/^[a-f0-9]* //')
        if echo "$subject" | grep -qi "^Add\|^Implement\|^Support\|^Enable\|^Introduce"; then
            features+="- $subject"$'\n'
        elif echo "$subject" | grep -qi "^Fix\|^Hotfix\|^Patch\|^Correct\|^Resolve"; then
            fixes+="- $subject"$'\n'
        else
            other+="- $subject"$'\n'
        fi
    done <<< "$(git log "$range" --format="%h %s" --no-merges)"

    local notes=""
    if [ -n "$features" ]; then
        notes+="### New Features"$'\n'$'\n'
        notes+="$features"$'\n'
    fi
    if [ -n "$fixes" ]; then
        notes+="### Bug Fixes"$'\n'$'\n'
        notes+="$fixes"$'\n'
    fi
    if [ -n "$other" ]; then
        notes+="### Other Changes"$'\n'$'\n'
        notes+="$other"$'\n'
    fi

    echo "$notes"
}

RELEASE_NOTES=$(generate_release_notes "$LOG_RANGE")
echo "$RELEASE_NOTES"

if $DRY_RUN; then
    echo ""
    echo "[DRY RUN] Would bump version to $NEW_VERSION, tag as $TAG, and create GitHub release."
    echo "[DRY RUN] Exiting without making changes."
    exit 0
fi

# --- Bump version ---
if [ "$NEW_VERSION" != "$CURRENT_VERSION" ]; then
    echo ""
    echo "--- Bumping version: $CURRENT_VERSION -> $NEW_VERSION ---"

    # package.json
    sed -i '' "s/\"version\": \"$CURRENT_VERSION\"/\"version\": \"$NEW_VERSION\"/" package.json

    # src-tauri/tauri.conf.json
    sed -i '' "s/\"version\": \"$CURRENT_VERSION\"/\"version\": \"$NEW_VERSION\"/" src-tauri/tauri.conf.json

    # src-tauri/Cargo.toml  (version = "X.Y.Z" on its own line)
    sed -i '' "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" src-tauri/Cargo.toml

    echo "  Updated package.json, tauri.conf.json, Cargo.toml"

    git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml
    git commit -m "Bump version to $NEW_VERSION"
    git push
    echo "  Committed and pushed version bump."
fi

# --- Build ---
if $SKIP_BUILD; then
    echo ""
    echo "--- Skipping build (--skip-build) ---"
else
    echo ""
    echo "--- Building universal macOS binary ---"
    "$SCRIPT_DIR/build-universal.sh"
fi

# --- Locate DMG ---
DMG_UNIVERSAL="src-tauri/target/universal-apple-darwin/release/bundle/dmg/CiderPress_${NEW_VERSION}_universal.dmg"
DMG_AARCH64="src-tauri/target/release/bundle/dmg/CiderPress_${NEW_VERSION}_aarch64.dmg"

ASSETS=()
if [ -f "$DMG_UNIVERSAL" ]; then
    ASSETS+=("$DMG_UNIVERSAL")
    echo "Found: $DMG_UNIVERSAL"
fi
if [ -f "$DMG_AARCH64" ]; then
    ASSETS+=("$DMG_AARCH64")
    echo "Found: $DMG_AARCH64"
fi

if [ ${#ASSETS[@]} -eq 0 ]; then
    echo ""
    echo "Warning: No DMG files found for version $NEW_VERSION."
    echo "  Expected: $DMG_UNIVERSAL"
    echo "  or:       $DMG_AARCH64"
    echo ""
    read -rp "Create release without assets? [y/N] " confirm
    if [[ ! "$confirm" =~ ^[Yy]$ ]]; then
        echo "Aborted."
        exit 1
    fi
fi

# --- Tag and release ---
echo ""
echo "--- Creating tag and GitHub release: $TAG ---"

git tag -a "$TAG" -m "Release $TAG"
git push origin "$TAG"

RELEASE_TITLE="CiderPress v$TAG"

if [ ${#ASSETS[@]} -gt 0 ]; then
    gh release create "$TAG" "${ASSETS[@]}" \
        --title "$RELEASE_TITLE" \
        --notes "$RELEASE_NOTES" \
        --repo appstart-one/ciderpress
else
    gh release create "$TAG" \
        --title "$RELEASE_TITLE" \
        --notes "$RELEASE_NOTES" \
        --repo appstart-one/ciderpress
fi

echo ""
echo "=== Release $TAG published ==="
echo "https://github.com/appstart-one/ciderpress/releases/tag/$TAG"

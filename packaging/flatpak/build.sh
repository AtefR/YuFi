#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
MANIFEST="$ROOT/packaging/com.yufi.app.yml"
BUILD_DIR="$ROOT/dist/flatpak/build"
REPO_DIR="$ROOT/dist/flatpak/repo"
BUNDLE="$ROOT/dist/yufi.flatpak"
APP_ID="com.yufi.app"

flatpak-builder --force-clean --allow=network --repo="$REPO_DIR" "$BUILD_DIR" "$MANIFEST"
flatpak build-bundle "$REPO_DIR" "$BUNDLE" "$APP_ID"

echo "Wrote $BUNDLE"

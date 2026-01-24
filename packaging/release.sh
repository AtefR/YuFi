#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="$(awk -F '\"' '/^version =/ {print $2; exit}' Cargo.toml)"
if [[ -z "$VERSION" ]]; then
  echo "Failed to read version from Cargo.toml" >&2
  exit 1
fi
ARCH="${ARCH:-x86_64}"
DIST="$ROOT/dist"
STAGE="$DIST/stage"

rm -rf "$STAGE"
mkdir -p "$STAGE/usr/bin"
mkdir -p "$STAGE/usr/share/applications"
mkdir -p "$STAGE/usr/share/icons/hicolor/scalable/apps"
mkdir -p "$STAGE/usr/share/licenses/yufi"

cargo build --release

install -Dm755 target/release/yufi "$STAGE/usr/bin/yufi"
install -Dm644 packaging/com.yufi.app.desktop "$STAGE/usr/share/applications/com.yufi.app.desktop"
install -Dm644 packaging/com.yufi.app.svg "$STAGE/usr/share/icons/hicolor/scalable/apps/com.yufi.app.svg"
install -Dm644 LICENSE "$STAGE/usr/share/licenses/yufi/LICENSE"

for size in 32 64 128 256; do
  src="packaging/icons/com.yufi.app-${size}.png"
  if [[ -f "$src" ]]; then
    install -Dm644 "$src" "$STAGE/usr/share/icons/hicolor/${size}x${size}/apps/com.yufi.app.png"
  fi
done

mkdir -p "$DIST"
TARBALL="$DIST/yufi-$VERSION-$ARCH.tar.gz"
rm -f "$TARBALL"
tar -C "$STAGE" -czf "$TARBALL" usr

( cd "$DIST" && sha256sum "$(basename "$TARBALL")" > SHA256SUMS )

echo "Wrote $TARBALL"

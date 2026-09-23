#!/usr/bin/env bash
# Packages Antigravity Unlocker for Linux release into a .tar.gz archive.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DIST_DIR="$ROOT_DIR/dist"
STAGE_DIR="$DIST_DIR/antigravity-unlocker-linux-x86_64"
ARCHIVE_NAME="antigravity-unlocker-linux-x86_64.tar.gz"

echo "==> Building release binary with cargo..."
cargo build --release

echo "==> Preparing release staging directory..."
rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR"

echo "==> Copying binary and installation scripts..."
cp "$ROOT_DIR/target/release/ag_unlocker" "$STAGE_DIR/ag_unlocker"
cp "$ROOT_DIR/linux/install.sh" "$STAGE_DIR/install.sh"
cp "$ROOT_DIR/linux/launch.sh" "$STAGE_DIR/launch.sh"
cp "$ROOT_DIR/linux/Antigravity-Unlocker.desktop" "$STAGE_DIR/Antigravity-Unlocker.desktop"
cp "$ROOT_DIR/linux/README.md" "$STAGE_DIR/README.md"
if [ -f "$ROOT_DIR/linux/icon.png" ]; then
    cp "$ROOT_DIR/linux/icon.png" "$STAGE_DIR/icon.png"
fi

chmod +x "$STAGE_DIR/ag_unlocker" "$STAGE_DIR/install.sh" "$STAGE_DIR/launch.sh"

echo "==> Creating $ARCHIVE_NAME..."
tar -czf "$DIST_DIR/$ARCHIVE_NAME" -C "$DIST_DIR" "antigravity-unlocker-linux-x86_64"

echo
echo "✓ Release bundle created at: $DIST_DIR/$ARCHIVE_NAME"
echo "  Contents:"
tar -ztvf "$DIST_DIR/$ARCHIVE_NAME"
echo
sha256sum "$DIST_DIR/$ARCHIVE_NAME"

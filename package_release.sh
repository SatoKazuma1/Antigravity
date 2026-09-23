#!/usr/bin/env bash
# Packages Antigravity Unlocker for Linux release into .tar.gz archives.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DIST_DIR="$ROOT_DIR/dist"

VERSION=$(grep -m1 '^version = ' "$ROOT_DIR/Cargo.toml" | cut -d'"' -f2)
echo "==> Packaging Antigravity Unlocker v${VERSION} for Linux (x86_64)..."

echo "==> Building release binary with cargo..."
cargo build --release

rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"

STAGE_COMPAT="$DIST_DIR/AG_${VERSION}_linux"
STAGE_NAMED="$DIST_DIR/antigravity-unlocker-linux-x86_64"

copy_payload() {
    local target="$1"
    mkdir -p "$target"
    cp "$ROOT_DIR/target/release/ag_unlocker" "$target/ag_unlocker"
    cp "$ROOT_DIR/linux/install.sh" "$target/install.sh"
    if [ -f "$ROOT_DIR/linux/uninstall.sh" ]; then
        cp "$ROOT_DIR/linux/uninstall.sh" "$target/uninstall.sh"
    fi
    cp "$ROOT_DIR/linux/launch.sh" "$target/launch.sh"
    cp "$ROOT_DIR/linux/Antigravity-Unlocker.desktop" "$target/Antigravity-Unlocker.desktop"
    cp "$ROOT_DIR/linux/README.md" "$target/README.md"
    if [ -f "$ROOT_DIR/linux/icon.png" ]; then
        cp "$ROOT_DIR/linux/icon.png" "$target/icon.png"
    fi
    chmod +x "$target/ag_unlocker" "$target/install.sh" "$target/launch.sh"
    if [ -f "$target/uninstall.sh" ]; then
        chmod +x "$target/uninstall.sh"
    fi
}

echo "==> Preparing payloads..."
copy_payload "$STAGE_COMPAT"
copy_payload "$STAGE_NAMED"

echo "==> Packaging AG_${VERSION}_linux.tar.gz (for tui.sh & direct installs)..."
tar -czf "$DIST_DIR/AG_${VERSION}_linux.tar.gz" -C "$DIST_DIR" "AG_${VERSION}_linux"

echo "==> Packaging antigravity-unlocker-linux-x86_64.tar.gz..."
tar -czf "$DIST_DIR/antigravity-unlocker-linux-x86_64.tar.gz" -C "$DIST_DIR" "antigravity-unlocker-linux-x86_64"

# Standalone binary
cp "$ROOT_DIR/target/release/ag_unlocker" "$DIST_DIR/ag_unlocker"

echo
echo "✓ Release bundles created in $DIST_DIR:"
ls -lh "$DIST_DIR"/*.tar.gz "$DIST_DIR/ag_unlocker"
echo
sha256sum "$DIST_DIR"/*.tar.gz "$DIST_DIR/ag_unlocker"

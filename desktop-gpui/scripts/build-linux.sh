#!/usr/bin/env bash
# Build release package for WA Bot Desktop GPUI on Linux.
#
# Requirements:
#   - Rust stable (cargo)
#   - Go 1.22+
#   - Linux GUI libraries:
#     Debian/Ubuntu: sudo apt install build-essential pkg-config libasound2-dev libx11-xcb-dev libssl-dev libxkbcommon-x11-dev libxkbcommon-dev
#     Fedora/RHEL:   sudo dnf install gcc pkgconf-pkg-config alsa-lib-devel libX11-devel openssl-devel libxkbcommon-devel libxkbcommon-x11-devel
#     Arch:          sudo pacman -S base-devel pkgconf alsa-lib libxkbcommon libxkbcommon-x11 openssl
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

OUTPUT_DIR="${OUTPUT_DIR:-$ROOT_DIR/compiled/gpui-linux}"
mkdir -p "$OUTPUT_DIR/be"

echo "==> [1/3] Building Go backend (Linux release)..."
go build -trimpath -ldflags "-s -w" -o "$ROOT_DIR/wa-bot-backend" ./cmd/api
cp "$ROOT_DIR/wa-bot-backend" "$OUTPUT_DIR/be/wa-bot-backend"
cp "$ROOT_DIR/wa-bot-backend" "$OUTPUT_DIR/wa-bot-backend"

echo "==> [2/3] Building GPUI desktop shell (Linux release)..."
cargo build --release --locked --manifest-path "$ROOT_DIR/desktop-gpui/Cargo.toml" -p wabot

cp "$ROOT_DIR/desktop-gpui/target/release/wabot" "$OUTPUT_DIR/wabot"
chmod +x "$OUTPUT_DIR/wabot" "$OUTPUT_DIR/wa-bot-backend" "$OUTPUT_DIR/be/wa-bot-backend"

echo "==> [3/3] Checking optional packaging (cargo-deb)..."
if command -v cargo-deb >/dev/null 2>&1; then
    echo "cargo-deb found. Generating Debian package (.deb)..."
    (cd "$ROOT_DIR/desktop-gpui" && cargo deb -p wabot --no-build) || true
    if compgen -G "$ROOT_DIR/desktop-gpui/target/debian/*.deb" > /dev/null; then
        cp "$ROOT_DIR"/desktop-gpui/target/debian/*.deb "$OUTPUT_DIR/" 2>/dev/null || true
        echo "Debian package copied to $OUTPUT_DIR"
    fi
else
    echo "cargo-deb not installed (optional). Skipping .deb creation."
    echo "Install via: cargo install cargo-deb"
fi

echo ""
echo "=== GPUI Linux Release Build Complete ==="
echo "Output Directory: $OUTPUT_DIR"
echo "  - $OUTPUT_DIR/wabot"
echo "  - $OUTPUT_DIR/wa-bot-backend"
echo "  - $OUTPUT_DIR/be/wa-bot-backend"
echo "To run:"
echo "  cd $OUTPUT_DIR && ./wabot"

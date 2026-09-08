#!/usr/bin/env bash
# Build release package for WA Bot Desktop GPUI on Windows (or cross-compiling from Linux).
#
# Requirements:
#   - Rust with x86_64-pc-windows-gnu target: rustup target add x86_64-pc-windows-gnu
#   - MinGW GCC (for CGO & cross-compilation on Linux):
#     Debian/Ubuntu: sudo apt install mingw-w64
#     Fedora/RHEL:   sudo dnf install mingw64-gcc
#     Arch:          sudo pacman -S mingw-w64-gcc
#   - Go 1.22+
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

OUTPUT_DIR="${OUTPUT_DIR:-$ROOT_DIR/compiled/gpui-windows}"
mkdir -p "$OUTPUT_DIR/be"

IS_WINDOWS=0
if [[ "${OS:-}" == "Windows_NT" ]] || [[ "$(uname -s)" =~ (MINGW|MSYS|CYGWIN) ]]; then
    IS_WINDOWS=1
fi

echo "==> [1/3] Building Go backend (Windows release)..."
if [[ "$IS_WINDOWS" -eq 1 ]]; then
    go build -trimpath -ldflags "-s -w" -o "$ROOT_DIR/wa-bot-backend.exe" ./cmd/api
else
    if command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
        CGO_ENABLED=1 CC=x86_64-w64-mingw32-gcc GOOS=windows GOARCH=amd64 \
            go build -trimpath -ldflags "-s -w" -o "$ROOT_DIR/wa-bot-backend.exe" ./cmd/api
    else
        echo "WARNING: x86_64-w64-mingw32-gcc not found. Building backend with CGO_ENABLED=0."
        echo "For SQLite CGO support, install mingw-w64."
        CGO_ENABLED=0 GOOS=windows GOARCH=amd64 \
            go build -trimpath -ldflags "-s -w" -o "$ROOT_DIR/wa-bot-backend.exe" ./cmd/api
    fi
fi
cp "$ROOT_DIR/wa-bot-backend.exe" "$OUTPUT_DIR/be/wa-bot-backend.exe"
cp "$ROOT_DIR/wa-bot-backend.exe" "$OUTPUT_DIR/wa-bot-backend.exe"

echo "==> [2/3] Building GPUI desktop shell (Windows release)..."
if [[ "$IS_WINDOWS" -eq 1 ]]; then
    cargo build --release --locked --manifest-path "$ROOT_DIR/desktop-gpui/Cargo.toml" -p wabot
    cp "$ROOT_DIR/desktop-gpui/target/release/wabot.exe" "$OUTPUT_DIR/wabot.exe"
else
    echo "Cross-compiling for target x86_64-pc-windows-gnu..."
    cargo build --release --locked --manifest-path "$ROOT_DIR/desktop-gpui/Cargo.toml" --target x86_64-pc-windows-gnu -p wabot
    cp "$ROOT_DIR/desktop-gpui/target/x86_64-pc-windows-gnu/release/wabot.exe" "$OUTPUT_DIR/wabot.exe"
fi

echo "==> [3/3] Checking optional packaging (cargo-wix)..."
if [[ "$IS_WINDOWS" -eq 1 ]] && command -v cargo-wix >/dev/null 2>&1; then
    echo "cargo-wix found. Building MSI installer (.msi)..."
    (cd "$ROOT_DIR/desktop-gpui" && cargo wix -p wabot --no-build) || true
    if compgen -G "$ROOT_DIR/desktop-gpui/target/wix/*.msi" > /dev/null; then
        cp "$ROOT_DIR"/desktop-gpui/target/wix/*.msi "$OUTPUT_DIR/" 2>/dev/null || true
        echo "MSI installer copied to $OUTPUT_DIR"
    fi
else
    echo "cargo-wix not run (requires native Windows with WiX Toolset). Skipping .msi creation."
fi

echo ""
echo "=== GPUI Windows Release Build Complete ==="
echo "Output Directory: $OUTPUT_DIR"
echo "  - $OUTPUT_DIR/wabot.exe"
echo "  - $OUTPUT_DIR/wa-bot-backend.exe"
echo "  - $OUTPUT_DIR/be/wa-bot-backend.exe"
echo "To run on Windows:"
echo "  cd $OUTPUT_DIR && wabot.exe"

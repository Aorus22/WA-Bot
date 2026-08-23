#!/usr/bin/env bash
# Build the desktop-gtk binary on Windows.
# This script MUST be run inside the MSYS2 mingw64 shell:
#
#   C:\msys64\mingw64.exe desktop-gtk/scripts/build-windows.sh
#
# (or run from a regular cmd/powershell and let it invoke mingw64.exe)
#
# Required MSYS2 packages:
#   pacman -S --needed mingw-w64-x86_64-toolchain mingw-w64-x86_64-pkg-config \
#     mingw-w64-x86_64-gtk4 mingw-w64-x86_64-libadwaita \
#     mingw-w64-x86_64-gobject-introspection

set -euo pipefail

cd "$(dirname "$0")/.."

OUTPUT="${OUTPUT:-../compiled/wa-bot-desktop.exe}"
mkdir -p "$(dirname "$OUTPUT")"

echo "Building wa-bot-desktop (Windows) -> $OUTPUT"
CGO_ENABLED=1 \
PKG_CONFIG_PATH="${PKG_CONFIG_PATH:-/mingw64/lib/pkgconfig}" \
go build -trimpath -ldflags="-s -w -H windowsgui" -o "$OUTPUT" .

echo "Done. Binary at: $OUTPUT"
echo "Note: at runtime, libgtk-4-1.dll and friends must be on PATH"
echo "(or in the same directory as the binary). See ../packaging/windows/."

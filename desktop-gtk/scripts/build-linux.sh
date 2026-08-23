#!/usr/bin/env bash
# Build the desktop-gtk binary on Linux.
# Requires: gcc, pkg-config, gtk4 (>= 4.14), libadwaita (>= 1.5), gobject-introspection
#   Debian/Ubuntu: sudo apt install build-essential pkg-config libgtk-4-dev libadwaita-1-dev libgirepository1.0-dev
#   Fedora/RHEL:   sudo dnf install gcc pkgconf-pkg-config gtk4-devel libadwaita-devel gobject-introspection-devel
#   Arch:          sudo pacman -S base-devel pkgconf gtk4 libadwaita gobject-introspection
#   Nix:           nix-shell -p gtk4 libadwaita pkg-config gobject-introspection

set -euo pipefail

cd "$(dirname "$0")/.."

OUTPUT="${OUTPUT:-../compiled/wa-bot-desktop}"
mkdir -p "$(dirname "$OUTPUT")"

echo "Building wa-bot-desktop (Linux) -> $OUTPUT"
CGO_ENABLED=1 go build -trimpath -ldflags="-s -w" -o "$OUTPUT" .

echo "Done. Run with: $OUTPUT"
echo "Note: GTK4 and libadwaita must be installed on the target system at runtime."

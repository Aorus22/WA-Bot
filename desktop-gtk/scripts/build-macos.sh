#!/usr/bin/env bash
# Build the desktop-gtk binary on macOS.
# Requires: Homebrew + go + gtk4 + libadwaita
#   brew install go gtk4 libadwaita pkg-config gobject-introspection

set -euo pipefail

cd "$(dirname "$0")/.."

OUTPUT="${OUTPUT:-../compiled/wa-bot-desktop}"
mkdir -p "$(dirname "$OUTPUT")"

# Ensure pkg-config can find Homebrew's GTK4 and libadwaita
GTK4_PREFIX="$(brew --prefix gtk4 2>/dev/null || true)"
ADWAITA_PREFIX="$(brew --prefix libadwaita 2>/dev/null || true)"

PKG_CONFIG_PATH=""
[ -n "$GTK4_PREFIX" ]    && PKG_CONFIG_PATH="$PKG_CONFIG_PATH:$GTK4_PREFIX/lib/pkgconfig"
[ -n "$ADWAITA_PREFIX" ] && PKG_CONFIG_PATH="$PKG_CONFIG_PATH:$ADWAITA_PREFIX/lib/pkgconfig"
PKG_CONFIG_PATH="${PKG_CONFIG_PATH#:}"

echo "Building wa-bot-desktop (macOS) -> $OUTPUT"
PKG_CONFIG_PATH="$PKG_CONFIG_PATH" \
CGO_ENABLED=1 \
go build -trimpath -ldflags="-s -w" -o "$OUTPUT" .

echo "Done. Run with: $OUTPUT"
echo "Note: GTK4 and libadwaita must be installed via Homebrew at runtime."

#!/usr/bin/env bash
# Build a portable Windows bundle: exe + all GTK4 runtime DLLs it needs.
#
# Run inside the MSYS2 mingw64 shell:
#   C:\msys64\mingw64.exe desktop-gtk/packaging/windows/make-portable.sh
#
# Output: ../compiled/wa-bot-desktop-portable/  (zip it for distribution)
set -euo pipefail

cd "$(dirname "$0")/../.."

APP=wa-bot-desktop
OUT="${OUT:-../compiled/wa-bot-desktop-portable}"
BACKEND="${BACKEND:-../compiled/wa-bot-backend.exe}"

echo "Building $APP.exe"
CGO_ENABLED=1 PKG_CONFIG_PATH="${PKG_CONFIG_PATH:-/mingw64/lib/pkgconfig}" \
  go build -trimpath -ldflags="-s -w -H windowsgui" -o "$OUT/$APP.exe" .

mkdir -p "$OUT/etc/gtk-4.0" "$OUT/lib/gdk-pixbuf-2.0"

echo "Copying GTK4 runtime DLLs (via ldd)"
ldd "$OUT/$APP.exe" | awk '/\/mingw64\/bin\/.*\.dll/{print $3}' | sort -u |
while read -r dll; do
  [ -f "$dll" ] && cp -u "$dll" "$OUT/"
done

# GDK Pixbuf loaders + GDK media backend need their own dirs copied so
# images decode and (with GStreamer present) audio/video plays inline.
echo "Copying pixbuf loaders & media backends"
for src in /mingw64/lib/gdk-pixbuf-2.0 /mingw64/lib/gtk-4.0; do
  [ -d "$src" ] && cp -ru "$src" "$OUT/lib/"
done

# GStreamer runtime for inline audio/video playback (optional; app falls
# back to external players without it).
if [ -d /mingw64/bin/gstreamer-1.0 ]; then
  echo "Bundling GStreamer runtime"
  ldd /mingw64/bin/libgtk-4-1.dll 2>/dev/null | awk '/gstreamer|gst.*\.dll/{print $3}' | sort -u |
    while read -r dll; do [ -f "$dll" ] && cp -u "$dll" "$OUT/"; done
  mkdir -p "$OUT/lib/gstreamer-1.0"
  cp -u /mingw64/lib/gstreamer-1.0/*.dll "$OUT/lib/gstreamer-1.0/" 2>/dev/null || true
fi

if [ -f "$BACKEND" ]; then
  echo "Bundling backend binary"
  cp -u "$BACKEND" "$OUT/"
else
  echo "NOTE: backend not found at $BACKEND — bundle wa-bot-backend.exe manually."
fi

cat > "$OUT/README.txt" <<'EOF'
WA Bot Desktop — portable build

Run wa-bot-desktop.exe. It looks for wa-bot-backend.exe next to itself,
spawns it, and connects automatically (port discovered via stdout).

Requirements: none beyond what ships here (GTK4 runtime is bundled).
If GStreamer DLLs are included, voice notes/video play inline; otherwise
clicking play opens your system media player.
EOF

echo "Done -> $OUT"

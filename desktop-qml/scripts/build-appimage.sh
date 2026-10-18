#!/usr/bin/env bash
# Build the QML/PySide6 client as a standalone AppImage.
# This build has its own AppDir, PyInstaller worktree, tool cache, desktop ID,
# and output directory; it does not call or modify the GPUI AppImage builder.
#
#   desktop-qml/scripts/build-appimage.sh          # build dist/appimage/*.AppImage
#   desktop-qml/scripts/build-appimage.sh --clean  # remove QML AppImage staging only

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(cd "$ROOT/.." && pwd)"
APP_ID="wa-bot-qml"
BIN_NAME="wa-bot"
WORK_DIR="${WA_BOT_QML_APPIMAGE_WORK:-$ROOT/build/appimage}"
APP_DIR="$WORK_DIR/$APP_ID.AppDir"
PYI_DIST_DIR="$WORK_DIR/pyinstaller-dist"
PYI_WORK_DIR="$WORK_DIR/pyinstaller-work"
BACKEND_BIN="$WORK_DIR/wa-bot-backend"
DIST_DIR="${WA_BOT_QML_APPIMAGE_DIST:-$ROOT/dist/appimage}"
QML_PYTHON="${QML_PYTHON:-$ROOT/.venv/bin/python}"
TOOLS_DIR="${WA_BOT_QML_APPIMAGE_TOOLS_DIR:-${XDG_CACHE_HOME:-${HOME:-/tmp}/.cache}/wa-bot-qml-appimage}"

LINUXDEPLOY_URL="${LINUXDEPLOY_URL:-https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage}"
APPIMAGETOOL_URL="${APPIMAGETOOL_URL:-https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage}"

case "${1:-}" in
  --clean)
    rm -rf "$WORK_DIR"
    echo "Removed QML AppImage staging: $WORK_DIR"
    exit 0
    ;;
  --help|-h)
    sed -n '2,8p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
    exit 0
    ;;
  "") ;;
  *) echo "Unknown option: $1" >&2; exit 2 ;;
esac

log() { printf '[qml-appimage] %s\n' "$1"; }
fail() { printf '[qml-appimage] %s\n' "$1" >&2; exit 1; }

[[ -x "$QML_PYTHON" ]] || fail "Python environment missing: $QML_PYTHON (see desktop-qml/README.md)"
"$QML_PYTHON" -m PyInstaller --version >/dev/null 2>&1 || fail "PyInstaller missing; install with: $QML_PYTHON -m pip install -e '$ROOT[build]'"
command -v go >/dev/null || [[ -n "${WABOT_BUNDLE_BACKEND:-}" ]] || fail "Go is required to build cmd/api; set WABOT_BUNDLE_BACKEND to a prebuilt backend to skip it"
command -v curl >/dev/null || fail "curl is required to fetch linuxdeploy and appimagetool"
command -v mksquashfs >/dev/null || fail "mksquashfs is required (install squashfs-tools)"

ARCH="$(uname -m)"
[[ "$ARCH" == "x86_64" ]] || fail "the bundled AppImage tools currently support x86_64 only (got $ARCH)"
VERSION="$(awk -F '"' '/^version[[:space:]]*=/ {print $2; exit}' "$ROOT/pyproject.toml")"
[[ -n "$VERSION" ]] || fail "could not read version from $ROOT/pyproject.toml"
OUT="$DIST_DIR/${APP_ID}_${VERSION}_${ARCH}.AppImage"

fetch_tool() {
  local url="$1" destination="$2"
  [[ -x "$destination" ]] && return
  log "Downloading $(basename "$destination")"
  mkdir -p "$(dirname "$destination")"
  curl -fL --retry 3 -o "$destination" "$url" || fail "download failed: $url"
  chmod 0755 "$destination"
}
run_appimage_tool() { "$1" --appimage-extract-and-run "${@:2}"; }

log "Building QML AppImage backend"
mkdir -p "$WORK_DIR"
if [[ -n "${WABOT_BUNDLE_BACKEND:-}" ]]; then
  [[ -x "$WABOT_BUNDLE_BACKEND" ]] || fail "backend override is not executable: $WABOT_BUNDLE_BACKEND"
  install -m 0755 "$WABOT_BUNDLE_BACKEND" "$BACKEND_BIN"
else
  (cd "$REPO_ROOT" && go build -trimpath -ldflags "-s -w" -o "$BACKEND_BIN" ./cmd/api)
fi

log "Building isolated PyInstaller onedir payload"
rm -rf "$PYI_DIST_DIR" "$PYI_WORK_DIR"
mkdir -p "$PYI_DIST_DIR" "$PYI_WORK_DIR"
cd "$ROOT"
"$QML_PYTHON" -m PyInstaller \
  --clean \
  --noconfirm \
  --onedir \
  --contents-directory _internal \
  --name "$BIN_NAME" \
  --distpath "$PYI_DIST_DIR" \
  --workpath "$PYI_WORK_DIR/work" \
  --specpath "$PYI_WORK_DIR/spec" \
  --additional-hooks-dir "$ROOT/packaging/hooks" \
  --collect-submodules bridge \
  --hidden-import gi.repository.Gst \
  --hidden-import gi.repository.GstVideo \
  --add-data "$ROOT/qml:qml" \
  --add-binary "$BACKEND_BIN:." \
  "$ROOT/main.py"

PAYLOAD="$PYI_DIST_DIR/$BIN_NAME"
[[ -x "$PAYLOAD/$BIN_NAME" ]] || fail "PyInstaller executable missing: $PAYLOAD/$BIN_NAME"

log "Smoke-checking the frozen QML payload"
"$QML_PYTHON" "$ROOT/scripts/smoke-bundle.py" "$PAYLOAD/$BIN_NAME"

log "Staging the QML AppDir"
rm -rf "$APP_DIR"
install -d "$APP_DIR/usr/bin" "$APP_DIR/usr/share/applications" \
  "$APP_DIR/usr/share/icons/hicolor/256x256/apps" "$APP_DIR/usr/share/icons/hicolor/scalable/apps"
cp -a "$PAYLOAD/." "$APP_DIR/usr/bin/"
install -m 0644 "$ROOT/packaging/wa-bot-qml.desktop" "$APP_DIR/usr/share/applications/$APP_ID.desktop"
install -m 0644 "$ROOT/packaging/icons/wa-bot-qml-256.png" \
  "$APP_DIR/usr/share/icons/hicolor/256x256/apps/$APP_ID.png"
install -m 0644 "$ROOT/packaging/icons/wa-bot-qml.svg" \
  "$APP_DIR/usr/share/icons/hicolor/scalable/apps/$APP_ID.svg"
cp "$ROOT/packaging/icons/wa-bot-qml-256.png" "$APP_DIR/.DirIcon"

fetch_tool "$LINUXDEPLOY_URL" "$TOOLS_DIR/linuxdeploy-$ARCH.AppImage"
fetch_tool "$APPIMAGETOOL_URL" "$TOOLS_DIR/appimagetool-$ARCH.AppImage"

log "Deploying shared-library dependencies"
run_appimage_tool "$TOOLS_DIR/linuxdeploy-$ARCH.AppImage" \
  --appdir "$APP_DIR" \
  --executable "$APP_DIR/usr/bin/$BIN_NAME" \
  --desktop-file "$APP_DIR/usr/share/applications/$APP_ID.desktop" \
  --icon-file "$APP_DIR/usr/share/icons/hicolor/256x256/apps/$APP_ID.png"

mkdir -p "$DIST_DIR"
log "Packing $OUT"
ARCH="$ARCH" VERSION="$VERSION" \
  run_appimage_tool "$TOOLS_DIR/appimagetool-$ARCH.AppImage" "$APP_DIR" "$OUT"
chmod 0755 "$OUT"
du -h "$OUT" | awk '{print "[qml-appimage] size: " $1}'
log "Done. GPUI packaging remains at desktop-gpui/scripts/build-appimage.sh"

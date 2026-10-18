#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PYTHON="${QML_PYTHON:-$ROOT/.venv/bin/python}"
DIST_DIR="${WA_BOT_QML_DIST:-$ROOT/dist/linux}"
WORK_DIR="$ROOT/build/pyinstaller"
BACKEND_OVERRIDE="${WABOT_BUNDLE_BACKEND:-}"

if [[ ! -x "$PYTHON" ]]; then
  echo "Python environment missing: $PYTHON" >&2
  echo "Create it with: python3 -m venv --system-site-packages .venv && .venv/bin/pip install -e '.[build]'" >&2
  exit 1
fi
if ! "$PYTHON" -m PyInstaller --version >/dev/null 2>&1; then
  echo "PyInstaller is missing. Install the build extra with: .venv/bin/pip install -e '.[build]'" >&2
  exit 1
fi
mkdir -p "$DIST_DIR" "$WORK_DIR"
BUNDLE_BACKEND="$WORK_DIR/wa-bot-backend"
if [[ -n "$BACKEND_OVERRIDE" ]]; then
  if [[ ! -x "$BACKEND_OVERRIDE" ]]; then
    echo "Executable wa-bot-backend not found: $BACKEND_OVERRIDE" >&2
    exit 1
  fi
  install -m 0755 "$BACKEND_OVERRIDE" "$BUNDLE_BACKEND"
else
  if ! command -v go >/dev/null 2>&1; then
    echo "Go is required to build the current backend. Set WABOT_BUNDLE_BACKEND to a prebuilt executable to skip it." >&2
    exit 1
  fi
  (cd "$ROOT/.." && go build -o "$BUNDLE_BACKEND" ./cmd/api)
fi

cd "$ROOT"
"$PYTHON" -m PyInstaller \
  --clean \
  --noconfirm \
  --onedir \
  --contents-directory _internal \
  --name wa-bot \
  --distpath "$DIST_DIR" \
  --workpath "$WORK_DIR" \
  --specpath "$WORK_DIR" \
  --additional-hooks-dir "$ROOT/packaging/hooks" \
  --collect-submodules bridge \
  --hidden-import gi.repository.Gst \
  --hidden-import gi.repository.GstVideo \
  --add-data "$ROOT/qml:qml" \
  --add-binary "$WORK_DIR/wa-bot-backend:." \
  "$ROOT/main.py"

cat > "$DIST_DIR/wa-bot/README.txt" <<'EOF'
WA Bot Desktop QML — Linux bundle

Run ./wa-bot from this directory. It includes the QML/Python/Qt runtime and
the backend executable used by this build. Backend state stays in ~/.wa-bot
unless WABOT_HOME is set. Existing backend discovery and explicit --url/--port
options work as documented in the project README.

This bundle targets the Linux distribution and architecture used to build it.
It still uses the host display server, GPU drivers, audio devices, session D-Bus,
PyGObject, GStreamer/OpenH264 plugins, and system libraries. It does not claim
cross-distro or Wayland layer-shell support.
Review THIRD_PARTY_NOTICES.md before redistributing this build.
EOF
cp "$ROOT/THIRD_PARTY_NOTICES.md" "$DIST_DIR/wa-bot/THIRD_PARTY_NOTICES.md"

"$PYTHON" "$ROOT/scripts/smoke-bundle.py" "$DIST_DIR/wa-bot/wa-bot"
echo "Bundle created: $DIST_DIR/wa-bot"

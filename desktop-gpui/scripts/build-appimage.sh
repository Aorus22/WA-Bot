#!/usr/bin/env bash
# Build a portable AppImage of WA Bot (GPUI shell + bundled Go backend).
#
#   desktop-gpui/scripts/build-appimage.sh              # build dist/*.AppImage
#   desktop-gpui/scripts/build-appimage.sh --install    # build, then register for this user
#   desktop-gpui/scripts/build-appimage.sh --clean      # drop the staged AppDir
#
# The AppImage runs the same way as win11-clipboard-history:
#   wa-bot.AppImage --popover   resident daemon (autostarted on login); starts
#                               the bundled backend, data in ~/.wa-bot
#   wa-bot.AppImage --toggle    show/hide the popover (what the panel button runs)
#
# Layout of the produced AppDir:
#   usr/bin/wabot               GPUI shell
#   usr/bin/wa-bot-backend      Go backend (found next to the shell at runtime)
#
# Bundling policy as in the clipboard AppImage: linuxdeploy collects the shell's
# shared libraries; GTK, libvulkan and GPU drivers stay on the host.
# Both helper tools are downloaded once (and cached) as AppImages and run with
# `--appimage-extract-and-run`, so no FUSE is required to build.

set -euo pipefail

DESKTOP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(cd "$DESKTOP_DIR/.." && pwd)"
BIN_NAME="wabot"
APP_ID="wa-bot"
APP_DIR="$DESKTOP_DIR/target/appimage/$APP_ID.AppDir"
DIST_DIR="$DESKTOP_DIR/dist"
# Share the helper-tool cache with the clipboard AppImage build when present.
CACHE_DIR="${APPIMAGE_TOOLS_DIR:-}"
if [[ -z "$CACHE_DIR" ]]; then
    if [[ -d "${XDG_CACHE_HOME:-$HOME/.cache}/win11-clipboard-appimage" ]]; then
        CACHE_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/win11-clipboard-appimage"
    else
        CACHE_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/wa-bot-appimage"
    fi
fi

LINUXDEPLOY_URL="${LINUXDEPLOY_URL:-https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage}"
APPIMAGETOOL_URL="${APPIMAGETOOL_URL:-https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage}"

INSTALL=0
for arg in "$@"; do
    case "$arg" in
        --install) INSTALL=1 ;;
        --clean)
            echo "Removing $APP_DIR"
            rm -rf "$APP_DIR"
            exit 0
            ;;
        -h|--help)
            sed -n '2,22p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "Unknown option: $arg" >&2
            exit 2
            ;;
    esac
done

log() { printf '\033[0;36m[appimage]\033[0m %s\n' "$1"; }
fail() { printf '\033[0;31m[appimage] %s\033[0m\n' "$1" >&2; exit 1; }

command -v cargo >/dev/null || fail "cargo not found (install Rust: https://rustup.rs)"
command -v go >/dev/null || fail "go not found (the bundled backend is built from cmd/api)"
command -v mksquashfs >/dev/null || fail "mksquashfs not found (install squashfs-tools)"

VERSION="$(awk -F'"' '/^version/ {print $2; exit}' "$DESKTOP_DIR/Cargo.toml")"
ARCH="$(uname -m)"
[[ "$ARCH" == "x86_64" ]] || fail "only x86_64 is supported by the bundled tools (got $ARCH)"
OUT_NAME="wa-bot_${VERSION}_${ARCH}.AppImage"

fetch_tool() {
    local url="$1" dest="$2"
    [[ -x "$dest" ]] && return
    log "Downloading $(basename "$dest")"
    mkdir -p "$(dirname "$dest")"
    curl -fL --retry 5 --retry-all-errors --retry-delay 3 -o "$dest" "$url" || fail "download failed: $url"
    chmod +x "$dest"
}
run_appimage_tool() { "$1" --appimage-extract-and-run "${@:2}"; }

# --- Build ------------------------------------------------------------------

log "Building Go backend (cmd/api)"
BACKEND_BIN="$DESKTOP_DIR/target/appimage/wa-bot-backend"
mkdir -p "$(dirname "$BACKEND_BIN")"
(cd "$REPO_ROOT" && go build -trimpath -ldflags "-s -w" -o "$BACKEND_BIN" ./cmd/api)

log "Building GPUI shell (cargo build --release)"
cargo build --release --manifest-path "$DESKTOP_DIR/Cargo.toml" -p wabot
BIN="$DESKTOP_DIR/target/release/$BIN_NAME"
[[ -x "$BIN" ]] || fail "release binary missing: $BIN"

log "Staging AppDir"
rm -rf "$APP_DIR"
install -Dm755 "$BIN" "$APP_DIR/usr/bin/$BIN_NAME"
install -Dm755 "$BACKEND_BIN" "$APP_DIR/usr/bin/wa-bot-backend"

mkdir -p "$APP_DIR/usr/share/applications"
sed 's|__BINDIR__/||g' "$DIST_DIR/$APP_ID.desktop" > "$APP_DIR/usr/share/applications/$APP_ID.desktop"
chmod 644 "$APP_DIR/usr/share/applications/$APP_ID.desktop"

ICON_BASE="$APP_DIR/usr/share/icons/hicolor"
install -Dm644 "$DIST_DIR/icons/wa-bot-128.png" "$ICON_BASE/128x128/apps/$APP_ID.png"
install -Dm644 "$DIST_DIR/icons/wa-bot-256.png" "$ICON_BASE/256x256/apps/$APP_ID.png"
install -Dm644 "$DIST_DIR/icons/wa-bot.svg" "$ICON_BASE/scalable/apps/$APP_ID.svg"
cp "$ICON_BASE/256x256/apps/$APP_ID.png" "$APP_DIR/.DirIcon"

fetch_tool "$LINUXDEPLOY_URL" "$CACHE_DIR/linuxdeploy-$ARCH.AppImage"
fetch_tool "$APPIMAGETOOL_URL" "$CACHE_DIR/appimagetool-$ARCH.AppImage"

log "Deploying shared-library dependencies (linuxdeploy)"
run_appimage_tool "$CACHE_DIR/linuxdeploy-$ARCH.AppImage" \
    --appdir "$APP_DIR" \
    --executable "$APP_DIR/usr/bin/$BIN_NAME" \
    --desktop-file "$APP_DIR/usr/share/applications/$APP_ID.desktop" \
    --icon-file "$ICON_BASE/256x256/apps/$APP_ID.png"

mkdir -p "$DIST_DIR"
OUT="$DIST_DIR/$OUT_NAME"
log "Packing $OUT_NAME (appimagetool)"
ARCH="$ARCH" VERSION="$VERSION" \
    run_appimage_tool "$CACHE_DIR/appimagetool-$ARCH.AppImage" "$APP_DIR" "$OUT"
chmod +x "$OUT"
log "Done: $OUT"
du -h "$OUT" | awk '{print "     size: " $1}'

# --- Optional user-level install -------------------------------------------

if [[ "$INSTALL" == "1" ]]; then
    BIN_DIR="$HOME/Applications"
    APP_DIR_USER="$HOME/.local/share/applications"
    ICON_HOME="$HOME/.local/share/icons/hicolor"

    log "Installing AppImage for this user"
    mkdir -p "$BIN_DIR" "$APP_DIR_USER"
    install -Dm755 "$OUT" "$BIN_DIR/$OUT_NAME"
    INSTALLED="$BIN_DIR/$OUT_NAME"

    sed "s|__BINDIR__/$BIN_NAME|$INSTALLED|g" "$DIST_DIR/$APP_ID.desktop" \
        > "$APP_DIR_USER/$APP_ID.desktop"
    install -Dm644 "$DIST_DIR/icons/wa-bot-128.png" "$ICON_HOME/128x128/apps/$APP_ID.png"
    install -Dm644 "$DIST_DIR/icons/wa-bot-256.png" "$ICON_HOME/256x256/apps/$APP_ID.png"
    install -Dm644 "$DIST_DIR/icons/wa-bot.svg" "$ICON_HOME/scalable/apps/$APP_ID.svg"
    update-desktop-database "$APP_DIR_USER" 2>/dev/null || true
    gtk-update-icon-cache -f -t "$ICON_HOME" 2>/dev/null || true

    # Autostart: the resident daemon (starts the backend, UI stays hidden).
    AUTOSTART_DIR="$HOME/.config/autostart"
    mkdir -p "$AUTOSTART_DIR"
    cat > "$AUTOSTART_DIR/$APP_ID.desktop" <<DESK
[Desktop Entry]
Type=Application
Name=WA Bot (daemon)
Comment=WA Bot popover daemon and backend
Exec=$INSTALLED --popover
Icon=$APP_ID
Terminal=false
Hidden=false
NoDisplay=true
X-GNOME-Autostart-enabled=true
Categories=Network;
DESK
    log "Registered autostart: $AUTOSTART_DIR/$APP_ID.desktop"

    # COSMIC panel button (popover-applet): point it at the installed AppImage.
    APPLET_INSTALLER="${POPOVER_APPLET_DIR:-$HOME/Project/popover-applet}/scripts/install-cosmic.sh"
    if [[ -x "$APPLET_INSTALLER" ]]; then
        SKIP_BUILD=1 "$APPLET_INSTALLER" dev.local.WabotPopover "WA Bot" \
            chat-message-new-symbolic -- "$INSTALLED" --toggle >/dev/null
    else
        log "No popover-applet installer at $APPLET_INSTALLER; panel button not updated"
    fi

    log "Installed: $INSTALLED"
    log "Data lives in ${WABOT_HOME:-$HOME/.wa-bot}. Start the daemon now: $INSTALLED --popover &"
fi

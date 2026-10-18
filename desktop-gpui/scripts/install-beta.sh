#!/usr/bin/env bash
# Download the rolling `beta` build (from the develop branch) and install it next
# to the stable app as "WA Bot (Beta)". Re-run it to update the beta.
#
#   desktop-gpui/scripts/install-beta.sh
#
# Beta and stable share the data in ~/.wa-bot, so only one may run at a time:
# the Beta launcher stops the stable daemon and backend first (and the other way
# round is simply "open stable again", which stops beta). Autostart stays with
# stable.

set -euo pipefail

REPO="${WABOT_REPO:-Aorus22/WA-Bot}"
BIN_DIR="$HOME/Applications"
APPS_DIR="$HOME/.local/share/applications"
ICON_HOME="$HOME/.local/share/icons/hicolor"
APP_ID="wa-bot-beta"

log() { printf '\033[0;36m[beta]\033[0m %s\n' "$1"; }
fail() { printf '\033[0;31m[beta] %s\033[0m\n' "$1" >&2; exit 1; }
command -v gh >/dev/null || fail "gh (GitHub CLI) not found"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
log "Downloading the beta release"
gh release download beta --repo "$REPO" --dir "$TMP"
(cd "$TMP" && sha256sum -c SHA256SUMS.txt) || fail "checksum mismatch"
SRC="$(ls "$TMP"/wa-bot_*_x86_64.AppImage)"
VERSION="$(basename "$SRC" | sed -E 's/^wa-bot_(.*)_x86_64\.AppImage$/\1/')"
OUT="$BIN_DIR/wa-bot-beta_${VERSION}_x86_64.AppImage"

mkdir -p "$BIN_DIR" "$APPS_DIR"
# Older beta files go away; stop a running beta first so the file can be replaced.
for old in $(pgrep -f '^[^ ]*/wa-bot-beta_[^ ]*\.AppImage' || true); do kill "$old" 2>/dev/null || true; done
sleep 1
rm -f "$BIN_DIR"/wa-bot-beta_*_x86_64.AppImage
install -Dm755 "$SRC" "$OUT"

# Launcher: stop stable (and every backend) unless beta already runs, then start beta.
LAUNCHER="$BIN_DIR/wa-bot-beta"
cat > "$LAUNCHER" <<LAUNCH
#!/bin/sh
APP="\$(ls "$BIN_DIR"/wa-bot-beta_*_x86_64.AppImage | tail -n 1)"
if ! pgrep -f '^[^ ]*/wa-bot-beta_[^ ]*\\.AppImage' >/dev/null; then
    for p in \$(pgrep -f '^[^ ]*/wa-bot_[0-9][^ ]*\\.AppImage'); do kill "\$p" 2>/dev/null; done
    sleep 2
    # The backends belong to the apps just stopped; beta starts its own.
    pkill -x wa-bot-backend 2>/dev/null
    sleep 1
    "\$APP" --popover >/dev/null 2>&1 &
    sleep 3
fi
exec "\$APP" --toggle
LAUNCH
chmod 755 "$LAUNCHER"

cat > "$APPS_DIR/$APP_ID.desktop" <<DESK
[Desktop Entry]
Type=Application
Name=WA Bot (Beta)
Comment=WA Bot beta build from the develop branch
Exec=$LAUNCHER
Icon=wa-bot
Terminal=false
Categories=Network;
StartupWMClass=wa-bot
DESK
update-desktop-database "$APPS_DIR" 2>/dev/null || true

log "Installed: $OUT"
log "Open it from the app menu: WA Bot (Beta)"

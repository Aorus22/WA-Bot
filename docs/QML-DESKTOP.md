# WA Bot Desktop QML client

This is a third native desktop frontend beside React and GPUI. The UI uses Qt Quick/QML with a small PySide6 bridge; business rules and persistent WhatsApp state remain in the shared Go backend. QML edits need no Rust, C++, or Qt plugin build.

## Development

From `desktop-qml/`:

```sh
python3 -m venv --system-site-packages .venv
.venv/bin/pip install -e . pytest
./scripts/dev.sh
```

Linux call video uses the host PyGObject/GStreamer runtime. Install GStreamer with
the OpenH264 encoder, decoder, `h264parse`, and app plugins before testing camera calls.

An explicit backend can be selected with `./scripts/dev.sh --url http://127.0.0.1:PORT/api` or `--port PORT`. Ordinary launches join the per-user resident instance; explicit backend launches stay standalone. The first ordinary instance owns the local socket and optional tray. Close hides the window only when a tray host is available; use **Quit** in the tray menu to exit.

Use a local mock server for interface work. For example:

```sh
.venv/bin/python -u tests/mock_backend.py chats rich calls composer bot
QT_QPA_PLATFORM=offscreen .venv/bin/python scripts/shot.py /tmp/chat.png \
  --route 'chat/6281111111111@s.whatsapp.net' --url http://127.0.0.1:PORT/api
```

The mock server prints its chosen URL. `scripts/shot.py` requires a loopback `--url` and uses a temporary settings file, so it refuses backend discovery and cannot fall back to the user's configured backend. Run the suite with `.venv/bin/python -m pytest -q tests`.

## Architecture

- `main.py` creates services and a `QQmlApplicationEngine`; each feature in `bridge/` registers context properties in `bridge/registry.py`.
- `bridge/api.py` uses Qt Network asynchronously. `bridge/events.py` handles the JSON WebSocket. The UI should not issue blocking HTTP or file operations.
- QML pages live under `qml/pages/`; reusable visual types live under `qml/ui/`.
- Keep API shapes aligned with `desktop-gpui/crates/backend-client` and Go handlers. Keep user preferences in `Settings` and server-owned settings in `/api/settings`.

## Linux bundle

Build from `desktop-qml/` with `.venv/bin/pip install -e '.[build]'` and the Go toolchain, then run
`./scripts/package-linux.sh`. The build creates `dist/linux/wa-bot/` as a PyInstaller onedir package
and runs `scripts/smoke-bundle.py` against the local mock. The smoke uses temporary data/config paths
and confirms the packaged app reaches auth/chat REST and the JSON WebSocket. The current 642 MB artifact was
built on CachyOS x86_64 from current Go backend source with the media-session route; the frozen codec smoke
verifies H.264 Annex-B encode/decode. Its copy-to-user-directory, desktop-entry, smoke, and uninstall steps passed
under temporary paths on this host; a clean host and another distribution have not been checked.

User-level install after building:

```sh
install_dir="$HOME/.local/opt/wa-bot-qml"
applications_dir="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
mkdir -p "$install_dir" "$applications_dir"
cp -a dist/linux/wa-bot/. "$install_dir/"
cat > "$applications_dir/wa-bot-qml.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=WA Bot Desktop
Exec="$install_dir/wa-bot"
Icon=$install_dir/_internal/qml/assets/icons/inbox.svg
Terminal=false
Categories=Network;InstantMessaging;
EOF
```

To uninstall:

```sh
rm -rf "$HOME/.local/opt/wa-bot-qml"
rm -f "$applications_dir/wa-bot-qml.desktop"
```

This leaves `~/.wa-bot` account data and settings in place.
The bundle includes Python, Qt, QML, and the backend executable; the host still supplies glibc,
display/GPU drivers, audio devices, session D-Bus, PyGObject, and GStreamer with `openh264enc`,
`openh264dec`, and `h264parse`. Build on the oldest Linux/glibc target you
intend to support. No Windows package is produced.

## Platform behavior

The client uses freedesktop notifications over the session D-Bus, Qt's system tray when a host is available, and a per-user local socket for single-instance commands. On supported Wayland sessions, `--popover` uses the optional KDE LayerShellQt QPA plugin and `org.kde.layershell` QML module to anchor the card to the configured panel edge, cache the button pointer, and dismiss on outside clicks. The module is a host dependency, not a bundled component; `wayland-info` must also report `zwlr_layer_shell_v1`. When the protocol or module is unavailable, `--toggle` opens the regular window and `--popover` stays resident until an open command arrives. GNOME and cross-distribution behavior remain unverified.

## Known limits

- `PARITY.md` and `parity/*.md` track verified behavior and mock-render screenshots. Matched GPUI/QML baselines cover chat in dark and light themes, Login, Calls history/detail/incoming/active, Status, Channels, and Settings at 1100×720; the older direct/group Info drawer captures are stale after the latest parity correction and need recapture.
- The prebuilt GPUI AppImage panicked during EGL initialization under Xvfb. The local GPUI debug build renders under Xvfb when Mesa llvmpipe is selected explicitly; real desktop behavior still needs verification.
- The Go router now exposes `POST /api/calls/{id}/media-session` using `CallService.CreateMediaSession`; QML consumes its one-use token and sends/decodes H.264 Annex-B access units over `/ws/calls/{id}/media`. Contract and codec tests pass. Real two-device A/V, device switching, latency, and long-call behavior remain unverified.
- An already-running backend built before the media-session route must be rebuilt/restarted from current Go source before QML call media can connect; the client does not replace or upgrade attached backend processes.
- Real QR pairing, camera/microphone, notification-daemon, system tray, Windows behavior, and LayerShellQt compositor behavior have not been tested in a real desktop session.
- Process-level smokes kept React, GPUI, and QML connected concurrently to both an isolated mock and the actual Go backend. A separate smoke verified that GPUI takes over a replacement backend after its QML owner exits. Electron also restarts its own backend after an unexpected child exit and republishes the new port. These smokes use fresh temporary homes with no stored account and do not scan generated QR codes. Electron following an external QML-owned port change and other owner combinations remain open. Packaging has a host-built Linux artifact and isolated codec/QML/REST/WS smoke, but cross-distribution behavior, clean-host install/uninstall, and performance comparison remain open. See `../.planning/QML-DESKTOP-PLAN.md` for the original acceptance checklist.

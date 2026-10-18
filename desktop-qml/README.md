# WA Bot Desktop — Qt Quick / QML

Third frontend next to `web/` (React) and `desktop-gpui/`. Native Qt Quick rendering with a thin
PySide6 bridge; all business logic stays in the Go backend. Editing QML needs **no compilation**.

## Run
```bash
python3 -m venv --system-site-packages .venv   # PySide6 >= 6.8 (installed system-wide here: 6.11.2)
.venv/bin/pip install -e . pytest              # or: pip install PySide6 pytest
./scripts/dev.sh                               # = python3 main.py
./scripts/dev.sh --port 3090                   # explicit backend, never touches core.json
./scripts/dev.sh --popover                     # start resident popover mode hidden; use --toggle to show
./scripts/dev.sh --toggle                      # show/hide the resident instance when one is running
```
On Linux, call video also requires the host PyGObject/GStreamer runtime with
`openh264enc`, `openh264dec`, `h264parse`, and app plugins; these native plugins are not pip packages.
Live call media also needs a backend built from the current Go source for
`POST /api/calls/{id}/media-session`; an older already-running backend cannot be upgraded by the QML client.

Backend selection (same precedence as GPUI `detect_backend_url`): `--url/--backend-url/--port/-p`,
env `BACKEND_URL`/`BACKEND_PORT`, then `~/.wa-bot/run/core.json` (validated: pid name, port owner and
`/api/status` must all match, else refused or treated as stale), else spawn the bundled
`wa-bot-backend` (`$WABOT_BACKEND`, `desktop-qml/`, `desktop-qml/be/`, repo root) with an ephemeral port,
then `.env PORT=`, then `:3090`. `$WABOT_HOME` moves the data folder (default `~/.wa-bot`).

## Ownership and shutdown
- This app stops **only** a backend it spawned (and removes its `core.json` record if it is still its own).
- An attached backend is never stopped. An owned backend that dies is restarted; an attached one is
  re-resolved when the WebSocket drops.
- A normal desktop launch owns a per-user local instance socket. A second normal launch asks the first to
  raise its window; `--toggle` asks it to show/hide. Explicit `--url`/`--port` development launches are
  standalone and do not join the resident instance.
- When a system tray host is available, closing the window hides it there. Use the tray menu's **Quit** to
  exit and stop only an owned backend. Without a tray host, close exits normally.
- Known interaction: GPUI also stops *its* spawned backend on quit, even if QML attached to it. Whoever
  spawned it owns it; closing the owner drops the other client's connection (it reconnects/respawns).

## Layout
`bridge/discovery.py` (stdlib, blocking, runs off the UI thread) · `bridge/backend.py` lifecycle ·
`bridge/api.py` async REST (`api.request(method, path, body, token)` -> `finished(token, status, json)`) ·
`bridge/events.py` JSON WS with 3 s reconnect · `qml/` UI.

## Test
```bash
.venv/bin/python -m pytest -q tests          # unit + real-backend lifecycle (needs wa-bot-backend; else skipped)
QT_QPA_PLATFORM=offscreen .venv/bin/python scripts/smoke.py     # read-only check against the running backend
.venv/bin/python scripts/concurrent-smoke.py # React + GPUI + QML against one isolated mock; needs Firefox, Xvfb, and a GPUI debug binary
WABOT_TEST_BACKEND="$PWD/build/pyinstaller/wa-bot-backend" .venv/bin/python scripts/concurrent-go-smoke.py # actual Go server, temporary empty home, no QR scan
```
## QML Linux onedir bundle
```bash
.venv/bin/pip install -e '.[build]'
./scripts/package-linux.sh  # builds current Go source, then runs isolated codec/QML/REST/WS smokes
./dist/linux/wa-bot/wa-bot --url http://127.0.0.1:3090/api
```
The package build needs the Go toolchain; set `WABOT_BUNDLE_BACKEND` to a prebuilt executable to skip the Go build.
The onedir bundle includes QML, the PyInstaller Python/Qt runtime, and a Go backend built from the
current `../cmd/api` source. Set `WABOT_BUNDLE_BACKEND` to use a prebuilt executable instead. It is
built for the current Linux architecture/distribution; the host still provides the display server, GPU drivers,
audio devices, session D-Bus, PyGObject, and GStreamer with the OpenH264 encoder/decoder plugins.
For an isolated startup check, use a temporary `WABOT_HOME`, an
explicit mock URL, and `QT_QPA_PLATFORM=offscreen`. Do not treat a successful build as proof of
cross-distribution packaging or real desktop integration.

User-level install/uninstall steps and the current Linux capability notes are in [`docs/QML-DESKTOP.md`](../docs/QML-DESKTOP.md).
Codec runtime requirements and third-party notices are in [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).

## QML AppImage

The AppImage has a separate build command, staging directory, helper-tool cache, desktop identity, and output directory:

```bash
./scripts/build-appimage.sh
# Output: dist/appimage/wa-bot-qml_<version>_x86_64.AppImage
./scripts/build-appimage.sh --clean  # remove only the QML AppImage staging files
```

This does not invoke or modify the GPUI AppImage command at `../desktop-gpui/scripts/build-appimage.sh`.
The QML AppImage build uses `build/appimage/` and `~/.cache/wa-bot-qml-appimage/`, separate from the
QML onedir build and the GPUI AppImage build. It targets x86_64. The host still provides GStreamer
with OpenH264 plugins, display/GPU drivers, and session D-Bus. To package call media before the
`call` branch is integrated into the current Go source, set `WABOT_BUNDLE_BACKEND` to a backend
binary built from that branch.

Platform scope: Linux first. The app requests Freedesktop notifications over the session D-Bus with the installed wa-bot-qml desktop-entry identity, and uses
Qt's system tray when a host is available. Unit/mock coverage includes notification policy, a private test
D-Bus, and local single-instance IPC; no GNOME/COSMIC tray host or real notification daemon was available
for a desktop run. `--popover` uses the host KDE LayerShellQt QPA plugin and `org.kde.layershell` QML module on supported Wayland sessions. The QML module is optional and is not bundled. Wayland popover mode also requires `wayland-info` to report `zwlr_layer_shell_v1`; when the protocol or Qt plugin is unavailable, `--toggle` opens the regular window and `--popover` remains resident until an open command arrives. The current GNOME session does not advertise the protocol; native dismissal and supported-compositor plugin loading still need desktop verification. X11 falls back to the regular window, matching GPUI when layer-shell is unavailable.

`PARITY.md` links the per-area status and screenshots. The PNGs are QML renders over local mock fixtures,
not GPUI baseline comparisons. Calls use the Go `POST /calls/{id}/media-session` route and binary media
WebSocket. Video uses GStreamer/OpenH264 to produce H.264 Annex-B access units; real device-to-device audio/video still needs
verification on a desktop with available microphone and camera devices.

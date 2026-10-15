# WA Bot Desktop — Qt Quick / QML

Third frontend next to `web/` (React) and `desktop-gpui/`. Native Qt Quick rendering with a thin
PySide6 bridge; all business logic stays in the Go backend. Editing QML needs **no compilation**.

## Run
```bash
python3 -m venv --system-site-packages .venv   # PySide6 >= 6.8 (installed system-wide here: 6.11.2)
.venv/bin/pip install -e . pytest              # or: pip install PySide6 pytest
./scripts/dev.sh                               # = python3 main.py
./scripts/dev.sh --port 3090                   # explicit backend, never touches core.json
```
Backend selection (same precedence as GPUI `detect_backend_url`): `--url/--backend-url/--port/-p`,
env `BACKEND_URL`/`BACKEND_PORT`, then `~/.wa-bot/run/core.json` (validated: pid name, port owner and
`/api/status` must all match, else refused or treated as stale), else spawn the bundled
`wa-bot-backend` (`$WABOT_BACKEND`, `desktop-qml/`, `desktop-qml/be/`, repo root) with an ephemeral port,
then `.env PORT=`, then `:3090`. `$WABOT_HOME` moves the data folder (default `~/.wa-bot`).

## Ownership and shutdown
- This app stops **only** a backend it spawned (and removes its `core.json` record if it is still its own).
- An attached backend is never stopped. An owned backend that dies is restarted; an attached one is
  re-resolved when the WebSocket drops.
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
```
Platform scope: Linux first (the `/proc`-based ownership checks are Linux-only; elsewhere they degrade to
"unknown" and attach/spawn still work). Windows/X11/tray/layer-shell are untested and not claimed.

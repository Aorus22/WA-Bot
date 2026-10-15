# Parity matrix (QML vs GPUI)

Status: **Phase 1 done (runtime + backend integration); Phases 2–7 not started.** Do not read
historical milestone audits as runtime proof; each row needs a test or screenshot before `done`.

## Contracts verified against current source (branch `qml`, from `main` 9aad3f7)
| Contract | Source of truth | Verified |
|---|---|---|
| Discovery `core.json` v1 `{schema_version,name=aorus22.wabot-core,pid,port,instance_id,started_at}` | `desktop-gpui/crates/app/src/backend.rs` | tests/test_discovery.py, live attach |
| Spawn env `PORT=:0 ALLOWED_ORIGINS=* DB_PATH MEDIA_PATH TZ=Asia/Jakarta`, cwd = data dir, handshake `BACKEND_PORT:<n>` on stdout | `crates/supervisor/src/lib.rs` | tests/test_backend_lifecycle.py (real binary) |
| REST base `<url>/api` (`/status`, `/qr-code`, `/chats`, `/chats/{id}/messages`, `/send-message` …) | `crates/backend-client/src/client.rs` | `/status` live; rest per phase |
| JSON WS `<root>/ws`, `{type,payload}`, no handshake, 3 s reconnect | `crates/backend-client/src/ws.rs`, `internal/delivery/http/routes/routes.go` | live connect, restart follow |
| Call media: binary WS `/ws/calls/{id}/media` | **not in `main`** — only on branch `call` (`docs/CALL-MEDIA-PROTOCOL.md`, uncommitted/branch work) | **open risk**: re-read after `call` merges |

## Routes (GPUI `router.rs`)
Chat, ChatDetail, Status, Channels, Calls, Triggers(+detail/new), Cron(+detail/new), Webhooks(+detail/new/logs),
Documentation, Settings.

| Area | GPUI file | QML | Endpoint/event | Test | Status |
|---|---|---|---|---|---|
| Backend attach/spawn/own | backend.rs, backend_process.rs | bridge/discovery.py, backend.py | core.json, spawn | pytest | done |
| REST transport | backend-client | bridge/api.py | all | smoke | done (transport only) |
| WS + reconnect | ws.rs | bridge/events.py | /ws | pytest | done (transport; typed stores pending) |
| Window shell / titlebar | components/titlebar.rs | qml/Main.qml (placeholder) | – | – | todo |
| Auth: QR / phone pairing | views/auth | placeholder shows QR text | /status /qr-code qr_code auth_success logged_out | – | todo |
| Theme (81 presets) | theme/ | – | – | – | todo |
| Chat list / conversation / composer | views/chat, components/* | – | /chats, /chats/{id}/messages, … | – | todo |
| Media, voice notes, groups, polls | components/rich_bubbles.rs, … | – | – | – | todo |
| Calls, Status, Channels | views/calls.rs, status.rs, channels.rs, call_media/ | – | – | – | todo |
| Bot tools, settings, docs | views/settings.rs, state/bot.rs | – | – | – | todo |
| Tray, notifications, popover | tray.rs, notifications/, popover.rs | – | – | – | todo |

## Phase 0 items still open
- Baseline GPUI screenshots (login, chat, media, settings, bot tools, calls, status, channels, compact, overlays) —
  needs a running GPUI build at fixed size/theme/DPI. Not captured yet.
- Per-screen pixel/font/icon inventory from GPUI components and `theme/themes_data.json`.
- Acceptance tests per screen before Phase 2 UI work.

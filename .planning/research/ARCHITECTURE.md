# Architecture Research: desktop-gpui Client for wa-bot

**Domain:** Native Rust-GPUI desktop client reusing existing Go backend via sidecar
**Researched:** 2026-09-08
**Confidence:** HIGH (all integration contracts read directly from source: `desktop/main.js`, `web/src/lib/api.ts`, `chatStore.ts`, `use-websocket.ts`, `ws-bus.ts`, `AuthContext.tsx`, `App.tsx`, `internal/delivery/http/routes/routes.go`, reference `../web-term/desktop-gpui` crates)

## Standard Architecture

### System Overview

The GPUI client is a **third presentation shell** over an unchanged Go backend. The backend already serves three clients (web, Electron, GTK); GPUI adds a fourth. No backend rewrite, no API breakage — only additive changes if ever needed.

```
┌─────────────────────────────────────────────────────────────────┐
│                    desktop-gpui/ (NEW, Rust)                      │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────────┐  │
│  │ wabot views/ │  │  AppState    │  │ backend-client         │  │
│  │ (10 views ≈  │→ │  (GPUI root  │→ │ (typed HTTP + WS pump) │──┼──┐
│  │  19 routes)  │  │   entity)    │  │                        │  │  │
│  └──────────────┘  └──────────────┘  └────────────────────────┘  │  │
│  ┌──────────────┐  ┌──────────────┐                              │  │
│  │ theme.rs     │  │ supervisor   │──spawns/manages──┐           │  │
│  │ (web 1:1)    │  │ (sidecar)    │                  │           │  │
│  └──────────────┘  └──────────────┘                  │           │  │
├─────────────────────────────────────────────────────┼───────────┼──┤
│              Existing Go backend (UNCHANGED)         │           │  │
│  ┌──────────────────────────────────────────────────▼──┐  ┌─────▼──┴──┐
│  │  wa-bot-backend sidecar (cmd/api + internal/)       │  │ REST /api │
│  │  delivery / usecase / domain / infrastructure       │  │ WS /ws    │
│  └─────────────────────────────────────────────────────┘  └───────────┘
├─────────────────────────────────────────────────────────────────┤
│  Existing clients (UNCHANGED, reference only): web/ │ desktop/ │ desktop-gtk/ │
└─────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| `supervisor` crate | Spawn + supervise `wa-bot-backend` sidecar, `BACKEND_PORT:` handshake, readiness probe, graceful kill, `--no-backend` dev override | Port of `desktop/main.js:startBackend` + web-term `supervisor/src/lib.rs`; tokio-only, no GPUI, headless-testable |
| `backend-client` crate | Typed DTOs + REST methods covering `api.ts` ApiClient (~70 endpoints); WS connect/authenticate/autoreconnect pump emitting typed events | Port of web-term `backend-client` (`rest.rs` + `terminal_ws.rs` pattern); reqwest rustls-tls + tokio-tungstenite + flume |
| `settings` crate | **UI prefs only**: backend path override, theme preset, window state, last backend URL | Copy of web-term `settings` (`lib.rs` + `paths.rs`); DB-backed app settings stay server-side via `GET/PUT /api/settings` |
| `wabot` app crate | Root `AppState` entity (chat store + auth + call + route view), 10 views, theme, main entry, bundling | Mirrors web-term `webterm` crate (`app_state.rs`, `views/`, `theme.rs`, `main.rs`, `bundle.rs`); `TOKIO_RT` static + `cx.spawn` bridge pattern |
| Go backend (existing) | Source of truth: REST + WS broadcast; zero changes | `internal/delivery/http/routes/routes.go`, `websocket.go`, `event_handler.go` — read-only contract |

## Recommended Project Structure

```
desktop-gpui/
├── Cargo.toml                # workspace: supervisor, settings, backend-client, wabot;
│                             #   exact pins (gpui-pre =0.3.3, gpui-component =0.6.0,
│                             #   tokio+reqwest rustls, serde) + committed Cargo.lock
├── Cargo.lock                # committed (proven web-term practice)
├── crates/
│   ├── supervisor/           # NEW — port of desktop/main.js spawn logic
│   │   └── src/lib.rs        # SpawnOptions, BackendStatus, parse_handshake_line, stop()
│   ├── settings/             # NEW — UI prefs only
│   │   └── src/{lib.rs,paths.rs}
│   ├── backend-client/       # NEW — full api.ts + ws-bus surface in Rust
│   │   └── src/{lib.rs,types.rs,rest.rs,wa_ws.rs}
│   └── wabot/                # NEW — app shell (name it `wabot`, not `webterm`)
│       └── src/
│           ├── main.rs       # settings load → supervisor spawn → GPUI run
│           ├── app_state.rs  # AppState entity: chat/auth/call/nav stores
│           ├── theme.rs      # 1:1 port of web themes.ts + ThemeProviders
│           ├── bundle.rs     # backend binary locate (resources/be vs dev path)
│           ├── actions.rs    # GPUI actions (nav, logout, call accept/…)
│           ├── icons.rs      # lucide → GPUI icon mapping
│           ├── window_state.rs
│           └── views/
│               ├── mod.rs / nav.rs        # AppLayout sidebar (10 items)
│               ├── login.rs               # QR + phone tabs, qr_code WS
│               ├── chat.rs                # ChatPage: sidebar + ChatArea + sheets
│               ├── status.rs / channels.rs / calls.rs
│               ├── triggers.rs / cron.rs / webhooks.rs  # list + editor + logs
│               ├── settings_view.rs / docs.rs
│               └── reconnect_banner.rs    # backend Starting/Failed/Crashed states
└── scripts/                  # Linux + Windows packaging (backend bundled)
```

### Structure Rationale

- **`crates/` split mirrors web-term because the dependency DAG demands it:** `wabot` → `backend-client` + `settings` + `supervisor`; `backend-client` → nothing internal. Supervisor stays GPUI-free so sidecar logic is unit-testable in CI without a display server — this is the single most valuable structural lesson from web-term.
- **One app crate, not one crate per view:** web-term keeps all views in the app crate (`views/*.rs` + one fat `app_state.rs`). The wa-bot surface is larger (10 views vs 6) but the pattern scales — splitting views into crates buys nothing and breaks the shared `AppState` borrow.
- **Crate name `wabot`:** workspace internal crates in web-term are prefixed (`webterm-supervisor`); use `wabot-supervisor`, `wabot-settings`, `wabot-backend-client`, `wabot` for the app crate.

## Architectural Patterns

### Pattern 1: Sidecar Supervisor (port of `desktop/main.js`)

**What:** The desktop app owns the backend process lifecycle: resolve binary path → ensure data dirs → spawn with `PORT=':0'` → parse `BACKEND_PORT:<port>` from stdout → readiness-probe → expose base URL to the client → kill on quit. Plus a `--no-backend --port <n>` escape hatch for dev (Electron supports it; GPUI must too).

**When to use:** Always — this is the core desktop integration. No GPUI client without it.

**Trade-offs:** Bundling the Go binary per platform doubles packaging work (Linux + Windows) but removes all network setup for users; the `--no-backend` flag keeps dev iteration fast.

**Electron contract to replicate exactly (from `main.js`):**

| Electron behavior | GPUI equivalent |
|---|---|
| Binary: `resources/be/wa-bot-backend[.exe]` packaged, else `<repo>/wa-bot-backend[.exe]` dev | `bundle.rs`: same two-tier lookup; `.exe` suffix on `cfg(windows)` |
| Data dirs: `<userData>/database`, `<userData>/media` created if missing | `settings::paths`: `dirs` crate base + ensure dirs |
| Env: `PORT=':0'`, `ALLOWED_ORIGINS='*'`, `DB_PATH`, `MEDIA_PATH`, `TZ='Asia/Jakarta'`, `cwd=userData` | `SpawnOptions::env_vars()` — **note: wa-bot has NO encryption key**, unlike web-term's `WEBTERM_ENCRYPTION_KEY`. Do not copy that field |
| Handshake: `stdout` contains `BACKEND_PORT:` → `backendPort`; IPC `get-backend-port` + `backend-ready` event; `did-finish-load` re-sends if already ready | `parse_handshake_line()` + `tokio::sync::watch<BackendStatus>` (`Starting → Ready | Failed{reason, stderr_tail} | Crashed{exit_code}`); app subscribes and renders `reconnect_banner` |
| Readiness: implicit (window loads, API fallback `localhost:8080`) | Explicit: poll `GET {base}/api/settings` ≤15 s (web-term pattern — strictly better; adopt it) |
| Quit: `backendProcess.kill()` on `will-quit`; `--no-backend --port` override | `Supervisor::stop(grace)` (SIGTERM→kill on unix, kill on Windows) + same CLI flags |

**Example (adapted from web-term `supervisor/src/lib.rs`, wa-bot env):**

```rust
// SpawnOptions for wa-bot: NO encryption key (unlike web-term).
pub fn env_vars(&self) -> Vec<(&'static str, String)> {
    vec![
        ("PORT", ":0".to_string()),
        ("ALLOWED_ORIGINS", "*".to_string()),
        ("DB_PATH", self.db_path.to_string_lossy().to_string()),
        ("MEDIA_PATH", self.media_path.to_string_lossy().to_string()),
        ("TZ", "Asia/Jakarta".to_string()),
    ]
}
pub fn parse_handshake_line(line: &str) -> Option<u16> {
    line.find("BACKEND_PORT:").and_then(|i| {
        line[i + "BACKEND_PORT:".len()..].chars()
            .take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok()
    })
}
```

### Pattern 2: Typed Backend Client (full `api.ts` surface)

**What:** `backend-client` mirrors every `ApiClient` method and DTO with serde structs: `Chat` (+ `normalize_chat` null-guard), `Message` (+ `MessageExtra`: poll/location/contact/viewOnce/linkPreview), `ChatState`, `HistorySyncStatus`, `Contact`, `GroupCache/GroupPreview/GroupParticipantInfo`, `StatusEntry/StatusGroup`, `Trigger/CronJob/Webhook/WebhookLog`, `CallState/CallLog/CallHistoryResponse`, `SettingsMap`. REST via reqwest (`rustls-tls`, **not** OpenSSL — avoids Linux CI system-dep pain, per web-term precedent). Error enum `ClientError::{Request, Status, Decode, WebSocket}`.

**When to use:** Every server round-trip goes through this crate; views never touch HTTP directly.

**Trade-offs:** ~70 endpoints is a large but mechanical port (estimate from `routes.go`: chats 12, msg-mgmt 6, msg-types 4, groups 10, stickers 3, contacts 1, status 6, channels 7, system 4, settings 2, triggers 5, cron 6, webhooks 8, docs/AI 2, calls 12+). Port in dependency order (chat → status/channels → calls → bot → settings), each slice independently testable with `TcpListener` stub servers (web-term `lib.rs` tests show the exact recipe).

**Endpoint inventory (from `routes.go`, authoritative):**

```
POST /send-message /send-media /send-sticker /send-bulk-{same,different}-messages
GET  /chats  /chats/{id}/messages?limit&before&after  /chats/{id}/search  /chats/{id}/messages/{msgId}/context
GET  /chats/{id}/{media,docs,links}  POST /chats/{id}/{read,presence-subscribe,pin,archive,mute}
GET  /chats/{id}/messages/{messageId}/media  POST|GET /history-sync{,/status}
POST /chats/{chatId}/messages/{id}/{delete,edit,reply,react,forward,vote}  POST /chats/{chatId}/{poll,location,contact,typing}
GET|POST|PATCH /groups/... (10 routes)   /stickers/... (3)   GET /contacts
GET|POST /statuses... (6)   /channels... (7)   GET /status /qr-code POST /logout GET /health
GET|PUT /settings   /triggers... (5)   /cron... (6)   /webhooks... (8)   GET /docs POST /ai/assistant
GET /calls/active /calls/history  POST /calls /calls/group  POST /calls/{id}/{answer,reject,hangup,video/*,participants}
WS   /ws  (+ top-level /ws/calls/{id}/media)
```

**`mediaURL()` must be ported:** `api.ts:mediaURL` resolves relative `mediaUrl` against the base URL (stripping trailing `/api`). Every GPUI image/audio/video element needs this helper — put it on `BackendClient`.

### Pattern 3: WS Event Pump → Central Dispatch (port of `ws-bus` + `AuthContext` + `CallContext`)

**What:** One WS connection per app lifetime (`ws://localhost:{port}/ws`, derived from supervisor `BackendInfo` via `normalize_ws_url`). On open, send `{"type":"authenticate","payload":{"userId":"user-1"}}`. Reconnect after 3 s on abnormal close (code ≠ 1000), re-resolving the URL each attempt (port can change). Incoming `{type, payload}` frames go over a flume channel into `cx.update` → `AppState::handle_ws_event`, which fans out exactly like `AuthContext.handleWSMessage` + `subscribeWS` listeners.

**When to use:** All realtime: messages, reactions, presence, calls, QR, status/channels invalidation.

**Trade-offs:** GPUI has no hook equivalent — the pump lives on `TOKIO_RT` (static multi-thread runtime, as in web-term `app_state.rs`) and crosses into GPUI via `cx_handle.update()`. Keep the WS→UI handoff to **one** method (`handle_ws_event`) so message-type coverage is auditable.

**Full WS type → AppState mapping (exhaustive from `AuthContext.tsx`, `CallContext.tsx`, page subscribers, Go broadcast sites):**

| WS `type` | Producer (Go) | Web consumer | GPUI handling |
|---|---|---|---|
| `qr_code` `{code}` | `server.go` | AuthContext → login QR | `login.rs` view state |
| `auth_success` | backend | AuthContext → `isLoggedIn=true` | AppState auth field + route to `/chat` |
| `new_message` full payload | `handler.go:338 BroadcastMessage` | AuthContext → `incomingMessage` + `chatUpdate`; ChatArea upsert | `upsert_message` + `upsert_chat` (move-to-top on lastMsg/lastTime change); dedup via `processed_msg_ids: HashSet` |
| `message_reaction` | backend | `patchMessage(reactions)` | same |
| `poll_update` | backend | `patchMessage(extra)` | same |
| `message_deleted` / `message_edited` | `event_handler.go`, `message_management_handler.go` | synthetic incomingMessage | same store ops (delete/patch) |
| `message_status` `{id,status}` | backend | `statusUpdate` → receipt ticks | `patch_message(id, {status})` |
| `chat_name_update`, `group_updated` | backend | patch chat name/avatar | patch chat row |
| `chat_state` `{chatId,…}` (pin/archive/mute) | backend | `patchChatState` | same |
| `chats_changed` | backend | `invalidateMessages()` + refetch `getChats()` | mark all entries `loaded=false`, refetch |
| `chat_presence` `{chatId}`, `presence` `{jid}` | `event_handler.go:1243` | ChatArea typing/online indicator | ephemeral per-chat presence field (not persisted) |
| `status_new` | backend | StatusPage refresh | refetch statuses |
| `channels_changed`, `channel_message`, `channel_update` | backend | ChannelsPage refresh / reload channel | refetch list or channel |
| `call.incoming/state/ended/peer_accepted/ready/video_state/video_upgrade_requested/group_state/participant_join` | `call/service.go` (11 sites) | CallContext → overlays | call store state machine (`CallStatus` 12 variants) + overlay views |
| `history-sync` progress | backend | history sync banner | status field + banner (confirm exact type string during implementation — not observed in web subscribers) |

**Example (pump shape, from web-term `terminal_ws.rs` + `use-websocket.ts` semantics):**

```rust
// wa_ws.rs: normalize http://127.0.0.1:PORT -> ws://127.0.0.1:PORT/ws
pub fn normalize_ws_url(base: &str) -> String { /* web-term pattern verbatim */ }

pub struct WaWsHandle { tx: flume::Sender<String> /* client→server */, }
pub async fn run_pump(base_url: String, events: flume::Sender<WsEvent>) {
    // loop: connect → send {"type":"authenticate","payload":{"userId":"user-1"}}
    // → forward tx→ws, ws→events; on abnormal close sleep 3s and reconnect,
    // re-resolving URL each attempt (port may change after supervisor restart)
}
```

### Pattern 4: GPUI AppState as `chatStore` + `AuthContext` + `CallContext` Combined

**What:** Web keeps three separate stores; GPUI collapses them into one root `AppState` entity (web-term precedent: one 1400-line `app_state.rs` holding everything). GPUI reactivity is `cx.notify()` on mutation — no fine-grained selectors, so colocate what renders together.

**When to use:** Structure `AppState` fields as direct ports:

| Web source | GPUI field | Notes |
|---|---|---|
| `chatStore.chats/chatsLoaded/chatsLoading` | `chats: Vec<Chat>, chats_loaded, chats_loading` | `upsert_chat` preserves `unread` unless carried; move-to-top on lastMsg/lastTime (port verbatim) |
| `chatStore.messagesByChat` + `ChatMessagesEntry` (messages/hasMore/hasMoreNext/loaded/loading/loadingMore/loadingNewer) | `messages: HashMap<String, ChatMessages>` same flags | Pagination actions `set/prepend/append` map 1:1 to `GET messages?before&after`; `getMessageContext` powers search-jump |
| `sortAsc` + `normalizeChat` guards | same functions in `backend-client::types` | Cheap, prevents whole classes of render bugs |
| `temp-` pending-send replace in `upsertMessage` | same | Optimistic send UX parity |
| `AuthContext` (isLoggedIn tri-state, qrCode, logout dialog) | `auth: AuthState` + `View::Login` routing | `GET /status.isLoggedIn` on boot; `POST /logout` |
| `CallContext` (CallState 12-status machine) | `call: Option<CallState>` + overlay flags | `IncomingCallOverlay` + `CallOverlay` always mounted (App.tsx pattern) |
| HashRouter 19 routes | `View` enum (~12 variants: Chat, Status, Channels, Calls, CronList, CronEditor, TriggerList, TriggerEditor, WebhookList, WebhookEditor, WebhookLogs, Settings, Docs, Login) | Route params (`:id`) become `selected_*` fields |
| `ThemeProvider` + `AppThemeProvider` + `themes.ts` | `theme.rs`: `ThemePreset` list + `apply_theme` + per-view color helpers (`bg()`, `card_bg()`, …) | Web-term `theme.rs` + `find_theme_preset` pattern; persist preset id in `settings` crate |

## Data Flow

### Request Flow

```
[GPUI view event, e.g. send message]
    ↓ (method on AppState, e.g. send_text(chat_id, content))
[optimistic upsert temp- message + cx.notify()]
    ↓ (TOKIO_RT.spawn backend_client.send_message(...))
[Go backend → WhatsApp → BroadcastMessage("new_message")]
    ↓ (wa_ws pump → flume → cx_handle.update(handle_ws_event))
[upsert_message replaces temp- entry (content/type match) + cx.notify()]
```

### State Management

```
[AppState entity]
    ↓ (field mutation + cx.notify())
[views re-render] ←→ [view events] → [AppState methods] → [TOKIO_RT async backend calls]
    ↑ (ws pump / fetch results return via cx_handle.update + cx.notify())
```

### Key Data Flows

1. **Boot:** `main.rs` loads `DesktopSettings` → resolves backend path (`bundle.rs`) → `Supervisor::spawn` (handshake + `GET /api/settings` probe) → `BackendClient::new(base_url)` → `normalize_ws_url` + pump start → `GET /status` (login?) → `GET /chats` → render. Failure at any step lands in `BackendStatus::Failed/Crashed` → `reconnect_banner` view with `stderr_tail` (never seen in Electron — strictly better diagnostics).
2. **Chat read path:** select chat → `ensure_chat_state` → `GET messages?limit=100` → `set_messages`; scroll-up → `?before=` → `prepend`; `hasMoreNext`/search-jump → `?after=` or `getMessageContext` → `append`. Mirrors `ChatArea.tsx` exactly.
3. **Call path:** `call.incoming` → overlay + ring; answer/reject/hangup → `POST /calls/{id}/…` → `call.state` stream drives overlay; `call.ended` → history refetch. Video flags (`video_enabled`, `remote_video_enabled`) map to overlay controls; actual media transport stays backend-side (`/ws/calls/{id}/media` — GPUI renders controls, not WebRTC).

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| Single user, local sidecar (this milestone) | Monolith app crate is fine; no change needed |
| Large chat histories (10k+ msgs/chat) | Keep per-chat pagination flags; GPUI `List` virtualization for message column; never `set_messages` unbounded — same discipline as web |
| Many chats (1k+) | Sidebar virtualized list; `upsert_chat` move-to-top is O(n) — fine at this scale |

### Scaling Priorities

1. **First bottleneck:** Message list render with media thumbnails — use GPUI virtualized list + `mediaURL()`-resolved async image loading, not eager fetch.
2. **Second bottleneck:** WS fan-out during history-sync bursts — `chats_changed` refetch is already debounced server-side; keep `processed_msg_ids` dedup set bounded (web leaks it — cap at e.g. 5k entries in Rust).

## Anti-Patterns

### Anti-Pattern 1: Copying web-term's encryption-key custody into wa-bot's supervisor

**What people do:** Copy `SpawnOptions::new(backend, db, encryption_key)` verbatim including `WEBTERM_ENCRYPTION_KEY` validation.
**Why it's wrong:** wa-bot backend takes `DB_PATH`/`MEDIA_PATH`/`PORT`/`ALLOWED_ORIGINS`/`TZ` — there is no encryption-key env. The spawn will "succeed" but the key env is dead weight and the validation gate blocks startup for no reason.
**Do this instead:** Write wa-bot's `SpawnOptions` from `desktop/main.js` env list (Pattern 1 table), keeping only web-term's *mechanics* (handshake parse, stderr ring, watch channel, readiness probe, `kill_on_drop`).

### Anti-Pattern 2: Storing DB-backed settings in the `settings` crate

**What people do:** Mirror `GET /api/settings` keys (Gemini key presence, TTS provider, …) into `DesktopSettings` JSON.
**Why it's wrong:** Those live in the backend SQLite and are shared with web/GTK; duplicating them locally forks truth and breaks cross-client consistency.
**Do this instead:** `settings` crate holds UI prefs only (theme preset, window state, backend path, last URL) — web-term's documented decision (`settings/src/lib.rs` header). App settings always round-trip `GET/PUT /api/settings`.

### Anti-Pattern 3: One GPUI entity per chat/message

**What people do:** Model each chat/message as its own `Model<T>` for "fine reactivity".
**Why it's wrong:** Hundreds of entities + cross-entity updates (new_message touches chat row + message list + unread badge) become borrow/cx-update spaghetti; web-term keeps one `AppState` for exactly this reason.
**Do this instead:** One `AppState` + plain Rust structs + targeted `cx.notify()`. Split only genuinely independent overlays (call overlay, modals) if profiling justifies it.

## Integration Points

### External Services (backend as seen by GPUI)

| Service | Integration Pattern | Notes |
|---|---|---|
| `wa-bot-backend` process | Child process, env-configured, stdout handshake | Binary path two-tier (bundled `resources/be` vs dev); `kill_on_drop(true)`; stderr ring 2000 chars for Failed view |
| `http://127.0.0.1:{port}/api` | reqwest JSON client, 5 s timeout; `is_ready()` 1 s probe | Base URL from `BackendInfo`; `mediaURL()` helper for relative media paths; secret header `VITE_API_SECRET`→ sticker endpoints need equivalent (confirm value source at build time — likely env/constant, not user input) |
| `ws://127.0.0.1:{port}/ws` | tokio-tungstenite, authenticate on open, 3 s reconnect | Re-resolve URL per attempt; single shared pump; typed `WsEvent` enum covering the 20+ types in Pattern 3 table |
| WhatsApp network | Never touched directly — all via backend | QR code arrives as `qr_code` WS payload; login state via `GET /status` |

### Internal Boundaries

| Boundary | Communication | Notes |
|---|---|---|
| `wabot` → `backend-client` | `BackendClient` clone (reqwest Client is cheap-clone) + `WaWsHandle` | Views call `AppState` methods; only `AppState` holds client handles |
| `wabot` → `supervisor` | `SpawnOptions` in, `watch::Receiver<BackendStatus>` out | Spawn happens in `main.rs` before GPUI run; status bridged into AppState |
| `wabot` → `settings` | Load at boot, `save()` on pref change | `custom_base` injectable for tests (web-term pattern) |
| tokio ↔ GPUI | `TOKIO_RT.spawn` + flume/mpsc + `cx_handle.update()` | Never block GPUI thread on network; never touch `cx` from tokio threads directly |
| GPUI views ↔ AppState | `view.update(cx, \|this, cx\| …)` + `cx.notify()` | Same idiom throughout web-term `app_state.rs` fetch methods — copy the shape |

### New vs Modified (explicit)

**NEW (all inside `desktop-gpui/`):** `Cargo.toml` + `Cargo.lock`, `crates/supervisor`, `crates/settings`, `crates/backend-client` (`types.rs` ~15 DTOs, `rest.rs` ~70 methods, `wa_ws.rs` pump), `crates/wabot` (`main.rs`, `app_state.rs`, `theme.rs`, `bundle.rs`, `actions.rs`, `icons.rs`, `window_state.rs`, `views/` ~14 files), `scripts/` packaging (Linux + Windows).
**MODIFIED (existing repo):** Nothing in `web/`, `internal/`, `cmd/`, `desktop/`, `desktop-gtk/` under normal circumstances. Only *additive* backend changes if a gap is proven (e.g. missing pagination cursor) — must be agreed, never a rewrite.
**REFERENCE-ONLY (read, don't depend):** `desktop/main.js` + `preload.js` (sidecar contract), `web/src/**` (parity oracle), `../web-term/desktop-gpui` (crate mechanics — not a build dependency), `desktop-gtk/` (second native-pattern reference).

## Build Order (dependency-respecting)

```
1. Workspace + supervisor (+ tests)        # no deps; unblocks everything; headless CI green day one
   └─ SpawnOptions (wa-bot env), handshake parse, readiness probe, stop(), --no-backend flag
2. settings crate (+ tests)                # no deps; paths + UI prefs + corrupt-file recovery
3. backend-client types.rs                  # no deps; DTOs + normalize_chat + sortAsc + mediaURL unit tests
4. backend-client rest.rs (chat slice)      # → types; getChats/messages/send/read/pin/archive/mute/history-sync
5. backend-client wa_ws.rs pump             # → types; connect/auth/reconnect + WsEvent enum (chat types first)
6. wabot shell: main.rs + bundle.rs + AppState boot + nav + reconnect_banner + theme.rs
   └─ → supervisor + settings + backend-client; first visible milestone: spawn → chats list
7. views/login.rs + auth flow               # → shell; qr_code/auth_success, GET /status, POST /logout
8. views/chat.rs (core → full parity)       # → rest chat slice + WS chat types; paginated lists, send/reply/edit/delete/react/forward
9. rest.rs remaining slices + matching views in order: status → channels → calls (+ overlays) → triggers/cron/webhooks(+logs) → settings → docs/AI
10. Chat parity extras (media viewer/player, voice notes, polls, location, contact, sticker, group panels, search, presence/receipts)
11. Packaging Linux + Windows (backend bundled) + --no-backend dev docs
```

Each step 4–9 slice is independently demoable against the real sidecar (`cargo run -- --no-backend --port` for backend-less UI work once the client exists).

## Sources

- HIGH — `desktop/main.js`, `desktop/preload.js` (sidecar + port discovery contract, read in full)
- HIGH — `web/src/lib/api.ts` (DTOs + ApiClient surface), `web/src/stores/chatStore.ts`, `web/src/hooks/use-websocket.ts`, `web/src/lib/ws-bus.ts`, `web/src/contexts/AuthContext.tsx`, `web/src/App.tsx` (state + realtime + routing model)
- HIGH — `internal/delivery/http/routes/routes.go` (authoritative endpoint inventory), `internal/delivery/http/websocket.go`, `internal/delivery/http/server.go`, `internal/delivery/whatsapp/event_handler.go`, `internal/infrastructure/call/service.go` (WS broadcast types)
- HIGH — `../web-term/desktop-gpui/Cargo.toml`, `crates/supervisor/src/lib.rs`, `crates/backend-client/{lib,rest,terminal_ws}.rs`, `crates/settings/src/lib.rs`, `crates/webterm/src/app_state.rs` (proven crate mechanics to adapt, not copy)
- MEDIUM — `CallContext.tsx`, page-level `subscribeWS` call sites, `CallHistoryPage` (call overlay + history refresh flows); history-sync WS type string unconfirmed — flag for phase research

---
*Architecture research for: desktop-gpui client (wa-bot milestone v1.0)*
*Researched: 2026-09-08*

# Stack Research: desktop-gpui client for wa-bot

**Domain:** Native Rust-GPUI desktop client reusing existing Go backend (HTTP+WS sidecar)
**Researched:** 2026-09-08
**Confidence:** HIGH (core pins proven in `../web-term/desktop-gpui`; backend contract read from source; supporting-crate versions verified against crates.io API)

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| `gpui` (package `gpui-pre`) | `=0.3.3` exact | GPU-accelerated UI framework (window, entities, views, async executor) | Proven in web-term/desktop-gpui. Use the `gpui-pre` channel, NOT registry `gpui 0.2.2`: the official `gpui` 0.2.x line is a stale Oct-2025 snapshot predating Zed's `gpui`/`gpui_platform` split, while `gpui-pre 0.3.3` tracks the current architecture web-term compiles against. Pre-1.0 GPUI has real breaking churn → exact pin + committed `Cargo.lock`. |
| `gpui-component` | `=0.6.0` exact | 60+ shadcn-style components (buttons, dialogs, lists, tables, dock layout, theme) | Design is explicitly shadcn/ui-based — the same design language as the wa-bot web client (`web/` uses shadcn/ui + Tailwind v4). This is the fastest path to the milestone's 1:1 visual parity requirement. MUST stay version-matched to `gpui-pre 0.3.3`: a mismatched gpui/gpui-component pair compiles but fails with `E0277` Render-trait mismatches at call sites. |
| `gpui-platform` (package `gpui-pre-platform`) | `=0.3.3` exact | App entry (`gpui_platform::application()`), windowing/text backends per OS | Required since the platform split; web-term's app crate depends on it directly. Same exact pin as `gpui-pre`. |
| Rust toolchain | recent stable (pin via `rust-toolchain.toml` at scaffold) | Compiler | GPUI HEAD-era code uses just-stabilized `std` APIs; a stale toolchain is the most common mysterious build failure. |
| Go backend (reused, NOT rewritten) | Go `1.25.0` (per `go.mod`, module `wa-bot`) | REST+WS server run as sidecar | Mature, shipped, ~90-method surface. Zero stack change on the backend side. |
| tokio | `=1.53.1`, features `rt-multi-thread, process, macros, sync, time, net` | Async runtime for supervisor (spawn/pipes), backend-client (HTTP/WS) | `process` for sidecar spawn, `net`+`time` for readiness probing. Multi-thread runtime bridged into GPUI via `cx.spawn` / `cx.update` — the web-term pattern. |
| reqwest | `=0.12.28`, `default-features=false`, features `json, multipart, rustls-tls` | Typed HTTP client for all `api.ts` endpoints | `rustls-tls` avoids the OpenSSL system dependency on Linux CI. `multipart` is mandatory: `sendMedia`, `sendAudio`, `setGroupPhoto` upload via multipart in `api.ts`. `json` for the standard `ApiClient.request()` POST/GET bodies. |
| tokio-tungstenite + futures-util | `=0.26.2` (`connect, handshake`, no default features) + `=0.3.32` | WS client for `/ws` | Exact web-term pins. Mirrors `use-websocket.ts`: connect → `authenticate {userId}` → JSON `{type, payload}` stream → 3s autoreconnect unless close code 1000. |
| flume | `=0.12.0` | WS→GPUI bridge channel | Backend-client runs on tokio; GPUI views update on the UI thread. flume channel + `cx.update` pump is the proven web-term bridge (replaces web's `ws-bus.ts` emit/subscribe). |
| serde / serde_json | `=1.0.229` (derive) / `=1.0.151` | DTOs for every endpoint + WS payloads | Covers the full `api.ts` type surface (chats, polls, reactions, calls, settings maps) and `{type, payload}` WS envelopes. |
| dirs | `=6.0.0` | OS config/data dirs for settings store + backend `DB_PATH`/`MEDIA_PATH` | Mirrors Electron `app.getPath('userData')` layout (`database/`, `media/` subdirs created at startup in `desktop/main.js`). Settings crate stores UI prefs as JSON here (backend SQLite stays the system of record). |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| thiserror | `=2.0.20` | Typed errors in library crates (supervisor, settings, backend-client) | Always in the three non-UI crates — web-term pattern. |
| anyhow | `=1.0.104` | Error context in the app crate | App shell only; keeps UI code free of error enums. |
| parking_lot | `=0.12.5` | Fast mutexes for shared supervisor/backend state | Supervisor port/ready state shared between tokio tasks and GPUI thread. |
| raw-window-handle | `=0.6.2` | Native handle interop | App crate only (tray/taskbar, native dialogs later). Proven web-term pin. |
| image | `=0.25.10` (verified current via crates.io), `default-features=false` + `jpeg,png,gif,webp` | Decode chat media + QR pixmaps into buffers convertible to GPUI `RenderImage` (BGRA) | NEW vs web-term (terminal needed no images; a WhatsApp client is media-heavy: chat images, stickers, avatars, group photos, status media). Feature-gate formats to keep the binary small. |
| qrcode | `=0.14.1` (verified current via crates.io) | Render `/login` QR natively from the `getQrCode` string | NEW vs web-term. Web renders QR in the browser; GPUI has no browser → encode locally and blit via the `image` pipeline. No system dependency. |
| rodio | `=0.22.2` (verified current via crates.io) | Voice-note + audio-message playback (`sendAudio`/`getChatMedia` audio) | NEW vs web-term. Lightweight pure-Rust-friendly playback; do NOT bundle a full media player for v1.0. |
| rand | `=0.10.2` | Request correlation IDs, temp names, jitter for WS backoff | supervisor + backend-client, as in web-term. |
| tempfile (dev) / libc (dev) | `=3.27.0` / `=0.2.189` | Hermetic tests for supervisor/settings/backend-client | Dev-dependencies only; supervisor lifecycle tests need process-signal control (`libc`). |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| cargo + `rust-toolchain.toml` | Build, pin stable toolchain | Commit `Cargo.lock` (workspace root) — reproducibility is the whole point of exact pins. |
| `[profile.release] lto = "thin"` | Binary size / perf | Copy from web-term root `Cargo.toml`; ~12MB-class binaries expected. |
| cargo-deb | Linux packaging (`.deb`) | Milestone targets Linux + Windows; `.deb` covers Ubuntu/Debian first. Bundle the `wa-bot-backend` Go binary alongside (same `resources/be` layout as Electron). |
| cargo-wix (WiX) | Windows packaging (`.msi`) | Bundle `wa-bot-backend.exe`; replicate `desktop/main.js` env (`PORT=:0`, `ALLOWED_ORIGINS=*`, `DB_PATH`, `MEDIA_PATH`, `TZ=Asia/Jakarta`). |
| `BACKEND_PORT:` stdout handshake | Sidecar port discovery | No new tooling: backend already prints `BACKEND_PORT:<port>` (`cmd/api/main.go:42`, `internal/delivery/http/server.go:150`). Supervisor parses it — same as Electron. |

## Installation

```bash
# Scaffold (inside wa-bot repo; reference only, not a build dependency)
cargo new --lib desktop-gpui --name wabot
# then create workspace members:
#   desktop-gpui/crates/{supervisor,settings,backend-client} + desktop-gpui/crates/app (bin+lib)

# Core (workspace Cargo.toml — exact pins, cf. ../web-term/desktop-gpui/Cargo.toml)
# gpui = { package = "gpui-pre", version = "=0.3.3" }
# gpui-component = "=0.6.0"
# gpui-pre-platform = "=0.3.3"   (as gpui-platform, app crate only)
# tokio = "=1.53.1", reqwest = "=0.12.28" (rustls-tls, json, multipart)
# tokio-tungstenite = "=0.26.2", futures-util = "=0.3.32", flume = "=0.12.0"
# serde = "=1.0.229", serde_json = "=1.0.151", dirs = "=6.0.0"
# thiserror = "=2.0.20", anyhow = "=1.0.104", parking_lot = "=0.12.5"
# image = "=0.25.10", qrcode = "=0.14.1", rodio = "=0.22.2"

cargo build -p wabot   # first build resolves + writes Cargo.lock — commit it
```

## Backend Integration Points (Go backend reused as-is)

| Concern | Contract (read from source) | GPUI implementation |
|---------|----------------------------|---------------------|
| Base URL discovery | `getApiBase()` (`web/src/lib/api.ts:1-21`): `__BACKEND_PORT__` → `http://localhost:<port>/api`, else `VITE_API_URL`, else same-origin `/api`, else `http://localhost:8080/api` | Supervisor spawns backend with `PORT=:0`, parses `BACKEND_PORT:<port>` from stdout, hands port to backend-client (`setBaseUrl`, mirroring `setBackendPort`). Support `--no-backend --port X` override exactly like Electron (`desktop/main.js:8-43`). Dev fallback `localhost:8080/api`. |
| HTTP semantics | `ApiClient.request()` (`api.ts:389-410`): JSON fetch, error shape `{error}`, throws `Error(error.error)`; `mediaURL()` (`api.ts:377-387`) resolves `/api/...`-prefixed media paths against base; media endpoints use multipart with a `secret` field (`VITE_API_SECRET`, default `"default-secret"`) | backend-client: one typed method per `api.ts` method (~90: chats, messages/media, contacts, stickers, groups, status, channels, calls+video-signal, triggers, cron, webhooks+logs, settings, AI assistant, docs, QR/logout). Replicate `mediaURL()` resolution and per-endpoint `secret` handling exactly — do not "simplify" auth. |
| WS protocol | `use-websocket.ts`: `ws://localhost:<port>/ws` (`/ws` route in `internal/delivery/http/routes/routes.go:224`, gorilla/websocket server); JSON `{type, payload}` (`WSMessage`); `sendMessage("authenticate", {userId})` on open; 3s reconnect unless close code 1000 | backend-client owns the socket on the tokio runtime; pumps decoded `WSMessage`s over a flume channel; app crate drains it via `cx.update` into GPUI entities (replaces `ws-bus.ts` emit/subscribe). Reconnect/backoff lives in backend-client, not UI code. |
| Sidecar lifecycle | `desktop/main.js:59-124`: spawn `wa-bot-backend[.exe]`, env `PORT=:0, ALLOWED_ORIGINS=*, DB_PATH, MEDIA_PATH, TZ=Asia/Jakarta`, cwd=userData; kill on quit (`will-quit`) | supervisor crate: same spawn/env/watch/kill, plus readiness probe before opening the main window; kill on app exit. Backend binary bundled next to the GPUI binary (Electron `resources/be` layout). |
| Backend changes | None required | Backend stays Go 1.25.0 untouched; any future need is additive API only, by agreement (PROJECT.md constraint). |

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| `gpui-pre =0.3.3` + `gpui-component =0.6.0` | Registry pair `gpui =0.2.2` + `gpui-component =0.5.1` | Only if the project bans the `gpui-pre` mirror channel: the registry pair is the last fully-published snapshot (Feb-2026 cutoff for component). Accept being ~2 releases behind and re-verify shadcn component coverage for chat widgets. |
| reqwest rustls-tls | reqwest default-tls (OpenSSL) | Never on Linux CI for this project — system OpenSSL breaks hermetic builds; web-term already decided this (see its `Cargo.toml` comment). |
| rodio (audio only) | Full media stack (mpv/libvlc bindings) for voice notes + video calls | Only if v1.0 must have in-client video-call rendering. Recommendation is rodio now; video calls ride backend signaling (create/answer/reject/hangup already in `api.ts`) with native video rendering flagged as phase-level research, not stack. |
| cargo-deb + cargo-wix | Tauri bundler / electron-builder reuse | Only if the team wants a single bundler across Electron+GPUI. Not recommended: pulls webview/Chromium assumptions into a native build. |
| Native GPUI QR (`qrcode` crate) | Reuse web QR via embedded browser | Never — defeats the milestone's no-browser goal. |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| Rewrite backend Go → Rust | Backend is shipped and stable; milestone explicitly out-of-scope; ~90 endpoints + whatsmeow session state would be re-implemented for zero user value | Reuse via HTTP+WS sidecar (this stack) |
| Electron / Chromium / `wry` WebView (incl. gpui-component `webview` feature) | Reintroduces the browser the milestone removes; experimental WebView element with many limitations | Pure GPUI views + `image`/`qrcode` pipelines |
| Tauri | Same as above — webview runtime + JS bridge contradict native GPUI goal | Supervisor sidecar pattern from `desktop/main.js` |
| GTK4 linkage in the GPUI client | `desktop-gtk/` already covers the GTK path; mixing toolkits doubles packaging pain (Linux+Windows) | GPUI renders its own UI via GPU; no system toolkit dep |
| `alacritty_terminal` | web-term-specific (terminal emulation); wa-bot needs chat bubbles, not a pty | gpui-component lists/tables (virtualized) for message lists |
| ORM / SQLite crates in the client (rusqlite, sqlx) | Backend owns SQLite (`mattn/go-sqlite3`); client-local DB invites split-brain with `chatStore.ts` semantics | backend-client talks HTTP; only UI prefs (theme, window state) in JSON via settings crate |
| Extra crypto/auth crates | Auth = existing `secret` field + QR session in backend; no new auth design | Mirror `api.ts` credential handling verbatim |
| `gpui-unofficial` or mixing gpui channels | Different gpui source than `gpui-component =0.6.0` was built against → `E0277` trait mismatches that look baffling (both halves compile) | Single matched pair: `gpui-pre =0.3.3` + `gpui-component =0.6.0`, bumped together |

## Stack Patterns by Variant

**If adding a new backend endpoint later (additive only):**
- Add the DTO to backend-client with serde types mirroring the TS types in `api.ts`, plus a WS event variant if it emits realtime updates — because the client is a 1:1 mirror, never a second source of truth.

**If Windows Media Foundation / Linux ALSA audio quirks appear with rodio:**
- Keep rodio's default backend per OS and gate voice-note playback behind a settings toggle — because audio-device variability is the known rodio support cost, and chat parity must not block on it.

**If `gpui-pre`/`gpui-component` need a monthly bump (pre-1.0 churn):**
- Bump both together, rebuild the world, run supervisor/backend-client tests, commit the new `Cargo.lock` — because partial bumps produce the E0277 trap above.

## Version Compatibility

| Package A | Compatible With | Notes |
|-----------|-----------------|-------|
| `gpui-component =0.6.0` | `gpui-pre =0.3.3` ONLY | Matched pair (verified: web-term workspace compiles this combo). |
| `gpui-pre-platform =0.3.3` | `gpui-pre =0.3.3` | Same release channel; app-crate entry point. |
| `tokio-tungstenite =0.26.2` | `tokio =1.53.1`, `futures-util =0.3.32` | Web-term proven combo; `connect, handshake` features suffice for `ws://` local backend (no TLS to sidecar). |
| `reqwest =0.12.28` | `tokio =1.53.1` (rt-multi-thread), rustls | rustls-tls needs no system OpenSSL — critical for Linux CI + Windows. |
| `qrcode =0.14.1` | `image =0.25.10` (qrcode `image` feature) | QR → image buffer → GPUI `RenderImage` pipeline. |
| Go backend | Go `1.25.0`, gorilla/websocket `v1.5.3` | Server side of `/ws`; client must speak plain WS + JSON, no subprotocol. |
| Crate `image =0.25.10` / `rodio =0.22.2` | independent of GPUI pins | Leaf dependencies; safe to bump alone. |

## Sources

- `../web-term/desktop-gpui/Cargo.toml` + per-crate manifests — proven pins (gpui-pre 0.3.3, gpui-component 0.6.0, tokio/reqwest/tungstenite/flume/dirs set) and crate split (supervisor/settings/backend-client/app) — HIGH
- `web/src/lib/api.ts` (1159 lines, ~90 methods), `web/src/hooks/use-websocket.ts`, `web/src/lib/ws-bus.ts` — HTTP semantics, WS protocol, base-URL discovery — HIGH
- `desktop/main.js` — sidecar spawn/env/BACKEND_PORT handshake/`--no-backend --port` override — HIGH
- `cmd/api/main.go:37-42`, `internal/delivery/http/server.go:150`, `internal/delivery/http/routes/routes.go:224`, `go.mod` (Go 1.25.0) — backend contract — HIGH
- crates.io API (`/api/v1/crates/{image,qrcode,rodio}`) — max stable versions 0.25.10 / 0.14.1 / 0.22.2 — HIGH
- crates.io + community build notes (hellno/deck LEARNINGS, Zed discussion #55271) — registry `gpui 0.2.2` staleness, matched-pair E0277 trap, `gpui_platform` split — MEDIUM (community, but cross-checked against web-term's working pins)

---
*Stack research for: wa-bot v1.0 desktop-gpui (NEW client only; Go backend, React web, Electron, GTK clients are validated and unchanged)*
*Researched: 2026-09-08*

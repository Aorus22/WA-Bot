# Project Research Summary

**Project:** wa-bot — desktop-gpui milestone (native Rust GPUI 1:1 parity client)
**Domain:** WhatsApp-management desktop client; fourth presentation shell over unchanged Go backend (REST+WS sidecar), Linux + Windows
**Researched:** 2026-09-08
**Confidence:** HIGH

## Executive Summary

This milestone adds a NEW native desktop client (`desktop-gpui/`, Rust + GPUI) that reproduces the shipped React web client 1:1 while reusing the Go backend as-is via an HTTP+WS sidecar. Experts build this as a strict parity port, not greenfield discovery: the web client (`web/src` — `api.ts`, `chatStore.ts`, `AuthContext`/`CallContext`, `use-websocket`/`ws-bus`, 10+ routes) is the spec, `desktop/main.js` is the sidecar contract, and the sibling `../web-term/desktop-gpui` workspace is the proven crate mechanics (exact pins, supervisor/settings/backend-client/app split, tokio↔GPUI bridge).

The recommended approach is dependency-ordered: workspace + exact pins + committed lockfile first, then GPUI-free `supervisor` (spawn/handshake/kill) and `settings` (UI prefs only), then `backend-client` (typed DTOs for the ~70-endpoint surface + WS pump with 3s reconnect and full dispatch table), then the `wabot` app shell (AppState entity, generated 60+-preset theme, router, toasts, overlays), then views in Chat → media/rich types → group admin → calls → status/channels → bot editors → settings/docs order, closing with per-OS packaging and an explicit parity audit. No backend rewrite, no browser/webview, no macOS in v1.

The key risks are all integration-shaped: pre-1.0 `gpui-pre`/`gpui-component` churn breaking builds, sidecar orphans and port races, WS reconnect that drops re-auth/QR/messages, `chatStore` semantics (ordering, temp-message replace, unread preservation, pagination flags, legacy-null normalization), QR dual-path expiry, the GPUI media void (no img/video/audio — voice notes, images, video need explicit native strategy), multipart field-contract mismatches, hand-ported theming that misses the 60-preset token pipeline, call state-machine drift, late Linux (Wayland+X11) + Windows packaging surprises, and silent parity drift across routes/message types. Each maps to a specific phase with a concrete verification (see Critical Pitfalls and Implications).

## Key Findings

### Recommended Stack

Rust GPUI client on a matched pre-1.0 pair, reusing the Go 1.25.0 backend untouched. Full rationale in `STACK.md`; pins are deliberate (web-term proven) and `Cargo.lock` is committed.

**Core technologies:**
- `gpui` (`gpui-pre =0.3.3` exact) + `gpui-platform` (`gpui-pre-platform =0.3.3` exact) — GPU UI framework + app entry/windowing; NOT registry `gpui 0.2.2` (stale pre-split snapshot)
- `gpui-component =0.6.0` exact — 60+ shadcn-style components; fastest path to 1:1 visual parity with web's shadcn/ui; must stay matched to `gpui-pre 0.3.3` or `E0277` Render-trait failures
- Go backend reused as sidecar (Go 1.25.0, module `wa-bot`) — ~90-method surface, zero stack change
- `tokio =1.53.1` (rt-multi-thread, process, macros, sync, time, net) — supervisor spawn/pipes + backend-client I/O, bridged via `cx.spawn`/`cx.update`
- `reqwest =0.12.28` (`json, multipart, rustls-tls`, no default features) — typed HTTP for all `api.ts` endpoints; rustls avoids Linux OpenSSL pain; multipart mandatory for media/photo uploads
- `tokio-tungstenite =0.26.2` + `futures-util =0.3.32` + `flume =0.12.0` — `/ws` client mirroring `use-websocket.ts` (authenticate on open, 3s reconnect unless close 1000) with WS→GPUI bridge channel
- `serde / serde_json`, `dirs =6.0.0`, `thiserror`, `anyhow`, `parking_lot` — DTOs, OS config/data dirs (userData layout), typed errors, shared state
- NEW vs web-term (media-heavy messenger): `image =0.25.10` (QR/media → `RenderImage` BGRA), `qrcode =0.14.1` (native `/login` QR), `rodio =0.22.2` (voice-note/audio playback; no full media player in v1)
- Tooling: `cargo` + `rust-toolchain.toml`, `[profile.release] lto="thin"`, `cargo-deb` (Linux) + `cargo-wix` (Windows), `BACKEND_PORT:` stdout handshake (no new tooling)

### Expected Features

This is a parity port — the shipped web client IS the spec (`FEATURES.md` catalogs every route, endpoint group, and message type from source). Web is the ceiling, GTK is the floor.

**Must have (table stakes — full 1:1, build in dependency order):**
- Backend-client + supervisor + app shell + theme + toasts + WS bus — nothing renders without these
- Login/QR (30s refresh + WS push + `GET /status` gate + logout dialog) — unlocks every route
- Chat: sidebar (list, live upsert/reorder/dedupe, search, pin/archive/mute, mark-read, new-group/join, history-sync) + conversation (pagination 100 + scroll prepend/append, date dividers, optimistic `temp-` send, reply/edit/delete, reactions, forward, in-chat search + context jump, presence, receipts, markdown, emoji, voice notes + media + viewer + download, link previews, stickers, polls, location, contact, GIF, info sheet media/docs/links tabs, full group-admin panels, call buttons)
- Full `Message.type`/`MessageExtra` matrix: text, image/video/document/audio/ptt, sticker, poll, location (+live), contact, view-once, gif, reply/forward, deleted/edited markers
- Calls: filterable history + 1:1/group create + incoming/active overlays (always mounted) + video-upgrade negotiation + native audio path + history refresh on `call.ended`
- Status (tray, text/media post, viewed, media fetch) + Channels (list, preview/follow/unfollow/mute, paginated feed, post reactions)
- Bot editors: triggers/cron/webhooks list + editor + test console + delete-all, webhook logs viewer, AI assistant sheet (`onApplyCode` into editors)
- Settings (theme swatches, AI/TTS keys with masked flags, read receipts, connection indicator, history-sync controls) + Documentation (backend `/docs` markdown) + Linux/Windows packaging with bundled backend
- ~70-endpoint `backend-client` checklist: see `FEATURES.md` "API Surface Catalog" (session, chats, messages, rich types, groups, status, channels, contacts, calls, bots, settings, WS inbound types) — the acceptance matrix for client coverage

**Should have (native-desktop value-adds, v1.x after parity validated):**
- Native notifications (message + incoming call, click-to-focus) — cheap, WS events already exist
- System-tray minimize + autostart + `wabot://chat/:id` deep link
- Global push-to-talk/mute hotkeys (Wayland limits apply — design around it)
- Cold-start/memory win over Electron is inherent — measure and advertise, don't build

**Defer (v2+):**
- Local FTS search index (HIGH cost; server search stays the fallback), multi-window pop-outs (GPUI maturity TBD), macOS build (explicitly out of scope), any web-divergent feature (needs its own web-port plan to avoid fork)

### Architecture Approach

Third presentation shell over the unchanged Go backend (web/Electron/GTK untouched); one WS connection per app lifetime pumping typed events into a single root `AppState` entity. Details + endpoint inventory + WS type table in `ARCHITECTURE.md`.

**Major components:**
1. `supervisor` crate — spawn + supervise `wa-bot-backend` sidecar: `PORT=':0'`, `ALLOWED_ORIGINS='*'`, `DB_PATH`/`MEDIA_PATH`, `TZ='Asia/Jakarta'`, `BACKEND_PORT:` parse, readiness probe, graceful kill, `--no-backend --port` override; tokio-only, GPUI-free, headless-testable (port of `desktop/main.js`)
2. `backend-client` crate — typed DTOs (`types.rs`) + REST (`rest.rs`, ~70 methods mirroring `api.ts` incl. `mediaURL()` + secret pass-through) + WS pump (`wa_ws.rs`: connect → authenticate → flume → `cx.update`, 3s reconnect, URL re-resolved each attempt)
3. `settings` crate — UI prefs ONLY (theme preset, window state, backend path override); DB-backed app settings stay server-side via `GET/PUT /api/settings`
4. `wabot` app crate — `main.rs` boot (settings → supervisor → GPUI run), `AppState` entity (`chatStore` + `AuthContext` + `CallContext` combined, `TOKIO_RT` static + `cx.spawn` bridge), `theme.rs` (generated 1:1 from `themes.ts`), `bundle.rs`/`actions.rs`/`icons.rs`/`window_state.rs`, 10 views (~19 routes: nav, login, chat, status, channels, calls, triggers/cron/webhooks + editors + logs, settings, docs, reconnect banner)
5. Go backend (existing, UNCHANGED) — REST `/api` + WS `/ws` source of truth; additive-only changes by agreement, never a rewrite

**Crate build order (dependency-respecting — keep this sequence in the roadmap):**
1. Workspace + `supervisor` (+ tests) → 2. `settings` (+ tests) → 3. `backend-client types.rs` → 4. `rest.rs` chat slice → 5. `wa_ws.rs` pump (chat types first) → 6. `wabot` shell (boot + nav + reconnect banner + theme; first visible milestone: spawn → chats list) → 7. `login.rs` + auth flow → 8. `chat.rs` core → full parity → 9. remaining `rest.rs` slices + matching views (status → channels → calls + overlays → triggers/cron/webhooks + logs → settings → docs/AI) → 10. chat parity extras (media viewer/player, voice notes, polls, location, contact, sticker, group panels, search, presence/receipts) → 11. Linux + Windows packaging + parity audit. Steps 4–9 slices are each independently demoable against the real sidecar.

### Critical Pitfalls

Top risks phase-mapped (all 11 + debt/perf/security/UX tables in `PITFALLS.md`):

1. **gpui-pre 0.3.3 pre-1.0 churn breaks the build** — exact `=` pins + committed `Cargo.lock` from day one; bump `gpui-pre`+`gpui-component` together as isolated PRs with Linux+Windows smoke (Foundation).
2. **Sidecar orphans/zombies + port races** — replicate `main.js` exactly (ephemeral `PORT=':0'`, handshake, health probe ≤15s, `kill_on_drop` + explicit stop, adopt-or-kill, `--no-backend` hatch); spawn→port→health→kill→no-orphan test (Foundation/backend-integration).
3. **WS reconnect "almost" right — drops auth/QR/messages** — state machine in `backend-client` (re-resolve URL every connect, authenticate every open, 3s backoff only if code ≠ 1000, single-connect guard, tolerate bad frames, seen-ID dedupe, full `AuthContext` dispatch table); per-type contract tests + kill-backend-mid-session test (Backend-client).
4. **ChatStore parity (order, temp-replace, unread, pagination flags, legacy-null, invalidate)** — port the six `chatStore.ts` rules literally with unit tests each; chat UI assumes the store is correct (App-state/store, before chat UI).
5. **QR dual-path expiry** — 30s poll AND WS `qr_code`/`auth_success` push AND `GET /status` gate; native `qrcode`→image render; expiry (>30s) + real-scan + backend-down tests (Auth).
6. **Media void (no img/video/audio in GPUI)** — decide in chat-media phase: `image`→`img` with LRU + disk cache off UI thread, `rodio` audio on background thread, explicit video go/no-go (FFmpeg cost vs thumbnail + open-externally fallback), `WAAudioPlayer` FNV-1a pseudo-waveform parity, `viewOnce` memory-only, `LazyMedia` viewport-gated fetch (Chat-media; packaging verifies on clean VMs).
7. **Multipart field-contract mismatches** — per-endpoint builders mirroring `api.ts` field-for-field (`send-media` secret+target+message+type+file+ptt/seconds/waveform/viewOnce; group photo file-only; status media type+caption+file) with live-backend round-trips (Backend-client + chat-compose).
8. **Theming 1:1 = generated 60+-preset token pipeline** — script-convert `themes.ts` (all presets, 16 tokens + sidebar aliases), preset-count parity test, single global `Theme`, hex→`Hsla` once, persisted via `settings`, hot-switch; theming lands BEFORE chat/calls views so all views consume tokens (Theming early).
9. **Calls without the `CallContext` state machine** — port all six fields + 12-status terminal set + REST+WS handling + optimistic rules; dedicated overlay window + ringtone; incoming-while-chatting / group / video-upgrade / mid-call-restart matrix (Calls, after WS + audio exist).
10. **Wayland+X11/Windows gaps found at the end** — both `wayland`+`x11` features, CI matrix incl. Windows from day one, per-OS bundle layout + system deps + clean-VM smoke, per-user DB/media dirs, scoped Wayland limits (no global hotkeys), file-dialog/drag-drop/IME per target (Foundation CI + Packaging; macOS stays out).
11. **Parity drift (the 1159-line `api.ts` silently shrinks)** — roadmap maps every route/endpoint/message-type to a phase; DTO contract tests; dedicated parity-audit phase before ship (reference precedent: phase-27-style audit); no phase completes without REQ/route mapping (Every phase + final audit).

## Implications for Roadmap

Suggested phase structure (follows the crate build order + FEATURES dependency tree + pitfall-to-phase mapping):

### Phase 1: Foundation (workspace, pins, CI matrix, supervisor, settings)
**Rationale:** Zero-view work unblocks everything; pin discipline and sidecar lifecycle are the two cheapest-to-prevent, most-expensive-to-fix risks.
**Delivers:** Workspace (`supervisor`, `settings`, `backend-client`, `wabot`) with exact pins + `Cargo.lock`, Linux+Windows CI (X11 and Wayland smoke from day one), `supervisor` crate (wa-bot env — NO encryption key — handshake, probe, stop, `--no-backend`), `settings` crate (UI prefs, paths, corrupt-file recovery).
**Addresses:** FEATURES backend-client/supervisor prerequisite; STACK install/packaging prerequisites.
**Avoids:** Pitfalls 1 (churn), 2 (orphans), 10 (platform gaps — early half).

### Phase 2: Backend-client core (types + chat REST + WS pump + upload builders)
**Rationale:** Every view depends on the typed client; chat slice first because chat is the product core and the largest consumer.
**Delivers:** `types.rs` (all DTOs + `normalize_chat` + `sortAsc` + `mediaURL` unit tests), `rest.rs` chat slice (chats/messages/send/read/pin/archive/mute/history-sync), `wa_ws.rs` pump (connect/auth/reconnect state machine, chat WS types first), multipart builders field-for-field with live-backend round-trips.
**Uses:** STACK tokio/reqwest/tungstenite/flume/serde pins; ARCHITECTURE Patterns 2–3.
**Avoids:** Pitfalls 3 (WS), 7 (multipart), 11 (contract tests start here).

### Phase 3: App shell + theming + auth
**Rationale:** Global singletons (auth tri-state, WS bus fan-out, theme, toasts, overlays, router) must exist before any route view; theming must precede all views or every view gets rewritten.
**Delivers:** `main.rs` boot (settings → supervisor → probe → client → pump → `GET /status` → `GET /chats`), `AppState` skeleton + nav router (deep-linkable chat id) + `reconnect_banner`, generated `theme.rs` (all presets, parity test, persistence, hot-switch), `login.rs` (QR 30s poll + WS push + `auth_success` transition + phone tab + logout dialog).
**Addresses:** FEATURES app shell + Auth & session table.
**Avoids:** Pitfalls 5 (QR), 8 (theming), 4-setup (store struct lands here or next, before chat UI).

### Phase 4: Chat core + store semantics
**Rationale:** Largest surface; store correctness gates all chat UI.
**Delivers:** App-state chat store (six `chatStore` rules with unit tests: sort-on-insert, temp-replace, unread preservation, per-chat pagination flags, null-coercion, invalidate), sidebar (list, live upsert, search, pin/archive/mute, mark-read, new-group/join, history-sync UI), conversation core (pagination, dividers, optimistic send, reply/edit/delete/react/forward, receipts, presence).
**Addresses:** FEATURES tables B + C-core + D-core.
**Avoids:** Pitfalls 4 (store), 11 (route/REQ mapping enforced).

### Phase 5: Chat media + rich types + group admin + search
**Rationale:** Media strategy decided here (not at packaging); rich types and group panels hang off the info sheet.
**Delivers:** Media pipeline (send/view/play/download, `LazyMedia`, image viewer, `WAAudioPlayer`-parity voice notes via rodio, view-once memory-only), markdown renderer (shared with docs/AI), emoji/sticker pickers, polls, location (static thumbnail + open-externally compromise), contact, GIF, link previews, info sheet (media/docs/links tabs), group panels (roster, roles, settings, invite link, photo, leave), in-chat search + context jump. Explicit video go/no-go recorded.
**Uses:** STACK `image`/`qrcode`/`rodio`; ARCHITECTURE media/chat-read flows.
**Avoids:** Pitfalls 6 (media), 7-reuse (uploaders), 11 (message-type matrix coverage).

### Phase 6: Calls (state machine + history + overlays + native audio)
**Rationale:** Needs WS layer (Phase 2) + audio backend (Phase 5) + chat header buttons (Phase 4); shipping call UI without audio produces a dead surface.
**Delivers:** Full `CallContext` port (six fields, 12-status machine, REST+WS, optimistic rules), history page with filters, incoming/active overlays (always mounted, always-on-top where allowed + ringtone), video negotiation controls, `call.ended` → history refetch, mid-call-restart resync via `getActiveCall`.
**Addresses:** FEATURES Calls table.
**Avoids:** Pitfalls 9 (calls), 3-reuse (call WS types), 6-reuse (audio path).

### Phase 7: Status + Channels
**Rationale:** Reuses media/reaction/viewer infra from Phase 5; smallest new-protocol cost.
**Delivers:** Status tray + text/media post + viewed + media fetch; channels list + preview/follow/unfollow/mute + paginated feed + post reactions.
**Addresses:** FEATURES Status + Channels tables.
**Avoids:** Pitfalls 6-reuse, 11 (coverage).

### Phase 8: Bot editors + AI sheet + webhook logs
**Rationale:** One shared editor+test-console component serves triggers/cron/webhooks; AI sheet embeds via `onApplyCode`.
**Delivers:** Trigger/cron/webhook list + editor + test console + delete-all; webhook logs viewer (filter, pagination, clear-all); AI assistant sheet (model picker, markdown+code render, apply-to-editor).
**Addresses:** FEATURES Bot management table.
**Avoids:** Pitfall 11 (biggest gap vs GTK — must not be dropped).

### Phase 9: Settings + Docs + native integrations
**Rationale:** Theme picker wires the Phase-3 global; docs reuses the Phase-5 markdown renderer; native integrations need user-validation before intrusiveness.
**Delivers:** Settings page (theme/mode picker, AI/TTS keys, receipts, connection indicator, history-sync controls), documentation page, toasts, window-state persistence + desktop polish, then P2 natives (notifications, tray, autostart) behind validation.
**Addresses:** FEATURES Settings/theming/docs + Differentiators v1.x.
**Avoids:** Pitfalls 8-wiring, UX pitfalls (shortcuts, startup progress, QR affordance, upload progress).

### Phase 10: Packaging + parity audit + hardening
**Rationale:** Packaging is a phase with acceptance, not a chore; parity audit is the milestone gate (reference precedent).
**Delivers:** `cargo-deb` + `cargo-wix` bundles with per-OS backend binary (`resources/be` layout), per-user DB/media dirs, clean-VM smoke (X11, Wayland, Windows: spawn + audio + dialogs + drag-drop + IME), secret/loopback/view-once security checks, route×feature×message-type checklist against live backend, remediation of gaps, `--no-backend` dev docs.
**Addresses:** FEATURES launch-with packaging line; anti-feature guards (no macOS, no webview, no backend edits beyond additive).
**Avoids:** Pitfalls 10 (late packaging), 11 (audit), 6-verify (native media deps ship), Security/Performance traps.

### Phase Ordering Rationale

- **Dependency order, not value order:** supervisor → client → shell → auth → chat → everything else mirrors the FEATURES dependency tree (`backend-client requires supervisor requires app shell requires login requires chat…`); building out of order creates untestable UI.
- **Risk-front-loaded:** churn (pins), orphans (supervisor), and theming (generated tokens) land before any view code because late recovery costs HIGH (theming rewrite, packaging failure) while early prevention is cheap.
- **Slice-demoable:** ARCHITECTURE build steps 4–9 each run against the real sidecar, so every phase ends with a runnable app, not mocks.
- **Shared-infra reuse:** markdown renderer (once → chat/docs/AI), media pipeline (once → chat/status/channels), editor component (once → triggers/cron/webhooks), audio backend (once → voice notes/ringtone/calls).

### Research Flags

Phases likely needing `/gsd-plan-phase --research-phase <N>` during planning:
- **Phase 5 (media/rich types):** video go/no-go (FFmpeg vs open-externally), rodio per-OS audio quirks (Windows Media Foundation / Linux ALSA), GPUI image-cache patterns — MEDIUM-confidence ecosystem area, packaging + licensing implications.
- **Phase 6 (calls):** native video capture/render if v1 keeps inline video (no `<video>` tag; hardest GPUI item), overlay always-on-top limits (Wayland layer-shell, Windows focus-steal), ringtone path — sparse GPUI-specific docs.
- **Phase 10 (packaging):** per-OS GPUI feature sets (`wayland`+`x11`, DirectX/DirectWrite, VS Build Tools + SDK), system-lib matrix, backend bundle layout on clean VMs.

Phases with standard patterns (skip research-phase):
- **Phase 1:** supervisor/settings mechanics copied from `../web-term/desktop-gpui` + `desktop/main.js` — HIGH-confidence, adapt-don't-invent.
- **Phase 2–4:** REST/WS/store ports with source-verified contracts (`api.ts`, `chatStore.ts`, `use-websocket.ts`, `AuthContext`) — mechanical, test-covered.
- **Phase 7–9:** status/channels/bot-editors/settings/docs reuse Phase-5 infra with fully inventoried endpoints — standard CRUD + render.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Core pins proven in `../web-term/desktop-gpui`; supporting versions verified via crates.io API; backend contract read from `go.mod`/source; only MEDIUM slice is community notes on registry staleness (cross-checked). |
| Features | HIGH | Catalogued directly from shipped web source (`App.tsx`, `api.ts` 1159 lines, contexts, store, chat pages); only MEDIUM slice is LoginPage phone-tab endpoint + deeper Settings sections (flagged for phase-level confirmation). |
| Architecture | HIGH | All integration contracts read from source (sidecar, REST inventory from `routes.go`, WS broadcast sites, crate mechanics from web-term); only MEDIUM slice is call-overlay/page-subscriber details + history-sync WS type string (unconfirmed — flag for implementation). |
| Pitfalls | HIGH (integration) / MEDIUM (GPUI platform gaps) | Integration pitfalls verified against `web/src` + reference workspace + its research; platform gaps (video, Wayland limits, Windows deps) rest on docs.rs/blog/community sources for a fast-moving pre-1.0 crate. |

**Overall confidence:** HIGH

### Gaps to Address

- **History-sync WS type string:** not observed in web subscribers — confirm exact string during backend-client implementation (grep Go broadcast sites); handle as `status`-style field + banner regardless.
- **LoginPage phone-tab endpoint:** verify what the web phone tab calls before porting; keep tab structure regardless (Phase 3).
- **Sticker secret source:** sticker endpoints need the secret equivalent — confirm value source at build time (likely env/constant like `VITE_API_SECRET`, not user input) (Phase 2).
- **Video scope (inline vs open-externally):** explicit go/no-go in Phase 5 with documented fallback; never let FFmpeg appear unplanned before packaging.
- **macOS exclusion guard:** packaging defaults must not pull macOS-only features (`font-kit` does nothing on Windows/Linux targets) — CI matrix is Linux + Windows only.

## Sources

### Primary (HIGH confidence)
- `web/src/lib/api.ts` (~1159 lines, ~70 methods + shared types incl. multipart contracts, message-type matrix) — HTTP surface, DTOs, upload fields
- `web/src/stores/chatStore.ts` — six store rules (sortAsc, temp-replace, unread, pagination flags, normalizeChat, invalidate)
- `web/src/hooks/use-websocket.ts` + `web/src/lib/ws-bus.ts` — `/ws` URL, authenticate handshake, 3s reconnect, clean-close exception, fan-out
- `web/src/contexts/AuthContext.tsx` (12-type dispatch + dedupe) + `web/src/contexts/CallContext.tsx` (six-field state machine + terminal statuses) + `web/src/App.tsx` (route inventory + always-mounted overlays)
- `web/src/pages/chat/` (ChatArea 1141 lines, sidebar, bubbles, compose dialogs, group panels, info/search sheets, pickers, viewer, `WAAudioPlayer` FNV-1a waveform) — component inventory
- `desktop/main.js` + `preload.js` — sidecar spawn/env/handshake (`BACKEND_PORT:`, `PORT=':0'`, `DB_PATH`/`MEDIA_PATH`, `TZ=Asia/Jakarta`, `--no-backend --port`)
- `cmd/api/main.go`, `internal/delivery/http/routes/routes.go` (authoritative endpoint inventory), `server.go`, `websocket.go`, `event_handler.go`, `call/service.go`, `go.mod` (Go 1.25.0) — backend contract
- `../web-term/desktop-gpui` workspace (Cargo pins `gpui-pre =0.3.3`/`gpui-component =0.6.0`, `crates/{supervisor,settings,backend-client,webterm}`, `TOKIO_RT` + `cx.spawn` bridge) — proven mechanics
- `web/src/data/themes.ts` (60+ presets × 16 tokens) + `AppThemeProvider` + `wa-bot-theme-preset` key — theming pipeline
- crates.io API for `image 0.25.10` / `qrcode 0.14.1` / `rodio 0.22.2` — max stable versions

### Secondary (MEDIUM confidence)
- `web/src/pages/{settings,login,status,channels,calls,bot,documentation}/` + `AIAssistant.tsx` + `components/call/*` (sampled headers; phone-tab endpoint + settings depth need confirmation)
- `CallContext.tsx` page-level `subscribeWS` sites + `CallHistoryPage` flows; history-sync WS string unconfirmed
- Zed `crates/gpui` README + "Linux when?" blog, docs.rs gpui platform matrix (pre-1.0 churn, X11+Wayland dual, per-OS renderer/text, hotkey limits)
- `../web-term/.planning/research/PITFALLS.md` (sidecar/churn lessons, parity-audit precedent), Zed discussion #55271 + community build notes (registry staleness, E0277 matched-pair trap, `gpui_platform` split — cross-checked vs working pins)

### Tertiary (LOW confidence)
- Community `gpui-video` (FFmpeg+CPAL) / `rodio` examples — ecosystem signal only; video decision needs phase-level research, not this summary.

---
*Research completed: 2026-09-08*
*Ready for roadmap: yes*

# Pitfalls Research: GPUI Desktop Parity Client for wa-bot

**Domain:** Adding a 1:1-parity Rust-GPUI desktop client to an existing Go (REST+WS) + React 19 system
**Researched:** 2026-09-08
**Confidence:** HIGH for integration pitfalls (verified against `web/src` source + shipped reference `../web-term/desktop-gpui` + its research); MEDIUM for GPUI platform gaps (docs.rs/blog sources, fast-moving pre-1.0 crate)

**Scope note:** These are pitfalls of *adding a parity client to this system* — not generic Rust advice. Every pitfall names the web behavior that must be matched, the warning sign, prevention, and the roadmap phase that owns it.

---

## Critical Pitfalls

### Pitfall 1: gpui-pre 0.3.3 pre-1.0 churn breaks the build with no local change

**What goes wrong:**
`cargo update` (or one new transitive dep) bumps `gpui-pre`/`gpui-component` across a breaking minor; the workspace stops compiling weeks later and nobody changed app code. The reference project (`../web-term/desktop-gpui`) exists precisely because of this: every core dep is pinned `=x.y.z` with a committed `Cargo.lock`, and the pins (`gpui-pre =0.3.3`, `gpui-component =0.6.0`, tokio/reqwest/serde exact) are documented as deliberate. Zed's own README warns GPUI is pre-1.0 with frequent breaking changes.

**Why it happens:**
`gpui`, `gpui-component`, and friends are all 0.x — minor version = breaking. Rust's default loose-requirement flow hides breakage until someone updates. `gpui-component` must also stay compatible with the exact `gpui` pin; bumping one without the other is a classic failure.

**How to avoid:**
Copy the reference pattern from day one: exact `=` pins for every core dep in workspace `Cargo.toml`, commit `Cargo.lock`, and make dependency upgrades deliberate isolated PRs with a build + smoke-test checklist (Linux + Windows). Verify `gpui-component ↔ gpui-pre` compatibility on every bump.

**Warning signs:**
Divergent builds between machines; CI green but local red after an unrelated change; "it worked on last month's lockfile"; errors inside `gpui-component` macros after a bump.

**Phase to address:**
Foundation phase (workspace + pins + lockfile + CI) — must be plan-level deliverable #1, before any view code exists.

---

### Pitfall 2: Sidecar supervisor that works on Linux but orphans/zombies the Go backend

**What goes wrong:**
The spawned `wa-bot-backend` outlives a crashed GPUI app (orphan holding the session), two backends race after rapid restart ("address already in use"), or `--no-backend` external-port mode is forgotten so devs test against a stale system backend. The Electron reference (`desktop/main.js`) solved this with a concrete protocol the GPUI supervisor must replicate: spawn backend with `PORT=':0'` (ephemeral), `ALLOWED_ORIGINS='*'`, per-user `DB_PATH`/`MEDIA_PATH` under userData, `TZ='Asia/Jakarta'`, parse `BACKEND_PORT:` from stdout, deliver port to UI (`backend-ready` IPC → `__BACKEND_PORT__` equivalent).

**Why it happens:**
Child-process supervision has boring-but-critical edges: `kill_on_drop` vs explicit graceful stop, Windows taskkill semantics (no SIGTERM), health-poll timeouts vs infinite retry, stdout-parsing races when the port line arrives before the window exists.

**How to avoid:**
One supervisor code path (mirror `crates/supervisor` from the reference, adapt env/dir layout): parent picks ephemeral port, passes via flag/env; backend honors it; health-poll with hard timeout and visible startup state; `kill_on_drop` + explicit stop; single-instance adopt-or-kill on startup; test crash/restart loops explicitly. Keep the `--no-backend --port=` escape hatch for dev parity with Electron.

**Warning signs:**
"Address already in use" after crashes; multiple `wa-bot-backend` in Task Manager; sessions that "come back" after app exit; blank window for 10s+ on cold start with no progress UI.

**Phase to address:**
Foundation / backend-integration phase (supervisor crate + port-discovery contract test: spawn → parse port → health-check → kill → no orphan).

---

### Pitfall 3: WS reconnect that is "almost" like `use-websocket.ts` — and drops auth/QR/messages

**What goes wrong:**
The GPUI client connects, but after a backend restart or network blip it never re-authenticates, misses the `qr_code`/`auth_success` sequence, or double-processes `new_message` (duplicate bubbles). The web reconnect model (`web/src/hooks/use-websocket.ts`) has five behaviors that must ALL be ported: (a) re-evaluate the WS URL on *every* connect (port changes in desktop mode); (b) send `authenticate {userId}` on every `onopen`; (c) reconnect after 3s on any close with `code !== 1000`, never on clean close; (d) guard against simultaneous connects (`isConnectingRef` + OPEN check); (e) tolerate unparseable frames without tearing down the socket. On top sits `AuthContext`'s 12-type dispatch (`qr_code`, `auth_success`, `new_message`, `message_reaction`, `poll_update`, `message_deleted`, `message_edited`, `message_status`, `chat_name_update`, `group_updated`, `chat_state`, `chats_changed`) plus the `processedMsgIds` dedupe set.

**Why it happens:**
Teams port "connect + onmessage" and treat reconnect as a retry loop, missing the URL re-resolution and re-auth steps that only matter in sidecar mode. The dedupe set and the `chats_changed → invalidateMessages + refetch` path look like edge cases until a flaky network produces duplicate or stale chats.

**How to avoid:**
Implement the WS layer as a state machine in `backend-client` (not in views): `disconnected → connecting → authenticating → connected → backoff(3s, only if code != 1000)`. Re-resolve base URL from supervisor on each attempt; re-authenticate on every open; keep a seen-ID set for `new_message`; mirror the full `AuthContext` dispatch table 1:1 and add a test per message type. Also note the web keeps an *unbounded* `messages[]` array — do NOT replicate that (see Performance Traps); dispatch into the store instead.

**Warning signs:**
Login works cold but fails after backend restart; duplicate messages after network flap; QR shown once then never refreshed; `chats_changed` leaves stale sidebar until manual reload.

**Phase to address:**
Backend-client phase (WS state machine + dispatch table with contract tests per message type, incl. kill-backend-mid-session test).

---

### Pitfall 4: ChatStore parity — ordering, temp-message replace, unread preservation, pagination flags

**What goes wrong:**
Messages render out of order, optimistic sends duplicate instead of resolving, unread badges clear spontaneously, or infinite-scroll loads the same page forever. `web/src/stores/chatStore.ts` encodes six non-obvious rules: (a) `sortAsc` re-sort on `setMessages`/`appendMessages`/`upsertMessage` because backend timestamps carry ties/out-of-order rows; (b) `upsertMessage` replaces a `temp-` pending message **in place** by content/type match when the real ID arrives; (c) `upsertChat` preserves `unread` unless the update explicitly carries one, and moves the chat to top only when `lastMsg`/`lastTime` changed; (d) per-chat entry tracks `hasMore/hasMoreNext/loaded/loading/loadingMore/loadingNewer` separately; (e) `normalizeChat` coerces legacy JSON-null `pinnedAt`/`mutedUntil`/name to non-null; (f) `invalidateMessages` marks entries unloaded without dropping them.

**Why it happens:**
A Rust `Vec<Message>` + `HashMap<chat_id, Vec>` looks sufficient, so the port drops the guards that were added after real bugs (the `sortAsc` comment says exactly why). The `temp-` replace heuristic (content equality OR media-type match) is the subtlest: miss it and every send shows twice.

**How to avoid:**
Port the store semantics literally into the GPUI app-state crate: same entry struct with the same flags, same sort-on-insert, same temp-replace rule, same unread-preservation, same `normalizeChat` null-coercion at the DTO boundary. Unit-test each rule (out-of-order insert, temp→real replace, unread-preserving upsert, null legacy chat). Keep the real-id/pending-id mapping explicit rather than content-matching where possible, but keep content-matching as fallback for media sends.

**Warning signs:**
Messages jump order on history-sync import; sent message appears twice; unread count resets when a reaction arrives; scroll-up triggers repeated identical fetches.

**Phase to address:**
App-state/store phase (before chat UI) — unit tests for all six rules; chat UI phase then assumes the store is correct.

---

### Pitfall 5: QR login that polls but never pushes (or pushes but never polls)

**What goes wrong:**
QR shows once and goes stale, or the app logs in but the UI never transitions. Web has *two* redundant QR paths that must both exist in GPUI: `LoginPage` polls `GET /qr-code` every 30s (WhatsApp QR expires ~20–30s) AND `AuthContext` reacts to WS `qr_code` push → `auth_success` push (clears QR, sets logged-in). Additionally `checkLoginStatus` (`GET /status`) on startup decides the initial route. A GPUI port that implements only polling misses instant login transitions; one that implements only WS shows nothing if the socket isn't up yet.

**Why it happens:**
QR feels like a static image feature; the expiry + dual-path delivery is invisible until tested against a real phone. GPUI also lacks `qrcode.react` — teams stall on rendering (need a `qrcode` crate → GPUI `img` from generated pixels) and ship a placeholder.

**How to avoid:**
Auth phase delivers: startup `GET /status` gate → login route; QR view with 30s refresh timer + WS `qr_code` live-update + `auth_success` transition; manual refresh button; error state ("Failed to load QR code" parity). Render QR via the `qrcode` crate into an image buffer fed to GPUI's `img` element. Test with real expiry (wait >30s, confirm refresh) and real scan (confirm instant transition without waiting for next poll).

**Warning signs:**
QR works in demo (scan within 5s) but fails in real use (scan at 40s → expired); login succeeds but UI stays on QR until restart; no error state when backend is down.

**Phase to address:**
Auth & session phase (QR + persisted session + logout dialog parity).

---

### Pitfall 6: Media void — GPUI has no `<img>`, `<video>`, or `<audio>`; wa-bot is a media-heavy messenger

**What goes wrong:**
Chat parity stalls at text bubbles because images, videos, voice notes, stickers, and status media have no GPUI-native widget. Web relies on: `ChatImageViewerModal` (fullscreen viewer), `LazyMedia` (lazy loading), `WAAudioPlayer` (play/pause + 30-bar pseudo-waveform + duration), `postStatusMedia`/channel media, `viewOnce` semantics, and `getChatMedia/getChatDocs/getChatLinks` tab endpoints. The ecosystem answer (community `gpui-video` via FFmpeg+CPAL, `rodio` for audio-only) pulls heavy native deps (FFmpeg dynamic linking!) into a milestone that also targets Windows packaging — a packaging + licensing + binary-size decision disguised as a UI task.

**Why it happens:**
"Render a message list" demos show text; the message-type matrix (`image/video/document/audio/ptt/voice/gif/sticker/contact/location/poll/viewOnce/linkPreview`) is only visible in `api.ts` types + `MessageBubbles`/`ChatMessageItem`. Voice notes additionally need `ptt/seconds/waveform` metadata round-trip (see Pitfall 7).

**How to avoid:**
Decide the media strategy in the chat-media phase, not during packaging: images via `image` crate → GPUI `img` with an LRU cache + disk cache (never decode on the UI thread); audio via `rodio` (playback + position for the progress/waveform UI) on a background thread with channel commands; video: explicit go/no-go — either FFmpeg-based playback (accept the packaging cost on both OSes) or v1 fallback (thumbnail + "open externally" via OS default app). Replicate `WAAudioPlayer`'s deterministic FNV-1a pseudo-waveform bars when the server sends no `waveform`, and render the real `waveform` string when present. Honor `viewOnce` (single view, no caching to disk). Keep `LazyMedia` semantics: only fetch when scrolled into view.

**Warning signs:**
Chat looks done with text fixtures but no phase has a "play a voice note on Windows" acceptance test; FFmpeg appears as a dependency two days before packaging; images decode on the render path (scroll jank); view-once media lands in a disk cache.

**Phase to address:**
Chat-media phase (explicit video go/no-go + audio-on-both-OSes test); packaging phase verifies the native media deps actually ship and load on clean VMs.

---

### Pitfall 7: Multipart upload mismatches — `sendMedia`/`sendAudio`/group-photo/status-media field contracts

**What goes wrong:**
Uploads fail with opaque backend errors because the Rust client sends the wrong field names or omits the `secret`. The contracts are exact and inconsistent-looking: `POST /send-media` multipart needs `secret + target + message + type + file` plus optional `ptt/seconds/waveform/viewOnce` (only when provided); `sendAudio` maps to `sendMedia` with type `ptt|audio`; group photo is `POST /groups/{id}/photo` with field `file` only (no secret); status media is `POST /statuses/media` with `type + caption + file`. `reqwest` multipart in Rust also differs from browser `FormData` (needs explicit `Part::bytes` with filename + MIME).

**Why it happens:**
Browser `FormData` hides field mechanics; a Rust port "sends the file" but forgets `secret`/`target`/`type` siblings, or sends `ptt=false` explicitly where web omits it. Filenames/MIME types matter to the Go backend and to WhatsApp.

**How to avoid:**
One `multipart` builder per endpoint in `backend-client` mirroring `api.ts` field-for-field, with round-trip tests against the real Go backend (send → assert received `id`; voice note with `seconds+waveform` → assert metadata preserved). Set filename + content-type on every file part. Cover all four upload endpoints (send-media, group photo, status media, plus docs flow if distinct).

**Warning signs:**
Text sends work but every attachment 400s; voice notes arrive as generic audio (lost ptt flag); group photo upload works on web but not desktop; tests mock HTTP instead of hitting the real backend.

**Phase to address:**
Backend-client phase (upload builders + live-backend round-trip tests); chat-compose phase reuses them for attachments/voice recording.

---

### Pitfall 8: Theming 1:1 means porting a 60+-preset token pipeline, not "dark mode toggle"

**What goes wrong:**
Desktop ships with one dark + one light theme while web has 60+ presets in `data/themes.ts` (each: name/label + 16 color tokens), layered as next-themes (system/light/dark) × `AppThemeProvider` (preset → 26 CSS variables incl. sidebar aliases + luminance-based `.dark` class toggle) × `localStorage` persistence (`wa-bot-theme-preset`) with cross-tab `StorageEvent` sync. GPUI has no CSS variables — every color must be threaded as `Hsla` through every view, and "close enough" theming is instantly visible side-by-side with web.

**Why it happens:**
Theming looks like a boolean; the actual surface is a data file (107 lines, 16 tokens × 60 presets) plus a runtime pipeline (hex→CSS-var→Tailwind). Ports that hardcode a few colors can't catch up without a rewrite of every view.

**How to avoid:**
Generate, don't hand-port: script-convert `themes.ts` into Rust theme structs (exact hex values, all presets, all 16 tokens + sidebar aliases), with a test asserting preset count and token parity against the TS source. Single `Theme` global in app state (preset name + mode), hex→`Hsla` conversion once, persisted via the `settings` crate (same key semantics), hot-switch without restart. The reference `theme.rs` in `webterm` shows the shape. Settings phase must include theme persistence; every later view phase consumes tokens from the global (never hardcoded colors).

**Warning signs:**
Colors hardcoded in view code; preset list hand-typed and already out of date; "we'll add the other themes later"; dark/light toggle exists but sidebar colors wrong (missed the sidebar alias variables).

**Phase to address:**
Theming phase early (generated tokens + global + persistence) — before chat/calls UI so all views consume it; settings phase wires the picker UI.

---

### Pitfall 9: Call overlays without the `CallContext` state machine — missed incoming calls, stuck "ringing"

**What goes wrong:**
Incoming calls don't pop, or the overlay gets stuck: active call shows after hangup, group-call participants never ring, video upgrade requests vanish. `CallContext` is a six-piece state machine (`activeCall`, `incomingCall`, `callHistory`, `peerAccepted`, `videoState`, `videoUpgradeRequested`) with terminal statuses (`ended/rejected/missed/busy/failed/interrupted`), driven by REST (`/calls`, `/calls/group`, answer/reject/hangup, video start/accept/reject/stop, history+filter) *and* WS call events, with optimistic updates (outgoing call clears incoming). GPUI additionally needs always-on-top overlay windows + ringtone audio — both platform-sensitive (Wayland layer-shell limits, Windows focus-steal rules).

**Why it happens:**
Calls look like "history list + buttons" until the realtime paths (incoming while in another chat, peer accept racing with local hangup, video upgrade mid-call) are exercised. The terminal-status set is the kind of constant that gets partially copied.

**How to avoid:**
Port `CallContext` as a unit into app state: same six fields, same terminal-status set, same REST+WS event handling, same optimistic rules. Overlay = dedicated GPUI window/view with always-on-top where the platform allows; ringtone via the same audio backend as voice notes (Pitfall 6). Test matrix: incoming while chatting, answer/reject/hangup each direction, group call add+ring participant, video upgrade accept/reject, backend restart mid-call (state resync via `getActiveCall`).

**Warning signs:**
Only the history list is demoed; incoming call tested by clicking a button, not by a real WS event; no test for "hangup during ringing"; overlay is a modal inside the main window that the user can miss.

**Phase to address:**
Calls phase (state machine + history + overlays + video controls), after the WS layer (Pitfall 3) and audio backend (Pitfall 6) exist.

---

### Pitfall 10: Linux (Wayland+X11) + Windows packaging gaps discovered at the end

**What goes wrong:**
App runs on the dev's GNOME/X11 box but breaks on KDE/Wayland or Windows: missing system libs (`libxcb`, `libwayland-client`, `libxkbcommon`, Vulkan), no text rendering (wrong `gpui_platform` features — Linux needs `wayland` and/or `x11`, macOS-only `font-kit` does nothing on Windows), shader compile failure on Windows (needs VS Build Tools + Windows SDK), backend `.exe` not bundled beside resources, DB/media dirs not created per-user, drag-and-drop/file-dialog behavior untested, global hotkeys silently no-op on Wayland (protocol limitation), IME gaps for CJK input.

**Why it happens:**
Dev happens on one Linux compositor; Windows is "just a build flag" until it isn't. GPUI's platform matrix (Metal macOS / Vulkan-or-OpenGL Linux / DirectX Windows, Win32 windowing, DirectWrite text) means each OS needs its own feature set, system deps, and smoke test. Electron hid all of this (Chromium + Node); GPUI exposes it.

**How to avoid:**
Packaging is a phase with its own acceptance, not a final chore: enable both `wayland` + `x11` features (Zed docs: safe cross-platform default is both; support both because users have both); CI builds + smoke-tests on Linux (X11 and Wayland sessions) and Windows from the foundation phase onward; bundle the Go backend binary per-OS (`.exe` on Windows, same resources-relative layout as Electron); create per-user DB/media dirs on first run; verify GPU fallback on machines without Vulkan; explicitly scope Wayland-limited features (no global hotkeys — design around it) and test file dialogs + drag-and-drop + IME on each target.

**Warning signs:**
Only one Linux compositor ever tested; Windows build attempted first in the packaging phase; backend path hardcoded (breaks installed layout); "works on my machine" with system Vulkan present; shortcuts assume global hotkeys work everywhere.

**Phase to address:**
Foundation phase (CI matrix incl. Windows from day one) + dedicated packaging phase per OS (bundle layout, system deps, clean-VM smoke tests). macOS stays out of scope per PROJECT.md — don't let it creep in via "cross-platform" defaults.

---

### Pitfall 11: Parity drift — the 1159-line `api.ts` + 10 routes + message-type matrix silently shrink

**What goes wrong:**
Milestone ships at "90% parity": poll/location/contact/sticker/gif/linkPreview/viewOnce message types missing, group panels or chat-info/search sheets dropped, bot (triggers/cron/webhooks+logs), settings (DB-backed), AI assistant, documentation, status, channels, or call history forgotten. Users keep opening the web UI and the desktop fails its own "1:1" bar. The surface is large: ~70 endpoints in `ApiClient`, 10 routes (`/chat /status /channels /calls /cron /triggers /webhooks(+logs) /settings /documentation /login`), plus the `MessageExtra` union and per-chat media/docs/links tabs.

**Why it happens:**
Parity is a checklist nobody holds in their head while building greenfield views. Each dropped item looks small in isolation ("we'll add polls later") but the sum breaks the milestone promise — which PROJECT.md explicitly sets as full 1:1, not chat-first.

**How to avoid:**
Treat the Active requirements list as the acceptance matrix: map every route/endpoint/message-type to a phase in the roadmap; add DTO contract tests (Rust structs vs real backend responses, incl. `normalizeChat` legacy-null cases); run an explicit parity-audit phase before ship (the reference project needed exactly this: phase 27 `parity-audit-desktop-hardening`). Rule: no phase completes without naming the REQs/routes it covers; backend stays untouched except additive changes.

**Warning signs:**
Phases completing without route/REQ mapping; "we'll get to X later" for any message type; DTOs hand-mirrored without tests; backend "small fix" PRs that change existing response shapes.

**Phase to address:**
Every phase (mapping enforced by roadmap) + dedicated parity-audit phase at the end with a route×feature checklist executed against the live backend.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Scattering reqwest calls in views instead of a `backend-client` crate | Faster first screens | Protocol drift across 70 endpoints; untestable UI | Never — the crate is cheap and the reference proves the shape |
| Hand-mirrored DTOs without contract tests | Faster client work | Silent breakage on backend change; legacy-null crashes | Prototype only; contract tests before chat phase |
| Hardcoded backend port / `localhost:8080` everywhere | Faster setup | Port collisions; breaks sidecar + multi-instance | Never — supervisor-owned URL from day one |
| Replicating web's unbounded WS `messages[]` log in GPUI state | Easier debugging | Memory climb on busy accounts | Never — dispatch into store, keep bounded ring buffer for diagnostics |
| Hardcoded dark/light colors in views | Faster first theme | Theming rewrite of every view later | Never — tokens global from the theming phase |
| Content-matching temp-message replace skipped ("we'll use real IDs") | Simpler store | Duplicate bubbles on every optimistic send | Never — port the rule with the store |
| Video "open externally" shipped silently as final | Unblocks packaging | Parity failure vs web inline player | Acceptable only as explicit, documented v1 scope cut with a follow-up issue |
| Single-crate GPUI app (no supervisor/settings/backend-client split) | Simpler start | gpui churn + backend logic infect all code | Never for this milestone's risk profile |

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Sidecar port discovery | Assuming fixed port / reading `__BACKEND_PORT__` once at startup | Supervisor parses `BACKEND_PORT:` from backend stdout; URL re-resolved on every WS (re)connect and HTTP client rebuild |
| WS framing | Reimplementing message envelope from memory | Mirror `{type, payload}` envelope exactly; reuse backend WS handler as spec; contract-test each of the 12 dispatch types |
| Re-auth | Authenticating once per app launch | `authenticate` on every socket open (backend restart invalidates prior auth) |
| QR delivery | Only polling or only WS push | Both: 30s poll + live WS `qr_code`/`auth_success`; startup `GET /status` gate |
| Multipart uploads | Sending file part only (missing `secret/target/type/message`) | Per-endpoint builder mirroring `api.ts` field-for-field; live-backend round-trip tests |
| Voice-note metadata | Dropping `ptt/seconds/waveform` | Forward all three; render server `waveform` when present, deterministic pseudo-bars otherwise |
| Theme persistence | New settings key diverging from web | Same preset-name semantics as `wa-bot-theme-preset`; settings crate owns it; all 60+ presets generated from `themes.ts` |
| Call signaling | REST-only implementation | REST + WS events together; resync via `getActiveCall` after reconnect |
| Timezone/media paths | Backend inherits desktop env | Supervisor sets `TZ=Asia/Jakarta`, per-user `DB_PATH`/`MEDIA_PATH` like Electron |
| Backend API evolution | Editing Go responses to suit Rust | Additive-only backend changes; Rust DTOs tolerate unknown fields (`#[serde(default)]` + deny nothing) |

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Full chat-list re-render per WS event | UI stutters on busy accounts | Entity-scoped updates; only touched chat/message re-renders | Immediately with high-traffic groups |
| Image decode on UI thread | Scroll jank, frozen window | Decode on background threads; LRU image cache + disk cache; `LazyMedia` viewport-gated fetch | First chat with photo history |
| Unbounded message history in memory | Memory climb over long sessions | Paginate (`before`/`count`) + evict off-screen pages; bounded diagnostic buffer | Large groups / long-running app |
| Blocking GPUI thread on reqwest | Frozen UI during uploads/listings | All backend I/O async (tokio) from day one; uploads stream with progress | First slow network or big file |
| Pseudo-waveform recomputed per frame | Wasted CPU in every voice bubble | Memoize 30 bars per message ID (FNV-1a, same as web) | Chat full of voice notes |
| Status/channel media fetched eagerly | Slow startup, wasted bandwidth | Lazy per-viewer fetch; thumbnails first | Accounts following media-heavy channels |

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Hardcoding/shipping `VITE_API_SECRET` default (`"default-secret"`) in desktop binary | Trivially extracted secret; auth bypass if backend trusts it | Same-secret parity for local sidecar only; never log it; loopback bind enforced by supervisor; document that real deployments must rotate it |
| Logging WS payloads / message content | Chat content + QR codes in plaintext logs | Log metadata only (type, chatId, sizes); exclude payloads, QR codes, secrets via tracing filters |
| Binding sidecar backend to `0.0.0.0` | LAN-exposed WhatsApp proxy on user machines | Loopback bind enforced by supervisor flags; verify with clean-VM port scan |
| Caching `viewOnce` media to disk | View-once privacy violated; media recoverable | Memory-only for view-once; never disk cache; clear on view close |
| Passing secrets via CLI args | Visible in process listings | Env vars or config for anything sensitive; CLI only for port/dev flags |
| Persisting session tokens next to UI prefs in plaintext | Session theft via settings file | Follow web's session model (backend-owned); desktop settings hold UI prefs + theme only |

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Web shortcuts copied blindly (browser muscle memory) | Ctrl+W/Ctrl+T fight OS conventions | Map to native desktop conventions; keep web parity where conflict-free; document the delta |
| Call overlay as in-window modal | Missed incoming calls when focused elsewhere | Dedicated overlay window, always-on-top where platform allows + ringtone + taskbar/dock flash |
| Backend "starting…" with no progress | App feels hung on cold start | Health-gated startup screen with progress + failure diagnostics (port, path, logs tail) |
| No window-state persistence | Resize/reposition every launch | Persist bounds + maximized state in settings crate (reference has `window_state.rs`) |
| Emoji/sticker pickers dropped as "nice-to-have" | Daily-driver chat feels broken | They're table stakes for a messenger — schedule with chat-compose, not after ship |
| QR expiry with no affordance | User scans dead QR, blames backend | Countdown/refresh affordance + auto-refresh + manual retry (web has manual refresh path) |
| Upload without progress | Big videos look frozen | Streaming upload with progress bar; background completion even if chat scrolled away |

## "Looks Done But Isn't" Checklist

- [ ] **WS reconnect:** Often missing the *backend-restart* case (vs socket drop) — kill the child process mid-session and verify re-auth + state resync
- [ ] **QR login:** Often missing expiry — wait >30s without scanning, confirm auto-refresh; scan after expiry, confirm success
- [ ] **Optimistic send:** Often missing temp→real replace — send with slow network, confirm single bubble, not two
- [ ] **Unread badges:** Often missing preservation — receive reaction/status update, confirm unread unchanged
- [ ] **Voice notes:** Often missing metadata — send ptt, confirm duration + waveform + ptt styling on both clients
- [ ] **Uploads:** Often missing field contracts — verify all four multipart endpoints against the live backend, not mocks
- [ ] **Theming:** Often missing preset count — assert generated preset count equals `themes.ts`; toggle mid-chat; restart, confirm persistence
- [ ] **Calls:** Often missing incoming-while-chatting — trigger real WS incoming event; verify overlay + ringtone + answer/reject paths
- [ ] **Windows:** Often missing clean-VM test — install on Windows without dev tools, verify backend spawn + audio + packaging
- [ ] **Wayland+X11:** Often missing one compositor — smoke-test both sessions, incl. file dialogs and window resize
- [ ] **Parity audit:** Often missing the matrix — every route × message-type × dialog checked against web before ship

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| gpui breaking change mid-milestone | LOW–MEDIUM | Roll back to locked version; re-do upgrade as isolated PR with build+smoke checklist |
| Orphaned backend processes reported | LOW | Add supervisor handshake + stale-instance adopt-or-kill; ship patch |
| WS/auth drift found late | MEDIUM | Re-extract dispatch table from `AuthContext` + `use-websocket.ts`; add per-type contract tests; retest restart loops |
| Store bugs (dupes/order/unread) | MEDIUM | Port the six `chatStore` rules literally with unit tests; no redesign needed |
| Media strategy wrong (FFmpeg too heavy) | MEDIUM–HIGH | Cut video to thumbnail + open-externally as documented scope cut; keep audio via rodio; revisit next milestone |
| Theming hardcoded everywhere | HIGH | Generate tokens from `themes.ts`; refactor views to consume global (this is why theming goes early) |
| Parity gap found at audit | MEDIUM | Matrix makes gaps explicit; remediation phase before ship; never silently descope 1:1 |
| Windows packaging broken late | HIGH | Earliest recovery is prevention (CI matrix from day one); late fix = packaging spike + clean-VM verification round |

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| 1 — gpui churn | Foundation (pins + lockfile + CI) | Exact pins + committed lockfile; Windows+Linux CI green |
| 2 — sidecar lifecycle | Foundation / backend-integration | Spawn→port→health→kill→no-orphan test; restart-loop test |
| 3 — WS reconnect + dispatch | Backend-client | Per-type dispatch tests; kill-backend-mid-session test; re-auth verified |
| 4 — ChatStore semantics | App-state/store | Unit tests for all six rules (order, temp-replace, unread, flags, null-coercion, invalidate) |
| 5 — QR dual-path | Auth & session | Expiry test (>30s) + real-scan transition test + backend-down error state |
| 6 — media void | Chat-media (video go/no-go) | Voice-note playback on Linux+Windows; image scroll perf; view-once memory-only |
| 7 — multipart contracts | Backend-client + chat-compose | Live-backend round-trip for all four upload endpoints |
| 8 — theming pipeline | Theming (early, generated) | Preset-count parity test; mid-chat toggle; restart persistence |
| 9 — call state machine | Calls | Incoming-while-chatting + group + video-upgrade + mid-call-restart matrix |
| 10 — Wayland/X11/Windows | Foundation CI matrix + packaging | Clean-VM smoke on X11, Wayland, Windows; backend bundle layout verified |
| 11 — parity drift | Every phase (REQ mapping) + final parity-audit | Route×feature checklist executed against live backend; zero silent descopes |

## Sources

- `web/src/hooks/use-websocket.ts` — reconnect model, URL re-resolution, 3s backoff, clean-close exception (HIGH, internal source)
- `web/src/contexts/AuthContext.tsx` — 12-type WS dispatch, dedupe set, `chats_changed` refetch (HIGH, internal source)
- `web/src/stores/chatStore.ts` — sortAsc guard, temp-replace, unread preservation, pagination flags (HIGH, internal source)
- `web/src/lib/api.ts` (~1159 lines, ~70 endpoints) — multipart field contracts, message-type matrix, call endpoints (HIGH, internal source)
- `web/src/pages/login/LoginPage.tsx` — 30s QR poll + expiry (HIGH, internal source)
- `web/src/pages/chat/WAAudioPlayer.tsx` — FNV-1a deterministic 30-bar waveform (HIGH, internal source)
- `web/src/data/themes.ts` + `components/AppThemeProvider.tsx` — 60+ presets, CSS-var pipeline, luminance toggle, storage key (HIGH, internal source)
- `desktop/main.js` — sidecar spawn, ephemeral port, `BACKEND_PORT:` protocol, per-user dirs, `TZ` (HIGH, internal source)
- `../web-term/desktop-gpui` workspace (`Cargo.toml` exact pins, `crates/{supervisor,settings,backend-client,webterm}`) — proven crate split + pin discipline (HIGH, internal reference)
- `../web-term/.planning/research/PITFALLS.md` — sidecar lifecycle, churn, ConPTY-scope lesson, parity-audit precedent (MEDIUM, sibling milestone)
- Zed `crates/gpui` README + blog "Linux when?" — pre-1.0 churn warning, X11+Wayland dual support, per-OS features/text backends (MEDIUM, upstream docs)
- docs.rs `gpui` platform matrix — per-OS renderer/text/dialogs, Wayland global-hotkey limitation (MEDIUM, upstream docs)
- Community `gpui-video` (FFmpeg+CPAL) / `rodio` music-player examples — media has no first-party answer; audio-on-thread pattern (LOW, ecosystem signal only)

---
*Pitfalls research for: wa-bot desktop-gpui parity client*
*Researched: 2026-09-08*

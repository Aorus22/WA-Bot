# Feature Research: desktop-gpui Parity Port

**Domain:** WhatsApp-management desktop client (Rust GPUI port of existing React web client, reuse Go backend)
**Researched:** 2026-09-08
**Confidence:** HIGH (catalogued directly from shipped web source: `web/src/App.tsx`, `web/src/lib/api.ts` ±1159 lines, `AuthContext`, `CallContext`, `chatStore`, `use-websocket`/`ws-bus`, chat pages)

> Scope note: this is NOT greenfield feature discovery. The web client is shipped and is the 1:1 parity spec. "Table stakes" below = everything the GPUI client MUST replicate. "Differentiators" = native-desktop value-adds on top of parity. "Anti-features" = things the milestone explicitly excludes or that would break parity.

## Route Inventory (from `web/src/App.tsx` — all must exist in GPUI)

| Web route | Page component | GPUI parity requirement |
|-----------|----------------|------------------------|
| `/chat`, `/chat/:id` | `ChatPage` (sidebar + area) | Full chat parity (largest surface) |
| `/status` | `StatusPage` | Stories viewer + post text/media + mark-viewed |
| `/channels` | `ChannelsPage` | Followed list, preview/follow/unfollow, mute, post feed + reactions |
| `/calls` | `CallHistoryPage` | Filterable history + start 1:1/group calls |
| `/cron`, `/cron/new`, `/cron/:id` | `CronManagementPage` + `CronEditorPage` | List + editor + test + delete-all |
| `/triggers`, `/triggers/new`, `/triggers/:id` | `BotManagementPage` + `TriggerEditorPage` | List + editor + test + delete-all |
| `/webhooks`, `/webhooks/new`, `/webhooks/:id` | `WebhookManagementPage` + `WebhookEditorPage` | List + editor + test + delete-all |
| `/webhooks/logs` | `WebhookLogPage` | Paginated log viewer + filter by webhook + clear-all |
| `/settings` | `SettingsPage` | Theme, AI/TTS keys, read receipts, history sync, logout |
| `/documentation` | `DocumentationPage` | Render backend `/docs` markdown |
| `/login` | `LoginPage` (QR + phone tabs) | QR render + 30s refresh + auth-gated redirect |
| Global overlays | `IncomingCallOverlay`, `CallOverlay`, `Toaster`, logout dialog | Must exist outside nav (always-mounted layer) |

App shell: `AppLayout` + `NavigationSidebar` (nav rail to all routes), `ThemeProvider` (system/light/dark) + `AppThemeProvider` + `themes.ts`, `AuthProvider` + `CallProvider` always mounted, `HashRouter` → GPUI needs an equivalent view-router with deep-linkable chat id.

## Feature Landscape

### Table Stakes (parity — missing any = port feels incomplete)

#### A. Auth & session

| Feature | Why Expected | Complexity | Notes / backend dep |
|---------|--------------|------------|---------------------|
| QR login with auto-refresh (~30s) | Only way to link device; QR expires in ~20–30s | LOW | `GET /qr-code`; WS `qr_code` push also updates code live |
| Phone-link tab (LoginPage second tab) | Web offers QR + phone side by side | LOW | Verify what endpoint the phone tab calls before porting; keep tab structure regardless |
| Auth gate (`isLoggedIn null/true/false` → spinner / app / login) | Prevents flash of app before session check | LOW | `GET /status` → `{isLoggedIn}`; WS `auth_success` flips state live |
| Logout + logout confirm dialog | Session teardown, expected in settings + sidebar | LOW | `POST /logout`; clears QR state |
| WS authenticate handshake (`authenticate {userId}` on open) | Backend associates socket; without it no pushes | LOW | `use-websocket.ts` sends on `onopen`; GPUI WS client must replicate |

#### B. Chat list (sidebar)

| Feature | Why Expected | Complexity | Notes / backend dep |
|---------|--------------|------------|---------------------|
| Chat list with avatar, name, last message, time, unread badge | Core of any messenger | MEDIUM | `GET /chats`; avatar proxy `/avatar/:id` via `mediaURL()`; legacy-null normalization (`normalizeChat`) must be ported — older DBs send JSON null |
| Live in-place update on new message (no flicker, reorder to top, dedupe by msgId) | Web does surgical `upsertChat`, not refetch | MEDIUM | WS `new_message` → `chatUpdate`; `chat_name_update`, `group_updated`, `chat_state`, `chats_changed` (full refetch + `invalidateMessages`) |
| Search filter over chats | Standard | LOW | Client-side filter in `ChatSidebar` |
| Archived-mode toggle | Users hide chats | LOW | `archived` flag on Chat; `POST …/archive` |
| Context menu: pin / archive / mute (off/8h/1w/forever) | Right-click chat management | MEDIUM | `POST …/pin`, `…/archive`, `…/mute`; `patchChatState` applies `ChatState` without refetch |
| Mark-as-read on open/select | Unread badges must clear | LOW | `POST /chats/:id/read`; also fired when update arrives for selected chat |
| New-group + join-via-link dialogs | Entry to group management from sidebar | MEDIUM | `POST /groups/create`, `POST /groups/join`, `GET /groups/preview?url=` |
| History-sync status + start button (Settings surface, affects chat) | Backfill path; progress UI | MEDIUM | `GET/POST /history-sync[/status]` → `HistorySyncStatus` state machine (idle/running/completed/partial/failed); completion triggers `getChats()` refresh |

#### C. Conversation view (largest surface — ~1141-line `ChatArea.tsx`)

| Feature | Why Expected | Complexity | Notes / backend dep |
|---------|--------------|------------|---------------------|
| Paginated message load (initial 100, scroll-up prepend, scroll-down newer) | Chats are long; full fetch is infeasible | MEDIUM | `GET /chats/:id/messages?limit&before&after`; store flags `hasMore/hasMoreNext/loading/loadingMore/loadingNewer/loaded`; chronological `sortAsc` guard on store |
| Date dividers (Today/Yesterday/locale date) + time stamps | Readability baseline | LOW | Pure client formatting (`formatDate`/`formatTime`) |
| Text send with optimistic `temp-` message + replace-on-echo | Perceived speed; web replaces pending msg when real echo arrives | MEDIUM | `POST /send-message {secret,target,message}`; `upsertMessage` temp-replace logic in `chatStore` must be ported exactly or duplicates appear |
| Reply (quoted) + edit + delete (for me/everyone semantics via backend) | Baseline message actions | LOW–MEDIUM | `POST …/messages/:id/reply`, `…/edit {content}`, `…/delete`; WS `message_edited` / `message_deleted` update other clients |
| Reactions (send, remove via empty emoji, chips with sender counts) | Expected social primitive | LOW | `POST …/messages/:id/react {emoji,from}`; WS `message_reaction` patches `reactions` |
| Forward to N chats with dialog | Standard | LOW | `POST …/messages/:id/forward {targets}`; `ForwardedLabel` marker on bubbles |
| In-chat search + jump-to-message with context load | Find old messages | MEDIUM | `GET /chats/:id/search?q&limit` + `GET …/messages/:id/context?limit` (loads ~30 around hit) |
| Presence subscribe (1:1 availability) | "online/last seen" style signal | LOW | `POST /chats/:id/presence-subscribe`; display in header |
| Read receipts / status ticks via WS | Users expect ✓✓ semantics | LOW | WS `message_status` → per-message `statusUpdate` |
| Markdown render + compose markdown mode toggle | Web renders formatted content (`renderMd`) | MEDIUM | Client-side; GPUI needs a markdown/rich-text span renderer (no DOM) — budget real effort here |
| Emoji picker popover | Expected in compose | LOW–MEDIUM | Client-side; needs emoji font/glyph coverage on Linux+Windows |
| Voice-note record → send as PTT with seconds (+ waveform) + `WAAudioPlayer` playback | Signature chat feature; waveform + durations | HIGH | `sendMedia(target,file,"ptt",…{ptt,seconds,waveform})` / `sendAudio()`; recorder + player both needed; hardest native widget after video |
| Media send (image/video/document/audio/gif) + caption + view-once flag | Core attachment flow | MEDIUM | `POST /send-media` multipart (secret,target,message,type,file,+ptt/seconds/waveform/viewOnce); view-once rendering (`ViewOnceMeta`) |
| Media viewer modal (image viewer) + download-to-disk | View + save attachments | MEDIUM | `api.mediaURL()` prefix logic (`/api/…` vs absolute); `handleDownload` blob→file; GPUI uses native file dialogs |
| Link-preview cards | URLs unfurl in bubbles | LOW–MEDIUM | `LinkPreviewMeta {url,title,description,thumbnailUrl}`; render-only |
| Sticker picker + favorites + send + add/remove favorite | Sticker-first users | MEDIUM | `GET /stickers/favorites`, `POST /stickers/favorite {secret,messageId,mediaUrl,isAnimated}`, `DELETE /stickers/favorites/:id`, `POST /send-sticker {secret,target,mediaUrl,isAnimated}` |
| Poll create dialog (question + ≥2 options, ≤12, multi-select) + poll bubble with live tally + vote/retract | Group coordination staple | MEDIUM | `POST /chats/:id/poll {secret,question,options,multiSelect}`, `POST …/messages/:id/vote {options}` (empty = retract); WS `poll_update` patches `extra`; % bar rendering |
| Location share (static + live + caption) + location bubble (map pin, name/address) | Expected rich type | MEDIUM | `POST /chats/:id/location {secret,latitude,longitude,name,address,live,caption}`; GPUI has no web map embed — static thumbnail + "open in browser/maps" is the parity compromise |
| Contact-card share + contact bubble | Expected rich type | LOW | `POST /chats/:id/contact {secret,displayName,phone,vcard}` |
| GIF send (via media pipeline type=gif) | Expected lightweight media | LOW | `sendMedia(…, "gif", …)` |
| Chat info sheet (modal): media/docs/links tabs with pagination | "Shared media" browser | MEDIUM | `GET /chats/:id/media|docs|links?limit&before`; `ChatInfoSheetModal` pattern |
| Group members & settings tab (inside info sheet): roster, admin promote/demote, add/remove, name/desc/locked/announce/join-approval/memberAddMode, invite link copy + revoke, photo upload, leave | Group admin is a whole sub-app | HIGH | `GET/PATCH /groups/:id`, `POST /groups/:id/participants {action: add\|remove\|promote\|demote, jids}`, `GET /groups/:id/invite-link[?reset=true]`, `POST /groups/:id/photo` multipart, `POST /groups/:id/leave`; `ownRole` gates admin UI |
| Call buttons in chat header (voice/video, group call) | Entry to calls from conversation | LOW | `CallContext.startCall/startGroupCall`; overlay takes over (see Calls) |

#### D. Message types matrix (every `Message.type` / `MessageExtra` the bubbles render)

| Type | Render | Complexity |
|------|--------|------------|
| text (+markdown, link preview) | bubble + `renderFormattedContent` | MEDIUM |
| image / video / document / audio / ptt/voice | `LazyMedia` + audio player + download | MEDIUM–HIGH |
| sticker (static + animated) | bubble image, favorite shortcut | LOW |
| poll (`extra.poll`) | tally-bar bubble, vote toggle | MEDIUM |
| location (`extra.location`, incl. live) | pin card + thumbnail | MEDIUM |
| contact (`extra.contact`, vcard) | contact card | LOW |
| view-once (`extra.viewOnce`) | gated open-once tile | LOW |
| gif (`extra.gif`) | looping media | LOW |
| reply/quote (`replyToId`), forwarded flag | quoted block + ForwardedLabel | LOW |
| deleted / edited markers | "message deleted" / edited flag | LOW |

#### E. Calls

| Feature | Why Expected | Complexity | Notes / backend dep |
|---------|--------------|------------|---------------------|
| Call history page with filters (limit/before/direction/type/status/target) | Call log baseline | LOW | `GET /calls/history?…` → `{logs: CallLog[]}` |
| 1:1 voice/video call create → ringing → connected → hangup | Core call flow | HIGH | `POST /calls {target,type}`, `GET /calls/active`, `POST /calls/:id/{answer,reject,hangup}`; `CallState` machine (12 statuses: preparing→…→ended/rejected/missed/busy/failed/interrupted) |
| Group call create + add/ring participants | Group voice coordination | HIGH | `POST /calls/group {group_jid,participants,type}`, `POST /calls/:id/participants {targets}`, `POST …/ring?target=` |
| Incoming-call overlay + active-call overlay (global, always mounted) | Must interrupt any route | MEDIUM | WS `call.incoming`, `call.state`, `call.ended`, `call.peer_accepted`, `call.ready`, `call.group_state`, `call.participant_join`; terminal-status clearing logic in `CallContext` |
| Video upgrade flow (request/accept/reject/stop) + `VideoStage` | Video is a state negotiation, not a flag | HIGH | `POST /calls/:id/video/{start,accept,reject,stop}`; WS `call.video_state`, `call.video_upgrade_requested` patch `video_enabled/remote_video_enabled`; **native video capture/render is the single hardest GPUI item** — needs platform camera + renderer, no `<video>` tag |
| Call audio path (`useCallMedia`) | Without mic/speaker plumbing calls are silent shells | HIGH | Native audio I/O per OS; TTS/media-mode (`live\|tts\|audio_file`, `MediaMode`) rides on same path |
| History refresh on `call.ended` | Log appears right after hangup | LOW | `getHistory()` re-fetch in WS handler |

#### F. Status (stories)

| Feature | Why Expected | Complexity | Notes / backend dep |
|---------|--------------|------------|---------------------|
| Grouped-by-sender story tray with viewed/unviewed + expiry | Stories metaphor | MEDIUM | `GET /statuses` → `StatusGroup[]` (`allViewed`, `latestTime`, `expiresAt`) |
| Post text status (text + ARGB background) | Cheapest story type | LOW | `POST /statuses/text {text,background}` (default `0xff075e54`) |
| Post image/video status with caption | Media stories | MEDIUM | `POST /statuses/media` multipart (type, caption, file); playback reuse from chat media |
| Mark-viewed (sends read receipt) + on-demand media fetch | View tracking | LOW | `POST /statuses/:id/viewed`; `statusMediaURL(id)` |

#### G. Channels (newsletters)

| Feature | Why Expected | Complexity | Notes / backend dep |
|---------|--------------|------------|---------------------|
| Followed-channel list (name, desc, subscribers, avatar, mute, verified) | Subscription inbox | LOW | `GET /channels` |
| Preview via invite link → follow / unfollow / mute toggle | Discovery flow | LOW–MEDIUM | `GET /channels/preview?url=`, `POST /channels {url}`, `DELETE /channels/:jid`, `POST /channels/:jid/mute {muted}` |
| Channel post feed (paginated) + react-to-post | Read + react | MEDIUM | `GET /channels/:jid/messages?count&before` (serverId-keyed), `POST …/messages/:serverId/react {emoji,messageId}` (empty = remove); views + `reactions: Record<string,number>` display |

#### H. Bot management (triggers / cron / webhooks)

| Feature | Why Expected | Complexity | Notes / backend dep |
|---------|--------------|------------|---------------------|
| Trigger list + editor (name, regex pattern, script, priority, active, description) + test-console + delete + delete-all | Bot brain of the product | MEDIUM | `GET/POST /triggers`, `PUT/DELETE /triggers/:id`, `DELETE /triggers`, `POST /triggers/test {pattern,script,message}`; script = JS evaluated server-side |
| Cron list + editor (name, cron schedule expr, script, active) + test + delete-all | Scheduled automation | MEDIUM | Same shape under `/cron…`; `POST /cron/test {script}`; GPUI needs cron-expr input with validation hints |
| Webhook list + editor (name, path, script, secret, active) + test + delete-all | Inbound HTTP automation | MEDIUM | Same shape under `/webhooks…`; `POST /webhooks/test {path,script,method,body}` |
| Webhook logs viewer (filter by webhook, paginated limit/offset, total count, clear-all) | Debugging webhooks without logs is blind | LOW | `GET /webhooks/logs?webhook_id&limit&offset` → `{logs,total,limit,offset}`; `DELETE /webhooks/logs` |
| AI assistant sheet (chat, model picker, markdown+code render, apply-code-to-editor) | Script-writing copilot inside bot editors | MEDIUM | `POST /ai/assistant {prompt,currentCode,model}` (models: Gemma 3 27B / Gemma 4 31B / 4 26B A4B / Gemini 3.1 Flash Lite); `AIAssistant` sheet with `onApplyCode` callback into the editor page |

#### I. Settings / theming / docs

| Feature | Why Expected | Complexity | Notes / backend dep |
|---------|--------------|------------|---------------------|
| Theme mode (system/light/dark) + app-theme swatches (`themes.ts`) applied app-wide | 1:1 look requirement | MEDIUM | `next-themes`-equivalent in GPUI + `AppThemeProvider` token mapping; persisted (`wa-bot-theme*` keys); GPUI draws everything manually — every component must read tokens, no CSS cascade |
| AI settings (Gemini key, AI server URL) + TTS settings (provider, default voice, FishAudio key/model/voice) with masked "has key" flags | Powers assistant + voice features | LOW | `GET/PUT /settings` (`SettingsMap` + `hasGeminiKey/hasFishKey`); secrets never round-trip in clear |
| Read-receipts toggle | Privacy baseline | LOW | Persisted via `/settings` |
| Connection indicator (WS online/offline) | Users must see live state | LOW | `isConnected` from WS hook; Settings shows Wifi/WifiOff |
| History-sync controls + progress (state, counts, errors) | Backfill UX | MEDIUM | Same `HistorySyncStatus` as chat sidebar |
| Documentation page rendering backend markdown | In-app help | LOW | `GET /docs` → raw markdown text; needs the same markdown renderer as chat |
| Toast notification system | Every mutation reports success/error | LOW | `Toaster` (sonner) equivalent — GPUI needs a lightweight toast layer |
| Desktop window polish (rounded corners when restored, shadow) | Native feel on desktop | LOW | `desktop-ipc` / Electron parity; GPUI window controls per OS |

### Differentiators (native-desktop value-adds beyond web parity)

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Cold-start + memory footprint far below Electron | Core milestone promise ("tanpa Electron") | LOW (inherent) | Free by choosing GPUI; measure and advertise, don't build |
| Global push-to-talk / mute hotkeys, media-key handling | Desktop-native call control | MEDIUM | No web equivalent; OS hotkey APIs per platform |
| Native notifications (message + incoming call) with click-to-focus | Desktop attention loop web can't do well | MEDIUM | OS notification APIs; wire to existing WS events (`new_message`, `call.incoming`) — cheap because events exist |
| System-tray minimize + autostart + deep-link `wabot://chat/:id` | Always-on messenger behavior | MEDIUM | Tray/autostart per OS; router already supports chat-id addressing |
| Local message-search index (SQLite FTS) for instant search | Web hits `…/search` per keystroke; local index is faster + offline | HIGH | Big build; only after parity — backend search stays the fallback |
| Multi-window (pop-out chat / call window) | Power-user desktop pattern | HIGH | GPUI multi-window support TBD; defer past v1 |

### Anti-Features (explicitly NOT building)

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Rewrite Go backend in Rust | Milestone decision: backend reused as-is; rewrite explodes scope and breaks Electron/GTK/web clients sharing it | HTTP+WS `backend-client` crate only; additive API changes only if agreed |
| macOS build/packaging in v1 | Out of scope (Linux + Windows first); signing/notarization is its own project | Ship Linux + Windows; macOS next milestone |
| New features with no web equivalent (except small native integrations above) | Parity milestone judges 1:1; novel features create spec drift and double maintenance (web vs GPUI) | Log ideas to backlog; build only tray/notifications/hotkeys-class items that don't change data semantics |
| Custom E2E-encryption / key management in client | WA protocol crypto lives server-side (whatsmeow); client-side crypto would fork the trust model | Keep `secret` (`VITE_API_SECRET`) pass-through pattern exactly as web does |
| Replacing server pagination/search with full local sync | `getMessages`/`getChannelMessages`/search are server-paginated by design; eager full-sync hammers the backend and duplicates history-sync's job | Keep paginated endpoints; history-sync remains the only bulk path |
| Embedding a browser/webview to "reuse" web UI | Defeats the milestone (native, no Electron); webview = Electron costs without Electron tooling | Real GPUI views per route |
| Client-side price-schedule/cron evaluation | Cron semantics live in Go backend; duplicating evaluation causes double-fire and timezone bugs | GPUI edits + tests only; backend fires |

## Feature Dependencies

```
Backend-client crate (HTTP+WS, mediaURL, secret pass-through, autoreconnect)
    └──requires──> Supervisor sidecar (spawn Go backend + port discovery, __BACKEND_PORT__ equiv)
                        └──requires──> App shell (router + AuthProvider + CallProvider + theme + toasts + overlays)
                                            ├──requires──> Login/QR (auth gate for everything below)
                                            ├──requires──> Chat (sidebar + area + store + bubbles + dialogs) [largest]
                                            │                   ├──requires──> Group panels (needs chat info sheet)
                                            │                   ├──requires──> Media pipeline (LazyMedia/viewer/player/downloader)
                                            │                   └──enhances──> Calls (header buttons start calls)
                                            ├──requires──> Calls (history + overlays + native audio/video)
                                            ├──requires──> Status (reuses media pipeline)
                                            ├──requires──> Channels (reuses media + reaction patterns)
                                            ├──requires──> Bot editors (triggers/cron/webhooks + test consoles)
                                            │                   └──requires──> AI assistant sheet (embedded in editors)
                                            │                   └──requires──> Webhook logs (debugging for webhooks)
                                            └──requires──> Settings (theme + keys + receipts + history-sync) + Docs (markdown renderer shared with chat/AI)
```

### Dependency Notes

- **Backend-client requires supervisor:** base URL (port discovery) must exist before any API call; mirror `getApiBase()` fallback chain (`__BACKEND_PORT__` → env → origin → `localhost:8080/api`) and WS URL derivation (`ws:`/`wss:` + `/ws`).
- **Everything requires app shell:** Auth state (`isLoggedIn` tri-state), WS bus (`emitWSMessage`/`subscribeWS` fan-out), and call overlays are global singletons in web — GPUI must build these before any route view.
- **Chat requires store semantics, not just endpoints:** `chatStore` behaviors (temp-msg replace, `sortAsc`, `patchChatState`, `invalidateMessages` on `chats_changed`, unread-preserve on upsert, legacy-null `normalizeChat`) are load-bearing; port the state machine, not just the fetch calls.
- **Calls conflict with silent-build shortcuts:** shipping call UI without native audio/video produces a dead control surface — plan the native media path in the same phase as call overlays.
- **Editors enhance each other:** trigger/cron/webhook editors share one editor+test-console pattern (`AIAssistant` `onApplyCode`); building the shared editor component once serves all three.
- **Markdown renderer is shared infra:** chat bubbles, docs page, and AI assistant answers all need it — build once, use three times.

## API Surface Catalog (what `backend-client` must cover — ~70 methods)

Backend base is identical for all: `GET /status`, session via `VITE_API_SECRET`-style secret on send endpoints. Multipart only for: `send-media`, `statuses/media`, `groups/:id/photo`.

| Group | Endpoints (web `ApiClient` method → HTTP) |
|-------|-------------------------------------------|
| Session | `getStatus`→`GET /status`, `getQrCode`→`GET /qr-code`, `logout`→`POST /logout` |
| Chats | `getChats`→`GET /chats`, `markAsRead`→`POST /chats/:id/read`, `pinChat`→`POST …/pin`, `archiveChat`→`POST …/archive`, `muteChat`→`POST …/mute`, `getHistorySyncStatus`→`GET /history-sync/status`, `startHistorySync`→`POST /history-sync` |
| Messages | `getMessages`→`GET /chats/:id/messages`, `searchMessages`→`GET …/search`, `getMessageContext`→`GET …/messages/:mid/context`, `sendMessage`→`POST /send-message`, `sendMedia`/`sendAudio`→`POST /send-media` (multipart), `deleteMessage`→`POST …/delete`, `editMessage`→`POST …/edit`, `replyMessage`→`POST …/reply`, `reactToMessage`→`POST …/react`, `forwardMessage`→`POST …/forward`, `subscribeChatPresence`→`POST …/presence-subscribe`, `getChatMedia/Docs/Links`→`GET …/media\|docs\|links` |
| Rich types | `sendPoll`→`POST …/poll`, `sendPollVote`→`POST …/vote`, `sendLocation`→`POST …/location`, `sendContact`→`POST …/contact`, `sendSticker`→`POST /send-sticker`, `getFavorites`→`GET /stickers/favorites`, `favoriteSticker`→`POST /stickers/favorite`, `deleteFavorite`→`DELETE /stickers/favorites/:id` |
| Groups | `getGroup`→`GET /groups/:id`, `updateGroup`→`PATCH /groups/:id`, `updateGroupParticipants`→`POST …/participants`, `getGroupInviteLink`→`GET …/invite-link[?reset]`, `previewGroupLink`→`GET /groups/preview?url=`, `joinGroupWithLink`→`POST /groups/join`, `createGroup`→`POST /groups/create`, `leaveGroup`→`POST …/leave`, `setGroupPhoto`→`POST …/photo` (multipart) |
| Status | `listStatuses`→`GET /statuses`, `postStatusText`→`POST /statuses/text`, `postStatusMedia`→`POST /statuses/media` (multipart), `markStatusViewed`→`POST /statuses/:id/viewed`, `statusMediaURL`→`GET /statuses/:id/media` |
| Channels | `listChannels`→`GET /channels`, `previewChannel`→`GET /channels/preview?url=`, `followChannel`→`POST /channels`, `unfollowChannel`→`DELETE /channels/:jid`, `setChannelMute`→`POST …/mute`, `getChannelMessages`→`GET …/messages`, `reactChannelMessage`→`POST …/react` |
| Contacts | `getContacts`→`GET /contacts` |
| Calls | `getActiveCall`→`GET /calls/active`, `createCall`→`POST /calls`, `createGroupCall`→`POST /calls/group`, `addCallParticipants`→`POST …/participants`, `ringCallParticipant`→`POST …/ring?target=`, `answerCall/rejectCall/hangupCall`→`POST …/{answer,reject,hangup}`, `getCallHistory`→`GET /calls/history?…`, `startVideo/acceptVideo/rejectVideo/stopVideo`→`POST …/video/{start,accept,reject,stop}` |
| Bots | `getTriggers/createTrigger/updateTrigger/deleteTrigger/deleteAllTriggers/testTrigger`, same ×6 for cron (`/cron…`), same ×6 for webhooks (`/webhooks…`), `getWebhookLogs`→`GET /webhooks/logs?…`, `deleteAllWebhookLogs`→`DELETE /webhooks/logs`, `chatAssistant`→`POST /ai/assistant`, `getDocs`→`GET /docs` (plain text markdown) |
| Settings | `getSettings`→`GET /settings` (envelope `{settings,hasGeminiKey,hasFishKey}` flattened), `updateSettings`→`PUT /settings` |
| WS events in | `qr_code`, `auth_success`, `new_message`, `message_reaction`, `poll_update`, `message_deleted`, `message_edited`, `message_status`, `chat_name_update`, `group_updated`, `chat_state`, `chats_changed`, `call.incoming`, `call.state`, `call.ended`, `call.peer_accepted`, `call.ready`, `call.video_state`, `call.video_upgrade_requested`, `call.group_state`, `call.participant_join` (+ `authenticate` outbound on connect; 3s autoreconnect on abnormal close) |

## MVP Definition (parity milestone = full 1:1, ordered for roadmap)

> The milestone demands full 1:1 at once, so "MVP" here means build order, not scope cuts. Order follows the dependency tree.

### Launch With (v1 — in dependency order)

- [ ] Backend-client + supervisor + app shell + theme + toasts + WS bus — nothing renders without these
- [ ] Login/QR + auth gate — unlocks every other route
- [ ] Chat (sidebar + conversation + all message types + search + info sheet + group panels) — the product core; biggest phase, split internally by (list → send/receive → actions → rich types → group admin)
- [ ] Calls (history + overlays + native audio; video negotiated) — paired with chat header buttons
- [ ] Status + Channels — reuse media/reaction/media-viewer infra from chat
- [ ] Bot editors (triggers/cron/webhooks + test consoles + AI sheet + webhook logs) — shared editor component
- [ ] Settings + Documentation — theme/keys/receipts/history-sync + markdown render
- [ ] Linux + Windows packaging with bundled backend

### Add After Validation (v1.x)

- [ ] Native notifications + tray + autostart — needs user validation on desired intrusiveness
- [ ] Global hotkeys (push-to-talk/mute) — validate with call-heavy users first
- [ ] Local FTS search index — trigger: server search latency complaints or offline demand

### Future Consideration (v2+)

- [ ] macOS build — explicitly deferred by milestone
- [ ] Multi-window pop-outs — depends on GPUI multi-window maturity; revisit after v1 stabilizes
- [ ] Any web-divergent feature — only after parity is accepted; each needs its own web-port plan to avoid fork

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Backend-client + supervisor + shell + theme | HIGH (enabler) | HIGH | P1 |
| Login/QR + auth gate | HIGH | LOW | P1 |
| Chat list + realtime updates | HIGH | MEDIUM | P1 |
| Send/receive + optimistic UI + receipts | HIGH | MEDIUM | P1 |
| Message actions (reply/edit/delete/react/forward) | HIGH | LOW | P1 |
| Media pipeline (send/view/play/download) + voice notes | HIGH | HIGH | P1 |
| In-chat search + context jump | MEDIUM | MEDIUM | P1 |
| Group admin panels | HIGH (admins) | HIGH | P1 |
| Calls + overlays + native audio/video | HIGH | HIGH | P1 |
| Status | MEDIUM | MEDIUM | P1 |
| Channels | MEDIUM | MEDIUM | P1 |
| Bot editors + test consoles + AI sheet | HIGH (bot users) | MEDIUM | P1 |
| Webhook logs | MEDIUM | LOW | P1 |
| Settings + docs | MEDIUM | LOW–MEDIUM | P1 |
| Linux + Windows packaging | HIGH | MEDIUM | P1 |
| Native notifications/tray | MEDIUM | MEDIUM | P2 |
| Hotkeys | LOW–MEDIUM | MEDIUM | P2 |
| Local FTS index | MEDIUM | HIGH | P3 |
| macOS | MEDIUM | HIGH | P3 |

## Competitor Feature Analysis (parity references, not market competitors)

| Feature | Web client (spec) | GTK client (`desktop-gtk`) | GPUI approach |
|---------|-------------------|---------------------------|---------------|
| Chat core | Full (bubbles, media, voice, reactions, polls) | Core only (chat, search, history sync) | Match web, not GTK — GTK is the floor, web is the ceiling |
| Calls | History + overlays + video controls | Call management present | Full port incl. video negotiation; native AV is new work in both |
| Bot/cron/webhook/settings | Full editors + logs + AI sheet | Absent (per PROJECT.md scope) | Must build — biggest gap vs GTK, reuse shared editor component |
| Theming | `ThemeProvider` + `AppThemeProvider` + `themes.ts` | Own theme work | Token-map 1:1 from `themes.ts`; manual application per view |
| Backend reuse | Direct HTTP+WS | Go-native calls | Rust HTTP+WS client; same endpoints, same WS event set |

## Sources

- `web/src/App.tsx` — route inventory (HIGH: read directly)
- `web/src/lib/api.ts` (±1159 lines, ~70 methods + all shared types) — API surface catalog (HIGH: read directly)
- `web/src/hooks/use-websocket.ts` + `web/src/lib/ws-bus.ts` — WS contract: `/ws`, authenticate handshake, 3s reconnect (HIGH: read directly)
- `web/src/contexts/AuthContext.tsx` — 12 inbound WS event handlers + login/logout flow (HIGH: read directly)
- `web/src/contexts/CallContext.tsx` — 9 `call.*` WS handlers + terminal-status clearing + video-state patching (HIGH: read directly)
- `web/src/stores/chatStore.ts` — message/chat state machine: temp-replace, sortAsc, patch/invalidate semantics (HIGH: read directly)
- `web/src/pages/chat/` (`ChatArea` 1141 lines, `ChatSidebar`, `ChatMessageItem`, `MessageBubbles`, `ChatComposeDialogs`, `GroupPanels`, `ChatInfoSheetModal`, `ChatSearchSheet`, sticker/emoji pickers, image viewer, `WAAudioPlayer`) — component inventory (HIGH: sampled directly, full line-level port still needs phase-level reads)
- `web/src/pages/{settings,login,status,channels,calls,bot,documentation}/` + `components/AIAssistant.tsx`, `components/call/*`, `data/themes.ts` — remaining surfaces (MEDIUM-HIGH: sampled headers + full api coverage; LoginPage phone-tab endpoint + Settings deeper sections need confirmation during planning)

---
*Feature research for: desktop-gpui parity port of wa-bot web client*
*Researched: 2026-09-08*

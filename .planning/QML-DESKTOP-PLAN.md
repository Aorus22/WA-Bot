# WA-Bot Desktop QML — Implementation Plan

> Status: IN PROGRESS. A checked item means its implementation exists in the current tree; open items below are specific evidence gates, remaining source-parity gaps, or real-device work. This document alone makes no code changes.
> Target: add **third frontend** Qt Quick/QML alongside existing GPUI and React, with visual and functional parity against current desktop-gpui; preserve Electron and other legacy frontends.
> Workspace: /home/aorus/Project/WA-Bot

## Hard constraints
- Never replace, delete, or regress `web/`, `desktop-gpui/`, `desktop/` or Go backend. Three clients (React, GPUI, QML) must independently connect to the **same** backend, including concurrent usage.
- Create new `desktop-qml/`; do not import GPUI code into QML or embed React through Qt WebEngine. Native Qt Quick rendering.
- **No native compilation on QML/Python UI edits**. Prefer Qt 6 QML + PySide6 thin Python bridge; QtMultimedia, QtWebSockets and other installed Qt modules where practical. No Rust/C++ helper until a measured feature gap warrants it. Document third-party prebuilt native dependencies.
- Exact layout, navigation, interaction, copy, states, theme tokens, keyboard behavior and responsive sizes should follow current **GPUI implementation**, not redesign. Reuse legally compatible assets/fonts/icons as files. Validate against screenshots at matching window sizes, theme, density and DPI.
- Port only behavior exposed by the current GPUI UI. Do not add controls or operations just because the Go API supports them; do not change existing behavior during the port.
- Use existing Go REST, JSON WS, and binary call-media WS protocols; do not fork business logic. Endpoint and protocol schemas must be checked against current source, not invented.
- Do not touch existing dirty worktree modifications. In particular, local call/media code and `docs/CALL-MEDIA-PROTOCOL.md` currently have uncommitted changes. Re-inspect actual latest worktree before implementing media.
- Never hardcode backend port or enumerate speculative ports. Match GPUI port discovery and explicit CLI/environment overrides; validate `core.json` instance/PID/port state before joining. Clearly define single ownership and shutdown semantics for shared backend.
- Linux/Wayland (GNOME and COSMIC) first; specify X11/Windows capability separately; do not claim universal tray/layer-shell compatibility without tests.

## Working layout (proposed; adjust to existing repo conventions)
```
desktop-qml/
  README.md
  pyproject.toml
  main.py
  qml/
    Main.qml
    components/     # visual atoms, chat components, overlays
    pages/          # routes and feature screens
    themes/         # tokens and 81 theme definitions where parity applies
    assets/         # icons, fonts, imagery
  bridge/
    backend.py      # discovery, owner/attach, lifecycle
    api.py          # typed REST + multipart
    events.py       # JSON WS, auth, reconnect, UI signals
    stores.py       # model state and optimistic updates
    platform.py     # clipboard, dialogs, notifications, tray
    media.py        # QtMultimedia/codec adapters
    call.py         # binary WS + audio/video transport
  tests/
  scripts/
```
Keep business logic in Go. Make QML declarative and the Python bridge small, testable, and asynchronous; no blocking HTTP or filesystem activity on UI thread.

## Phase 0 — Inventory & contracts
- [x] Inventory the current GPUI routes, components, theme/settings, chat, calls, popover, tray, notifications, and Go API/WS behavior used by the QML port. React remains unchanged and is included in the concurrent-client smoke record.
- [x] Build the route/area traceability matrix in `desktop-qml/PARITY.md` and add per-area notes under `desktop-qml/parity/`; remaining visual/device evidence is called out below.
- [ ] Capture matched-baseline screenshots for login, main chat, media, settings, bot tools, calls, status, channels, compact/resized layout, every important overlay. Record pixels, padding, fonts, icon source, focus/hover/disabled states, and selected theme.
- [x] Read and document the backend discovery and call-media contracts in `desktop-qml/README.md`, `docs/QML-DESKTOP.md`, and `desktop-qml/PARITY.md`; host/device risks remain explicit.
- [x] Track implementation blockers in the parity matrix and add focused mock/unit coverage under `desktop-qml/tests/`; test suites are not rerun for every visual porting edit.

## Phase 1 — Runtime & backend integration
- [x] Provide the PySide6/Qt6 runtime, `python main.py` development entry point, and setup instructions in `desktop-qml/README.md`; QML/Python edits require no native rebuild.
- [ ] Add a fully pinned Python dependency lockfile; current setup uses lower-bounded `pyproject.toml` dependencies.
- [x] Build the standard Qt `ApplicationWindow`, custom titlebar, and window controls; native window-manager behavior remains a host verification item.
- [x] Implement validated backend attach/spawn, dynamic-port handshake, owner-only shutdown, and WebSocket-triggered re-resolution in `bridge/discovery.py`, `bridge/backend.py`, and `bridge/events.py`.
- [x] Match GPUI CLI/environment/core.json precedence and local-instance ownership; OS-specific host behavior remains open below.
- [x] Implement asynchronous REST, multipart uploads, auth state, typed DTO mapping, JSON WebSocket reconnect/event sync, and optimistic state in `desktop-qml/bridge/`.
- [x] Add mock backend fixtures, smoke startup test, and 3-client concurrent-connect test. `desktop-qml/scripts/concurrent-smoke.py` kept React/Firefox, GPUI and QML alive against one isolated mock; the latest run observed 3 WebSocket clients, 3 status requests and 5 chat-list requests.

## Phase 2 — Shell & pixel-level foundation
- [x] Implement the GPUI theme presets, theme selection, font sizes, transparency, and persistence through `bridge/theme.py` and `qml/themes/themes_data.json`.
- [x] Implement reusable typography, icons, buttons, inputs, menus, popovers, dialogs, toasts, and loading/empty/error states. IME behavior and full screen-reader coverage remain unverified.
- [x] Implement sidebar, titlebar, route navigation, chat list, avatars, unread counters, status indication, searchable lists, and virtualized message/chat views.
- [ ] Match GPUI baseline screenshots in light/dark and small/large windows. Avoid introducing a new design language.
- [x] Re-align Calls history filter chips/empty state and selected-call detail against the 1100×720 GPUI baseline; refreshed captures and a QML mouse-click filter test pass, with minor chip-width differences recorded in `desktop-qml/parity/calls.md`.

## Phase 3 — Auth & chat parity
- [x] Match GPUI login behavior: QR refresh, logout, and WS connection/reconnection banners. Phone-code pairing remains a source placeholder (“coming soon”) because GPUI exposes no phone-pairing flow.
- [x] Implement chat history, pagination, search, chat selection, presence, read/unread, pinned/starred state, and chat actions in `bridge/chats.py`, `bridge/messages.py`, and the chat QML pages.
- [x] Match message-search behavior from GPUI: merge decoded local history with deduplicated server results, show the initial/loading/empty states and sender/date/time rows, focus on open, close with Escape, and jump to the selected message. The QML initial-state capture is refreshed; matched GPUI search captures remain open.
- [x] Implement the GPUI composer flows: multiline text, Markdown mode, emoji/sticker pickers, attachment pickers, image clipboard paste, send shortcuts, and poll/location/contact forms. Native picker and IME behavior remain verification items.
- [x] Match the GPUI emoji/sticker pickers: category tabs and mouse selection, with the shared Emoji/Sticker switch. GPUI exposes no picker search or keyboard grid navigation, so QML omits those extra behaviors.
- [x] Implement grouped messages, replies, edits, deletes, reactions, links/previews, disappearing messages, delivery/read indicators, context menus, selection/copy/forward; remaining visual comparisons and historical-media edge cases are listed in Phase 4.
- [ ] REST/WS contract tests for optimistic sends, rollback, reconnect, duplicate/out-of-order events.
- [x] Implement and QTest PageUp/PageDown/Home/End conversation scrolling with composer focus, message-list focus, and search-field focus; `tests/test_qml_chat_keys.py` also verifies Home/End stay on the caret in a nonempty draft. IME composition and other text-field focus parity remain open.
- [x] Add accessible chat-row buttons with Space activation and ArrowUp/ArrowDown focus movement; `tests/test_qml_accessibility.py` verifies the name, chat opening, and both focus directions. Full application-wide traversal remains open.
- [x] Verify sidebar Tab order from search to Archived to the first chat row with `tests/test_qml_accessibility.py`; traversal outside the sidebar remains open.
- [x] Make the Archived sidebar row accessible by name and Space; `tests/test_qml_accessibility.py` verifies that it opens the archived-chat view.
- [x] Connect `Messages.revealRequested` to the conversation list and briefly highlight the target bubble; `tests/test_qml_message_navigation.py` verifies a jump to an older message makes it visible.
- [x] Preserve pending and REST-acked optimistic rows across stale same-chat history refreshes until the server snapshot contains their ID; `tests/test_messages_runtime.py` verifies `ViewOnceMeta` DTO data survives pending state, ID rekey and authoritative refresh.
- [x] Verify plain Enter/Ctrl+Enter send, Shift+Enter newline, and Markdown Enter/Shift+Enter newline plus Ctrl+Enter send with actual QML key events against the mock using `tests/test_qml_composer_keys.py`; IME and caret parity remain open.
- [x] Verify selected-message Copy via Space (clipboard content and selection-mode exit) and Delete via two-Space confirmation (mock POST, revoked state, and mode exit) in `tests/test_qml_message_selection.py`; reactions and remaining context actions are still open.
- [x] Make the Forward dialog keyboard accessible and verify search focus, Tab to the target list, Space selection, and Tab/Space submission to the REST mock with `tests/test_qml_message_forward.py`; multi-message forward selection and other message actions remain open.
- [x] Enable accessible message selection with Space and verify two selected messages forward in chronological order through the selection bar and QML target dialog in `tests/test_qml_message_forward.py`.
- [x] Match message context-menu actions to GPUI: Reply/React/Message info/Select/Forward/Pin/Star/Copy/Save/Favorite/Edit/Delete, with GPUI's direct Delete action; expiry and other message-rendering parity remain open.
- [x] Add GPUI's outgoing-message Message info action and receipt drawer; `tests/test_qml_message_actions.py` verifies group read/delivered/remaining lists and direct read/delivered stages through the existing receipts route.
- [x] Keep the `ViewOnceMeta` DTO field while matching GPUI media behavior: images/stickers use the normal image viewer; video messages do not get a QML playback card because GPUI does not expose one.
- [x] Match GPUI's attachment menu and picker semantics: Emoji, Sticker, Media, Document, Audio, GIF, Poll, Location, Contact and Markdown, with one selected file per picker action. The separate view-once send option was omitted because the current GPUI composer does not expose it.
- [x] Verify reaction chips are focusable, named, and Space-activatable with an optimistic model update and mock POST in `tests/test_qml_reaction_chip.py`.
- [x] Match GPUI clipboard paste: stage clipboard images for captioned sends and let text paste fall through to the editor. File URLs and OS file drops are not exposed by GPUI and are omitted.

## Phase 4 — Rich media, groups & history
- [x] Port the media UI currently exposed by GPUI: image/sticker thumbnails and viewer, document card and Save action, plus poll/location/contact cards. QML does not add video/audio playback cards or retry actions absent from GPUI.
- [x] Verify an image thumbnail mouse click opens the media viewer and requests the fixture image through `tests/test_qml_media_viewer.py`; older-message scroll walkthrough and authenticated/expired-media states remain open.
- [x] Verify the QML document Save button downloads the mock PDF to the selected temporary path and returns success through `tests/test_qml_media_viewer.py`; native desktop picker behavior remains open.
- [x] Match GPUI image-viewer controls: fit/zoom/pan, wheel and double-click zoom, image-only gallery navigation, Save, image context menu and Forward when opened from a message.
- [x] Use GPUI's loading/failure display in the image viewer without adding a Retry action.
- [x] Implement voice-note recording/send and canonical WAV upload in `bridge/composer_voice.py` and `bridge/composer_upload.py`; real device capture/playback stays open below.
- [x] Verify the QML record button reports an unavailable microphone, returns to idle, and sends no attachment with `tests/test_qml_voice.py`; real capture/playback remains open.
- [x] Unit-test fragmented microphone PCM, stereo downmix, 48 kHz to 16 kHz resampling, and canonical mono 16-bit WAV output without an audio device in `tests/test_composer_voice.py`.
- [x] Click the QML record/send controls with synthetic captured PCM and verify `ptt=true`, seconds, filename, MIME type, and WAV format through the mock backend in `tests/test_qml_voice.py`.
- [ ] Verify real microphone permissions/capture, backend Opus transcoding, and playback through target-system audio devices.
- [x] Keep the group Info page aligned with GPUI: description, Add action, read-only roster with admin badges, and two-click Exit group. QML-only settings, invite-link, edit/photo, and promote/demote/remove controls have been removed; source parity and the X11 capture are in place.
- [x] Implement the Add member contact picker with eligible-chat filtering, exclusion of archived/group/already-added members, and multi-select. Matched GPUI visual and failure-state comparison remains open.
- [x] Port the GPUI poll/contact/location cards and group drawer actions; the direct/group X11 captures show the right drawer reflow and outgoing message bubble. Real group membership changes and roster scrolling remain open below.
- [x] Bring GPUI Info quick actions, media summary, notification-duration choices, disappearing-message timer, starred entry and encryption note into the QML info drawer; click-test every mute and disappearing duration against the REST mock in `tests/test_qml_chat_info.py`.
- [x] Keep Add member in the GPUI quick-action row; exact contact-picker parity remains open as recorded above.
- [x] Refresh QML direct/group Info captures at 1100×720 after aligning the controls; the group description is suppressed in the comparison fixture to match the GPUI baseline.
- [x] Match GPUI drawer layout: Info fills the window in `pages` mode and uses a 320 px right column otherwise; Search and Message Info use the right column, while Starred fills `pages` mode when no chat is selected. Keep panels opaque and animate side-drawer reflow.
- [x] Compare the refreshed X11/llvmpipe QML captures to GPUI and record remaining vertical spacing and group-message scroll/timestamp differences; avatar crops and solid drawer fills now render.
- [x] Match GPUI's Info → Media, Docs and Links subpage flow, dynamic header title and Back action; `tests/test_qml_chat_info.py` opens each page and returns via the header.
- [x] Open and save a historical image from the Info media strip even when its message is outside the loaded page; `tests/test_qml_media_viewer.py` verifies the QML thumbnail click.
- [x] Implement Info Media/Docs/Links: three-column 88 px image grid, standalone image preview, external video/GIF opening, dated document rows, and one row per URL. Matched GPUI subpage captures and final visual comparison remain.
- [ ] Walk through historical-media open/save failure states and verify full member-roster scroll and live group updates on an account with real group data.
- [x] Verify a poll option mouse click updates the card optimistically and posts the vote through the existing REST contract with `tests/test_qml_rich_cards.py`.
- [x] Verify the contact card Message button opens the selected recipient chat through QML with `tests/test_qml_rich_cards.py`.
- [x] Verify the location card Open in Maps button builds the expected coordinate URL through a captured opener (no browser launch) using `tests/test_qml_rich_cards.py`.
- [ ] Confirm historical image loading/failure behavior and bounded memory on target systems; GPUI does not expose media Retry controls.
- [x] Keep clipboard paste parity to image staging and normal text insertion; file URL paste and DropArea file staging are omitted because GPUI does not expose them.
- [x] Verify a selected file from the QML FileDialog reaches multipart upload with the right target, type, name, and bytes through `tests/test_qml_file_dialog.py`; native GTK/portal picker behavior remains unverified.
- [ ] Validate native GTK/portal file dialogs and permissions on supported Linux desktops.

## Phase 5 — Calls, status, channels
- [x] Integrate the QML media client with the call-media contract owned by branch `call`: request the single-use token from `POST /api/calls/{id}/media-session`, then exchange PCM and H.264 Annex-B access units over the binary WebSocket. The Go endpoint remains in branch `call`.
- [x] Implement call history, incoming/ringing controls, accept/decline, active overlay, mic/speaker toggles, group participants, and video start/stop. Live two-device call behavior remains open below.
- [x] Drive incoming CallOverlay Answer/Decline controls with QtTest against a temporary call fixture; assert REST action and server WebSocket state updates.
- [x] Drive the active CallOverlay End control with QtTest and verify the hangup action clears the mock call over JSON WebSocket.
- [x] Drive the group-call Add participant control with QtTest and verify the participant list updates through the shared REST/JSON WebSocket contract.
- [x] Verify active-call Video/Stop video controls send the existing video start/stop REST actions and follow the JSON WebSocket state using media-free mock calls.
- [x] Verify active-call Mute/Speaker controls toggle their QML state with a mock TTS call and no device session.
- [x] Verify the mock reuses an unconsumed token, then issues a fresh single-use token after the first WebSocket consumes it; two clients exchange PCM plus H.264 Annex-B frames bidirectionally with `tests/test_call_media_video.py`. Real Go call/device path remains open.
- [x] Post a text status through the QML New status dialog with mouse clicks and verify the own-status list updates from the mock REST response.
- [x] Match GPUI's text-only status composer, background swatches, reply behavior and viewer navigation; media-post controls are omitted because the current GPUI view does not expose them.
- [ ] Run latency, A/V sync, reconnect, device-switch, permissions and long-call tests. Feature not counted complete until real device tests pass.
- [x] Port the current GPUI Status and Channels screens: story list, image/text viewer, text-only status composer/replies, channel directory/feed and reactions. Status media posting, client-side expiry pruning and auto-advance are not exposed by GPUI and are omitted.
- [x] Cover the audio wire framing, jitter buffer, PCM codecs, and non-native 48 kHz stereo capture/playback conversion with deterministic tests; device permission and latency evidence remains a separate acceptance item.
- [x] Verify the Status story-row mouse path opens the QML viewer against the local mock using `tests/test_qml_status.py`.
- [x] Verify that opening a QML story also posts the viewed receipt for its first entry through the mock REST route.
- [x] Verify a status viewer reply reaches `/statuses/{id}/reply` and the QML viewer exposes the returned chat action with `tests/test_qml_status.py`.
- [x] Verify the Channels row mouse path selects the channel and loads its feed against the local mock using `tests/test_qml_channels.py`.
- [x] Verify the channel detail mute toggle with a real QML mouse click and the shared REST contract using `tests/test_qml_channels.py`.
- [x] Verify a QML quick-reaction click updates the post optimistically and reaches the shared REST endpoint using `tests/test_qml_channels.py`.
- [x] Verify channel discovery preview and Follow controls with QML mouse clicks against the shared REST endpoints using `tests/test_qml_channels.py`.

## Phase 6 — Bot, administration, settings & desktop integration
- [x] Mirror the current GPUI centered placeholders for Triggers, Cron Jobs, Webhooks, and Documentation. Do not expose the React-only admin CRUD, logs, or AI assistant flows in the QML port.
- [x] Send Freedesktop notifications over the session D-Bus with the wa-bot-qml desktop-entry identity; tests cover notification enablement, mute/read/age/archive filters, cross-client claim receipts, and the desktop-entry hint. A real notification daemon/permission check remains open.
- [x] Implement optional QSystemTrayIcon, close-to-tray when a host exists, per-user single-instance show/toggle, and owner-only backend shutdown. QML close/show/toggle, private watcher registration, and backend-owner tests pass; real tray-host behavior remains open.
- [x] Verify fragmented local-instance commands and open/toggle delivery to the Platform signals with tests/test_platform.py; actual window-manager activation and tray-host availability remain open.
- [x] Verify Qt system-tray registration and activation-to-show routing using a private StatusNotifierWatcher under Xvfb with tests/test_platform.py; GNOME/COSMIC host integration remains open.
- [x] Verify Main.qml intercepts close, hides the test window, and restores it on show/toggle signals with tests/test_platform.py.
- [x] Port GPUI resident `--popover`/`--toggle` using the optional KDE LayerShellQt QPA plugin and QML module when present: panel-edge anchors, pointer cache, 120 ms fallback placement, outside-click dismissal, and normal-window fallback when unavailable.
- [ ] Verify the LayerShellQt path and fallback on actual supported/unsupported desktops; GNOME extension integration is not claimed.
- [ ] Complete the cross-route accessibility audit for focus order, keyboard navigation, contrast, screen-reader output, and DPI/font scaling; targeted controls already have accessible names and keyboard activation.
- [x] Give custom `IconButton` and `Btn` controls accessible names and Enter/Space activation; `tests/test_qml_accessibility.py` checks both the status New and Post status actions. The wider focus/screen-reader/contrast audit remains open.
- [x] Give chat-row, image open, poll-option, and link-preview actions accessible names and Enter/Space activation with visible focus; `test_qml_media_viewer.py` and `test_qml_rich_cards.py` verify the keyboard paths. The wider screen-reader/contrast/DPI audit remains open.

## Phase 7 — Validation, performance & packaging
- [x] Create the route/area traceability matrix and identify GPUI features QML intentionally omits. Matched screenshots and interaction walkthroughs for every route remain open below.
- [ ] Compare at least cold start, warm start, idle CPU, PSS/RSS, scrolling FPS, image-heavy history, memory after extended idle; compare GPUI and QML under same backend and conditions. No speculative claims of lower RAM.
- [ ] Regression tests: React and GPUI unchanged; shared backend with simultaneous connections, owner exit, reconnect, error handling.
- [x] Implement separate Linux PyInstaller onedir and QML AppImage commands, plus the current-host onedir install/uninstall flow. Clean-host/cross-distro validation and a Windows artifact remain open.
- [x] Document run/develop/debug/install/uninstall, media system dependencies, platform scope, and known gaps in `desktop-qml/README.md`, `docs/QML-DESKTOP.md`, and `desktop-qml/THIRD_PARTY_NOTICES.md`.
- [ ] Obtain screenshots and test logs from actual runtime; report tested vs inferred honestly.
- [x] Rebuild the 642 MB Linux onedir bundle after the latest QML work; isolated frozen-app smoke passed H.264 encode/decode, QML load, mock auth/chat REST, and JSON WebSocket. Clean-host behavior remains open.

## Definition of Done
- [x] React, GPUI and QML launch independently against the same Go backend; `scripts/concurrent-go-smoke.py` records three WebSocket clients, status requests, and chat-list requests while all three processes stay alive.
- [ ] GPUI visual parity approved across representative screens at equal size, DPI and themes; deviation list documented.
- [ ] GPUI feature-parity matrix fully checked with tests, or remaining exceptions explicitly documented and approved.
- [ ] Chat, media, real-time events, calls and background lifecycle tested end-to-end on Linux, not just unit mocked.
- [x] QML visual change requires no `cargo build`, `cmake` or C++ rebuild; the latest UI work was verified through QML captures and the PyInstaller bundle smoke.
- [ ] No regressions, no backend ownership conflicts, no silent protocol assumptions.

## Execution discipline
Do phases in order; finish each with proof (changed files, test commands/results, before/after screenshots and checked boxes). Favor small modular QML files and thin, isolated bridges. Avoid modifying Go contracts unless unavoidable and explicitly justified. Leave existing uncommitted work untouched. Do not mark tasks done solely for creating placeholder UI.

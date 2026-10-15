# desktop-qml conventions (read before touching anything)

Goal: a Qt Quick/QML client with **visual and functional parity with `desktop-gpui`** (the spec), talking to the
unchanged Go backend. GPUI source = `../desktop-gpui/crates/app/src` (+ `crates/backend-client` for every REST
path/DTO/WS event). Reuse its numbers (px/rem sizes, colours via theme tokens, copy/strings, states, shortcuts). Do not
redesign. Do not import GPUI/Rust code or embed web content.

## Hard rules
- **Never touch the user's real data.** The user's live backend (an AppImage, port in `~/.wa-bot/run/core.json`), `~/.wa-bot`
  and `~/.config/wa-bot-desktop/settings.json` are off limits for writes, and you must not `pkill`/`kill` any `wa-bot*`
  process you did not start. Test against `tests/mock_backend.py` (no WhatsApp account involved), or against a real
  backend you spawn yourself with `WABOT_HOME=<tmpdir>` (unlinked, fresh). Never send messages, call, post, or log out
  through anything but a mock. `Settings(path)` takes an explicit path for tests.
- **Do not run git commands that change the tree** (no commit/checkout/stash/reset/clean/add). The orchestrator commits.
  Other agents are editing other files in this same working tree right now: only edit files you own (below) plus tiny,
  additive edits to shared files; never revert or reformat someone else's file.
- No Rust/C++/compilation for UI work. Python 3.14 + PySide6 6.11 (system). Extra pip deps only if truly needed
  (install into `.venv`, add to `pyproject.toml`, say so in your report).
- **Do NOT use `qmlRegisterType/Singleton*`, `@QmlElement`, or QML `pragma Singleton` objects that need services.** With
  this PySide6/Python build, any qmlRegister call breaks nested QML items, and QML singletons cannot see context
  properties. All Python objects reach QML as **context properties** via `services.expose("Name", obj)`; QML types are
  plain `.qml` files imported by directory (`import "../../ui"`).
- No blocking HTTP/disk work on the UI thread. REST goes through `Api` (QtNetwork, async). Keep files small and modular.
- Business logic stays in the Go backend. Use existing REST/WS contracts exactly as in `backend-client`. Don't invent
  endpoints; if the backend lacks something GPUI uses, mirror GPUI's fallback, and report it.

## Layout / where things live
```
main.py                     entry; build() creates services + engine (also used by tests/shot.py)
bridge/registry.py          loads feature modules `bridge/<name>.py` -> `register(services)`; list = FEATURE_MODULES
bridge/{backend,api,events,appstate,settings,theme,toast,models,icons}.py     core (orchestrator-owned)
qml/Main.qml, qml/shell/    window, titlebar, nav rail, router outlet (PageHost.qml has the page table), toasts
qml/ui/                     reusable atoms (AppText Icon IconButton Btn AppTextField AppMenu AppDialog Scroll
                            AppSwitch AppSlider Spinner ToolTipBubble ThinScrollBar). Add NEW files freely (unique names);
                            edit existing ones only for generic fixes. Types in qml/ui are used via `import "../../ui"`
                            (relative to the importing file) — there is no qmldir in ui/, every file is a type.
qml/pages/<area>/           your pages
qml/assets/icons/*.svg      Lucide icons (`Icon { name: "check" }`). Missing one? copy it from
                            ~/.cargo/registry/src/*/gpui-kit-assets-0.7.1/assets/icons/<name>.svg
tests/mock_backend.py       mock REST+WS backend; area fixtures go in tests/mock_<area>.py with `install(mb)`
scripts/shot.py             offscreen screenshot:  .venv/bin/python scripts/shot.py OUT.png --route chat \
                              --preset whatsapp-dark --size 1100x720 --url <mock url> --wait 2500
```
Run tests: `.venv/bin/python -m pytest -q tests`. Run the app: `QT_QPA_PLATFORM=offscreen` only (no display here: it's a
tty session); use `shot.py` (it can also drive actions: extend it or write your own script using `main.build()` and
`win.grabWindow()`; look at the PNG with the Read tool to judge visuals).

## QML globals (context properties — no import needed)
`Theme` colours `Theme.background/foreground/card/cardForeground/primary/primaryForeground/secondary/.../border/input/ring`,
chat tokens `Theme.chatBubbleOut` etc. (+`Theme.hasChat`), `Theme.rem` (16px × UI scale; Tailwind rem values are
`Theme.rem * 0.875`), `Theme.chatScale`, `Theme.alpha(c,a)`, `Theme.mix`, `Theme.glass(c)` (translucent window fill — use
`Theme.glass(Theme.background)` for page backgrounds like GPUI's `glass_fill`), `Theme.fastMs=140`, `Theme.baseMs=220`.
`Backend` (state, apiUrl, root_url) · `Api` (`Api.request(method,path,bodyJson,token)` + `finished` signal for QML;
`Api.mediaUrl(path)`, `Api.enc(s)`) · `Events` (`connected`, `event(type, payloadJson)`) · `Settings` (`get/set`, `changed(key)`;
keys are those of GPUI `DesktopSettings`, same file) · `AppState` (`path/section/param`, `navigate("chat/ID")`,
`openChat(id)`, `activeChatId`, `auth` = checking|unauthenticated|authenticated, `unlinked`, `connection`, `windowVisible`)
· `Toast` (`show/success/error`). Python side: `services.api.fetch(method, path, body_obj, cb(status, parsed))`,
`services.hub.event` signal (type, payloadJson), `services.expose(name, obj)`, `services.toast`, `services.app`,
`services.settings`, `services.theme`. `bridge/models.py` `DictListModel(roles, key)` is the list model to use
(reset/upsert/patch/remove/prepend/extend/sort_by). Each feature module exports `register(services)`.
Python objects exposed to QML: use `@Property(..., notify=...)`, `@Slot(...)`, `Signal`s. Pass structured data to QML as
dict/list (QVariantMap/List) or models.

## Cross-module contract (agents work in parallel — honour these names)
| Context property | Module | Owner | Provides |
|---|---|---|---|
| `Auth` | bridge/auth.py | auth agent | login state, QR, pairing, logout; sets `AppState.set_auth(...)` |
| `Chats` | bridge/chats.py | chat-list agent | `model` (DictListModel of chats; roles = REST `/chats` fields: id name avatar lastMsg lastTime unread hasUnreadMention isActive isGroup archived pinnedAt muteMode mutedUntil disappearingSecs + derived `typing`), `get(chatId)`→dict, chat actions (pin/archive/mute/read/delete/clear), groups, `requestForward(chatId, messageId)` signal-slot pair `forwardRequested(chatId, messageId)`, `requestSheet(name, chatId)` / `sheetRequested` |
| `Messages` | bridge/messages.py | chat-messages agent | `model` (active chat's messages; roles = REST message fields flattened + display fields), `loadOlder()`, `jumpTo(messageId)`, `addPending(row)`, `ackPending(localId, serverId)`, `failPending(localId, error)`, per-message actions (delete, react, star, pin, forward-send, receipts), WS `new_message/message_*` handling |
| `Composer` | bridge/composer.py | composer agent | text/media/voice/poll/location/contact/sticker sending (calls `Messages.addPending/ackPending/failPending` for optimistic rows), reply/edit state `replyTo`, `startReply(messageId)`, `startEdit(messageId)`, `cancel()`, drafts, link-preview |
| `Statuses` / `Channels` | bridge/statuses.py / channels.py | status-channels agent | their models/actions |
| `Calls` | bridge/calls.py | calls agent | call state (`active`, history model), `start(chatId, "audio"|"video")`, accept/decline/hangup, media transport |
| `BotAdmin` | bridge/botadmin.py | bot agent | triggers/cron/webhooks/docs/AI-assistant stores |
| `System` | bridge/system.py | settings agent | settings-page backing, history sync/media, GTK palette (calls `services.theme.register_preset`) |
| `Platform` | bridge/platform.py | platform agent | notifications, tray, single instance, popover/toggle mode |
Chat files: `pages/chat/ChatPage.qml`, `sidebar/*`, `conversation/ConversationPane.qml`, `conversation/ChatHeader.qml`,
`sheets/*`, `dialogs/*` = chat-list agent; `conversation/MessageList.qml`, `messages/*`, `viewer/*` = chat-messages agent;
`composer/*` = composer agent. Cross-calls go through the context properties above (e.g. a bubble menu "Reply" calls
`Composer.startReply(id)`, "Forward" calls `Chats.requestForward(...)`, the search sheet's result calls `Messages.jumpTo(id)`).
If you need something from another module that doesn't exist yet, code against the contract and tolerate its absence
(`typeof Composer !== "undefined"` guards in QML; `try/except` or `getattr` in Python) — it will land.

## Definition of done for your area
1. Every screen/state/action of the GPUI area exists in QML with the same layout, sizes, copy and behaviour (list what you
   compared in your report). Stub screens do not count.
2. Python bridge has unit tests (pytest) against the mock backend with fixtures for your area; QML smoke test = loads
   without `[qml]` warnings via shot.py and a PNG you inspected.
3. Update/create `desktop-qml/parity/<area>.md`: table `GPUI file -> QML file -> endpoint/event -> test -> status`, with
   honest status (done / partial / not done + why). Mention anything unverified or that needs a real device/backend.
4. Final report (to the orchestrator): files created, tests run + results, known gaps. Be honest: tested vs inferred.

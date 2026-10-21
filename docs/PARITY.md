# gpui / web parity

The desktop app (`desktop-gpui/`) and the web app (`web/`) are two clients of the
same Go backend. **They must behave the same.** A change to one needs the matching
change in the other, in the same commit or the same PR.

## The rule

- A feature, setting, label, default or behavior that exists in one client exists in
  the other, with the same wording and the same options.
- Backend changes (`internal/`, `cmd/`) reach both clients: check that both still
  work, and that both read any new field or endpoint.
- A bug fix counts too. If the bug is in logic both clients share (a tick, an unread
  count, a typing line), look for the same bug in the other one.
- Layout may follow each toolkit (a dropdown is a `Select` in the web and a popover in
  gpui), but the information, options and order stay the same.

## Checklist before you commit

1. Which client did I touch? Open the equivalent screen in the other one.
2. New or changed setting key? Both settings pages read and write it.
3. New endpoint or WebSocket event? Both clients handle it (`backend-client` for gpui,
   `web/src/lib/api.ts` and `ws-bus` for the web).
4. Copy: same title, same hint text, same button labels.
5. Update the table below if you add a feature or an intentional difference.

## Where the same thing lives

| Feature | gpui | web |
| --- | --- | --- |
| Settings page | `crates/app/src/views/settings.rs` and `settings_*.rs` | `src/pages/settings/SettingsPage.tsx` |
| Chat history and media rows | `views/settings_history.rs` | `SettingsPage.tsx`, `HistoryMediaSection.tsx` |
| Backup | `views/settings_backup.rs` | `BackupSection.tsx` |
| AI provider | `views/settings_ai.rs` | `SettingsPage.tsx` (AI) |
| Theme picker | `settings.rs` (modal) | `SettingsPage.tsx` (dialog) |
| Typing indicator | `state/chat.rs`, `chat_list.rs`, `conversation/typing.rs` | `stores/typingStore.ts`, `ChatSidebar.tsx`, `ChatArea.tsx` |
| Calls page | `views/calls.rs` | `src/pages/calls/CallHistoryPage.tsx` |
| Message ticks | `state/chat.rs` (`MessageStatus`), `message_bubble.rs` | chat store, message bubble |
| Unread and read receipts | `chat_actions.rs`, `data/loading.rs` | `ChatSidebar.tsx` |
| API client | `crates/backend-client/src/client.rs`, `dto/` | `src/lib/api.ts` |

## Intentional differences

These exist because of the platform, not by accident. Keep this list short.

| Difference | Why |
| --- | --- |
| Window transparency, backdrop blur, interface and chat text size, chat layout | A native window has them; a browser tab does not. |
| Window controls (minimize, maximize, close) sit in the chat headers, after a separator, and the empty header space drags the window | The web page has no window of its own. The title bar row is gone on the chat page; other pages keep it. |
| When the window is too narrow for the rail, the list and a 560 px conversation, `Auto` layout shows the chat list and the conversation as separate pages | Lets the window be tiled to half of a small screen. The web follows its own breakpoints. |
| OS notifications, autostart, resident daemon | Desktop only. |
| In-app updater, beta channel | The web is served, not installed. Beta builds hide the update buttons. |
| Backup folder: native folder picker (gpui), typed absolute path (web) | A browser cannot return an absolute path. Both talk to `/api/backup/*`. |
| Settings layout details (popover vs `Select`, modal vs dialog) | Each toolkit's own control. |

## Behavior both clients share

- Only the avatar and name in a conversation header open the info sheet. The rest of
  the header does nothing. In gpui, pressing and dragging that block moves the window,
  so a narrow window can still be moved.
- Read receipts off: opening a chat does not clear its unread state, anywhere. Only an
  explicit "Mark as read" (`POST /chats/{id}/read?explicit=1`) does.
- Typing indicators need the account online. Opening any chat, group or private, calls
  `presence-subscribe`.
- Day separators ('TODAY', '28 SEPTEMBER 2026') are small content-sized pills; digits are tabular so dates of one length match.
- Day separators ('TODAY', '28 SEPTEMBER 2026') are pills of one fixed width.
- The window controls sit over the top right corner of the chat page in gpui, so anything
  full-width up there (image viewer, video viewer, sheet headers) leaves `controls_room` free.
- Call history rows show the contact's name and profile picture (the backend adds
  `peer_id` and `peer_name`), and no total count.
- Progress bars show only while something runs; afterwards one summary line.

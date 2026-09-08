# Phase 4: Chat Sidebar + Percakapan Inti - Context

**Gathered:** 2026-09-09
**Status:** Ready for planning
**Mode:** Autonomous (1:1 Web Parity)

<domain>
## Phase Boundary

User bisa mengelola daftar chat di sidebar dan bercakap teks lengkap dengan aksi pesan inti secara live:
1. Sidebar: Avatar, nama, snippet pesan terakhir, waktu, unread badge, realtime update di tempat tanpa flicker, pencarian chat, toggle arsip, context menu (pin, archive, mute), auto mark-as-read, new group dialog, join group dialog, history-sync indicator.
2. Area Percakapan: Header (nama lawan, presence live, search-in-chat button), feed pesan terpaginasi (100 pesan awal, scroll pagination), date dividers, ticks status (sent/delivered/read), optimistic temporary message replacement (`temp-`), bubble teks markdown, reaction chips, aksi pesan (reply, edit, delete, forward ke N chat), emoji picker popover.

</domain>

<decisions>
## Implementation Decisions

### D-04-1: ChatStore State Architecture (Parity with web `chatStore.ts`)
- Implement `ChatStore` in `desktop-gpui/crates/app/src/state/chat.rs` replicating all 6 key semantics:
  1. Stable ascending timestamp sorting (`sort_asc`).
  2. Unread preservation during upsert if not explicitly updated.
  3. Move to top on `lastMsg` or `lastTime` change.
  4. Optimistic `temp-` message in-place replacement matching pending text/media type without duplication.
  5. Invalidation support for reconnect/sync resets.
  6. Paginated entry tracking (`has_more`, `has_more_next`, `loading_more`, `loading_newer`).

### D-04-2: Layout & Components
- `ChatSidebarView`: Search bar, filter pills, history sync badge, virtualized list of chat items with context menu for pin/archive/mute.
- `ChatAreaView`: Top bar with peer presence status (`Online`, `Offline`, `Typing...`), message viewport with date separators and unread banner, message compose bar with markdown toggle, emoji picker button, attachment menu, and send button.
- `MessageBubble`: Sent/received styling, sender info in groups, timestamp + ticks indicator (grey single tick for pending/sent, double grey for delivered, double blue for read), reaction pills with emoji count.
- Action overlays & modals: `ForwardDialog`, `NewGroupDialog`, `JoinGroupDialog`, `EmojiPickerPopover`.

### D-04-3: Realtime WS Dispatch
- Subscribe to `WsEvent` in `wabot_app` event loop:
  - `WsEvent::Message`: upsert into `ChatStore`, update last message & unread if not currently open.
  - `WsEvent::Receipt`: update message status (`sent`, `delivered`, `read`).
  - `WsEvent::Presence`: update peer presence map in active chat.
  - `WsEvent::HistorySync`: update history sync progress indicator.

</decisions>

<code_context>
## Existing Code Insights
- `wabot_backend_client::HttpClient`: already has `get_chats`, `get_messages`, `send_message`, `edit_message`, `delete_message`, `react_message`, `forward_messages`, `mark_as_read`, `create_group`, `join_group`, `get_history_sync_status`, `trigger_history_sync`.
- `wabot_backend_client::WsClient`: pushes events through broadcast channel.
- `wabot_app::theme::manager::ThemeManager`: semantic colors available via `theme.color(ThemeToken::...)`.
</code_context>

<specifics>
## Specific Requirements
- CHAT-01: Chat list (avatar, nama, pesan terakhir, waktu, unread badge)
- CHAT-02: Live updates in place without flicker
- CHAT-03: Client-side search/filter
- CHAT-04: Archive toggle
- CHAT-05: Pin, archive, mute
- CHAT-06: Auto mark-as-read
- CHAT-07: Create group & join via link
- CHAT-08: History sync status & trigger
- CONV-01: Paginated messages
- CONV-02: Date divider & timestamps
- CONV-03: Optimistic temp- send
- CONV-04: Reply, edit, delete
- CONV-05: Reactions with chips
- CONV-06: Forward to N chats
- CONV-07: In-chat search
- CONV-08: Peer presence in header
- CONV-09: Live read receipts & ticks
- CONV-10: Markdown message rendering
- CONV-11: Emoji picker popover
</specifics>

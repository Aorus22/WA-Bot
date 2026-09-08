# Phase 4: Chat Sidebar + Percakapan Inti - UI Design Contract

**Created:** 2026-09-09
**Status:** Approved

## 1. Visual Hierarchy & Dimensions

- **Chat Sidebar Width:** Fixed `340px`, border right `1px solid border_subtle`.
- **Chat List Item Height:** `72px`, padding `12px 16px`, avatar `48x48px` circle with fallback initials.
- **Unread Badge:** Pill shape, background `accent`, text `accent_foreground`, font-size `11px`, bold, min-width `20px`, padding `2px 6px`.
- **Pin & Mute Icons:** Size `14x14px`, text `text_muted`.
- **Chat Header Height:** `64px`, border bottom `1px solid border_subtle`, peer avatar `40x40px`, peer title + presence subtitle (`text_xs text_muted` / green text when Online).
- **Date Divider:** Centered capsule, background `bg_subtle`, border `1px solid border_subtle`, text `text_muted`, font-size `12px`, padding `4px 12px`, margin `16px 0`.
- **Message Bubbles:**
  - Sent: Aligned right, background `primary`, text `primary_foreground`, border radius `12px 12px 2px 12px`, max-width `70%`.
  - Received: Aligned left, background `bg_surface`, text `text_default`, border `1px solid border_subtle`, border radius `12px 12px 12px 2px`, max-width `70%`.
  - Status Ticks: Grey single tick (pending/sent), grey double tick (delivered), blue double tick (read).
  - Reaction Chips: Floating container below bubble, pill background `bg_surface` with border `1px solid border_subtle`, emoji + count number.
- **Composer Bar:** Height min `56px`, max `160px` auto-growing input, attachment clip button, emoji smile button, send paper-plane button.

## 2. Interaction Flows

1. **Selecting Chat:** Clicking a chat item highlights it with `bg_active`, switches active conversation in `ChatArea`, fires `mark_as_read` if unread > 0.
2. **Sending Message:** Pressing Enter (without Shift) in compose bar creates optimistic message with ID `temp-<timestamp>`, status `pending`, adds to list immediately, dispatches HTTP `send_message`. When backend WS confirms, in-place replaces `temp-` message.
3. **Message Actions:** Hover on message shows action trigger (...) menu: Reply, React, Edit (if own message), Forward, Delete.
4. **Emoji Picker:** Opens floating popover above composer with common emoji grid; clicking emoji appends to compose input.
5. **Group Modals:** "New Group" modal allows entering subject & member list; "Join via Link" allows pasting invite URL/code.


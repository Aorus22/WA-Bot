---
phase: 04-chat-sidebar-percakapan-inti
plan: 01
title: ChatStore state management with 6 key semantics and unit tests
status: passed
---

# Plan 04-01 Summary: ChatStore State Management

## Accomplishments
- Implemented `ChatStore` and `ChatMessagesEntry` in `desktop-gpui/crates/app/src/state/chat.rs`.
- Enforced all 6 critical semantic rules matching `web/src/stores/chatStore.ts`:
  1. Ascending stable timestamp sorting via `sort_asc`.
  2. Unread preservation during chat upserts unless explicitly updated.
  3. Move-to-top reordering on `last_msg` or `last_time` modifications.
  4. Optimistic `temp-` message in-place replacement matching pending text/media type without duplication.
  5. State invalidation for message streams.
  6. Paginated entry tracking (`has_more`, `has_more_next`, `loading_more`, `loading_newer`).
- Added peer presence tracking (`set_presence`, `get_presence`).
- Unit test suite (`test_chat_store_sort_asc`, `test_chat_store_temp_message_replacement`, `test_chat_store_upsert_moves_to_top_on_last_msg_change`, `test_chat_store_invalidate_messages`) passes cleanly.

## Key Files
- `desktop-gpui/crates/app/src/state/chat.rs`
- `desktop-gpui/crates/app/src/state/mod.rs`
- `desktop-gpui/crates/backend-client/src/ws.rs`

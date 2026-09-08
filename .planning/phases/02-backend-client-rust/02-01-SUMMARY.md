# Phase 2 Plan 01: DTOs & Core REST HTTP Client — Summary

**Phase:** 02-backend-client-rust | **Plan:** 01 | **Date:** 2026-09-09
**Requirements:** CORE-04
**Status:** complete

## Objective

Mendefinisikan seluruh DTO ber-type dan mengimplementasikan `HttpClient` untuk seluruh endpoint REST dari `web/src/lib/api.ts` (~70 endpoint), lengkap dengan penanganan error, normalisasi, dan URL resolusi.

## What was built

- `desktop-gpui/crates/backend-client/src/error.rs` — `ClientError` (Http, Json, Api, WebSocket, Io, Custom) dengan `thiserror`.
- `desktop-gpui/crates/backend-client/src/dto/` — DTOs lengkap:
  - `chat.rs`: `Chat`, `ChatState`, `HistorySyncStatus`, `HistorySyncError`
  - `message.rs`: `Message`, `ReactionEntry`, `PollMeta`, `LocationMeta`, `ContactMeta`, `ViewOnceMeta`, `LinkPreviewMeta`, `MessageExtra`
  - `group.rs`: `GroupCache`, `GroupParticipantInfo`, `GroupPreview`, `UpdateGroupChanges`
  - `status.rs`: `StatusEntry`, `StatusGroup`
  - `channel.rs`: `Channel`, `ChannelPreview`, `ChannelMessage`
  - `contact.rs`: `Contact`, `StickerFavorite`
  - `call.rs`: `CallStatus`, `CallType`, `CallDirection`, `CallSource`, `MediaMode`, `CallState`, `CallLog`, `CallHistoryResponse`, `CallHistoryFilter`
  - `bot.rs`: `Trigger`, `CronJob`, `Webhook`, `WebhookLog`, `WebhookLogResponse`
  - `settings.rs`: `SettingsResponse`, `AssistantResponse`
  - `system.rs`: `StatusResponse`, `QrCodeResponse`, `StatusResult`, `IdResult`, `OptIdResult`, `GroupCreateResult`, `GroupInviteLinkResult`, `JoinGroupResult`
- `desktop-gpui/crates/backend-client/src/client.rs` — `HttpClient` yang mengimplementasikan semua endpoint REST:
  - System: `get_status`, `get_qr_code`, `logout`, `health_check`
  - Chats: `get_chats`, `mark_as_read`, `pin_chat`, `archive_chat`, `mute_chat`, `subscribe_chat_presence`
  - Messages: `get_messages`, `search_messages`, `get_message_context`, `get_chat_media`, `get_chat_docs`, `get_chat_links`, `send_message`, `delete_message`, `edit_message`, `reply_message`, `react_to_message`, `forward_message`, `send_poll`, `send_poll_vote`, `send_typing`, `send_location`, `send_contact`
  - History Sync: `get_history_sync_status`, `start_history_sync`
  - Contacts & Stickers: `get_contacts`, `get_favorite_stickers`, `favorite_sticker`, `delete_favorite_sticker`, `send_sticker`
  - Groups: `get_group`, `update_group`, `update_group_participants`, `get_group_invite_link`, `preview_group_link`, `join_group_with_link`, `create_group`, `leave_group`
  - Status: `list_statuses`, `post_status_text`, `mark_status_viewed`
  - Channels: `list_channels`, `preview_channel`, `follow_channel`, `unfollow_channel`, `set_channel_mute`, `get_channel_messages`, `react_channel_message`
  - Calls: `get_active_call`, `create_call`, `create_group_call`, `add_call_participants`, `ring_call_participant`, `answer_call`, `reject_call`, `hangup_call`, `get_call_history`, `start_video`, `accept_video`, `reject_video`, `stop_video`
  - Bots & AI: `get_triggers`, `create_trigger`, `update_trigger`, `delete_trigger`, `delete_all_triggers`, `test_trigger`, `get_cron_jobs`, `create_cron_job`, `update_cron_job`, `delete_cron_job`, `delete_all_cron_jobs`, `test_cron_job`, `get_webhooks`, `create_webhook`, `update_webhook`, `delete_webhook`, `delete_all_webhooks`, `test_webhook`, `get_webhook_logs`, `delete_all_webhook_logs`, `get_docs`, `chat_assistant`
  - Settings: `get_settings`, `update_settings`

## Verification

- `cargo check -p wabot-backend-client` passes.
- DTO roundtrip tests verify correct field mapping and null-tolerant defaults.

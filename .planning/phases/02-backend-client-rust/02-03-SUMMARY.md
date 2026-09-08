# Phase 2 Plan 03: WebSocket Client & Live Contract Tests — Summary

**Phase:** 02-backend-client-rust | **Plan:** 03 | **Date:** 2026-09-09
**Requirements:** CORE-04, AUTH-05
**Status:** complete

## Objective

Mengimplementasikan `WsClient` dan WebSocket event pump dengan authenticate handshake, reconnect loop ~3s pada close abnormal, dynamic URL re-resolution, parsing strongly-typed `WsEvent`, dan live contract round-trip tests melawan backend Go lokal.

## What was built

- `desktop-gpui/crates/backend-client/src/ws.rs`:
  - `WsMessage` & `WsEvent` enum: covers `NewMessage`, `MessageStatus`, `MessageDeleted`, `MessageEdited`, `MessageReaction`, `PollUpdate`, `ChatState`, `ChatNameUpdate`, `ChatsChanged`, `GroupUpdated`, `StatusNew`, `ChannelMessage`, `ChannelUpdate`, `ChannelsChanged`, `ChatPresence`, `CallEvent`, `Connected`, `Disconnected`, `Raw`.
  - `WsClient`:
    - Mengelola connection loop menggunakan `tokio-tungstenite`.
    - Dynamic `url_resolver` untuk re-evaluasi URL port di setiap percobaan reconnect.
    - Automatic `authenticate` handshake packet pada setiap connection open (`{"type": "authenticate", "payload": {"userId": ...}}`).
    - Abnormal close handling: otomatis reconnect dengan interval ~3 detik kecuali ditutup dengan status code normal (1000).
    - Multi-subscriber support via `flume` channels (`subscribe()`).
- `desktop-gpui/crates/backend-client/tests/live_contract.rs`:
  - Test DTO deserialization.
  - Test media URL resolving.
  - Test live contract against spawned Go backend:
    - Health check: `health_check()` ok.
    - Status check: `get_status()` ok.
    - Chat listing: `get_chats()` ok.
    - Settings: `get_settings()` ok.
    - WebSocket handshake: `WsClient` connects and receives `WsEvent::Connected` after handshake.

## Verification

- `cargo test -p wabot-backend-client --test live_contract` passed (3/3 tests ok).
- Entire backend client suite passes: 6 tests ok.

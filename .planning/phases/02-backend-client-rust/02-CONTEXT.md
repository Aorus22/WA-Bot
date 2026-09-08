# Phase 2: Backend-client Rust - Context

**Gathered:** 2026-09-09
**Status:** Ready for planning
**Mode:** Autonomous (requirements locked in REQUIREMENTS.md & web/src/lib/api.ts)

<domain>
## Phase Boundary

Menyediakan crate Rust ber-type penuh `wabot-backend-client` yang mencakup 100% permukaan API web (`web/src/lib/api.ts` ~70 endpoint REST + multipart builders) dan WebSocket pump (`web/src/hooks/use-websocket.ts` & `web/src/lib/ws-bus.ts` auth handshake, reconnect 3s, URL re-resolve, typed event dispatch).

</domain>

<decisions>
## Implementation Decisions

### Architecture & Module Layout
- `dto/`: Domain Data Transfer Objects (`chat`, `message`, `group`, `status`, `channel`, `contact`, `call`, `bot`, `settings`). Derives `Debug, Clone, Serialize, Deserialize, PartialEq`. `serde(rename_all = "camelCase")` or explicit serde attributes matching JSON output from Go backend.
- `http/`: `HttpClient` wrapping `reqwest::Client`, base URL configuration, typed method calls for every REST endpoint, and error conversions (`thiserror::Error`).
- `multipart/`: Typed builders for multipart forms: `SendMediaForm` (with target, secret, type, ptt, seconds, waveform, viewOnce), `GroupPhotoForm`, and `StatusMediaForm`.
- `ws/`: `WsClient` and `WsEventBus` using `tokio-tungstenite`. Connects to `/ws`, sends `authenticate` message on open, reconnects every 3s on abnormal close (ignoring code 1000 normal close), re-resolves base URL dynamically, parses incoming events into strongly-typed `WsEvent` enum, and dispatches via broadcast/flume channels.

### Discretion & Defaults
- Gunakan `reqwest` dengan `rustls` (sudah di-pin di workspace) untuk HTTP client.
- Gunakan `tokio-tungstenite` untuk WebSocket transport.
- Nilai null/missing pada field chat, status, dll. ditangani dengan toleran menggunakan `Option<T>` dan helper normalisasi (mirip `normalizeChat` dan `normalizeHistorySyncStatus` di web).
- URL media resolving (`media_url`) mendukung relative `/api/media/...` dan absolute URLs.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `wabot-supervisor` (Phase 1): provides spawned backend URL and port for integration tests.
- `wabot-settings` (Phase 1): paths and configs.
- `web/src/lib/api.ts`: 1,160 baris TypeScript referensi eksak endpoint, DTO, query parameter, dan payload.
- `web/src/hooks/use-websocket.ts`: referensi logic WebSocket reconnect, port fallback, dan authenticate packet.
- `internal/delivery/http/routes/routes.go`: referensi rute Go backend.

### Established Patterns
- Pinned exact dependencies (`=x.y.z`).
- Tokio async runtime.
- Crate-level tests dengan live Go backend jika biner tersedia.

</code_context>

<specifics>
## Specific Ideas

- Pastikan semua ~70 endpoint dari `api.ts` ter-cover tanpa ada endpoint yang tertinggal.
- Pastikan event websocket mencakup pesan masuk, chat updates, call updates, status updates, presence, dan history-sync.

</specifics>

<deferred>
## Deferred Ideas

- UI GPUI rendering views (dimulai di Phase 3).
- Media player GUI integration (Phase 5).
- WebRTC native audio/video piping (Phase 6).

</deferred>

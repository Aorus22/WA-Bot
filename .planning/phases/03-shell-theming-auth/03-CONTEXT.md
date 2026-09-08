# Phase 3: Shell + Theming + Auth - Context

**Gathered:** 2026-09-09
**Status:** Ready for planning
**Mode:** Autonomous (requirements locked in REQUIREMENTS.md & web client parity)

<domain>
## Phase Boundary

Menyediakan kerangka aplikasi desktop GPUI lengkap yang mencakup:
1. **App Shell & Router**: Frameless window title bar dengan drag region dan kontrol window (min/max/close), navigation sidebar 1:1 web (Chats, Status, Channels, Calls, Triggers, Cron, Webhooks, Documentation, Settings), dan content outlet.
2. **Theming Engine**: Porting 81 preset tema dari `web/src/data/themes.ts` ke dalam representasi Rust yang strongly typed (17 warna CSS token: background, foreground, card, cardForeground, primary, primaryForeground, secondary, secondaryForeground, muted, mutedForeground, accent, accentForeground, destructive, destructiveForeground, border, input, ring). Mendukung pergantian tema panas (hot-switching) tanpa restart dan sinkronisasi dengan `wabot_settings::DesktopSettings`.
3. **Auth Gate & Views**: Gate transisi status autentikasi (`Checking` dengan spinner, `Unauthenticated` dengan halaman login, `Authenticated` dengan shell utama). Tampilan login memiliki tab QR code (auto-refresh ~30s, live update via WS push, retry button) dan tab Phone link berdampingan. Logout selalu melalui dialog konfirmasi.
4. **Status Koneksi & Toast System**: Indikator status koneksi WebSocket (online/offline) dan banner reconnecting saat koneksi terputus. Sistem toast notification untuk feedback setiap mutasi aksi pengguna.

</domain>

<decisions>
## Implementation Decisions

### Architecture & Module Layout in `crates/app`
- `theme/`:
  - `preset.rs`: Definisi `ThemePreset` dan 17 semantic colors (RGBA / HSLA) serta katalog statis seluruh 81 preset web dari `web/src/data/themes.ts`.
  - `manager.rs`: `ThemeManager` global GPUI yang mengelola preset aktif dan mode (System/Light/Dark), mendengarkan perubahan dari settings atau picker tema, dan mengupdate tema `gpui-component` serta token lokal.
- `state/`:
  - `auth.rs`: `AuthState` GPUI model/entity yang mengelola status login (`Checking`, `Unauthenticated`, `Authenticated`), QR code aktif, dan integrasi dengan `HttpClient` dan `WsClient`.
  - `connection.rs`: `ConnectionState` yang memantau status koneksi WebSocket (`Connected`, `Connecting`, `Disconnected`) dan jumlah reconnect attempt.
- `components/`:
  - `titlebar.rs`: Custom titlebar dengan drag region, app title ("WA Bot"), dan window control buttons (minimize, maximize/restore, close).
  - `nav_sidebar.rs`: Navigation sidebar vertikal dengan ikon-ikon 1:1 web (Chats, Status, Channels, Calls, Triggers, Cron, Webhooks, Docs, Settings) dengan tooltip dan indikator active border.
  - `toast.rs`: Integrasi toast notification menggunakan `gpui-component::notification::NotificationList` dan `WindowExt::push_notification`.
  - `connection_banner.rs`: Banner status koneksi kuning/merah saat offline/reconnecting dan titik hijau/abu-abu di sidebar.
  - `dialogs/`: `LogoutConfirmDialog` menggunakan modal konfirmasi dengan tombol Batal dan Logout.
- `views/`:
  - `auth/`: `LoginView` dengan tab switch QR Code vs Phone Link, render QR matrix via SVG/drawing, tombol refresh QR, langkah instruksi scan WhatsApp, dan status enkripsi end-to-end.
  - `shell/`: `AppShellView` membungkus `Root`, `titlebar`, `nav_sidebar`, `connection_banner`, dan area halaman aktif (router outlet).

### Discretion & Defaults
- Menggunakan `gpui-component` (versi `=0.6.0` yang sudah di-pin) untuk widget foundation (`Root`, `Button`, `NotificationList`, `Dialog`, `Icon`).
- QR code rendering di desktop: generate QR matrix menggunakan crate Rust `qrcode` atau render SVG path langsung ke canvas GPUI.
- Penyimpanan preferensi tema: membaca dan menulis ke `wabot_settings::DesktopSettings` yang disimpan di OS config dir.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `wabot_supervisor`: Lifecycle manager untuk backend sidecar.
- `wabot_settings`: `DesktopSettings`, `ThemeMode`, `WindowState`, `paths`.
- `wabot_backend_client`: `HttpClient` (`get_status()`, `get_qr_code()`, `logout()`), `WsClient` (`WsEvent::QrCode`, `WsEvent::AuthSuccess`), DTOs.
- `web/src/App.tsx`: Referensi route, layout, dan auth gate.
- `web/src/pages/layout/NavigationSidebar.tsx`: Referensi nav items dan visual styling.
- `web/src/pages/login/LoginPage.tsx`: Referensi form login, tab QR/Phone, dan auto-refresh interval.
- `web/src/data/themes.ts`: 81 preset warna lengkap.

### Established Patterns
- Strongly-typed structs dengan Serde.
- Safe lifecycle, asynchronous handling dengan Tokio.
- Zero-panic error handling dengan `Result` dan `Option`.

</code_context>

<specifics>
## Specific Ideas

- Hot-reload tema harus langsung merefleksikan perubahan warna pada background, teks, border, button, dan sidebar tanpa perlu restart aplikasi.
- Saat WS terputus, banner koneksi muncul dengan teks "Connecting to WhatsApp..." atau "Connection lost, retrying...", dan menghilang saat `Connected`.
- Ketika backend merespons `auth_success` via WebSocket atau polling, gate auth langsung berganti ke `Authenticated` secara instan.

</specifics>

<deferred>
## Deferred Ideas

- View per route spesifik seperti Chat list & Message Bubbles (Phase 4).
- Media view & Attachment modal (Phase 5).
- WebRTC Calls overlay (Phase 6).
- Bot & Trigger editor (Phase 7).

</deferred>

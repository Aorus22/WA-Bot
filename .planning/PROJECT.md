# wa-bot

## What This Is

wa-bot adalah aplikasi manajemen WhatsApp full-stack: backend Go (REST + WebSocket di `cmd/api`, `internal/`) plus web client React 19 + Vite + Tailwind/shadcn (`web/`). Sudah ada dua kemasan desktop: `desktop/` (Electron wrapper — serve `web/dist` + spawn Go backend sebagai sidecar) dan `desktop-gtk/` (client GTK4 native Go dengan chat core, call management, search, theme). Milestone ini menambah kemasan ketiga: client desktop native berbasis Rust GPUI yang 1:1 fitur dan tampilan dengan versi web.

## Core Value

User desktop mendapat seluruh kemampuan web wa-bot dalam aplikasi native yang cepat — tanpa browser, tanpa Electron — dengan tampilan dan perilaku yang identik dengan web.

## Current Milestone: v1.0 desktop-gpui

**Goal:** Membangun client desktop Rust-GPUI yang 1:1 fitur dan tampilan dengan web wa-bot, reuse Go backend via HTTP+WS.

**Target features:**
- Fondasi workspace Cargo + backend-client (HTTP+WS) + supervisor sidecar Go backend
- Auth & session (QR login, logout, persisted session) + app shell/nav 1:1 web
- Chat parity penuh (sidebar, bubbles, media, voice note, reply/forward/edit/delete, reaction, poll, location, contact, sticker, search, group management, presence/receipts)
- Calls (history + overlay create/answer/reject/hangup, video controls), Status, Channels
- Bot management (triggers, cron, webhooks + logs), Settings (DB-backed), AI assistant, Documentation
- Theming 1:1 (skala web: ThemeProvider + AppThemeProvider + themes.ts) + packaging Linux & Windows

## Requirements

### Validated

- ✓ Web app penuh (chat, calls, channels, status, bot/cron/trigger/webhook, settings, AI assistant, docs, login/QR) — shipped, basis parity
- ✓ Go backend REST+WS (messages, groups, calls, status, channels, bot, settings, AI) — shipped, dipakai ulang tanpa rewrite
- ✓ Electron desktop wrapper (serve web/dist + spawn backend sidecar + `__BACKEND_PORT__` discovery) — shipped, pola sidecar jadi acuan
- ✓ GTK4 desktop client (chat core, call management, search, theme, history sync) — shipped, jadi pembanding kedua pola native

### Active

- [ ] Workspace `desktop-gpui/` Cargo (pola `../web-term/desktop-gpui`: supervisor, settings, backend-client crates + app crate) dengan pin versi eksak + Cargo.lock
- [ ] Backend-client Rust menutupi seluruh permukaan `web/src/lib/api.ts` + `ws-bus`/`use-websocket` (HTTP + autoreconnect WS)
- [ ] Supervisor spawn & supervise Go backend sidecar + port discovery (setara `desktop/main.js` + `__BACKEND_PORT__`)
- [ ] Seluruh route web ter-port 1:1: /chat, /status, /channels, /calls, /cron, /triggers, /webhooks(+logs), /settings, /documentation, /login
- [ ] Chat parity: semua tipe pesan & aksi pesan, media viewer/player, group panels, chat info/search sheets
- [ ] Theming 1:1 dengan web (system/light/dark + AppThemeProvider + themes.ts)
- [ ] Packaging Linux + Windows (backend dibundel)

### Out of Scope

- Rewrite backend Go ke Rust — backend dipakai ulang apa adanya (keputusan milestone)
- macOS build — defer ke milestone berikut (alasan: fokus Linux + Windows dulu)
- Paritas `desktop-gtk` di luar yang sudah ada di web — acuan 1:1 adalah web, bukan GTK
- Mobile app — web-first, tidak berubah

## Context

- Backend: Go, `cmd/api` + `internal/` (delivery/usecase/domain/infrastructure). API surface besar (~70 endpoint di `ApiClient`, `web/src/lib/api.ts` ±1159 baris). Realtime via WS (`use-websocket.ts`, `ws-bus.ts` emit/subscribe).
- Web: React 19, react-router HashRouter, zustand `chatStore.ts` (persist chats + paginated messages), contexts `AuthContext`/`CallContext`, shadcn/ui + Tailwind v4, next-themes + AppThemeProvider + `data/themes.ts`.
- Electron (`desktop/main.js`, `preload.js`): pola sidecar — spawn `wa-bot-backend`, expose port via `__BACKEND_PORT__`, fallback `localhost:8080/api`. GPUI mengadopsi pola ini via crate supervisor.
- Referensi GPUI: `../web-term/desktop-gpui` (workspace Cargo: supervisor, settings, backend-client, terminal, webterm; `gpui-pre =0.3.3`, `gpui-component =0.6.0`, tokio+reqwest rustls, semua dep di-pin eksak). Boleh dibaca saat struggle — pola, bukan copy-paste buta.
- Bahasa pengantar milestone: Indonesia.

## Constraints

- **Tech stack**: UI Rust GPUI (ikut pola pin eksak web-term); backend tetap Go, tanpa perubahan API kecuali aditif yang disepakati
- **Parity**: 1:1 fitur dan tampilan dengan web — setiap route, dialog, dan aksi web harus ada padanannya
- **Platform**: Linux + Windows dalam milestone ini; macOS ditunda
- **Reference**: `../web-term/desktop-gpui` hanya referensi opsional, bukan dependensi build

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Reuse Go backend via HTTP+WS sidecar | Backend matang & stabil; pola terbukti di Electron dan web-term/desktop-gpui | — Pending |
| Target Linux + Windows, macOS menyusul | Fokus effort build/packaging | — Pending |
| Full 1:1 sekaligus (bukan chat-first bertahap) | Keinginan eksplisit milestone | — Pending |
| Ikuti pola crate web-term (supervisor/settings/backend-client) | Terbukti jalan di project lain yang sama | — Pending |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-09-08 after milestone v1.0 start*

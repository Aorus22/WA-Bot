# Roadmap: wa-bot desktop-gpui (v1.0)

## Overview

Milestone v1.0 membangun client desktop Rust-GPUI yang 1:1 fitur dan tampilan dengan web wa-bot, reuse Go backend via HTTP+WS sidecar. Perjalanan: fondasi workspace + supervisor + CI (Phase 1) → backend-client ber-type penuh + WS pump (Phase 2) → shell + theming + auth (Phase 3) → chat sidebar + percakapan inti (Phase 4) → media + rich types + group admin (Phase 5) → calls + status + channels (Phase 6) → bot editors + settings + docs + packaging + parity audit (Phase 7). Setiap phase berakhir dengan aplikasi runnable melawan sidecar asli.

## Phases

**Phase Numbering:**

- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: Fondasi Workspace & Supervisor** - Workspace Cargo ter-pin, supervisor sidecar, settings crate, CI Linux+Windows (completed 2026-09-09)
- [x] **Phase 2: Backend-client Rust** - DTO + REST ~70 endpoint + WS pump reconnect + multipart builders (completed 2026-09-09)
- [x] **Phase 3: Shell + Theming + Auth** - App shell, router, tema 1:1, QR login, toast, banner koneksi (completed 2026-09-09)
- [x] **Phase 4: Chat Sidebar + Percakapan Inti** - Daftar chat live, store semantics, kirim/aksi pesan inti (completed 2026-09-09)
- [x] **Phase 5: Media + Rich Types + Group Admin** - Voice note, media pipeline, sticker/poll/lokasi/kontak, panel grup (completed 2026-09-09)
- [x] **Phase 6: Calls + Status + Channels** - Riwayat + overlay panggilan, audio/video native, stories, channel feed (completed 2026-09-09)
- [ ] **Phase 7: Bot + Settings + Docs + Packaging + Audit** - Editor bot + AI sheet, settings, dokumentasi, installer, parity audit

## Phase Details

### Phase 1: Fondasi Workspace & Supervisor

**Goal**: Workspace ter-build reproduksibel di Linux + Windows dengan sidecar Go yang tersupervisi bersih
**Depends on**: Nothing (first phase)
**Requirements**: CORE-01, CORE-02, CORE-03, CORE-05
**Success Criteria** (what must be TRUE):

  1. Developer bisa `cargo build` dan CI Linux + Windows hijau dari fondasi (tanpa gap platform telat)
  2. Aplikasi men-spawn Go backend sebagai sidecar, handshake port berhasil, dan tidak meninggalkan proses orphan saat ditutup atau crash-restart
  3. Preferensi UI (termasuk kunci tema `wa-bot-theme*`) tersimpan di OS config dir dan bertahan antar restart, termasuk file korup
  4. Semua dependensi inti ter-pin eksak dengan Cargo.lock ter-commit (gpui-pre 0.3.3 + gpui-component 0.6.0 tetap berpasangan)

**Plans**: 3 plans

Plans:

- [x] 01-01-PLAN.md — Toolchain + workspace ter-pin + Cargo.lock + stub member ter-build hijau
- [x] 01-02-PLAN.md — Crate supervisor: spawn/handshake/kill sidecar tanpa orphan + --no-backend
- [x] 01-03-PLAN.md — Crate settings (UI prefs + tema keys + corrupt recovery) + CI Linux/Windows

**Risks (PITFALLS.md)**: #1 gpui churn (pin+lockfile+CI), #2 sidecar orphans/port races, #10 platform gaps (awal)

### Phase 2: Backend-client Rust

**Goal**: Seluruh permukaan `api.ts` + `ws-bus` tersedia sebagai client Rust ber-type dengan reconnect yang benar
**Depends on**: Phase 1
**Requirements**: CORE-04, AUTH-05
**Success Criteria** (what must be TRUE):

  1. Seluruh ~70 endpoint (session, chats, messages, rich types, groups, status, channels, contacts, calls, bots, settings) bisa dipanggil dari Rust dengan DTO ter-type dan lolos round-trip ke backend asli
  2. Client mengirim WS authenticate handshake di setiap koneksi terbuka, reconnect ~3s hanya saat close abnormal (bukan 1000), URL di-resolve ulang tiap percobaan, tanpa pesan ganda
  3. Builder multipart (send-media + secret/target/type/ptt/seconds/waveform/viewOnce, foto grup, media status) sama field-for-field dengan web dan lolos round-trip live

**Plans**: TBD
**Risks (PITFALLS.md)**: #3 WS reconnect/auth/QR/messages, #7 multipart contracts, #11 contract tests dimulai

### Phase 3: Shell + Theming + Auth

**Goal**: User bisa login dan memakai kerangka aplikasi bertema 1:1 web dengan status koneksi yang jujur
**Depends on**: Phase 2
**Requirements**: AUTH-01, AUTH-02, AUTH-03, AUTH-04, SET-01, SET-04, SET-07
**Success Criteria** (what must be TRUE):

  1. User login via QR dengan auto-refresh ~30s dan update live via WS push, atau via tab phone-link berdampingan
  2. Aplikasi menampilkan gate yang benar (spinner saat cek, app saat login, halaman login saat logout) dan logout selalu via dialog konfirmasi
  3. Seluruh 60+ preset tema web teraplikasi 1:1 (mode system/light/dark + swatch) ke semua komponen dan berganti panas tanpa restart
  4. User selalu tahu status koneksi (indikator WS online/offline + banner reconnect) dan setiap mutasi melaporkan hasil via toast

**Plans**: TBD
**UI hint**: yes
**Risks (PITFALLS.md)**: #5 QR dual-path expiry, #8 theming token pipeline (generated, sebelum semua view)

### Phase 4: Chat Sidebar + Percakapan Inti

**Goal**: User bisa mengelola daftar chat dan bercakap teks lengkap dengan aksi pesan inti secara live
**Depends on**: Phase 3
**Requirements**: CHAT-01, CHAT-02, CHAT-03, CHAT-04, CHAT-05, CHAT-06, CHAT-07, CHAT-08, CONV-01, CONV-02, CONV-03, CONV-04, CONV-05, CONV-06, CONV-07, CONV-08, CONV-09, CONV-10, CONV-11
**Success Criteria** (what must be TRUE):

  1. User melihat daftar chat (avatar, nama, pesan terakhir, waktu, badge unread) yang ter-update live tanpa flicker, bisa dicari, di-pin/archive/mute, dan toggle arsip
  2. Chat yang dibuka otomatis mark-as-read; user bisa buat grup baru, join via link, dan melihat/menjalankan history-sync (idle/running/completed/partial/failed)
  3. User membuka percakapan ter-paginasi (100 awal, scroll atas/bawah) dengan date divider dan mengirim teks optimistis `temp-` yang terganti rapi tanpa duplikat
  4. User bisa reply, edit, delete, kirim/hapus reaction (chips jumlah), dan forward ke N chat dengan label Forwarded
  5. User bisa cari dalam chat dan lompat ke pesan, melihat presence di header dan ticks receipt yang live, serta menulis dengan markdown dan emoji picker

**Plans**: TBD
**UI hint**: yes
**Risks (PITFALLS.md)**: #4 ChatStore semantics (6 aturan + unit test: sort, temp-replace, unread, flag paginasi, null-coercion, invalidate)

### Phase 5: Media + Rich Types + Group Admin

**Goal**: User bisa berkirim semua tipe pesan kaya web dan mengelola grup penuh dari info sheet
**Depends on**: Phase 4
**Requirements**: CONV-12, CONV-13, CONV-14, CONV-15, CONV-16, CONV-17, CONV-18, CONV-19, CONV-20, CONV-21, MSGT-01, MSGT-02, MSGT-03, MSGT-04, MSGT-05, MSGT-06, MSGT-07, MSGT-08, MSGT-09, MSGT-10
**Success Criteria** (what must be TRUE):

  1. User merekam dan mengirim voice note (durasi + waveform) serta memutarnya; mengirim lampiran media (image/video/document/audio/gif) dengan caption/view-once dan melihatnya di viewer modal + mengunduh via dialog native
  2. User melihat link-preview cards, memilih/mengirim/memfavoritkan sticker (termasuk animasi), dan membuat poll lalu vote/retract dengan tally live
  3. User berbagi lokasi (statis + live + caption) dan kontak (displayName, phone, vcard) serta melihat bubble-nya 1:1 web
  4. Semua bubble text/markdown, reply/quote, forwarded, deleted, dan edited ter-render 1:1 web termasuk tile view-once yang gated
  5. Admin grup mengelola anggota (add/remove/promote/demote), info grup (nama/deskripsi/locked/announce/join-approval/memberAddMode), invite link (copy/revoke), foto grup, dan leave dari info sheet

**Plans**: TBD
**UI hint**: yes
**Risks (PITFALLS.md)**: #6 media void (keputusan video go/no-go eksplisit di phase ini; audio rodio di kedua OS; view-once memory-only), #7-reuse uploaders

### Phase 6: Calls + Status + Channels

**Goal**: User bisa menelepon (suara/video, 1:1 dan grup), mem-post status, dan membaca channel seperti di web
**Depends on**: Phase 5
**Requirements**: CALL-01, CALL-02, CALL-03, CALL-04, CALL-05, CALL-06, CALL-07, STAT-01, STAT-02, STAT-03, STAT-04, CHAN-01, CHAN-02, CHAN-03
**Success Criteria** (what must be TRUE):

  1. User melihat riwayat panggilan dengan filter (direction/type/status/target) yang ter-refresh otomatis saat panggilan berakhir
  2. User membuat panggilan suara/video 1:1 dan group call (tambah/ring peserta); overlay incoming-call dan active-call muncul global di atas route mana pun
  3. Video upgrade flow (request/accept/reject/stop) dengan render video native dan jalur audio native (mic/speaker) per OS
  4. User melihat story tray per pengirim (viewed/unviewed + expiry), mem-post status teks/media dengan read receipt, serta melihat daftar channel, preview/follow/unfollow/mute, dan feed post dengan reaction

**Plans**: TBD
**UI hint**: yes
**Risks (PITFALLS.md)**: #9 call state machine (6 field + 12 status + matriks incoming-while-chatting/grup/video-upgrade/restart), #3-reuse call WS types, #6-reuse audio path; research-phase disarankan (video native, overlay always-on-top)

### Phase 7: Bot + Settings + Docs + Packaging + Audit

**Goal**: User bot-manager, pengaturan, dan dokumentasi lengkap; installer Linux + Windows lolos parity audit
**Depends on**: Phase 6
**Requirements**: BOT-01, BOT-02, BOT-03, BOT-04, BOT-05, SET-02, SET-03, SET-05, SET-06, SET-08, CORE-06
**Success Criteria** (what must be TRUE):

  1. User mengelola trigger, cron job, dan webhook (list + editor + test console + delete + delete-all) serta melihat log webhook (filter, paginasi, clear-all)
  2. User memakai AI assistant sheet di dalam editor bot (chat, pilih model, render markdown+code, apply-code ke editor)
  3. User mengatur AI (Gemini key, AI server URL) dan TTS (provider, voice, FishAudio) dengan flag masked, toggle read-receipts, mengontrol history-sync + progres, dan membaca dokumentasi dari backend
  4. Window desktop rapi per OS (rounded corners saat restored, shadow, kontrol window)
  5. Installer Linux (cargo-deb) + Windows (cargo-wix) dengan backend terbundel ter-install di clean VM (spawn + audio + dialog + drag-drop + IME) dan checklist parity route×fitur×tipe-pesan melawan backend live tanpa gap sunyi

**Plans**: TBD
**UI hint**: yes
**Risks (PITFALLS.md)**: #10 packaging per-OS (layout, system deps, per-user DB/media dirs; macOS tetap out), #11 parity audit final; #8-wiring tema picker

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 5 → 6 → 7

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Fondasi Workspace & Supervisor | 3/3 | Complete    | 2026-09-09 |
| 2. Backend-client Rust | 3/3 | Complete    | 2026-09-09 |
| 3. Shell + Theming + Auth | 3/3 | Complete    | 2026-09-09 |
| 4. Chat Sidebar + Percakapan Inti | 3/3 | Complete    | 2026-09-09 |
| 5. Media + Rich Types + Group Admin | 3/3 | Complete    | 2026-09-09 |
| 6. Calls + Status + Channels | 3/3 | Complete    | 2026-09-09 |
| 7. Bot + Settings + Docs + Packaging + Audit | 0/TBD | Not started | - |

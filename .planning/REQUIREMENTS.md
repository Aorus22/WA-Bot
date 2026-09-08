# Requirements: wa-bot desktop-gpui

**Defined:** 2026-09-08
**Core Value:** User desktop mendapat seluruh kemampuan web wa-bot dalam aplikasi native yang cepat — tanpa browser, tanpa Electron — dengan tampilan dan perilaku yang identik dengan web.

## v1 Requirements

Parity 1:1 penuh dengan web. Setiap requirement dipetakan ke tepat satu phase roadmap.

### Foundation

- [x] **CORE-01**: Workspace `desktop-gpui/` Cargo ter-build dengan pin versi eksak + Cargo.lock ter-commit (gpui-pre 0.3.3, gpui-component 0.6.0, tokio, reqwest rustls, tungstenite)
- [x] **CORE-02**: Crate `supervisor` men-spawn Go backend sebagai sidecar, handshake port, dan kill bersih tanpa orphan
- [x] **CORE-03**: Crate `settings` menyimpan preferensi UI di OS config dir (termasuk kunci tema `wa-bot-theme*`)
- [x] **CORE-04**: Crate `backend-client` menutupi seluruh permukaan `api.ts` (~70 endpoint: session, chats, messages, rich types, groups, status, channels, contacts, calls, bots, settings) + WS pump dengan reconnect dan tabel dispatch penuh
- [x] **CORE-05**: CI matrix Linux + Windows build dari fondasi (mencegah packaging gap telat)
- [ ] **CORE-06**: Packaging Linux (cargo-deb) + Windows (cargo-wix) dengan backend dibundel

### Auth & Session

- [x] **AUTH-01**: User login via QR dengan auto-refresh (~30s) dan update live via WS push
- [x] **AUTH-02**: User login via tab phone-link berdampingan dengan QR
- [x] **AUTH-03**: Aplikasi menampilkan auth gate yang benar (spinner saat cek, app saat login, halaman login saat logout)
- [x] **AUTH-04**: User logout via dialog konfirmasi (sidebar + settings)
- [x] **AUTH-05**: Client melakukan WS authenticate handshake saat koneksi terbuka

### Chat List (Sidebar)

- [x] **CHAT-01**: User melihat daftar chat (avatar, nama, pesan terakhir, waktu, badge unread)
- [x] **CHAT-02**: Daftar chat ter-update live di tempat tanpa flicker (reorder, dedupe, patch state)
- [x] **CHAT-03**: User memfilter chat via pencarian client-side
- [x] **CHAT-04**: User menyembunyikan/menampilkan chat arsip (toggle mode)
- [x] **CHAT-05**: User pin / archive / mute (off/8h/1w/forever) chat via context menu
- [x] **CHAT-06**: Chat yang dibuka otomatis mark-as-read (badge ter-clear)
- [x] **CHAT-07**: User membuat grup baru dan join grup via link dari sidebar
- [x] **CHAT-08**: User melihat status dan memulai history-sync (state machine idle/running/completed/partial/failed)

### Conversation

- [x] **CONV-01**: User membuka percakapan dengan pesan ter-paginasi (100 awal, scroll atas/bawah)
- [x] **CONV-02**: User melihat date divider dan timestamp yang terformat
- [x] **CONV-03**: User mengirim teks dengan optimistic `temp-` message yang terganti saat echo tiba
- [x] **CONV-04**: User reply, edit, dan delete pesan
- [x] **CONV-05**: User mengirim/menghapus reaction dengan chips jumlah pengirim
- [x] **CONV-06**: User meneruskan pesan ke N chat via dialog (dengan label Forwarded)
- [x] **CONV-07**: User mencari dalam chat dan lompat ke pesan dengan context load
- [x] **CONV-08**: User melihat presence lawan chat di header
- [x] **CONV-09**: User melihat read receipt / status ticks yang live via WS
- [x] **CONV-10**: User membaca pesan markdown yang ter-render dan menulis dengan mode compose markdown
- [x] **CONV-11**: User memilih emoji via picker popover
- [x] **CONV-12**: User merekam dan mengirim voice note (PTT + durasi + waveform) serta memutarnya via audio player
- [x] **CONV-13**: User mengirim lampiran media (image/video/document/audio/gif) dengan caption dan flag view-once
- [x] **CONV-14**: User melihat media di viewer modal dan mengunduh ke disk via dialog native
- [x] **CONV-15**: User melihat link-preview cards pada URL di bubble
- [x] **CONV-16**: User memilih, mengirim, dan memfavoritkan sticker (termasuk animasi)
- [x] **CONV-17**: User membuat poll, vote/retract, dan melihat tally live
- [x] **CONV-18**: User berbagi lokasi (statis + live + caption) dan melihat bubble lokasi
- [x] **CONV-19**: User berbagi kontak (displayName, phone, vcard) dan melihat bubble kontak
- [x] **CONV-20**: User membuka chat info sheet (tab media/docs/links ter-paginasi + tab anggota & pengaturan grup)
- [x] **CONV-21**: Admin grup mengelola anggota (add/remove/promote/demote), info grup (nama/deskripsi/locked/announce/join-approval/memberAddMode), invite link (copy/revoke), foto grup, dan leave

### Message Types Matrix

- [x] **MSGT-01**: Bubble text (+markdown, link preview) ter-render 1:1 web
- [x] **MSGT-02**: Bubble image/video/document/audio/ptt dengan lazy load + player + download
- [x] **MSGT-03**: Bubble sticker (statis + animasi) dengan shortcut favorit
- [x] **MSGT-04**: Bubble poll dengan tally bar dan vote toggle
- [x] **MSGT-05**: Bubble lokasi (pin card + thumbnail + buka di browser/maps)
- [x] **MSGT-06**: Bubble kontak (kartu kontak + vcard)
- [x] **MSGT-07**: Tile view-once (gated, buka-sekali)
- [x] **MSGT-08**: Bubble gif (looping)
- [x] **MSGT-09**: Bubble reply/quote dan label forwarded
- [x] **MSGT-10**: Marker deleted dan edited

### Calls

- [x] **CALL-01**: User melihat riwayat panggilan dengan filter (direction/type/status/target)
- [x] **CALL-02**: User membuat panggilan suara/video 1:1 (ringing → connected → hangup)
- [x] **CALL-03**: User membuat group call dan menambah/me-ring peserta
- [x] **CALL-04**: Overlay incoming-call dan active-call muncul global di atas route mana pun
- [x] **CALL-05**: User melakukan video upgrade flow (request/accept/reject/stop) dengan render video native
- [x] **CALL-06**: Panggilan memiliki jalur audio native (mic/speaker) per OS
- [x] **CALL-07**: Riwayat ter-refresh otomatis saat panggilan berakhir

### Status

- [x] **STAT-01**: User melihat story tray yang dikelompokkan per pengirim (viewed/unviewed + expiry)
- [x] **STAT-02**: User mem-post status teks (teks + background ARGB)
- [x] **STAT-03**: User mem-post status image/video dengan caption
- [x] **STAT-04**: Status yang dilihat terkirim read receipt dan media ter-fetch on-demand

### Channels

- [x] **CHAN-01**: User melihat daftar channel yang di-follow (nama, deskripsi, subscriber, avatar, mute, verified)
- [x] **CHAN-02**: User preview via invite link lalu follow/unfollow/mute
- [x] **CHAN-03**: User membaca feed post channel ter-paginasi dan memberi/menghapus reaction

### Bot Management

- [ ] **BOT-01**: User mengelola trigger (list + editor: nama, regex, script, prioritas, aktif, deskripsi + test console + delete + delete-all)
- [ ] **BOT-02**: User mengelola cron job (list + editor: nama, ekspresi jadwal, script, aktif + test + delete-all)
- [ ] **BOT-03**: User mengelola webhook (list + editor: nama, path, script, secret, aktif + test + delete-all)
- [ ] **BOT-04**: User melihat log webhook (filter per webhook, paginasi, total, clear-all)
- [ ] **BOT-05**: User memakai AI assistant sheet di dalam editor bot (chat, pilih model, render markdown+code, apply-code ke editor)

### Settings / Theming / Docs

- [x] **SET-01**: Tema 1:1 web (mode system/light/dark + swatch `themes.ts`) teraplikasi ke seluruh komponen
- [ ] **SET-02**: User mengatur AI (Gemini key, AI server URL) dan TTS (provider, voice, FishAudio key/model/voice) dengan flag masked
- [ ] **SET-03**: User toggle read-receipts
- [x] **SET-04**: User melihat indikator koneksi (WS online/offline)
- [ ] **SET-05**: User mengontrol history-sync + melihat progres (state, counts, errors)
- [ ] **SET-06**: User membaca halaman dokumentasi (markdown dari backend `/docs`)
- [x] **SET-07**: Setiap mutasi melaporkan hasil via sistem toast
- [ ] **SET-08**: Window desktop rapi per OS (rounded corners saat restored, shadow, kontrol window)

## v2 Requirements

Native value-adds setelah parity tervalidasi. Tidak di roadmap milestone ini.

### Native Desktop

- **NATV-01**: Notifikasi native (pesan + incoming call) dengan click-to-focus
- **NATV-02**: System-tray minimize + autostart + deep-link `wabot://chat/:id`
- **NATV-03**: Hotkey global push-to-talk/mute dan media-key handling
- **NATV-04**: Local message-search index (SQLite FTS), pencarian server tetap fallback
- **NATV-05**: Multi-window (pop-out chat / call window)

## Out of Scope

| Feature | Reason |
|---------|--------|
| Rewrite backend Go ke Rust | Keputusan milestone: backend dipakai ulang apa adanya; rewrite merusak client lain yang berbagi backend |
| Build/packaging macOS | Ditunda ke milestone berikut (fokus Linux + Windows) |
| Fitur baru tanpa padanan web | Milestone parity dinilai 1:1; fitur novel bikin spec drift dan maintenance ganda |
| Enkripsi E2E / key management di client | Kripto protokol WA ada di server (whatsmeow); pola `secret` pass-through dipertahankan seperti web |
| Full local sync menggantikan paginasi server | Paginasi server by design; bulk path satu-satunya adalah history-sync |
| Embed browser/webview untuk "reuse" UI web | Mengalahkan tujuan milestone (native, tanpa Electron) |
| Evaluasi cron/price-schedule di client | Semantik cron milik backend Go; duplikasi menyebabkan double-fire dan bug timezone |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| CORE-01 | Phase 1 | Complete |
| CORE-02 | Phase 1 | Complete |
| CORE-03 | Phase 1 | Complete |
| CORE-04 | Phase 2 | Complete |
| CORE-05 | Phase 1 | Complete |
| CORE-06 | Phase 7 | Pending |
| AUTH-01 | Phase 3 | Complete |
| AUTH-02 | Phase 3 | Complete |
| AUTH-03 | Phase 3 | Complete |
| AUTH-04 | Phase 3 | Complete |
| AUTH-05 | Phase 2 | Complete |
| CHAT-01 | Phase 4 | Complete |
| CHAT-02 | Phase 4 | Complete |
| CHAT-03 | Phase 4 | Complete |
| CHAT-04 | Phase 4 | Complete |
| CHAT-05 | Phase 4 | Complete |
| CHAT-06 | Phase 4 | Complete |
| CHAT-07 | Phase 4 | Complete |
| CHAT-08 | Phase 4 | Complete |
| CONV-01 | Phase 4 | Complete |
| CONV-02 | Phase 4 | Complete |
| CONV-03 | Phase 4 | Complete |
| CONV-04 | Phase 4 | Complete |
| CONV-05 | Phase 4 | Complete |
| CONV-06 | Phase 4 | Complete |
| CONV-07 | Phase 4 | Complete |
| CONV-08 | Phase 4 | Complete |
| CONV-09 | Phase 4 | Complete |
| CONV-10 | Phase 4 | Complete |
| CONV-11 | Phase 4 | Complete |
| CONV-12 | Phase 5 | Complete |
| CONV-13 | Phase 5 | Complete |
| CONV-14 | Phase 5 | Complete |
| CONV-15 | Phase 5 | Complete |
| CONV-16 | Phase 5 | Complete |
| CONV-17 | Phase 5 | Complete |
| CONV-18 | Phase 5 | Complete |
| CONV-19 | Phase 5 | Complete |
| CONV-20 | Phase 5 | Complete |
| CONV-21 | Phase 5 | Complete |
| MSGT-01 | Phase 5 | Complete |
| MSGT-02 | Phase 5 | Complete |
| MSGT-03 | Phase 5 | Complete |
| MSGT-04 | Phase 5 | Complete |
| MSGT-05 | Phase 5 | Complete |
| MSGT-06 | Phase 5 | Complete |
| MSGT-07 | Phase 5 | Complete |
| MSGT-08 | Phase 5 | Complete |
| MSGT-09 | Phase 5 | Complete |
| MSGT-10 | Phase 5 | Complete |
| CALL-01 | Phase 6 | Complete |
| CALL-02 | Phase 6 | Complete |
| CALL-03 | Phase 6 | Complete |
| CALL-04 | Phase 6 | Complete |
| CALL-05 | Phase 6 | Complete |
| CALL-06 | Phase 6 | Complete |
| CALL-07 | Phase 6 | Complete |
| STAT-01 | Phase 6 | Complete |
| STAT-02 | Phase 6 | Complete |
| STAT-03 | Phase 6 | Complete |
| STAT-04 | Phase 6 | Complete |
| CHAN-01 | Phase 6 | Complete |
| CHAN-02 | Phase 6 | Complete |
| CHAN-03 | Phase 6 | Complete |
| BOT-01 | Phase 7 | Pending |
| BOT-02 | Phase 7 | Pending |
| BOT-03 | Phase 7 | Pending |
| BOT-04 | Phase 7 | Pending |
| BOT-05 | Phase 7 | Pending |
| SET-01 | Phase 3 | Complete |
| SET-02 | Phase 7 | Pending |
| SET-03 | Phase 7 | Pending |
| SET-04 | Phase 3 | Complete |
| SET-05 | Phase 7 | Pending |
| SET-06 | Phase 7 | Pending |
| SET-07 | Phase 3 | Complete |
| SET-08 | Phase 7 | Pending |

**Coverage:**

- v1 requirements: 77 total
- Mapped to phases: 77
- Unmapped: 0 ✓

---
*Requirements defined: 2026-09-08*
*Last updated: 2026-09-08 after initial definition*

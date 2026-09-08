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

- [ ] **CHAT-01**: User melihat daftar chat (avatar, nama, pesan terakhir, waktu, badge unread)
- [ ] **CHAT-02**: Daftar chat ter-update live di tempat tanpa flicker (reorder, dedupe, patch state)
- [ ] **CHAT-03**: User memfilter chat via pencarian client-side
- [ ] **CHAT-04**: User menyembunyikan/menampilkan chat arsip (toggle mode)
- [ ] **CHAT-05**: User pin / archive / mute (off/8h/1w/forever) chat via context menu
- [ ] **CHAT-06**: Chat yang dibuka otomatis mark-as-read (badge ter-clear)
- [ ] **CHAT-07**: User membuat grup baru dan join grup via link dari sidebar
- [ ] **CHAT-08**: User melihat status dan memulai history-sync (state machine idle/running/completed/partial/failed)

### Conversation

- [ ] **CONV-01**: User membuka percakapan dengan pesan ter-paginasi (100 awal, scroll atas/bawah)
- [ ] **CONV-02**: User melihat date divider dan timestamp yang terformat
- [ ] **CONV-03**: User mengirim teks dengan optimistic `temp-` message yang terganti saat echo tiba
- [ ] **CONV-04**: User reply, edit, dan delete pesan
- [ ] **CONV-05**: User mengirim/menghapus reaction dengan chips jumlah pengirim
- [ ] **CONV-06**: User meneruskan pesan ke N chat via dialog (dengan label Forwarded)
- [ ] **CONV-07**: User mencari dalam chat dan lompat ke pesan dengan context load
- [ ] **CONV-08**: User melihat presence lawan chat di header
- [ ] **CONV-09**: User melihat read receipt / status ticks yang live via WS
- [ ] **CONV-10**: User membaca pesan markdown yang ter-render dan menulis dengan mode compose markdown
- [ ] **CONV-11**: User memilih emoji via picker popover
- [ ] **CONV-12**: User merekam dan mengirim voice note (PTT + durasi + waveform) serta memutarnya via audio player
- [ ] **CONV-13**: User mengirim lampiran media (image/video/document/audio/gif) dengan caption dan flag view-once
- [ ] **CONV-14**: User melihat media di viewer modal dan mengunduh ke disk via dialog native
- [ ] **CONV-15**: User melihat link-preview cards pada URL di bubble
- [ ] **CONV-16**: User memilih, mengirim, dan memfavoritkan sticker (termasuk animasi)
- [ ] **CONV-17**: User membuat poll, vote/retract, dan melihat tally live
- [ ] **CONV-18**: User berbagi lokasi (statis + live + caption) dan melihat bubble lokasi
- [ ] **CONV-19**: User berbagi kontak (displayName, phone, vcard) dan melihat bubble kontak
- [ ] **CONV-20**: User membuka chat info sheet (tab media/docs/links ter-paginasi + tab anggota & pengaturan grup)
- [ ] **CONV-21**: Admin grup mengelola anggota (add/remove/promote/demote), info grup (nama/deskripsi/locked/announce/join-approval/memberAddMode), invite link (copy/revoke), foto grup, dan leave

### Message Types Matrix

- [ ] **MSGT-01**: Bubble text (+markdown, link preview) ter-render 1:1 web
- [ ] **MSGT-02**: Bubble image/video/document/audio/ptt dengan lazy load + player + download
- [ ] **MSGT-03**: Bubble sticker (statis + animasi) dengan shortcut favorit
- [ ] **MSGT-04**: Bubble poll dengan tally bar dan vote toggle
- [ ] **MSGT-05**: Bubble lokasi (pin card + thumbnail + buka di browser/maps)
- [ ] **MSGT-06**: Bubble kontak (kartu kontak + vcard)
- [ ] **MSGT-07**: Tile view-once (gated, buka-sekali)
- [ ] **MSGT-08**: Bubble gif (looping)
- [ ] **MSGT-09**: Bubble reply/quote dan label forwarded
- [ ] **MSGT-10**: Marker deleted dan edited

### Calls

- [ ] **CALL-01**: User melihat riwayat panggilan dengan filter (direction/type/status/target)
- [ ] **CALL-02**: User membuat panggilan suara/video 1:1 (ringing → connected → hangup)
- [ ] **CALL-03**: User membuat group call dan menambah/me-ring peserta
- [ ] **CALL-04**: Overlay incoming-call dan active-call muncul global di atas route mana pun
- [ ] **CALL-05**: User melakukan video upgrade flow (request/accept/reject/stop) dengan render video native
- [ ] **CALL-06**: Panggilan memiliki jalur audio native (mic/speaker) per OS
- [ ] **CALL-07**: Riwayat ter-refresh otomatis saat panggilan berakhir

### Status

- [ ] **STAT-01**: User melihat story tray yang dikelompokkan per pengirim (viewed/unviewed + expiry)
- [ ] **STAT-02**: User mem-post status teks (teks + background ARGB)
- [ ] **STAT-03**: User mem-post status image/video dengan caption
- [ ] **STAT-04**: Status yang dilihat terkirim read receipt dan media ter-fetch on-demand

### Channels

- [ ] **CHAN-01**: User melihat daftar channel yang di-follow (nama, deskripsi, subscriber, avatar, mute, verified)
- [ ] **CHAN-02**: User preview via invite link lalu follow/unfollow/mute
- [ ] **CHAN-03**: User membaca feed post channel ter-paginasi dan memberi/menghapus reaction

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
| CHAT-01 | Phase 4 | Pending |
| CHAT-02 | Phase 4 | Pending |
| CHAT-03 | Phase 4 | Pending |
| CHAT-04 | Phase 4 | Pending |
| CHAT-05 | Phase 4 | Pending |
| CHAT-06 | Phase 4 | Pending |
| CHAT-07 | Phase 4 | Pending |
| CHAT-08 | Phase 4 | Pending |
| CONV-01 | Phase 4 | Pending |
| CONV-02 | Phase 4 | Pending |
| CONV-03 | Phase 4 | Pending |
| CONV-04 | Phase 4 | Pending |
| CONV-05 | Phase 4 | Pending |
| CONV-06 | Phase 4 | Pending |
| CONV-07 | Phase 4 | Pending |
| CONV-08 | Phase 4 | Pending |
| CONV-09 | Phase 4 | Pending |
| CONV-10 | Phase 4 | Pending |
| CONV-11 | Phase 4 | Pending |
| CONV-12 | Phase 5 | Pending |
| CONV-13 | Phase 5 | Pending |
| CONV-14 | Phase 5 | Pending |
| CONV-15 | Phase 5 | Pending |
| CONV-16 | Phase 5 | Pending |
| CONV-17 | Phase 5 | Pending |
| CONV-18 | Phase 5 | Pending |
| CONV-19 | Phase 5 | Pending |
| CONV-20 | Phase 5 | Pending |
| CONV-21 | Phase 5 | Pending |
| MSGT-01 | Phase 5 | Pending |
| MSGT-02 | Phase 5 | Pending |
| MSGT-03 | Phase 5 | Pending |
| MSGT-04 | Phase 5 | Pending |
| MSGT-05 | Phase 5 | Pending |
| MSGT-06 | Phase 5 | Pending |
| MSGT-07 | Phase 5 | Pending |
| MSGT-08 | Phase 5 | Pending |
| MSGT-09 | Phase 5 | Pending |
| MSGT-10 | Phase 5 | Pending |
| CALL-01 | Phase 6 | Pending |
| CALL-02 | Phase 6 | Pending |
| CALL-03 | Phase 6 | Pending |
| CALL-04 | Phase 6 | Pending |
| CALL-05 | Phase 6 | Pending |
| CALL-06 | Phase 6 | Pending |
| CALL-07 | Phase 6 | Pending |
| STAT-01 | Phase 6 | Pending |
| STAT-02 | Phase 6 | Pending |
| STAT-03 | Phase 6 | Pending |
| STAT-04 | Phase 6 | Pending |
| CHAN-01 | Phase 6 | Pending |
| CHAN-02 | Phase 6 | Pending |
| CHAN-03 | Phase 6 | Pending |
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

# HANDOFF — QML desktop port (branch `qml`, dari `main`)

Ditulis saat sesi Claude kena rate limit (reset 20:40 WIB). Semua 9 sub-agen mati di tengah jalan; pekerjaan
mereka tersimpan di working tree (di-commit sebagai WIP). Dokumen ini = cara melanjutkan.
Spec: `../QML-DESKTOP-PLAN.md` (rencana asli, belum di-commit), aturan teknis: `CONVENTIONS.md` (WAJIB dibaca dulu).

## Status ringkas
| Bagian | Status |
|---|---|
| Phase 1: backend discovery/spawn/owner, REST (`Api`), WS (`EventHub`), tes | **selesai**, 18 tes hijau (butuh `wa-bot-backend` di repo root untuk 6 tes lifecycle) |
| Fondasi: tema 83 preset, ikon Lucide, shell (titlebar, nav rail, router `PageHost`, toast), atom `qml/ui/*`, mock backend, `scripts/shot.py` | **selesai & di-commit** (`fbd41dc`, `+ mock fixtures`) |
| 9 area fitur | **parsial, belum diverifikasi** — lihat tabel di bawah |
| Integrasi, parity matrix lengkap, benchmark, packaging, README final | belum |

Hasil tes terakhir pada kode parsial (tanpa `test_bridge.py`/`test_backend_lifecycle.py`): 43 lulus, 1 gagal
(`tests/test_status_channels.py::test_status_list_rings_and_labels`: grup status "sendiri" belum terpisah → 3 ≠ 4).
Tes lain di area itu belum dijalankan setelah `-x` berhenti — jalankan ulang tanpa `-x`.

## Cara kerja (singkat)
- Python 3.14 + PySide6 6.11 sistem; venv: `python3 -m venv --system-site-packages .venv && .venv/bin/pip install pytest segno`.
- **Jangan pakai `qmlRegister*`/QML `pragma Singleton`** — di build ini merusak item bersarang / singleton tak bisa baca context property.
  Semua objek Python → QML lewat `services.expose("Nama", obj)` (context property). Tema = `Theme.*` (Python `bridge/theme.py`).
- Modul fitur: `bridge/<nama>.py` dengan `register(services)`, daftar di `bridge/registry.py::FEATURE_MODULES`.
- Halaman: `qml/pages/<area>/`, tabel route di `qml/shell/PageHost.qml`.
- Tes tanpa akun WA: `tests/mock_backend.py` (+ `mock_<area>.py`, fixture bersama `mock_common.py`).
- Tidak ada display (tty): pakai `QT_QPA_PLATFORM=offscreen`, `scripts/shot.py OUT.png --route chat --url <mock url>` lalu lihat PNG.
- **Jangan sentuh data nyata**: backend AppImage user (`~/.wa-bot/run/core.json`), `~/.wa-bot`, `~/.config/wa-bot-desktop/settings.json`.
  Jangan kill proses `wa-bot*` yang bukan milik sendiri. Tes real-backend pakai `WABOT_HOME=<tmp>`.
- Referensi GPUI: `../desktop-gpui/crates/app/src` (+ `crates/backend-client` untuk REST/WS). Halaman bot (Triggers/Cron/Webhooks/Docs) di GPUI
  hanya placeholder → spec-nya app React di `../web/src/pages`.
- Panggilan: kode media call (protokol biner) hanya ada di branch `call` (`git show call:docs/CALL-MEDIA-PROTOCOL.md`,
  `git show call:desktop-gpui/crates/app/src/call_media/...`); `main` hanya punya signaling REST/WS. Jangan checkout/merge branch lain di tree ini.

## Pembagian area & apa yang sudah ada (belum diverifikasi, baca sendiri kodenya)
Tiap agen diminta menulis `parity/<area>.md` — **belum ada satupun** (`parity/` kosong). Prompt asli tiap area ada di bagian bawah ringkas.

| Area | File yang sudah ada | Yang masih kurang (perkiraan) |
|---|---|---|
| auth | `bridge/auth.py`, `qml/pages/auth/*` (LoginPage, QrPanel, PhoneTab, StartupGate, …), `tests/mock_auth.py`; dep `segno` ditambah ke pyproject; ada `zxing_cpp*.whl` nyasar di `desktop-qml/` (di-.gitignore, hapus saja) | `tests/test_auth.py`, screenshot+verifikasi visual, `parity/auth.md`, hubungkan ke `Main.qml` (diedit agen) |
| chat-list | `bridge/{chats,chatlist,chatfmt,listsync}.py`, `qml/ui/Avatar.qml`, `tests/mock_chats.py` | seluruh QML: `ChatPage`, `sidebar/*`, `ChatHeader`, `sheets/*`, `dialogs/*`, `ConversationPane` (masih stub), tes, parity |
| chat-messages | `bridge/{messages,messages_text}.py`, `qml/pages/chat/messages/*` | `MessageList.qml` (stub), viewer, selection bar, tes, parity |
| composer | `bridge/{composer,composer_upload,composer_util,composer_voice,upload}.py`, `tests/mock_composer.py` | seluruh QML `composer/*` (stub), tes, parity |
| status+channels | `bridge/{statuses,channels}.py`, `tests/mock_{status,channels}.py`, `tests/test_status_channels.py` (1 gagal) | seluruh QML `pages/status`, `pages/channels` (stub), parity |
| calls | `bridge/call_media/*` (mulai: permissions, audio) | `bridge/calls.py`, QML `CallsPage`/`CallOverlay` (stub), video, tes, parity |
| bot admin | `bridge/{botadmin,mdblocks}.py`, `tests/mock_bot.py`, `tests/test_botadmin.py` | QML `pages/bot/*` (stub), parity |
| settings | `bridge/{system,gtk_theme,history_text}.py`; edit di `bridge/settings.py`, `bridge/theme.py` | QML `SettingsPage` (stub, halaman terbesar), tes, parity |
| platform | `bridge/platform_core.py`, `tests/platform_fakes/`; edit kecil di `bridge/backend.py`, `bridge/discovery.py`, `qml/Main.qml` | `bridge/platform.py` (modul yang di-load registry), tray, notifikasi, single-instance, install/bench scripts, `docs/QML-DESKTOP.md`, tes, parity |

Hampir semua area: **Python bridge sudah mulai, QML halaman belum** (kecuali auth). Prioritas yang disarankan:
1. Rapikan & tes yang sudah ada (`pytest` tanpa `-x`, perbaiki yang merah).
2. Chat inti dulu (chat-list → messages → composer) karena paling sering dipakai; lalu auth → settings → status/channels → bot → calls → platform.
3. Setelah tiap area: screenshot offscreen, baca PNG, bandingkan dengan sumber GPUI; tulis `parity/<area>.md` jujur (done/partial/belum + alasan).
4. Terakhir: integrasi penuh, `PARITY.md` di-update, benchmark (`scripts/bench.py`), `docs/QML-DESKTOP.md`, README.

## Batasan yang HARUS dijujurkan di laporan akhir
- Tidak ada display di sesi ini → belum ada verifikasi di jendela asli (Wayland/GNOME/COSMIC), hanya render offscreen.
- Tidak ada screenshot baseline GPUI → "pixel parity" hanya berdasar angka di source, belum dibandingkan gambar.
- Panggilan suara/video, mikrofon, kamera, notifikasi D-Bus, tray, layer-shell: tidak bisa diuji tanpa perangkat/akun.
- Tidak ada klaim performa/RAM lebih rendah tanpa pengukuran (belum ada benchmark).

## Melanjutkan sub-agen lama (opsional, setelah 20:40 WIB)
Agen yang berhenti bisa dilanjutkan dengan `SendMessage` ke id-nya; tapi konteksnya hilang kalau sesi ini ditutup. Lebih aman mulai agen baru
dengan prompt: "baca `desktop-qml/HANDOFF.md`, `CONVENTIONS.md`, lanjutkan area X dari kode yang sudah ada".
Template prompt per area (dipakai sebelumnya) intinya: *area, file referensi GPUI, kontrak `Context property`, file yang dimiliki, tes via mock,
screenshot offscreen, tulis `parity/<area>.md`, jangan jalankan git yang mengubah tree*.

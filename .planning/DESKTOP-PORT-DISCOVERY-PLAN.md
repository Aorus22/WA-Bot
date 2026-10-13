# Plan: Sederhanakan Port Discovery GPUI

**Status:** IMPLEMENTED — logika di `desktop-gpui/crates/app/src/backend.rs`; diverifikasi lewat unit test (tanpa menjalankan UI).

## Tujuan

Ganti penyimpanan `last_backend_url` dari `~/.config/wa-bot-desktop/settings.json` menjadi file runtime di folder data WA-Bot yang **sudah ada**:

`~/.wa-bot/run/core.json`

Gunakan `backend::data_dir()` agar `WABOT_HOME` tetap didukung. **Tidak perlu sistem service discovery yang rumit.** Go sidecar sudah menerima `PORT=:0` dan mengirim `BACKEND_PORT:<port>` ke supervisor Rust; mekanisme itu **tetap dipakai**.

## Format `core.json`

```json
{
  "schema_version": 1,
  "name": "aorus22.wabot-core",
  "pid": 12345,
  "port": 45321,
  "instance_id": "550e8400-e29b-41d4-a716-446655440000",
  "started_at": "2026-10-13T07:00:00Z"
}
```

`schema_version` untuk kompatibilitas format; `name` adalah identitas logis (tidak perlu rename executable); `pid` adalah PID backend Go, bukan GPUI; `port` dari handshake; `instance_id` UUID acak per spawn untuk mencegah instance lama menghapus registry instance baru; `started_at` timestamp UTC untuk informasi. `instance_id` **bukan autentikasi backend**: tetap validasi PID, port pemilik socket, dan endpoint. Jangan simpan secret di file ini.

## Yang perlu dikerjakan

- [x] Tambahkan helper kecil di sisi Rust untuk **read / atomic write / conditional delete** `<data_dir>/run/core.json`. Buat `run/` bila belum ada; jangan membuat database/folder data baru di luar lokasi existing.
- [x] Saat GPUI start **tanpa override eksplisit**, baca `core.json` terlebih dahulu. Jika PID masih hidup, merupakan `wa-bot-backend`, memiliki listening port tersebut, dan `GET /api/status` memberi respons WA-Bot yang valid, **reconnect tanpa spawn**.
- [x] Jika file tidak ada, rusak, atau backend yang tercatat sudah mati, gunakan supervisor lama untuk spawn `wa-bot-backend` dengan `PORT=:0`. Setelah handshake + readiness sukses, tulis seluruh metadata ke `core.json` secara atomik, termasuk `instance_id` baru dan `started_at`.
- [x] Jika PID/port ambigu atau ada indikasi backend lain masih mengakses data yang sama, **jangan kill ataupun adopt proses yang tak terverifikasi**. Kembalikan error yang jelas daripada berisiko menggandakan sesi SQLite.
- [x] Hilangkan pencarian otomatis lewat `PROBE_PORTS = [3090, 3000, 8080]` dan **jangan gunakan `last_backend_url` lama sebagai fallback tanpa validasi**. Field lama di `settings.json` boleh dibiarkan demi kompatibilitas, tetapi bukan lagi sumber discovery.
- [x] Pada shutdown normal, backend milik GPUI tetap distop oleh supervisor lama. Hapus `core.json` **hanya jika** `instance_id`, PID, dan port di file masih cocok dengan backend yang dihentikan; ketika window ditutup ke tray, jangan hapus.
- [x] Pada restart backend, ambil port baru dari handshake, update `core.json`, dan pastikan HTTP/WebSocket client GPUI menggunakan URL/port terbaru (termasuk reconnect WS).
- [x] Pertahankan prioritas override eksplisit CLI (`--url`, `--backend-url`, `--port`, `-p`) dan env (`BACKEND_URL`, `BACKEND_PORT`); mode override tidak mengubah `core.json`.

## File yang kemungkinan disentuh

- `desktop-gpui/crates/app/src/backend.rs` — discovery, penyimpanan port setelah spawn, restart/shutdown.
- `desktop-gpui/crates/app/src/main.rs` — memilih URL backend tanpa probe hardcoded atau fallback stale.
- `desktop-gpui/crates/app/src/backend_process.rs` — **reuse** validasi PID dan port yang sudah ada.
- `desktop-gpui/crates/supervisor/` — hanya bila helper registry lebih rapi ditempatkan di sini.

## Yang **tidak** diubah

Docker, `.env`, Go backend, HTTP API, WebSocket protocol, SQLite, build/packaging, serta implementasi FFI. Jangan refactor besar, menambah daemon, mengganti nama binary, atau membuat mekanisme lock/registry kompleks tanpa kebutuhan nyata.

## Pengecekan sederhana

- [x] First launch → spawn dengan port otomatis dan tulis `core.json`.
- [x] Backend dari registry masih hidup → reconnect, tidak spawn kedua.
- [x] Backend mati / registry stale → spawn baru dan perbarui file.
- [x] Port sudah dipakai proses lain → jangan salah adopt/kill.
- [x] Restart → port baru tersimpan dan HTTP/WS berpindah.
- [x] Quit → registry milik backend yang distop dibersihkan; close-to-tray tetap berjalan.
- [x] Override explicit dan Docker tetap berperilaku seperti sebelumnya.

**Untuk agent:** implementasi cukup di Rust/GPUI, gunakan mekanisme supervisor yang sudah ada, tandai checklist selesai. Tidak perlu menjalankan UI secara manual; verifikasi melalui kode/unit test yang relevan.

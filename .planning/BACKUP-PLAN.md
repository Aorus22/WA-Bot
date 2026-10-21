# WA-Bot: Backup ke Cloud (Google Drive / folder lokal / GitHub)

Status: TAHAP 1 selesai di `develop`; berikutnya tahap 2.
Lokasi: /home/aorus/Project/WA-Bot
Cakupan: backend Go (mesin backup), desktop gpui (Settings), target yang bisa ditukar.

## 1. Tujuan

Backup chat + media per akun ke tempat penyimpanan di luar mesin, dengan jaminan:
yang tersimpan di target tidak mungkin korup tanpa terdeteksi, dan kerusakan satu
bagian tidak pernah merusak seluruh backup (pelajaran dari backup WA asli yang
satu file rusak = semua hilang).

### Keputusan

- [x] Sesi WA (`database/wa-bot-session.db`) TIDAK di-backup. Setelah restore: scan QR ulang.
- [x] Tanpa enkripsi. Chunk diberi nama sha256 isinya.
- [x] Target diabstraksi (interface `Target`): `LocalFolder` dulu, lalu Google Drive, GitHub opsional.
- [x] Isi backup per akun: `accounts/<hp>/database/wa-bot-messages.db` (chat),
      `accounts/<hp>/database/objectstorage.sqlite` (media), plus `database/wa-bot-app.db` (pengaturan app).

## 2. Format di target

```
chunks/<2 hex>/<sha256>                 bytes tersimpan (zstd untuk segmen tabel, mentah untuk media)
manifests/<akun>/<id>-<sha256>.json     satu per backup, lengkap (bukan selisih)
manifests-copy/<akun>/<id>-<sha256>.json salinan kedua
```

- Nama manifest memuat sha256 isinya sendiri: manifest yang rusak langsung ketahuan.
- Tabel SQLite dipecah jadi segmen independen. Tabel yang punya `chat_id` + `timestamp`
  dipecah per chat per bulan (maks 5000 baris per segmen); tabel lain per 5000 baris
  menurut primary key. Satu segmen rusak = hanya bagian itu yang hilang.
- Baris disimpan bertipe (`null/integer/real/text/blob`) dan dimasukkan ulang lewat bind
  parameter, jadi isi backup tidak pernah dieksekusi sebagai SQL. Skema (`sqlite_master`)
  dijalankan di DB baru dengan authorizer yang menolak ATTACH.
- Media: satu objek = satu chunk (objek > 32 MB dipecah per 32 MB). Nama chunk = sha256
  yang sudah tercatat di objectstorage, jadi dedup antar-backup gratis.

## 3. Alur backup

1. Snapshot `wa-bot-messages.db` dan `wa-bot-app.db` dengan `VACUUM INTO` (aman saat app jalan).
2. Verifikasi snapshot: `integrity_check`, `foreign_key_check`. Media: `integrity_check` DB
   media, lalu tiap objek dibaca dan di-hash ulang saat di-upload.
3. Segmen/objek yang chunk-nya sudah ada di target dilewati (dedup).
4. Upload chunk (atomik per file), cek checksum yang dikembalikan target.
5. Manifest ditulis PALING AKHIR (dua salinan) = titik commit. Gagal di tengah = tidak ada
   manifest = backup tidak dianggap ada; chunk yatim dibersihkan prune.
6. Prune: simpan N manifest terbaru per akun, hapus chunk yang tidak dirujuk dan berumur > 24 jam.

## 4. Alur restore

1. Pilih manifest (cek salinan; kalau keduanya rusak, pakai generasi sebelumnya).
2. Download + cek sha256 tiap chunk. Gagal → coba lagi → (tahap 2: parity) → catat hilang, LANJUT.
3. Bangun DB baru di folder staging: skema dari manifest, baris dari segmen, media lewat
   `SQLiteStore.Put`.
4. Verifikasi: `integrity_check`, jumlah baris dan hash isi per tabel dibandingkan manifest.
5. (Tahap 2) Tukar ke DB aktif: backend dihentikan, DB lama → `*.before-restore`, rename atomik.
6. Laporan: apa yang dipulihkan, apa yang hilang.

## 5. Tahapan

### Tahap 1: mesin backup + LocalFolder
- [x] Paket `internal/infrastructure/backup`: Target, LocalFolder, manifest, segmen tabel, media.
- [x] Backup, restore ke folder staging, check (verifikasi isi target), prune.
- [x] CLI `wa-bot-backend backup run|list|restore|check|prune`.
- [x] Tes: round-trip, dedup (backup kedua tanpa perubahan tidak upload apa pun),
      chunk rusak/hilang → restore parsial jalan, manifest rusak → pakai salinan/generasi lama,
      DB sumber rusak → ditolak, upload putus → tidak ada backup setengah jadi, prune.
- [x] Uji data asli 2026-10-19 (app sedang jalan): 147.371 baris + 2.861 media (2,1 GB) →
      1,3 GB di target (media kembar disimpan sekali), 50 detik. Backup kedua: 0 chunk, 5 detik.
      `check --deep` ok. Restore 1,5 menit, isi identik dengan DB asli (dicek terpisah via sqlite3).

### Tahap 2: parity + restore ke DB aktif
- [ ] Parity Reed-Solomon per grup chunk, perbaikan otomatis saat restore/check.
- [ ] Mode Gabung (default) dan Ganti total; tukar DB lewat supervisor; tombol batal 7 hari.

### Tahap 3: Google Drive
- [ ] OAuth desktop (PKCE, loopback), scope `drive.appdata`, token di keyring OS.
- [ ] Resumable upload, cek `sha256Checksum`, cek kuota.

### Tahap 4: UI Settings (desktop-gpui)
- [ ] Pilih target, Connect, Backup sekarang + progres, daftar backup, restore + laporan.

### Tahap 5: jadwal + scrub
- [ ] Backup terjadwal, scrub berkala (download sampel bergilir, upload ulang chunk rusak).

### Tahap 6 (opsional): target GitHub (Git Data API, update ref atomik)

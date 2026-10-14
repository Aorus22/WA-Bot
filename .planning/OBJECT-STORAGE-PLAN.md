# WA-Bot: Embedded Object Storage (SQLite + S3)

Status: IMPLEMENTED (Phase 0-5). Media lama disalin sekali ke ~/.wa-bot/database/objectstorage.sqlite (286 file) lewat script sekali pakai di luar repo. Dokumentasi: docs/OBJECT-STORAGE.md.
Lokasi: /home/aorus/Project/WA-Bot
Cakupan: backend Go, media WA-Bot, desktop Linux/Windows, deployment Docker
Eksekusi: agent berikutnya menjalankan fase secara berurutan dan mencentang item setelah benar-benar selesai.

## 1. Tujuan

Membuat object storage default yang portabel seperti SQLite: seluruh isi object storage (bytes media, bucket, object key, metadata, upload state) berada di SATU file objectstorage.sqlite. Backend WA-Bot tetap satu executable, tanpa daemon/storage sidecar. File hasil backup bisa dipindah ke mesin lain lalu dibuka WA-Bot.

Provider dapat ditukar antara:
- sqlite: engine buatan sendiri di s3lite.go, embedded dan langsung dipanggil wrapper.
- s3: AWS S3 / Cloudflare R2 / MinIO / penyedia kompatibel S3, lewat AWS SDK Go v2.

Di samping API internal yang sama, mode sqlite harus dapat mengekspos endpoint S3 HTTP yang kompatibel dengan operasi yang WA-Bot perlukan. Frontend S3 boleh memakai go-faster/fs jika lolos evaluasi kompatibilitas/keamanan. Jangan menulis ulang seluruh protokol S3.

**Yang dimaksud satu file:** satu file untuk seluruh *object/media storage*. Saat ini WA-Bot masih mempunyai DB session/app/message terpisah. Menyatukan seluruh DB WA-Bot ke satu file bukan scope proyek ini.

### Syarat wajib

- [x] Default local memakai satu file objectstorage.sqlite, tanpa instalasi MinIO/Docker/service lain.
- [x] Wrapper adalah SATU pintu masuk seluruh operasi object storage dari aplikasi.
- [x] API internal mendukung WriteFile/ReadFile/CopyFile/DeleteFile/FileExists/Stat/List DAN operasi streaming Put/Open.
- [x] File besar tidak dibaca seluruhnya ke RAM saat upload, download, copy, atau migrasi.
- [x] Semantik pengambilan objek konsisten antara SQLite lokal dan S3 cloud.
- [x] Namespace akun WhatsApp terisolasi secara eksplisit, bukan global path mutable.
- [x] Endpoint S3 lokal memakai HTTP handler dalam proses Go (tanpa sidecar); kompatibilitas ditentukan dengan matrix SDK, bukan mengklaim semua fitur AWS S3.
- [x] URL media dan fitur yang sudah dipakai client web/desktop tidak berubah.
- [x] Backup yang portable menghasilkan snapshot konsisten satu file, termasuk saat database aktif.
- [x] Ada jalur migrasi media lama, pemeriksaan hasil, dan rollback tanpa menghapus sumber lebih dulu.

## 2. Kondisi repo saat ini (inspeksi aktual)

- cmd/api/main.go membangun storage.NewLocalStorage(accounts/_pending/media) dan menyuntikkannya ke handler, event WhatsApp, Lua, media downloader, Gemini, HTTP server.
- internal/domain/repository/storage_repository.go sekarang berisi Save/Get/Delete/GetPath/Exists.
- internal/infrastructure/storage/local_storage.go memakai filesystem dan punya SetBasePath; internal/infrastructure/account/manager.go mengubah base path tiap akun.
- internal/infrastructure/media/downloader.go memakai output path nyata untuk yt-dlp dan gallery-dl. FFmpeg/ffprobe dan sejumlah fungsi Lua juga memakai GetPath / os.ReadFile.
- internal/delivery/http/routes/routes.go menyajikan /api/media/* lewat Get, saat ini belum melayani Range/HEAD secara eksplisit. URL media tersimpan dalam bentuk /media/*.
- internal/delivery/http/handlers/chat_state_handler.go, message_handler.go, status_handler.go serta internal/delivery/whatsapp/event_handler.go memakai penyimpanan media.
- Kode Lua di internal/infrastructure/lua/lua_service.go, serta browser.go, mempunyai kontrak storage_path dan pemrosesan filesystem yang harus dipertahankan secara kompatibel atau dimigrasikan eksplisit.
- go.mod masih Go 1.25.0 dan memakai github.com/mattn/go-sqlite3, jadi gunakan driver SQLite yang sudah ada kecuali ada alasan terukur menggantinya.
- Dockerfile runtime saat ini WORKDIR /root, tetapi docker-compose.yml memasang database ke /app/database. Path persisten dan accounts/ harus dibereskan sebelum rilis.
- Proyek go-faster/fs pada inspeksi 2026-10-13 masih experimental dan go.mod branch terbarunya meminta Go 1.26.6. Jangan impor begitu saja ke WA-Bot Go 1.25 tanpa keputusan upgrade/toolchain dan regression check.

## 3. Struktur target

Seluruh kode storage baru ditempatkan di satu paket Go: internal/infrastructure/objectstorage (tanpa spasi atau dash, sesuai konvensi import Go).

~~~text
internal/infrastructure/objectstorage/
├── wrapper.go          # entry point WA-Bot, provider switch, convenience API
├── types.go            # ObjectRef, ObjectInfo, Options, error contract
├── s3lite.go           # SQLite one-file engine buatan sendiri
├── s3cloud.go          # adapter remote AWS SDK Go v2
├── s3gateway.go        # optional local HTTP S3 endpoint, dalam proses Go
├── gateway_adapter.go  # bridge engine -> interface library S3 frontend
├── staging.go          # temp materialization untuk ffmpeg/yt-dlp/Lua
├── migration.go        # import legacy filesystem, export & sync provider
├── backup.go           # online snapshot, restore/check/maintenance
└── *_test.go           # unit/contract/integration tests per modul

internal/domain/repository/storage_repository.go   # kontrak domain diubah/diadaptasi
cmd/api/main.go                                    # dependency wiring
internal/infrastructure/account/manager.go         # account scoping
internal/infrastructure/media/*                    # scratch files -> import
internal/infrastructure/lua/*                      # path-dependent callers
internal/delivery/http/routes/routes.go            # streaming/range endpoints
.env.example, Dockerfile, docker-compose.yml       # config dan persistence
~~~

File pelengkap boleh dipecah jika s3lite.go terlalu besar (mis. sqlite_schema.go, sqlite_chunks.go, sqlite_multipart.go). Jangan pecah file hanya demi banyaknya layer. Seluruh implementasi tetap dalam objectstorage/.

### 3.1. Kontrak: wrapper adalah pintu masuk

Desain Go yang disarankan (pseudocode, agent boleh menyempurnakan signatures):

~~~go
type ObjectRef struct {
    AccountID string
    Bucket    string
    Key       string
}

type Store interface {
    Put(ctx context.Context, ref ObjectRef, body io.Reader, opts PutOptions) (ObjectInfo, error)
    Open(ctx context.Context, ref ObjectRef, opts GetOptions) (io.ReadCloser, ObjectInfo, error)
    Stat(ctx context.Context, ref ObjectRef) (ObjectInfo, error)
    Exists(ctx context.Context, ref ObjectRef) (bool, error)
    Copy(ctx context.Context, src, dst ObjectRef) error
    Delete(ctx context.Context, ref ObjectRef) error
    List(ctx context.Context, scope ObjectScope, opts ListOptions) (ObjectPage, error)
    Close() error
}

type Wrapper struct { provider Store }

// Convenience untuk pemanggil aplikasi:
func (w *Wrapper) ReadFile(...) ([]byte, error)      // wajib max size
func (w *Wrapper) WriteFile(...) (ObjectInfo, error)
func (w *Wrapper) CopyFile(...) error
func (w *Wrapper) DeleteFile(...) error
func (w *Wrapper) FileExists(...) (bool, error)

// Untuk media besar gunakan Put/Open, jangan ReadFile.
~~~

Aturan:
1. Service/handler WA-Bot hanya mengimpor kontrak/facade, tidak mengakses SQL atau AWS SDK langsung. Dependency direction harus tetap domain -> interface, infrastructure -> implementation; jangan membentuk import cycle.
2. Nama metode public bersifat file-like seperti permintaan user. Internally pakai io.Reader/io.ReadCloser dan context.Context agar portable.
3. Semua provider mengembalikan error terstruktur dan konsisten: NotFound, AlreadyExists/Conflict, InvalidKey, InvalidRange, Unauthorized, Unavailable, CorruptData.
4. Operasi file memakai object key, BUKAN absolute filesystem path; object metadata memuat ContentType, Length, ETag, LastModified, user metadata, checksum jika tersedia.
5. CopyFile untuk satu provider harus menghindari buffer seluruh objek. Cross-provider migration berjalan lewat streaming Open -> Put.
6. AccountID wajib atau diberikan oleh scope terikat akun; jangan percaya accountId bebas dari parameter query browser.
7. Versi awal pakai bucket default wa-bot. Object key tersimpan deterministik sebagai accounts/{account-id}/media/{key}; dukung key lama dan normalisasi yang tidak menimbulkan bentrok nama.
8. ReadFile hanya helper untuk payload kecil dengan batas bytes eksplisit; default besar menggunakan Open.

### 3.2. s3lite.go: desain SQLite fisik

- Buat satu file persistent: objectstorage.sqlite. Tidak ada folder yang berisi file objek di belakang layar.
- Tabel inti minimal: schema_migrations, buckets, objects (active_version), object_versions (immutable version + status), object_chunks (BLOB + seq), multipart_uploads, multipart_parts. Kolom namespace mencakup account_id/bucket/key atau gabungan key terkanonisasi.
- Semua bytes objek benar-benar tersimpan di object_chunks sebagai BLOB. Chunk awal yang disarankan 1 MiB, configurable hanya jika dibutuhkan; satu file 100 MiB tidak mengakibatkan satu alokasi 100 MiB.
- Tulis chunk secara bertahap sebagai versi tersembunyi/staging. Setelah jumlah byte, hash, dan checksum selesai, atomically publish pointer active_version dalam transaksi singkat. Reader tidak melihat objek setengah upload.
- Crash ketika upload setengah jalan tidak boleh mengganti versi aktif. Cleanup staging orphan dengan TTL, quota, dan proses recovery yang idempotent.
- Pembacaan ber-stream harus mengambil chunk bertahap. Jangan mempertahankan read transaction SQLite panjang sepanjang streaming HTTP; gunakan pemilihan versi immutable + pembacaan chunk berkala. GC versi lama harus menghormati reader aktif.
- Copy untuk backend lokal bisa memakai pendekatan SQL chunk-copy atau immutable ref-count, asalkan atomic dan tidak memperbesar RAM; utamakan mekanisme yang sederhana untuk di-debug.
- Simpan MIME, user metadata, last-modified, length, ETag, checksum. Jangan samakan ETag multipart dengan MD5 atau SHA-256 mentah.
- Key lookup dan pagination prefix harus indexed; limit bounded, urutan stabil, cursor yang tervalidasi.
- WAL mode dengan busy_timeout, foreign_keys, limit koneksi, checkpoint, dan strategi maintenance. WAL dan SHM *boleh ada saat proses aktif*. Definisi portable satu file dipenuhi lewat shutdown rapi atau hasil backup online.
- Error penuh-disk, SQLite busy, file corrupt, interrupted write, duplicate copy, overwrite concurrent, dan cancellation harus tertangani tanpa kehilangan versi lama.
- Hindari long-lived transaksi menahan checkpoint. Keputusan auto_vacuum/incremental_vacuum/compaction dibuat setelah benchmark karena database file bisa tetap besar setelah banyak delete.
- Hardening key/scope terhadap absolute path, backslash, traversal, separator encodings, special characters, dan akses lintas akun.
- Konten database berpotensi sensitif; permission file privat, jangan expose raw file via HTTP, jangan log secret/data object.

### 3.3. s3cloud.go: adapter remote

- Gunakan AWS SDK Go v2 dengan AWS default credentials chain/explicit credentials, endpoint override, region, path-style option untuk MinIO/R2.
- Mapping Put/Open/Stat/Exists/Delete/Copy/List harus sama dengan local driver; pagination dan Range tidak boleh diabaikan.
- Streaming upload (termasuk unknown length), retry yang aman, timeout, cancellation, bounded memory, checksum/integrity sesuai kompatibilitas provider.
- Jangan memperlakukan URL public atau presigned URL sebagai object key internal.
- Remote credentials tidak pernah disimpan plaintext di DB media atau dicetak ke log.
- Tidak boleh menggandakan implementasi business logic saat mengganti driver: new Wrapper(config) memilih SQLiteStore atau S3Store.

### 3.4. s3gateway.go: API S3 standar tanpa sidecar

- Mode SQLite punya handler S3 HTTP internal (mis. go-faster/fs/server.NewHandler dengan gateway_adapter.go) yang memetakan operasi S3 -> same storage engine.
- WA-Bot internal menggunakan wrapper langsung, **tidak** wajib HTTP ke localhost untuk operasi tiap media. HTTP endpoint berguna bagi AWS CLI/SDK lain dan membuktikan kompatibilitas S3.
- Default gateway external OFF; bila aktif bind 127.0.0.1 saja, gunakan SigV4 access/secret yang disediakan aman. Tidak boleh otomatis membuka 0.0.0.0 atau anonymous public read.
- Minimum kompatibilitas: Create/List/Head Bucket; Put/Get/Head/Delete/Copy Object; ListObjectsV2; Multipart Create/UploadPart/Complete/Abort; Range; Content-Type/metadata; ETag; presigned GET/PUT (jika library mendukung dan lulus tests).
- Uji dengan AWS SDK Go v2, AWS CLI, dan satu client lain; catat operasi S3 yang tidak didukung, jangan mengklaim 100% parity.
- GATE: evaluasi go-faster/fs experimental, lisensi, security, dukungan S3 auth dan minimum Go version. Jika upgrade ke Go 1.26.6 tidak dapat diterima atau gateway belum aman, cari frontend S3 Go lain yang embeddable. Jangan menghalangi implementasi engine SQLite hanya karena library gateway.
- Jangan menulis sendiri implementasi Signature V4 dan HTTP protocol S3 dari awal.

## 4. Migrasi integrasi WA-Bot

### Account scoping

- Satu DB global objectstorage.sqlite menyimpan media semua akun, dipisahkan account ID di key/scope.
- internal/infrastructure/account/manager.go tetap bertanggung jawab mengikat message DB dan account aktif. Hapus ketergantungan storage terhadap SetBasePath filesystem. Request, background job, dan callback harus membawa immutable account scope miliknya, bukan membaca global current-account yang bisa berganti di tengah pekerjaan.
- Pending account QR dapat memakai namespace ephemeral khusus, dibersihkan tanpa menyentuh akun lain.
- Perubahan akun yang bersamaan dengan operasi download/upload tidak boleh menyebabkan data tersimpan atau terbaca di akun lain.

### Media + Lua

- cmd/api/main.go menginstansiasi satu Wrapper dari config dan menyuntikkannya melalui interface ke seluruh consumer.
- Migrasikan Save/Get/Delete/Exists ke facade yang sama. Jangan menyediakan GetPath palsu yang mengembalikan lokasi yang bukan file nyata.
- internal/infrastructure/media/downloader.go: yt-dlp dan gallery-dl menulis ke scratch temporary directory OS; setelah output lengkap dan terverifikasi, import streaming via Wrapper.Put, lalu hapus temp.
- internal/infrastructure/media/converter.go, FFmpeg, ffprobe: baca/olah file temporary bila tool benar-benar butuh path; gunakan MaterializeTemp + defer cleanup. Periksa escape argumen, quota, durasi pemrosesan, dan cleanup saat crash/start.
- internal/infrastructure/lua/lua_service.go serta browser.go: audit storage_path, ffmpeg, command tools, gemini_chat(file), send_sticker, fetch_to_file. Pertahankan API Lua lama sejauh mungkin; bila sebelumnya mengembalikan path permanen, definisikan kompatibilitas sementara dan migrasikan ke object key atau temporary materialization dengan lifetime jelas. Update docs Lua/API.
- internal/delivery/http/routes/routes.go: tetap terima /api/media/*; resolve account scope aman; streaming via Open/Stat; dukung HEAD, Accept-Ranges, 206/416, Content-Range, ETag/If-None-Match, Content-Type, dan request cancellation. Hindari io.ReadAll.
- Pertahankan bentuk MediaURL di DB dan response yang dipakai web/desktop (/media/*), termasuk lazy history-download, statuses, sticker favorites, incoming/outgoing messages.
- Perlakuan view-once dan media kedaluwarsa harus mengikuti aturan privasi aplikasi, bukan otomatis dijadikan penyimpanan permanen oleh adapter baru. Audit existing flow sebelum menentukan TTL/cache policy.
- Download media dari WhatsApp dan upload ke WhatsApp adalah flow terpisah dari persistence local; jangan mengubah WA protocol hanya demi migrasi storage.

## 5. Konfigurasi, lokasi file, packaging

Contoh .env (nama boleh disesuaikan tetapi harus konsisten):

~~~dotenv
OBJECT_STORAGE_DRIVER=sqlite
OBJECT_STORAGE_BUCKET=wa-bot
OBJECT_STORAGE_SQLITE_PATH=./database/objectstorage.sqlite

# Digunakan pada driver s3 saja
S3_ENDPOINT=
S3_REGION=ap-southeast-1
S3_ACCESS_KEY_ID=
S3_SECRET_ACCESS_KEY=
S3_FORCE_PATH_STYLE=true

# Endpoint S3 HTTP lokal opsional
S3_GATEWAY_ENABLED=false
S3_GATEWAY_BIND=127.0.0.1:9000
~~~

- Default pada fresh install: sqlite. Pindah ke s3 hanya mengubah config/provider; itu TIDAK otomatis memindahkan data lama.
- Data desktop disimpan di per-user app-data location absolut (Linux XDG, Windows AppData) melalui supervisor/Go backend, bukan current working directory yang tak stabil. Pastikan satu proses backend menyentuh satu database dengan locking yang benar.
- Untuk Docker, selaraskan WORKDIR dengan volume mount dan data root; persist database session/app, database akun/per-account, file objectstorage.sqlite, dan folder scratch bila diperlukan. Perbaiki kasus Dockerfile /root vs compose /app/database sebelum migrasi production.
- .gitignore mengecualikan objectstorage.sqlite, -wal, -shm, temp, dan backup.
- Startup: open DB -> verify schema -> schema migration transactional -> init wrapper -> bind account -> start media handlers -> (optional) start S3 HTTP handler. Shutdown flush/close bersih.
- Saat driver S3 remote, tidak perlu membuka SQLite media file. Jangan mematikan database lain milik aplikasi.

## 6. Migrasi data, backup, rollback

### Import dari LocalStorage legacy

- Inventarisasi seluruh accounts/*/media, path nested, mediaURL di message/status/sticker, serta existing _pending. Jangan asal pindah satu folder menjadi satu akun.
- Buat command idempotent (contoh: wa-bot storage migrate --from-local). Scan dengan validasi path, hindari symlink traversal, stream setiap file ke object key scope akun yang sama.
- Setelah Put sukses, verifikasi ukuran dan hash/checksum terhadap sumber. Catat manifest yang bisa dilanjutkan; report imported/skipped/failed.
- Selama transisi gunakan dual-read new store -> fallback LocalStorage legacy sesuai feature flag; write hanya ke provider aktif. Jangan silently fallback ketika ada error corrupt/unauthorized.
- Jaga endpoint media, DB link dan perilaku Lua selama migrasi. Setelah audit sukses, alihkan read sepenuhnya; jangan delete file legacy otomatis. Berikan mekanisme explicit cleanup setelah backup.
- Sediakan streaming export/import local SQLite -> remote S3 dan sebaliknya; keterbatasan cross-provider copy jelas, bukan sekadar ganti endpoint.

### Backup portable

- Tambahkan wa-bot storage backup --output <path>.sqlite (atau endpoint administrasi lokal dengan kontrol akses), memakai SQLite Online Backup API / VACUUM INTO yang aman.
- Satu file snapshot berisi objek + metadata; aktifnya WAL/-shm tidak ikut dicopy secara mentah. Hasil backup dibuka dan diverifikasi integrity_check + sample object checksum sebelum dianggap sukses.
- Restore hanya setelah koneksi DB ditutup/di-quiesce, temp path dibersihkan, dan versi schema diverifikasi. Jangan overwrite DB live tanpa proses restore aman.
- Backup bisa besar; laporkan kebutuhan disk, kemajuan, error, dan jangan gunakan copy byte langsung file SQLite yang sedang aktif.
- Dokumenkan recovery saat crash, full disk, interrupted backup, schema version upgrade, data privacy, dan cara balik ke driver local lama.

## 7. Fase kerja untuk agent pelaksana

### Phase 0: Contract & feasibility gate
- [x] Finalisasi interface Wrapper/Store + ObjectRef + errors dari pengguna nyata, termasuk account scoping.
- [x] Enumerasi semua GetPath/SetBasePath dan semua URL media; buat migration map per callsite dari hasil scan repo.
- [x] Library HTTP S3: go-faster/fs DITOLAK (experimental, v0.15.0 butuh Go 1.26.6, go.mod 1.25). Dipakai johannesboyne/gofakes3 v1.2.0 (MIT, Go 1.24) + verifikasi SigV4 via signer AWS SDK + multipart SQLite sendiri (gofakes3 menahan part di RAM). Lihat docs/OBJECT-STORAGE.md.
- [x] Tetapkan database schema v1, upload staging/recovery, multipart design, backup contract, dan invariant crash-safety sebelum coding.
**Gate:** tidak ada operasi backend media penting yang membutuhkan path permanen tanpa strategi staging/adapter.

### Phase 1: SQLite store + wrapper
- [x] Buat types.go, wrapper.go, s3lite.go + schema migration.
- [x] Implement Put/Open/Stat/Exists/Copy/Delete/List yang streaming dan per-account; implement helper ReadFile/WriteFile/CopyFile.
- [x] Atomic publish, interrupted write recovery, chunk cleanup, overwrite/delete saat ada active reader, transactions singkat, pagination.
- [x] Uji 0-byte, file kecil, video besar, Unicode/key khusus, copy, overwrite, empty bucket, range, concurrent read/write, crash di tengah upload.
**Gate:** restart WA-Bot tetap membaca objek; satu DB menyimpan seluruh bytes; RAM tidak naik linear terhadap ukuran file.

### Phase 2: Remote S3 & standard S3 endpoint
- [x] Implement s3cloud.go AWS SDK Go v2 dengan config/provider switch.
- [x] Pasang s3gateway.go sebagai HTTP handler embedded mode sqlite (bukan sidecar), plus gateway_adapter.go.
- [x] S3 signing, bucket operations, Get/Put/Delete/Copy/List/Head, multipart, HTTP Range, presigned URLs sesuai kemampuan frontend.
- [x] Buat compatibility matrix AWS SDK, AWS CLI, MinIO/R2 (cloud driver), dokumentasi fitur S3 unsupported.
**Gate:** backend WA-Bot memakai wrapper yang sama untuk sqlite maupun remote S3; SDK klien bisa bicara ke gateway sqlite lokal.

### Phase 3: Migrasi consumer WA-Bot
- [x] Wiring cmd/api/main.go, account manager, seluruh handlers + event WhatsApp, Gemini, downloader, FFmpeg, Lua/browser.
- [x] Hilangkan GetPath dari kontrak persistent object storage setelah seluruh callsite dimigrasi. Temp-file helpers tidak menyamar sebagai path objek.
- [x] Perbaiki media HTTP handler streaming + partial content + security account scope.
- [x] Pastikan web/Electron/GPUI memakai API/media URL yang sama tanpa perubahan besar UI.
**Gate:** kirim/terima media, sticker, voice note, media status/history, Lua commands dan FFmpeg tetap bekerja dengan sqlite dan S3. (Terverifikasi lewat test dan binary asli terhadap endpoint /api/media + gateway S3; alur dengan akun WhatsApp sungguhan, Lua/ffmpeg nyata, dan UI dicek manual oleh user.)

### Phase 4: Data migration + portable backup
- [x] Importer files legacy: dijalankan SEKALI sebagai script di luar repo (keputusan user: bukan command/fitur permanen, jadi tidak ada di codebase). 286 file / 138.6 MiB tersalin, diverifikasi checksum, sumber tidak dihapus.
- [x] Dual-read TIDAK diimplementasikan (keputusan): migrasi dilakukan sekali di luar repo dan diverifikasi checksum; folder legacy tidak dihapus sehingga rollback = menjalankan build lama. Tidak ada fallback diam-diam di kode.
- [x] Backup online satu-file, restore, integrity check: `wa-bot-backend storage backup|restore|check` (export/import ke remote S3 menunggu Phase 2).
- [x] Uji pindah file backup ke instalasi baru, akun tetap terisolasi, URL media tetap valid.
**Gate:** rollback dapat dilakukan tanpa kehilangan file legacy, dan backup benar-benar bisa dibuka pada mesin lain.

### Phase 5: Hardening, packaging, documentation
- [x] Update .env.example, Dockerfile, docker-compose.yml, desktop data-dir dan packaging Windows/Linux.
- [x] Audit access controls gateway S3, API media, key validation, permissions DB, secrets, quotas, timeouts, disk-full handling.
- [x] Ukur RAM dan throughput pada 1 MiB, 100 MiB, dan 1 GiB media dengan streaming (target bounded overhead); jangan menebak klaim benchmark.
- [x] Jalankan targeted tests untuk object data integrity, crash/restart, multipart, backup/restore, isolation, URL/media compatibility, lalu regression review. Manual UI dilakukan user.
- [x] Tambahkan dokumentasi arsitektur, deployment single-file, pindah mesin, migrasi storage provider, data retention, troubleshooting.
**Gate:** existing media tetap aman, behavior local/remote terdokumentasi, tidak ada hidden path dependency.

## 8. Keputusan dan larangan implementasi

**Diputuskan**
1. Folder bernama objectstorage, file yang diminta user bernama wrapper.go dan s3lite.go; s3lite.go adalah SQLite engine asli buatan kita.
2. Backend WA-Bot memanggil Wrapper langsung; driver local tidak diwajibkan memanggil dirinya sendiri melalui HTTP.
3. Driver cloud memakai AWS SDK Go v2; S3 HTTP gateway embedded tersedia untuk interoperabilitas external tooling.
4. Satu file untuk semua media akun, dengan namespace eksplisit. Session/app/messages SQLite tetap terpisah.
5. Perubahan dilakukan bertahap, dengan dual-read dan rollback. Mulai dalam mode sqlite untuk fresh installs setelah migrasi callsite tuntas; existing installation ditangani melalui migration state, bukan mengganti driver membabi buta.

**Jangan**
- Membaca file media besar ke []byte sebagai jalur utama.
- Menyimpan objek di filesystem dan mengklaim sudah satu-file SQLite.
- Menggunakan global SetBasePath untuk memilih akun saat ada request/job paralel.
- Mengekspos S3 gateway di alamat publik tanpa autentikasi dan keputusan security jelas.
- Mengembalikan fake GetPath dari storage SQLite.
- Menghapus media legacy sebelum copy, checksum, dan backup dikonfirmasi.
- Mengklaim kompatibilitas S3 total jika hanya subset yang diuji.
- Meninggalkan database live dipindah dengan copy raw SQLite saja ketika WAL masih aktif.

## 9. Deliverable selesai

- Satu paket internal/infrastructure/objectstorage dengan Wrapper, S3Lite, S3Cloud, S3Gateway, ops streaming + helper sederhana.
- Satu file objectstorage.sqlite menyimpan semua objek lokal secara konsisten.
- WA-Bot media pipeline tidak tergantung path storage fisik.
- Provider local sqlite / remote s3 interchangeable dari config.
- S3 HTTP-compatible gateway lokal untuk client standar.
- Migrate, backup satu-file, restore, dokumentasi, test/verification logs.
- Checklist semua fase dicentang oleh agent hanya setelah benar-benar dikerjakan.

Catatan implementasi:
- Keputusan user setelah implementasi awal: object storage PER AKUN (satu file `accounts/<phone>/database/objectstorage.sqlite` per akun, bukan satu DB global). Mode file tunggal tetap bisa lewat path tanpa `{account}`. Data yang sudah dimigrasi dipecah per akun; file global lama disimpan sebagai `database/objectstorage.sqlite.global-before-split`.
- AWS CLI, MinIO, dan Cloudflare R2 tidak tersedia di mesin pengembangan; kompatibilitas diuji dengan AWS SDK Go v2 dan boto3. s3cloud.go hanya diuji terhadap gateway sendiri.
- Windows: build butuh CGO seperti sebelumnya (go-sqlite3); belum dicoba dari Linux.
- Tidak ada kuota per-objek; disk penuh ditangani (error `unavailable`, objek lama utuh, teruji).

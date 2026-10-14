# Object storage (media)

WA-Bot keeps media — images, video, voice notes, stickers, documents, status
media, files made by Lua scripts — in an **object storage** instead of a folder.
By default **every WhatsApp account has its own object storage: one SQLite
file** next to the rest of that account's data,
`accounts/<phone>/database/objectstorage.sqlite` (relative to the backend's
working directory; `~/.wa-bot/` for the desktop app). There is no MinIO, no
extra daemon, no folder of loose files, and two accounts never share a file —
backing up, moving or deleting an account takes its media with it.

Message history, the WhatsApp session and the app settings stay in their own
SQLite files; only media moved.

```
                       ┌────────────────────────────┐
 handlers, Lua, ffmpeg │ objectstorage.Wrapper      │  the single door
 ───────────────────▶  │  Put/Open/Stat/Copy/...    │
 (via StorageRepository)└──────┬──────────────┬──────┘
                              │              │
                      driver=sqlite     driver=s3
                       SQLiteStore       S3Store (AWS SDK v2)
                       one .sqlite       AWS S3 / R2 / MinIO / ...
                              │
                       optional S3 gateway (loopback, SigV4)
```

## What lives where

| Thing | Location |
|---|---|
| Media bytes, keys, metadata | `accounts/<phone>/database/objectstorage.sqlite` (driver `sqlite`) or the remote bucket (driver `s3`) |
| Media before a number is linked (QR phase) | `tmp/pending-objectstorage.sqlite`, deleted on every new QR phase |
| Media from before accounts existed | `accounts/_legacy/database/objectstorage.sqlite` |
| WhatsApp session | `database/wa-bot-session.db` |
| App settings, triggers, cron | `database/wa-bot-app.db` |
| Messages per account | `accounts/<phone>/database/wa-bot-messages.db` |
| Scratch files for ffmpeg/yt-dlp/Lua | `tmp/staging/` (disposable, wiped at start) |

Objects are addressed by **account + key** (`628123.../history/chat/a.jpg`), never
by path. Each linked WhatsApp number is its own namespace; media seen before
login lives in a throw-away `_pending` namespace that is cleared on every QR
phase. The HTTP URLs clients use (`/api/media/<key>`, `/media/<key>` in message
rows) did not change.

## Configuration

See `.env.example`.

```dotenv
OBJECT_STORAGE_DRIVER=sqlite                 # or s3
OBJECT_STORAGE_BUCKET=wa-bot
OBJECT_STORAGE_SQLITE_PATH=./accounts/{account}/database/objectstorage.sqlite
```

`{account}` is replaced by the phone number: one database per account. A path
without `{account}` puts all accounts in one shared file instead (single-file
mode; everything below still applies, per file).

A fresh install uses `sqlite`. Changing the driver only changes where **new**
media goes — it never moves existing media (see *Moving between providers*).

## How the SQLite engine works

* An object is an immutable **version** made of 1 MiB chunk rows. Memory use is
  one chunk, whatever the object size (measured below).
* An upload writes a hidden *staging* version and **publishes** it by pointing
  `objects.active_version` at it in one short transaction. Readers never see a
  half-written object; a crash or a cancelled upload never replaces the live
  version, and the next start removes the leftovers.
* Overwrite/delete while someone is downloading: the reader keeps its version
  until it finishes, then the old chunks are freed.
* WAL mode, `busy_timeout`, foreign keys on. While the app runs the `-wal` and
  `-shm` files exist next to the database; a clean shutdown folds them back, so
  the database is **one file** again. The file is created owner-only (0600).
* Use the database from one backend process at a time. (Short-lived tools such
  as `storage backup`/`check`/`copy` alongside a running backend are fine: they
  open it without running start-up recovery.)

### Measured (Linux, NVMe, `OBJECTSTORAGE_BENCH=1 go test ... -run Throughput`)

| Object | Upload | Copy | Download | Peak Go heap |
|---|---|---|---|---|
| 1 MiB | 123 MiB/s | 275 MiB/s | 856 MiB/s | 1 MiB |
| 100 MiB | 144 MiB/s | 332 MiB/s | 1441 MiB/s | 8 MiB |
| 1 GiB | 146 MiB/s | 285 MiB/s | 1375 MiB/s | 8 MiB |

The benchmark uploads one object and copies it, so the database ends up holding
two copies of the payload (2050 MiB for the 1 GiB run); a copy duplicates the
chunks. Heap is the Go heap only (SQLite's own page cache is extra and small,
~2 MiB by default).

## Scratch files (ffmpeg, yt-dlp, Lua)

Programs that need a real file get a **working copy**: `StorageRepository`'s
`Work*` methods (and Lua's `storage_path`). The object is copied out to
`tmp/staging/<account>/<key>`; whatever a program writes there is stored back
when the script/command finishes (or when the file is read through the
storage). Idle files that are safely stored are deleted every few minutes.
A working path is temporary — refer to media by key.

## Backup, restore, move to another machine

```bash
wa-bot-backend storage backup  --output backups/                      # every account -> backups/<phone>.sqlite
wa-bot-backend storage backup  --account 628123 --output one.sqlite   # a single account
wa-bot-backend storage check                                           # integrity + checksum sample, all accounts
wa-bot-backend storage restore --account 628123 --input one.sqlite     # stop WA-Bot first
```

* `backup` uses SQLite `VACUUM INTO` (safe while WA-Bot runs; never a raw copy of
  a database with an active WAL), then opens the snapshot, runs
  `integrity_check` and re-checksums a random sample of objects before the file
  appears at the destination. The size of the result is the disk space needed.
* `restore` verifies the snapshot first, keeps the previous database as
  `*.before-restore`, and removes stale `-wal`/`-shm` files. Run it with the
  backend stopped.
* To move machines, restore each `<phone>.sqlite` with `storage restore
  --account <phone>` on the new one (plus the account's other databases, or just
  copy the account's whole `accounts/<phone>/` folder). Accounts stay isolated and media
  URLs stay valid because they are keys, not paths.

Recovery notes: a crash mid-upload leaves nothing visible and is cleaned at the
next start; a full disk fails the write with an "unavailable" error and leaves
existing objects untouched; an interrupted backup leaves only a `*.partial`
file that is removed on the next attempt; a database written by a newer build
is refused with a clear error rather than modified.

## Moving between providers

```bash
# S3 settings come from the environment (S3_ENDPOINT, S3_ACCESS_KEY_ID, ...)
wa-bot-backend storage copy --from sqlite --to s3
wa-bot-backend storage copy --from s3 --to sqlite     # go back to local
```

Streams each object (never loading it whole), skips objects that already exist
with the same checksum (so it can be rerun), verifies sizes/checksums, and
**never deletes** anything on either side. Then set `OBJECT_STORAGE_DRIVER` and
restart. To return to the old filesystem layout, run a build from before this
change: the original `accounts/<phone>/media` folders were copied, not removed.

## S3 endpoint for external tools (optional)

```dotenv
S3_GATEWAY_ENABLED=true
S3_GATEWAY_BIND=127.0.0.1:9000
S3_GATEWAY_ACCESS_KEY_ID=...
S3_GATEWAY_SECRET_ACCESS_KEY=...        # >= 8 characters
```

* Runs **inside the backend process** (no sidecar). WA-Bot itself does not use it
  — it calls the wrapper directly.
* Off by default. It refuses to bind anything but a loopback address, and every
  request must be SigV4-signed (header or presigned URL) with the configured key.
  There is no anonymous access.
* One bucket (`wa-bot`). Keys look like `accounts/<phone>/media/<key>`. Anything
  else is rejected, so a client can reach exactly the objects WA-Bot manages.

```bash
export AWS_ACCESS_KEY_ID=... AWS_SECRET_ACCESS_KEY=...
aws --endpoint-url http://127.0.0.1:9000 s3 ls s3://wa-bot/accounts/628123/media/
```

### Compatibility matrix

The frontend library is [gofakes3](https://github.com/johannesboyne/gofakes3)
v1.2.0 (MIT, Go 1.24). `go-faster/fs` was evaluated and rejected: it is
experimental and its latest release requires Go 1.26.6 while this module is on
Go 1.25. gofakes3 does not verify signatures, so WA-Bot wraps it with a SigV4
verifier that uses the AWS SDK's own signer; its multipart handling holds parts
in memory, so WA-Bot supplies its own SQLite-backed multipart implementation.

Covered by tests (`go test ./internal/infrastructure/objectstorage`; the boto3 test is skipped when Python/boto3 is missing):

| Operation | AWS SDK Go v2 | boto3 (Python) |
|---|---|---|
| ListBuckets / HeadBucket | ✅ | – |
| PutObject / GetObject / HeadObject / DeleteObject | ✅ | ✅ |
| Range GET (`bytes=a-b`) | ✅ | – |
| CopyObject (incl. metadata REPLACE) | ✅ | – |
| DeleteObjects | ✅ | – |
| ListObjectsV2 (prefix, delimiter, paging) | ✅ | ✅ |
| Multipart upload/complete/abort, ListParts, ListMultipartUploads | ✅ | ✅ (upload) |
| Presigned GET / PUT (SigV4) | ✅ | ✅ (GET) |
| Zero-byte objects | ✅ | – |

**Not supported / not tested:** SigV2 (including boto3's *default* presigned
URLs — set `signature_version="s3v4"`), bucket creation/deletion, versioning,
ACLs, tagging, lifecycle, SSE, `CopyObject` metadata for non-REPLACE directives,
anonymous access, virtual-hosted-style addressing, listing multipart uploads
across accounts, S3 "folder marker" objects (`dir/` is the same key as `dir`), and
the `STREAMING-AWS4-HMAC-SHA256-PAYLOAD-TRAILER`/unsigned-trailer upload variants
(plain `aws-chunked` from the Go SDK works; per-chunk signatures are not
re-verified, only the signed request and its declared payload hash are).
Requests carrying any `x-amz-*` header that the signature does not cover are
refused. A multipart object's ETag
carries the `-<parts>` suffix when read through the internal API but is shown
without it by the gateway (gofakes3 renders ETags as hex of an MD5). A request
whose `Content-Length: 0` is part of the signature may not verify when the
client signs it (the AWS signer omits zero lengths); sign without it. The AWS
CLI, MinIO and Cloudflare R2 were **not** available to test against here:
`s3cloud.go` is verified against our own gateway only, so try it with your
provider before relying on it.

## Remote driver (`s3`)

Uses AWS SDK Go v2 against any S3-compatible service (`S3_ENDPOINT`,
`S3_FORCE_PATH_STYLE=true` for MinIO/R2). The bucket must already exist; objects
are stored under `accounts/<phone>/media/<key>`. Uploads stream through the SDK's
multipart uploader (8 MiB parts, 2 in flight: about 16 MiB of memory). A SHA-256
is recorded as object metadata after upload (via a metadata-only self-copy, so
objects above 5 GiB are stored without one). Credentials come from
`S3_ACCESS_KEY_ID`/`S3_SECRET_ACCESS_KEY` or the AWS default chain, are never
stored in a database, and are never logged. Object metadata values must be
printable ASCII on this driver.

## Security and privacy

* The database may contain private media: it is created 0600, never served over
  HTTP, and its contents are never logged.
* `/api/media/*` serves only the **currently bound account**, and keys are
  validated (no absolute paths, backslashes, `..`, control characters).
* Scratch files live in an owner-only folder and symlinks in it are never
  followed into the store.
* Media is stored as WhatsApp delivered it; the storage layer adds no retention
  of its own. View-once and expired media follow the app's existing rules —
  deleting an object really frees its chunks.

## Troubleshooting

| Symptom | Likely cause |
|---|---|
| `object storage (sqlite): ... unavailable` at start | disk full, or the file is read-only/locked by another process |
| `database schema vN is newer than this build` | the file was written by a newer WA-Bot; upgrade, don't downgrade |
| Media 404 after switching to `s3` | data was not copied: run `storage copy --from sqlite --to s3` |
| `S3 gateway ... not a loopback address` | by design; bind `127.0.0.1` and tunnel if you need remote access |
| `SignatureDoesNotMatch`/403 from the gateway | wrong key, wrong region (`S3_GATEWAY_REGION`, default `us-east-1`), clock skew > 15 min, or SigV2 client |
| Large `tmp/staging` | a script keeps producing files; idle stored files are swept every 5 minutes |
| Database much larger than the media | deleted objects free pages inside the file, they do not shrink it; run `storage backup` + `restore` to compact |

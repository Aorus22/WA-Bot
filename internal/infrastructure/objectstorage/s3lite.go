package objectstorage

import (
	"context"
	"crypto/md5"
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"hash"
	"io"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/mattn/go-sqlite3"
)

// DefaultChunkSize is how much of an object lives in one SQLite row. Every
// upload, download and copy moves at most this much through memory at a time.
const DefaultChunkSize = 1 << 20

// DefaultStagingTTL is how old an unfinished upload must be before startup
// recovery discards it.
const DefaultStagingTTL = time.Hour

// MultipartTTL is how long an unfinished multipart upload is kept.
const MultipartTTL = 7 * 24 * time.Hour

// SQLiteOptions configures an embedded store.
type SQLiteOptions struct {
	Path       string
	ChunkSize  int           // 0 = DefaultChunkSize
	StagingTTL time.Duration // 0 = DefaultStagingTTL
	// SkipRecovery leaves leftovers of interrupted runs alone. Tools that open a
	// database another process is serving (backup, check, copy) set it: recovery
	// of an unreferenced version would pull it from under a running download.
	SkipRecovery bool
}

// SQLiteStore keeps every object — bytes, keys, metadata — in one SQLite file.
//
// Objects are immutable versions made of fixed-size chunk rows. An upload
// writes a hidden "staging" version chunk by chunk and publishes it by pointing
// objects.active_version at it in one short transaction, so readers never see a
// half-written object and a crash never replaces the version that was live.
// Replaced or deleted versions are purged once no reader in this process still
// streams them.
//
// One process at a time should use the file (WAL needs shared memory on the
// same machine). A second short-lived process, such as the migration command,
// is safe: publishing is atomic and recovery only touches staging rows older
// than StagingTTL.
type SQLiteStore struct {
	db        *sql.DB
	path      string
	chunkSize int

	mu      sync.Mutex // guards the maps and serializes version purges
	readers map[int64]int
	retired map[int64]bool
	closed  bool
}

var _ Store = (*SQLiteStore)(nil)

var migrations = []string{
	// v1
	`CREATE TABLE buckets (
		name       TEXT PRIMARY KEY,
		created_at INTEGER NOT NULL
	);
	CREATE TABLE object_versions (
		id           INTEGER PRIMARY KEY AUTOINCREMENT,
		account_id   TEXT    NOT NULL,
		bucket       TEXT    NOT NULL,
		key          TEXT    NOT NULL,
		status       TEXT    NOT NULL CHECK (status IN ('staging', 'active')),
		size         INTEGER NOT NULL DEFAULT 0,
		content_type TEXT    NOT NULL DEFAULT '',
		etag         TEXT    NOT NULL DEFAULT '',
		sha256       TEXT    NOT NULL DEFAULT '',
		metadata     TEXT    NOT NULL DEFAULT '{}',
		chunk_size   INTEGER NOT NULL,
		created_at   INTEGER NOT NULL,
		modified_at  INTEGER NOT NULL
	);
	CREATE INDEX idx_object_versions_status ON object_versions (status, created_at);
	CREATE TABLE objects (
		account_id     TEXT    NOT NULL,
		bucket         TEXT    NOT NULL,
		key            TEXT    NOT NULL,
		active_version INTEGER NOT NULL REFERENCES object_versions (id),
		PRIMARY KEY (account_id, bucket, key)
	) WITHOUT ROWID;
	CREATE TABLE object_chunks (
		version_id INTEGER NOT NULL REFERENCES object_versions (id) ON DELETE CASCADE,
		seq        INTEGER NOT NULL,
		data       BLOB    NOT NULL,
		PRIMARY KEY (version_id, seq)
	);`,
	// v2: multipart uploads. Part bytes are chunk rows like object bytes, and are
	// streamed into the final object on completion, so no part sits in memory.
	`CREATE TABLE multipart_uploads (
		id           TEXT PRIMARY KEY,
		account_id   TEXT    NOT NULL,
		bucket       TEXT    NOT NULL,
		key          TEXT    NOT NULL,
		content_type TEXT    NOT NULL DEFAULT '',
		metadata     TEXT    NOT NULL DEFAULT '{}',
		created_at   INTEGER NOT NULL
	);
	CREATE INDEX idx_multipart_uploads_key ON multipart_uploads (account_id, bucket, key);
	CREATE TABLE multipart_parts (
		upload_id   TEXT    NOT NULL REFERENCES multipart_uploads (id) ON DELETE CASCADE,
		part_number INTEGER NOT NULL,
		etag        TEXT    NOT NULL DEFAULT '',
		size        INTEGER NOT NULL DEFAULT 0,
		created_at  INTEGER NOT NULL,
		PRIMARY KEY (upload_id, part_number)
	);
	CREATE TABLE multipart_chunks (
		upload_id   TEXT    NOT NULL,
		part_number INTEGER NOT NULL,
		seq         INTEGER NOT NULL,
		data        BLOB    NOT NULL,
		PRIMARY KEY (upload_id, part_number, seq),
		FOREIGN KEY (upload_id, part_number) REFERENCES multipart_parts (upload_id, part_number) ON DELETE CASCADE
	);`,
}

// OpenSQLite opens (creating if needed) the store at opts.Path, migrates its
// schema and recovers from an interrupted run.
func OpenSQLite(opts SQLiteOptions) (*SQLiteStore, error) {
	if opts.Path == "" {
		return nil, errors.New("objectstorage: sqlite path is empty")
	}
	if opts.ChunkSize <= 0 {
		opts.ChunkSize = DefaultChunkSize
	}
	if opts.StagingTTL <= 0 {
		opts.StagingTTL = DefaultStagingTTL
	}
	path, err := filepath.Abs(opts.Path)
	if err != nil {
		return nil, err
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		return nil, fmt.Errorf("objectstorage: create folder: %w", err)
	}
	// The file may hold private media: create it owner-only before SQLite does.
	if f, err := os.OpenFile(path, os.O_CREATE|os.O_RDWR, 0o600); err != nil {
		return nil, fmt.Errorf("objectstorage: create %s: %w", path, err)
	} else {
		f.Close()
	}

	escaped := strings.NewReplacer("%", "%25", "?", "%3f", "#", "%23").Replace(path)
	dsn := "file:" + escaped + "?_busy_timeout=15000&_journal_mode=WAL&_foreign_keys=on&_synchronous=NORMAL&_txlock=immediate"
	db, err := sql.Open("sqlite3", dsn)
	if err != nil {
		return nil, err
	}
	db.SetMaxOpenConns(8)

	s := &SQLiteStore{
		db:        db,
		path:      path,
		chunkSize: opts.ChunkSize,
		readers:   map[int64]int{},
		retired:   map[int64]bool{},
	}
	if err := s.migrate(); err != nil {
		db.Close()
		return nil, err
	}
	if !opts.SkipRecovery {
		if err := s.recover(opts.StagingTTL); err != nil {
			db.Close()
			return nil, err
		}
	}
	return s, nil
}

// Path is the absolute path of the database file.
func (s *SQLiteStore) Path() string { return s.path }

// DatabaseSize is the size of the database in bytes: the pages SQLite has
// allocated, free ones included, as the file occupies them on disk. The
// account is ignored; a store of this kind is one file.
func (s *SQLiteStore) DatabaseSize(ctx context.Context, _ string) (int64, error) {
	if s.isClosed() {
		return 0, ErrStoreClosed
	}
	return sqliteSize(ctx, s.db)
}

// sqliteSize is page_count x page_size of the database behind db.
func sqliteSize(ctx context.Context, db *sql.DB) (int64, error) {
	var pages, pageSize int64
	if err := db.QueryRowContext(ctx, "PRAGMA page_count").Scan(&pages); err != nil {
		return 0, mapErr(err)
	}
	if err := db.QueryRowContext(ctx, "PRAGMA page_size").Scan(&pageSize); err != nil {
		return 0, mapErr(err)
	}
	return pages * pageSize, nil
}

func (s *SQLiteStore) migrate() error {
	if _, err := s.db.Exec(`CREATE TABLE IF NOT EXISTS schema_migrations (
		version INTEGER PRIMARY KEY, applied_at INTEGER NOT NULL)`); err != nil {
		return fmt.Errorf("objectstorage: schema: %w", mapErr(err))
	}
	var current int
	if err := s.db.QueryRow(`SELECT COALESCE(MAX(version), 0) FROM schema_migrations`).Scan(&current); err != nil {
		return fmt.Errorf("objectstorage: schema: %w", mapErr(err))
	}
	if current > len(migrations) {
		return fmt.Errorf("%w: database schema v%d is newer than this build (v%d)", ErrUnsupported, current, len(migrations))
	}
	for v := current + 1; v <= len(migrations); v++ {
		tx, err := s.db.Begin()
		if err != nil {
			return mapErr(err)
		}
		if _, err := tx.Exec(migrations[v-1]); err != nil {
			tx.Rollback()
			return fmt.Errorf("objectstorage: migrate to v%d: %w", v, mapErr(err))
		}
		if _, err := tx.Exec(`INSERT INTO schema_migrations (version, applied_at) VALUES (?, ?)`, v, time.Now().UnixMilli()); err != nil {
			tx.Rollback()
			return mapErr(err)
		}
		if err := tx.Commit(); err != nil {
			return mapErr(err)
		}
	}
	return nil
}

// recover removes what a crash can leave behind: uploads that never published
// and versions that were replaced but not yet purged. Idempotent.
func (s *SQLiteStore) recover(ttl time.Duration) error {
	cutoff := time.Now().Add(-ttl).UnixMilli()
	if _, err := s.db.Exec(`DELETE FROM object_versions
		WHERE status = 'staging' AND created_at < ?`, cutoff); err != nil {
		return fmt.Errorf("objectstorage: recover: %w", mapErr(err))
	}
	if _, err := s.db.Exec(`DELETE FROM object_versions
		WHERE status = 'active' AND id NOT IN (SELECT active_version FROM objects)`); err != nil {
		return fmt.Errorf("objectstorage: recover: %w", mapErr(err))
	}
	// Multipart uploads nobody finished within a week are abandoned.
	if _, err := s.db.Exec(`DELETE FROM multipart_uploads WHERE created_at < ?`,
		time.Now().Add(-MultipartTTL).UnixMilli()); err != nil {
		return fmt.Errorf("objectstorage: recover: %w", mapErr(err))
	}
	return nil
}

// mapErr maps SQLite failures onto the shared error contract.
func mapErr(err error) error {
	if err == nil {
		return nil
	}
	var se sqlite3.Error
	if errors.As(err, &se) {
		switch se.Code {
		case sqlite3.ErrBusy, sqlite3.ErrLocked, sqlite3.ErrFull, sqlite3.ErrIoErr, sqlite3.ErrCantOpen, sqlite3.ErrReadonly:
			return fmt.Errorf("%w: %v", ErrUnavailable, err)
		case sqlite3.ErrCorrupt, sqlite3.ErrNotADB:
			return fmt.Errorf("%w: %v", ErrCorruptData, err)
		}
	}
	return err
}

func (s *SQLiteStore) isClosed() bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.closed
}

// Close closes the database. Readers still open fail afterwards.
func (s *SQLiteStore) Close() error {
	s.mu.Lock()
	if s.closed {
		s.mu.Unlock()
		return nil
	}
	s.closed = true
	s.mu.Unlock()
	// Fold the WAL back into the main file so a clean shutdown leaves one file.
	_, _ = s.db.Exec(`PRAGMA wal_checkpoint(TRUNCATE)`)
	return s.db.Close()
}

type versionRow struct {
	id        int64
	info      ObjectInfo
	chunkSize int64
}

const versionColumns = `v.id, v.size, v.content_type, v.etag, v.sha256, v.metadata, v.chunk_size, v.modified_at`

type rowScanner interface{ Scan(dest ...any) error }

func scanVersion(row rowScanner, ref ObjectRef) (versionRow, error) {
	var (
		v        versionRow
		meta     string
		modified int64
	)
	if err := row.Scan(&v.id, &v.info.Size, &v.info.ContentType, &v.info.ETag, &v.info.SHA256, &meta, &v.chunkSize, &modified); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return v, fmt.Errorf("%w: %s", ErrNotFound, ref)
		}
		return v, mapErr(err)
	}
	v.info.Ref = ref
	v.info.LastModified = time.UnixMilli(modified).UTC()
	if meta != "" && meta != "{}" {
		if err := json.Unmarshal([]byte(meta), &v.info.Metadata); err != nil {
			return v, fmt.Errorf("%w: metadata of %s", ErrCorruptData, ref)
		}
	}
	return v, nil
}

type queryer interface {
	QueryRowContext(ctx context.Context, query string, args ...any) *sql.Row
}

func (s *SQLiteStore) lookup(ctx context.Context, q queryer, ref ObjectRef) (versionRow, error) {
	row := q.QueryRowContext(ctx, `SELECT `+versionColumns+`
		FROM objects o JOIN object_versions v ON v.id = o.active_version
		WHERE o.account_id = ? AND o.bucket = ? AND o.key = ?`, ref.AccountID, ref.Bucket, ref.Key)
	return scanVersion(row, ref)
}

// Stat returns the metadata of the live version of ref.
func (s *SQLiteStore) Stat(ctx context.Context, ref ObjectRef) (ObjectInfo, error) {
	ref, err := canonical(ref)
	if err != nil {
		return ObjectInfo{}, err
	}
	if s.isClosed() {
		return ObjectInfo{}, ErrStoreClosed
	}
	v, err := s.lookup(ctx, s.db, ref)
	return v.info, err
}

func (s *SQLiteStore) Exists(ctx context.Context, ref ObjectRef) (bool, error) {
	_, err := s.Stat(ctx, ref)
	switch {
	case err == nil:
		return true, nil
	case errors.Is(err, ErrNotFound):
		return false, nil
	default:
		return false, err
	}
}

// Put streams body into a new version of ref and publishes it atomically.
func (s *SQLiteStore) Put(ctx context.Context, ref ObjectRef, body io.Reader, opts PutOptions) (ObjectInfo, error) {
	return s.put(ctx, ref, body, opts, "")
}

// put is Put with an optional ETag that replaces the computed MD5 (multipart
// objects carry "<md5 of part md5s>-<parts>").
func (s *SQLiteStore) put(ctx context.Context, ref ObjectRef, body io.Reader, opts PutOptions, etagOverride string) (ObjectInfo, error) {
	ref, err := canonical(ref)
	if err != nil {
		return ObjectInfo{}, err
	}
	if s.isClosed() {
		return ObjectInfo{}, ErrStoreClosed
	}
	meta, err := encodeMetadata(opts.Metadata)
	if err != nil {
		return ObjectInfo{}, err
	}

	now := time.Now().UnixMilli()
	res, err := s.db.ExecContext(ctx, `INSERT INTO object_versions
		(account_id, bucket, key, status, content_type, metadata, chunk_size, created_at, modified_at)
		VALUES (?, ?, ?, 'staging', ?, ?, ?, ?, ?)`,
		ref.AccountID, ref.Bucket, ref.Key, opts.ContentType, meta, s.chunkSize, now, now)
	if err != nil {
		return ObjectInfo{}, mapErr(err)
	}
	id, _ := res.LastInsertId()

	published := false
	defer func() {
		if !published {
			// ctx may be the reason we are here; clean up regardless.
			cctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
			defer cancel()
			_, _ = s.db.ExecContext(cctx, `DELETE FROM object_versions WHERE id = ? AND status = 'staging'`, id)
		}
	}()

	md5h, sha := md5.New(), sha256.New()
	buf := make([]byte, s.chunkSize)
	var size int64
	for seq := int64(0); ; seq++ {
		if err := ctx.Err(); err != nil {
			return ObjectInfo{}, err
		}
		n, rerr := io.ReadFull(body, buf)
		if n > 0 {
			md5h.Write(buf[:n])
			sha.Write(buf[:n])
			if _, err := s.db.ExecContext(ctx, `INSERT INTO object_chunks (version_id, seq, data) VALUES (?, ?, ?)`, id, seq, buf[:n]); err != nil {
				return ObjectInfo{}, mapErr(err)
			}
			size += int64(n)
		}
		if rerr == io.EOF || rerr == io.ErrUnexpectedEOF {
			break
		}
		if rerr != nil {
			return ObjectInfo{}, rerr
		}
	}

	etag, sum := hex.EncodeToString(md5h.Sum(nil)), hex.EncodeToString(sha.Sum(nil))
	if etagOverride != "" {
		etag = etagOverride
	}
	modified := time.Now()
	prev, err := s.publish(ctx, id, ref, size, etag, sum, modified)
	if err != nil {
		return ObjectInfo{}, err
	}
	published = true
	s.retire(prev)

	return ObjectInfo{
		Ref: ref, Size: size, ContentType: opts.ContentType, ETag: etag, SHA256: sum,
		LastModified: modified.UTC().Truncate(time.Millisecond), Metadata: opts.Metadata,
	}, nil
}

// publish makes version id the live one of ref and returns the version it
// replaced (0 when there was none).
func (s *SQLiteStore) publish(ctx context.Context, id int64, ref ObjectRef, size int64, etag, sum string, modified time.Time) (int64, error) {
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return 0, mapErr(err)
	}
	defer tx.Rollback()

	var prev int64
	err = tx.QueryRowContext(ctx, `SELECT active_version FROM objects WHERE account_id = ? AND bucket = ? AND key = ?`,
		ref.AccountID, ref.Bucket, ref.Key).Scan(&prev)
	if err != nil && !errors.Is(err, sql.ErrNoRows) {
		return 0, mapErr(err)
	}
	if _, err := tx.ExecContext(ctx, `UPDATE object_versions
		SET status = 'active', size = ?, etag = ?, sha256 = ?, modified_at = ? WHERE id = ?`,
		size, etag, sum, modified.UnixMilli(), id); err != nil {
		return 0, mapErr(err)
	}
	if _, err := tx.ExecContext(ctx, `INSERT OR IGNORE INTO buckets (name, created_at) VALUES (?, ?)`,
		ref.Bucket, time.Now().UnixMilli()); err != nil {
		return 0, mapErr(err)
	}
	if _, err := tx.ExecContext(ctx, `INSERT INTO objects (account_id, bucket, key, active_version) VALUES (?, ?, ?, ?)
		ON CONFLICT (account_id, bucket, key) DO UPDATE SET active_version = excluded.active_version`,
		ref.AccountID, ref.Bucket, ref.Key, id); err != nil {
		return 0, mapErr(err)
	}
	if err := tx.Commit(); err != nil {
		return 0, mapErr(err)
	}
	return prev, nil
}

func encodeMetadata(m map[string]string) (string, error) {
	if len(m) == 0 {
		return "{}", nil
	}
	b, err := json.Marshal(m)
	if err != nil {
		return "", err
	}
	return string(b), nil
}

// retire purges a version that is no longer live, now or — when a reader is
// still streaming it — when the last reader closes.
func (s *SQLiteStore) retire(id int64) {
	if id == 0 {
		return
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.readers[id] > 0 {
		s.retired[id] = true
		return
	}
	s.purgeLocked(id)
}

func (s *SQLiteStore) purgeLocked(id int64) {
	delete(s.retired, id)
	if s.closed {
		return
	}
	// Only unreferenced versions: guards against purging a version that became
	// live again (it cannot today, but the check is cheap).
	_, _ = s.db.Exec(`DELETE FROM object_versions
		WHERE id = ? AND id NOT IN (SELECT active_version FROM objects)`, id)
}

// Open streams the live version of ref (or the byte range of it).
func (s *SQLiteStore) Open(ctx context.Context, ref ObjectRef, opts GetOptions) (io.ReadCloser, ObjectInfo, error) {
	ref, err := canonical(ref)
	if err != nil {
		return nil, ObjectInfo{}, err
	}
	if s.isClosed() {
		return nil, ObjectInfo{}, ErrStoreClosed
	}
	var v versionRow
	for attempt := 0; ; attempt++ {
		v, err = s.lookup(ctx, s.db, ref)
		if err != nil {
			return nil, ObjectInfo{}, err
		}
		// Pin the version, then make sure it was not purged in between.
		s.mu.Lock()
		s.readers[v.id]++
		s.mu.Unlock()
		var one int
		perr := s.db.QueryRowContext(ctx, `SELECT 1 FROM object_versions WHERE id = ?`, v.id).Scan(&one)
		if perr == nil {
			break
		}
		s.release(v.id)
		if !errors.Is(perr, sql.ErrNoRows) {
			return nil, ObjectInfo{}, mapErr(perr)
		}
		if attempt >= 3 {
			return nil, ObjectInfo{}, fmt.Errorf("%w: %s changed while opening", ErrConflict, ref)
		}
	}

	start, end := int64(0), v.info.Size
	if r := opts.Range; r != nil {
		size := v.info.Size
		last := r.End
		if last < 0 || last >= size {
			last = size - 1
		}
		switch {
		case size == 0 && r.Start == 0:
			start, end = 0, 0
		case r.Start < 0 || r.Start >= size || last < r.Start:
			s.release(v.id)
			return nil, v.info, fmt.Errorf("%w: %d-%d of %d", ErrInvalidRange, r.Start, r.End, size)
		default:
			start, end = r.Start, last+1
		}
	}
	return &sqliteReader{s: s, ctx: ctx, id: v.id, size: v.info.Size, chunk: v.chunkSize, base: start, end: end, pos: start, curSeq: -1}, v.info, nil
}

func (s *SQLiteStore) release(id int64) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.readers[id] > 1 {
		s.readers[id]--
		return
	}
	delete(s.readers, id)
	if s.retired[id] {
		s.purgeLocked(id)
	}
}

// sqliteReader reads one immutable version chunk by chunk. Each chunk is a
// separate short query, so no transaction stays open while a client downloads.
// Seek positions are relative to the start of the opened range.
type sqliteReader struct {
	s         *SQLiteStore
	ctx       context.Context
	id        int64
	size      int64 // whole object
	chunk     int64
	base, end int64 // absolute window [base, end)
	pos       int64 // absolute
	cur       []byte
	curSeq    int64
	closed    bool
}

func (r *sqliteReader) Read(p []byte) (int, error) {
	if r.closed {
		return 0, ErrStoreClosed
	}
	if len(p) == 0 {
		return 0, nil
	}
	if r.pos >= r.end {
		return 0, io.EOF
	}
	if err := r.ctx.Err(); err != nil {
		return 0, err
	}
	seq := r.pos / r.chunk
	if seq != r.curSeq {
		var data []byte
		err := r.s.db.QueryRowContext(r.ctx, `SELECT data FROM object_chunks WHERE version_id = ? AND seq = ?`, r.id, seq).Scan(&data)
		if err != nil {
			if errors.Is(err, sql.ErrNoRows) {
				return 0, fmt.Errorf("%w: chunk %d of version %d missing", ErrCorruptData, seq, r.id)
			}
			return 0, mapErr(err)
		}
		want := r.chunk
		if rest := r.size - seq*r.chunk; rest < want {
			want = rest
		}
		if int64(len(data)) != want {
			return 0, fmt.Errorf("%w: chunk %d of version %d has %d bytes, want %d", ErrCorruptData, seq, r.id, len(data), want)
		}
		r.cur, r.curSeq = data, seq
	}
	off := r.pos - seq*r.chunk
	n := copy(p, r.cur[off:])
	if remain := r.end - r.pos; int64(n) > remain {
		n = int(remain)
	}
	r.pos += int64(n)
	return n, nil
}

func (r *sqliteReader) Seek(offset int64, whence int) (int64, error) {
	var abs int64
	switch whence {
	case io.SeekStart:
		abs = r.base + offset
	case io.SeekCurrent:
		abs = r.pos + offset
	case io.SeekEnd:
		abs = r.end + offset
	default:
		return 0, fmt.Errorf("%w: whence %d", ErrInvalidRange, whence)
	}
	if abs < r.base {
		return 0, fmt.Errorf("%w: negative position", ErrInvalidRange)
	}
	r.pos = abs
	return abs - r.base, nil
}

func (r *sqliteReader) Close() error {
	if r.closed {
		return nil
	}
	r.closed = true
	r.cur = nil
	r.s.release(r.id)
	return nil
}

// Copy duplicates src as dst inside one transaction, with SQL doing the
// chunk copy so nothing passes through Go memory.
func (s *SQLiteStore) Copy(ctx context.Context, src, dst ObjectRef) error {
	src, err := canonical(src)
	if err != nil {
		return err
	}
	dst, err = canonical(dst)
	if err != nil {
		return err
	}
	if s.isClosed() {
		return ErrStoreClosed
	}
	if src == dst {
		_, err := s.Stat(ctx, src)
		return err
	}

	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return mapErr(err)
	}
	defer tx.Rollback()

	v, err := s.lookup(ctx, tx, src)
	if err != nil {
		return err
	}
	var prev int64
	err = tx.QueryRowContext(ctx, `SELECT active_version FROM objects WHERE account_id = ? AND bucket = ? AND key = ?`,
		dst.AccountID, dst.Bucket, dst.Key).Scan(&prev)
	if err != nil && !errors.Is(err, sql.ErrNoRows) {
		return mapErr(err)
	}
	meta, err := encodeMetadata(v.info.Metadata)
	if err != nil {
		return err
	}
	now := time.Now().UnixMilli()
	res, err := tx.ExecContext(ctx, `INSERT INTO object_versions
		(account_id, bucket, key, status, size, content_type, etag, sha256, metadata, chunk_size, created_at, modified_at)
		VALUES (?, ?, ?, 'active', ?, ?, ?, ?, ?, ?, ?, ?)`,
		dst.AccountID, dst.Bucket, dst.Key, v.info.Size, v.info.ContentType, v.info.ETag, v.info.SHA256, meta, v.chunkSize, now, now)
	if err != nil {
		return mapErr(err)
	}
	newID, _ := res.LastInsertId()
	if _, err := tx.ExecContext(ctx, `INSERT INTO object_chunks (version_id, seq, data)
		SELECT ?, seq, data FROM object_chunks WHERE version_id = ?`, newID, v.id); err != nil {
		return mapErr(err)
	}
	if _, err := tx.ExecContext(ctx, `INSERT OR IGNORE INTO buckets (name, created_at) VALUES (?, ?)`, dst.Bucket, now); err != nil {
		return mapErr(err)
	}
	if _, err := tx.ExecContext(ctx, `INSERT INTO objects (account_id, bucket, key, active_version) VALUES (?, ?, ?, ?)
		ON CONFLICT (account_id, bucket, key) DO UPDATE SET active_version = excluded.active_version`,
		dst.AccountID, dst.Bucket, dst.Key, newID); err != nil {
		return mapErr(err)
	}
	if err := tx.Commit(); err != nil {
		return mapErr(err)
	}
	s.retire(prev)
	return nil
}

// Delete removes ref. Deleting an object that does not exist is not an error.
func (s *SQLiteStore) Delete(ctx context.Context, ref ObjectRef) error {
	ref, err := canonical(ref)
	if err != nil {
		return err
	}
	if s.isClosed() {
		return ErrStoreClosed
	}
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return mapErr(err)
	}
	defer tx.Rollback()
	var id int64
	err = tx.QueryRowContext(ctx, `SELECT active_version FROM objects WHERE account_id = ? AND bucket = ? AND key = ?`,
		ref.AccountID, ref.Bucket, ref.Key).Scan(&id)
	if errors.Is(err, sql.ErrNoRows) {
		return nil
	}
	if err != nil {
		return mapErr(err)
	}
	if _, err := tx.ExecContext(ctx, `DELETE FROM objects WHERE account_id = ? AND bucket = ? AND key = ?`,
		ref.AccountID, ref.Bucket, ref.Key); err != nil {
		return mapErr(err)
	}
	if err := tx.Commit(); err != nil {
		return mapErr(err)
	}
	s.retire(id)
	return nil
}

// List returns one page of the objects of scope whose key starts with
// opts.Prefix, ordered by key.
func (s *SQLiteStore) List(ctx context.Context, scope ObjectScope, opts ListOptions) (ObjectPage, error) {
	if err := ValidateAccountID(scope.AccountID); err != nil {
		return ObjectPage{}, err
	}
	if scope.Bucket == "" {
		scope.Bucket = DefaultBucket
	}
	if err := validateBucket(scope.Bucket); err != nil {
		return ObjectPage{}, err
	}
	for _, p := range []string{opts.Prefix, opts.Cursor} {
		if len(p) > maxKeyBytes || strings.ContainsAny(p, "\\\x00") {
			return ObjectPage{}, fmt.Errorf("%w: list prefix or cursor", ErrInvalidKey)
		}
	}
	if s.isClosed() {
		return ObjectPage{}, ErrStoreClosed
	}
	limit := opts.Limit
	if limit <= 0 {
		limit = DefaultListLimit
	}
	if limit > MaxListLimit {
		limit = MaxListLimit
	}

	query := `SELECT o.key, ` + versionColumns + `
		FROM objects o JOIN object_versions v ON v.id = o.active_version
		WHERE o.account_id = ? AND o.bucket = ? AND o.key >= ? AND o.key > ?`
	args := []any{scope.AccountID, scope.Bucket, opts.Prefix, opts.Cursor}
	if upper, ok := prefixUpperBound(opts.Prefix); ok {
		query += ` AND o.key < ?`
		args = append(args, upper)
	}
	query += ` ORDER BY o.key LIMIT ?`
	args = append(args, limit+1)

	rows, err := s.db.QueryContext(ctx, query, args...)
	if err != nil {
		return ObjectPage{}, mapErr(err)
	}
	defer rows.Close()

	var page ObjectPage
	for rows.Next() {
		var (
			key      string
			v        versionRow
			meta     string
			modified int64
		)
		if err := rows.Scan(&key, &v.id, &v.info.Size, &v.info.ContentType, &v.info.ETag, &v.info.SHA256, &meta, &v.chunkSize, &modified); err != nil {
			return ObjectPage{}, mapErr(err)
		}
		v.info.Ref = ObjectRef{AccountID: scope.AccountID, Bucket: scope.Bucket, Key: key}
		v.info.LastModified = time.UnixMilli(modified).UTC()
		if meta != "" && meta != "{}" {
			_ = json.Unmarshal([]byte(meta), &v.info.Metadata)
		}
		page.Objects = append(page.Objects, v.info)
	}
	if err := rows.Err(); err != nil {
		return ObjectPage{}, mapErr(err)
	}
	if len(page.Objects) > limit {
		page.Objects = page.Objects[:limit]
		page.NextCursor = page.Objects[limit-1].Ref.Key
	}
	return page, nil
}

// prefixUpperBound is the smallest string greater than every string with the
// given prefix.
func prefixUpperBound(prefix string) (string, bool) {
	b := []byte(prefix)
	for i := len(b) - 1; i >= 0; i-- {
		if b[i] < 0xff {
			b[i]++
			return string(b[:i+1]), true
		}
	}
	return "", false
}

// Verify re-reads every chunk of ref and checks size and SHA-256 against what
// was recorded at upload time.
func (s *SQLiteStore) Verify(ctx context.Context, ref ObjectRef) error {
	rc, info, err := s.Open(ctx, ref, GetOptions{})
	if err != nil {
		return err
	}
	defer rc.Close()
	var h hash.Hash = sha256.New()
	n, err := io.Copy(h, rc)
	if err != nil {
		return err
	}
	if n != info.Size || hex.EncodeToString(h.Sum(nil)) != info.SHA256 {
		return fmt.Errorf("%w: %s does not match its recorded checksum", ErrCorruptData, info.Ref)
	}
	return nil
}

func jsonUnmarshal(s string, v any) error { return json.Unmarshal([]byte(s), v) }

// SetMetadata replaces the content type and user metadata of the live version
// of ref without touching its bytes. An empty contentType keeps the old one.
func (s *SQLiteStore) SetMetadata(ctx context.Context, ref ObjectRef, contentType string, metadata map[string]string) error {
	ref, err := canonical(ref)
	if err != nil {
		return err
	}
	meta, err := encodeMetadata(metadata)
	if err != nil {
		return err
	}
	res, err := s.db.ExecContext(ctx, `UPDATE object_versions SET metadata = ?, content_type = CASE WHEN ? = '' THEN content_type ELSE ? END
		WHERE id = (SELECT active_version FROM objects WHERE account_id = ? AND bucket = ? AND key = ?)`,
		meta, contentType, contentType, ref.AccountID, ref.Bucket, ref.Key)
	if err != nil {
		return mapErr(err)
	}
	if n, _ := res.RowsAffected(); n == 0 {
		return fmt.Errorf("%w: %s", ErrNotFound, ref)
	}
	return nil
}

// scanComposed implements gatewayStore.scanComposed for one database.
func (s *SQLiteStore) scanComposed(ctx context.Context, bucket, account, prefix, after string, inclusive bool, limit int) ([]ObjectInfo, error) {
	cmp := " >"
	if inclusive {
		cmp = " >="
	}
	const composed = `('accounts/' || o.account_id || '/media/' || o.key)`
	query := `SELECT o.account_id, o.key, ` + versionColumns + `
		FROM objects o JOIN object_versions v ON v.id = o.active_version
		WHERE o.bucket = ? AND ` + composed + ` >= ? AND ` + composed + cmp + ` ?`
	args := []any{bucket, prefix, after}
	if account != "" {
		query += ` AND o.account_id = ?`
		args = append(args, account)
	}
	if up, ok := prefixUpperBound(prefix); ok {
		query += ` AND ` + composed + ` < ?`
		args = append(args, up)
	}
	query += ` ORDER BY ` + composed + ` LIMIT ?`
	args = append(args, limit)

	rows, err := s.db.QueryContext(ctx, query, args...)
	if err != nil {
		return nil, mapErr(err)
	}
	defer rows.Close()
	var out []ObjectInfo
	for rows.Next() {
		var (
			acct, key string
			v         versionRow
			meta      string
			modified  int64
		)
		if err := rows.Scan(&acct, &key, &v.id, &v.info.Size, &v.info.ContentType, &v.info.ETag, &v.info.SHA256, &meta, &v.chunkSize, &modified); err != nil {
			return nil, mapErr(err)
		}
		v.info.Ref = ObjectRef{AccountID: acct, Bucket: bucket, Key: key}
		v.info.LastModified = time.UnixMilli(modified).UTC()
		out = append(out, v.info)
	}
	return out, mapErr(rows.Err())
}

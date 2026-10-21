package backup

import (
	"bytes"
	"context"
	"database/sql"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"wa-bot/internal/infrastructure/objectstorage"
)

func init() { retryDelay = 0 }

const testAccount = "628111"

type fixture struct {
	dir      string
	messages string
	app      string
	media    *objectstorage.SQLiteStore
	target   LocalFolder
	clock    time.Time
}

func newFixture(t *testing.T) *fixture {
	t.Helper()
	old := mediaPartSize
	mediaPartSize = 1024
	t.Cleanup(func() { mediaPartSize = old })

	dir := t.TempDir()
	f := &fixture{
		dir:      dir,
		messages: filepath.Join(dir, "live", "wa-bot-messages.db"),
		app:      filepath.Join(dir, "live", "wa-bot-app.db"),
		target:   LocalFolder{Root: filepath.Join(dir, "target")},
		clock:    time.Date(2026, 10, 19, 12, 0, 0, 0, time.UTC),
	}
	os.MkdirAll(filepath.Dir(f.messages), 0o700)
	db := openDB(t, f.messages)
	mustExec(t, db, `
		CREATE TABLE chats (id TEXT PRIMARY KEY, name TEXT, last_time INTEGER);
		CREATE TABLE messages (
			id TEXT PRIMARY KEY, chat_id TEXT, content TEXT, timestamp INTEGER,
			raw_message BLOB, score REAL, created DATETIME,
			FOREIGN KEY (chat_id) REFERENCES chats(id) ON DELETE CASCADE);
		CREATE TABLE seq_items (id INTEGER PRIMARY KEY AUTOINCREMENT, v TEXT);
		CREATE TABLE plain (a, b);
		CREATE INDEX idx_messages_chat ON messages(chat_id);
		CREATE TRIGGER bump AFTER INSERT ON messages BEGIN
			UPDATE chats SET last_time = NEW.timestamp WHERE id = NEW.chat_id;
		END;
		PRAGMA user_version = 7;`)
	for c := 0; c < 3; c++ {
		mustExec(t, db, `INSERT INTO chats (id, name, last_time) VALUES (?, ?, 0)`, fmt.Sprintf("chat%d@s.whatsapp.net", c), fmt.Sprintf("Chat %d", c))
	}
	// Messages over three months, milliseconds like the real store.
	base := time.Date(2026, 8, 1, 0, 0, 0, 0, time.UTC).UnixMilli()
	for i := 0; i < 300; i++ {
		ts := base + int64(i)*int64(24*time.Hour/time.Millisecond)/3
		mustExec(t, db, `INSERT INTO messages (id, chat_id, content, timestamp, raw_message, score, created) VALUES (?, ?, ?, ?, ?, ?, ?)`,
			fmt.Sprintf("m%04d", i), fmt.Sprintf("chat%d@s.whatsapp.net", i%3), fmt.Sprintf("hello <%d> & \"quoted\" ✓", i), ts,
			[]byte{0, 1, 2, byte(i)}, 0.1+float64(i)/7, "2026-10-19 12:00:00")
	}
	mustExec(t, db, `INSERT INTO messages (id, chat_id, content, timestamp) VALUES ('nulls', 'chat0@s.whatsapp.net', NULL, NULL)`)
	mustExec(t, db, `INSERT INTO messages (id, chat_id, content, timestamp) VALUES ('badutf8', 'chat1@s.whatsapp.net', CAST(X'FF00FE41' AS TEXT), 5)`)
	mustExec(t, db, `INSERT INTO seq_items (v) VALUES ('a'), ('b'), ('c'); DELETE FROM seq_items WHERE v = 'c'`)
	mustExec(t, db, `INSERT INTO plain VALUES (1, 'x'), (2.5, X'00'), (NULL, 'y')`)
	db.Close()

	adb := openDB(t, f.app)
	mustExec(t, adb, `CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT); INSERT INTO app_settings VALUES ('theme', 'dark')`)
	adb.Close()

	media, err := objectstorage.OpenSQLite(objectstorage.SQLiteOptions{Path: filepath.Join(dir, "live", "objectstorage.sqlite")})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { media.Close() })
	f.media = media
	f.putMedia(t, "img/a.jpg", bytes.Repeat([]byte("A"), 100))
	f.putMedia(t, "img/b.jpg", bytes.Repeat([]byte("B"), 1024))
	f.putMedia(t, "video/c.mp4", bytes.Repeat([]byte("0123456789"), 350)) // 4 parts
	f.putMedia(t, "empty.bin", nil)
	return f
}

func (f *fixture) putMedia(t *testing.T, key string, data []byte) {
	t.Helper()
	ref := objectstorage.ObjectRef{AccountID: testAccount, Bucket: objectstorage.DefaultBucket, Key: key}
	if _, err := f.media.Put(context.Background(), ref, bytes.NewReader(data), objectstorage.PutOptions{ContentType: "application/octet-stream", Metadata: map[string]string{"k": key}}); err != nil {
		t.Fatal(err)
	}
}

func (f *fixture) source() Source {
	return Source{Account: testAccount, MessagesDB: f.messages, AppDB: f.app, Media: f.media}
}

func (f *fixture) backup(t *testing.T) *BackupReport {
	t.Helper()
	f.clock = f.clock.Add(time.Hour)
	now := f.clock
	rep, err := Backup(context.Background(), f.target, f.source(), Options{WorkDir: f.dir, Now: func() time.Time { return now }})
	if err != nil {
		t.Fatal(err)
	}
	return rep
}

func (f *fixture) restore(t *testing.T, id string) *RestoreReport {
	t.Helper()
	out, err := os.MkdirTemp(f.dir, "restore-")
	if err != nil {
		t.Fatal(err)
	}
	rep, err := Restore(context.Background(), f.target, testAccount, id, out, Options{})
	if err != nil {
		t.Fatal(err)
	}
	return rep
}

func openDB(t *testing.T, path string) *sql.DB {
	t.Helper()
	db, err := sql.Open("sqlite3", "file:"+path+"?_foreign_keys=on")
	if err != nil {
		t.Fatal(err)
	}
	return db
}

func mustExec(t *testing.T, db *sql.DB, q string, args ...any) {
	t.Helper()
	if _, err := db.Exec(q, args...); err != nil {
		t.Fatalf("%s: %v", q, err)
	}
}

func queryString(t *testing.T, db *sql.DB, q string) string {
	t.Helper()
	var s sql.NullString
	if err := db.QueryRow(q).Scan(&s); err != nil {
		t.Fatalf("%s: %v", q, err)
	}
	return s.String
}

// sameRows compares a query's result on the live and the restored database.
func sameRows(t *testing.T, live, restored *sql.DB, q string) {
	t.Helper()
	dump := func(db *sql.DB) string {
		rows, err := db.Query(q)
		if err != nil {
			t.Fatalf("%s: %v", q, err)
		}
		defer rows.Close()
		cols, _ := rows.Columns()
		var b strings.Builder
		for rows.Next() {
			vals := make([]any, len(cols))
			ptrs := make([]any, len(cols))
			for i := range vals {
				ptrs[i] = &vals[i]
			}
			rows.Scan(ptrs...)
			fmt.Fprintf(&b, "%#v\n", vals)
		}
		return b.String()
	}
	if a, b := dump(live), dump(restored); a != b {
		t.Fatalf("%s differs:\nlive:\n%s\nrestored:\n%s", q, a, b)
	}
}

func chunkPath(f *fixture, sum string) string {
	return filepath.Join(f.target.Root, filepath.FromSlash(chunkName(sum)))
}

func loadLatest(t *testing.T, f *fixture) *Manifest {
	t.Helper()
	list, err := ListBackups(context.Background(), f.target, testAccount)
	if err != nil || len(list) == 0 {
		t.Fatalf("no backups: %v", err)
	}
	m, err := LoadManifest(context.Background(), f.target, list[0])
	if err != nil {
		t.Fatal(err)
	}
	return m
}

func TestRoundTrip(t *testing.T) {
	f := newFixture(t)
	rep := f.backup(t)
	if rep.MediaObjects != 4 || rep.Uploaded == 0 || len(rep.SkippedMedia) != 0 {
		t.Fatalf("unexpected report %+v", rep)
	}
	m := loadLatest(t, f)
	var msgs *Table
	for i := range m.Databases[0].Tables {
		if m.Databases[0].Tables[i].Name == "messages" {
			msgs = &m.Databases[0].Tables[i]
		}
	}
	// 3 chats x 3-4 months, plus the odd rows.
	if msgs == nil || len(msgs.Segments) < 9 {
		t.Fatalf("messages not split per chat per month: %+v", msgs)
	}

	res := f.restore(t, "")
	if !res.Complete() {
		t.Fatalf("restore incomplete: lost=%+v mismatch=%v", res.Lost, res.Mismatch)
	}
	live := openDB(t, f.messages)
	defer live.Close()
	got := openDB(t, filepath.Join(res.OutDir, MessagesFile))
	defer got.Close()
	for _, q := range []string{
		`SELECT id, chat_id, content, timestamp, raw_message, score, typeof(score), created, typeof(content) FROM messages ORDER BY id`,
		`SELECT * FROM chats ORDER BY id`,
		`SELECT * FROM seq_items ORDER BY id`,
		`SELECT a, typeof(a), b, typeof(b) FROM plain ORDER BY rowid`,
		`SELECT * FROM sqlite_sequence`,
		`SELECT type, name, sql FROM sqlite_master WHERE name NOT LIKE 'sqlite_autoindex%' ORDER BY name`,
		`PRAGMA user_version`,
	} {
		sameRows(t, live, got, q)
	}
	if queryString(t, got, `PRAGMA integrity_check`) != "ok" {
		t.Fatal("restored database fails integrity_check")
	}

	gotApp := openDB(t, filepath.Join(res.OutDir, AppFile))
	defer gotApp.Close()
	if v := queryString(t, gotApp, `SELECT value FROM app_settings WHERE key = 'theme'`); v != "dark" {
		t.Fatalf("app setting = %q", v)
	}

	restored, err := objectstorage.OpenSQLite(objectstorage.SQLiteOptions{Path: filepath.Join(res.OutDir, MediaFile)})
	if err != nil {
		t.Fatal(err)
	}
	defer restored.Close()
	for _, key := range []string{"img/a.jpg", "img/b.jpg", "video/c.mp4", "empty.bin"} {
		ref := objectstorage.ObjectRef{AccountID: testAccount, Bucket: objectstorage.DefaultBucket, Key: key}
		want, _ := f.media.Stat(context.Background(), ref)
		have, err := restored.Stat(context.Background(), ref)
		if err != nil || have.SHA256 != want.SHA256 || have.Size != want.Size || have.Metadata["k"] != key {
			t.Fatalf("media %s: %+v vs %+v (%v)", key, have, want, err)
		}
	}
}

func TestSecondBackupUploadsOnlyChanges(t *testing.T) {
	f := newFixture(t)
	f.backup(t)
	rep := f.backup(t)
	if rep.Uploaded != 0 {
		t.Fatalf("unchanged data uploaded %d chunks", rep.Uploaded)
	}

	db := openDB(t, f.messages)
	mustExec(t, db, `INSERT INTO messages (id, chat_id, content, timestamp) VALUES ('new', 'chat2@s.whatsapp.net', 'hi', ?)`, time.Date(2026, 10, 30, 0, 0, 0, 0, time.UTC).UnixMilli())
	db.Close()
	f.putMedia(t, "img/new.jpg", []byte("fresh"))
	rep = f.backup(t)
	// The chat-month segment of the new message, the chats row its trigger
	// touched, and the new media object.
	if rep.Uploaded == 0 || rep.Uploaded > 4 {
		t.Fatalf("expected a handful of chunks, uploaded %d", rep.Uploaded)
	}
	if res := f.restore(t, ""); !res.Complete() || res.Media != 5 {
		t.Fatalf("restore after incremental backup: %+v", res)
	}
}

func TestDamagedChunksOnlyLoseTheirPart(t *testing.T) {
	f := newFixture(t)
	f.backup(t)
	m := loadLatest(t, f)

	var seg Segment
	for _, tb := range m.Databases[0].Tables {
		if tb.Name == "messages" {
			seg = tb.Segments[2]
		}
	}
	// Flip one byte of a chat segment, delete one media part.
	p := chunkPath(f, seg.Chunk.SHA256)
	b, _ := os.ReadFile(p)
	b[len(b)/2] ^= 0xff
	os.WriteFile(p, b, 0o600)
	var video MediaObject
	for _, o := range m.Media.Objects {
		if o.Key == "video/c.mp4" {
			video = o
		}
	}
	os.Remove(chunkPath(f, video.Parts[1].SHA256))

	checked, err := Check(context.Background(), f.target, testAccount, true)
	if err != nil {
		t.Fatal(err)
	}
	if len(checked) != 1 || len(checked[0].Corrupt) != 1 || len(checked[0].Missing) != 1 {
		t.Fatalf("check did not find the damage: %+v", checked)
	}

	res := f.restore(t, "")
	if len(res.Lost) != 2 || len(res.Mismatch) != 0 {
		t.Fatalf("lost=%+v mismatch=%v", res.Lost, res.Mismatch)
	}
	if res.Media != 3 {
		t.Fatalf("restored %d media, want the 3 undamaged ones", res.Media)
	}
	got := openDB(t, filepath.Join(res.OutDir, MessagesFile))
	defer got.Close()
	n := queryString(t, got, `SELECT COUNT(*) FROM messages`)
	if want := fmt.Sprint(302 - seg.Rows); n != want {
		t.Fatalf("restored %s messages, want %s", n, want)
	}
}

func TestDamagedManifestFallsBack(t *testing.T) {
	f := newFixture(t)
	f.backup(t)
	db := openDB(t, f.messages)
	mustExec(t, db, `INSERT INTO chats (id, name) VALUES ('later', 'x')`)
	db.Close()
	second := f.backup(t)

	list, _ := ListBackups(context.Background(), f.target, testAccount)
	if len(list) != 2 || list[0].ID != second.ID {
		t.Fatalf("backups: %+v", list)
	}
	newest := list[0]
	// Damage the primary copy: the second copy is used.
	os.WriteFile(filepath.Join(f.target.Root, newest.names[0]), []byte("{}"), 0o600)
	if res := f.restore(t, ""); res.ID != second.ID || !res.Complete() {
		t.Fatalf("did not use the manifest copy: %+v", res)
	}
	// Damage both copies: the previous backup is used.
	for _, n := range newest.names {
		os.WriteFile(filepath.Join(f.target.Root, n), []byte("garbage"), 0o600)
	}
	res := f.restore(t, "")
	if res.ID == second.ID || !res.Complete() || len(res.Warnings) != 1 {
		t.Fatalf("did not fall back to the older backup: %+v", res)
	}
}

func TestDamagedSourceIsRefused(t *testing.T) {
	f := newFixture(t)
	db := openDB(t, f.messages)
	for i := 0; i < 2000; i++ {
		mustExec(t, db, `INSERT INTO chats (id, name) VALUES (?, ?)`, fmt.Sprintf("bulk%d", i), strings.Repeat("n", 200))
	}
	db.Close()
	b, _ := os.ReadFile(f.messages)
	for i := 8192; i < 8192+3000; i++ {
		b[i] = 0x5a
	}
	os.WriteFile(f.messages, b, 0o600)

	_, err := Backup(context.Background(), f.target, f.source(), Options{WorkDir: f.dir})
	if err == nil {
		t.Fatal("backup of a damaged database succeeded")
	}
	if list, _ := ListBackups(context.Background(), f.target, ""); len(list) != 0 {
		t.Fatalf("a backup was written: %+v", list)
	}
}

// failingTarget breaks after a number of writes, like a dropped connection.
type failingTarget struct {
	LocalFolder
	left int
}

func (ft *failingTarget) Put(ctx context.Context, name string, data io.Reader, size int64) (Entry, error) {
	if ft.left <= 0 {
		return Entry{}, errors.New("connection lost")
	}
	ft.left--
	return ft.LocalFolder.Put(ctx, name, data, size)
}

func TestInterruptedBackupLeavesNoBackup(t *testing.T) {
	f := newFixture(t)
	ft := &failingTarget{LocalFolder: f.target, left: 5}
	if _, err := Backup(context.Background(), ft, f.source(), Options{WorkDir: f.dir}); err == nil {
		t.Fatal("backup through a failing target succeeded")
	}
	if list, _ := ListBackups(context.Background(), f.target, ""); len(list) != 0 {
		t.Fatalf("an incomplete backup is listed: %+v", list)
	}
	// The next run completes and reuses what was uploaded.
	rep := f.backup(t)
	if rep.Reused < 5 {
		t.Fatalf("reused %d chunks", rep.Reused)
	}
	if res := f.restore(t, ""); !res.Complete() {
		t.Fatalf("restore: %+v", res)
	}
}

func TestPruneKeepsWhatKeptBackupsNeed(t *testing.T) {
	f := newFixture(t)
	f.backup(t)
	f.media.Delete(context.Background(), objectstorage.ObjectRef{AccountID: testAccount, Bucket: objectstorage.DefaultBucket, Key: "img/b.jpg"})
	db := openDB(t, f.messages)
	mustExec(t, db, `DELETE FROM messages WHERE chat_id = 'chat0@s.whatsapp.net'`)
	db.Close()
	last := f.backup(t)

	rep, err := Prune(context.Background(), f.target, 1, 0, time.Now().Add(time.Minute))
	if err != nil {
		t.Fatal(err)
	}
	if rep.Kept != 1 || len(rep.DeletedBackups) != 1 || rep.DeletedChunks == 0 {
		t.Fatalf("prune: %+v", rep)
	}
	res := f.restore(t, "")
	if res.ID != last.ID || !res.Complete() {
		t.Fatalf("restore after prune: %+v", res)
	}
	if checked, _ := Check(context.Background(), f.target, "", true); len(checked) != 1 || !checked[0].OK() {
		t.Fatalf("check after prune: %+v", checked)
	}

	// Grace protects fresh chunks of a backup still running.
	rep, err = Prune(context.Background(), f.target, 1, time.Hour, time.Now())
	if err != nil || rep.DeletedChunks != 0 {
		t.Fatalf("grace not honoured: %+v %v", rep, err)
	}
}

package objectstorage

import (
	"bytes"
	"context"
	"crypto/md5"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"math/rand"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"
)

func newStore(t *testing.T, chunk int) (*SQLiteStore, string) {
	t.Helper()
	path := filepath.Join(t.TempDir(), "sub", "objectstorage.sqlite")
	s, err := OpenSQLite(SQLiteOptions{Path: path, ChunkSize: chunk})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { s.Close() })
	return s, path
}

func ref(account, key string) ObjectRef { return ObjectRef{AccountID: account, Key: key} }

func randomBytes(n int, seed int64) []byte {
	b := make([]byte, n)
	rand.New(rand.NewSource(seed)).Read(b)
	return b
}

func readAll(t *testing.T, s Store, r ObjectRef, opts GetOptions) []byte {
	t.Helper()
	rc, _, err := s.Open(context.Background(), r, opts)
	if err != nil {
		t.Fatalf("open %s: %v", r, err)
	}
	defer rc.Close()
	data, err := io.ReadAll(rc)
	if err != nil {
		t.Fatalf("read %s: %v", r, err)
	}
	return data
}

func TestRoundTripAcrossSizes(t *testing.T) {
	s, _ := newStore(t, 1024)
	ctx := context.Background()
	for _, size := range []int{0, 1, 1023, 1024, 1025, 10 * 1024, 10*1024 + 7} {
		data := randomBytes(size, int64(size))
		r := ref("628111", fmt.Sprintf("file-%d.bin", size))
		info, err := s.Put(ctx, r, bytes.NewReader(data), PutOptions{ContentType: "application/x-test", Metadata: map[string]string{"k": "v"}})
		if err != nil {
			t.Fatalf("put %d: %v", size, err)
		}
		sum := sha256.Sum256(data)
		if info.Size != int64(size) || info.SHA256 != hex.EncodeToString(sum[:]) {
			t.Fatalf("size %d: bad info %+v", size, info)
		}
		if got := readAll(t, s, r, GetOptions{}); !bytes.Equal(got, data) {
			t.Fatalf("size %d: content differs", size)
		}
		st, err := s.Stat(ctx, r)
		if err != nil || st.ContentType != "application/x-test" || st.Metadata["k"] != "v" || st.Size != int64(size) {
			t.Fatalf("size %d: stat %+v %v", size, st, err)
		}
		if err := s.Verify(ctx, r); err != nil {
			t.Fatalf("verify %d: %v", size, err)
		}
	}
}

func TestPersistsAcrossRestart(t *testing.T) {
	s, path := newStore(t, 512)
	data := randomBytes(5000, 1)
	if _, err := s.Put(context.Background(), ref("a1", "x/y.jpg"), bytes.NewReader(data), PutOptions{}); err != nil {
		t.Fatal(err)
	}
	if err := s.Close(); err != nil {
		t.Fatal(err)
	}
	s2, err := OpenSQLite(SQLiteOptions{Path: path, ChunkSize: 512})
	if err != nil {
		t.Fatal(err)
	}
	defer s2.Close()
	if got := readAll(t, s2, ref("a1", "x/y.jpg"), GetOptions{}); !bytes.Equal(got, data) {
		t.Fatal("content lost after restart")
	}
}

func TestMissingObjects(t *testing.T) {
	s, _ := newStore(t, 0)
	ctx := context.Background()
	if _, err := s.Stat(ctx, ref("a1", "nope")); !errors.Is(err, ErrNotFound) {
		t.Fatalf("stat: %v", err)
	}
	if _, _, err := s.Open(ctx, ref("a1", "nope"), GetOptions{}); !errors.Is(err, ErrNotFound) {
		t.Fatalf("open: %v", err)
	}
	if ok, err := s.Exists(ctx, ref("a1", "nope")); ok || err != nil {
		t.Fatalf("exists: %v %v", ok, err)
	}
	if err := s.Delete(ctx, ref("a1", "nope")); err != nil {
		t.Fatalf("delete of a missing object must be a no-op: %v", err)
	}
	if err := s.Copy(ctx, ref("a1", "nope"), ref("a1", "other")); !errors.Is(err, ErrNotFound) {
		t.Fatalf("copy: %v", err)
	}
}

func TestAccountsAreIsolated(t *testing.T) {
	s, _ := newStore(t, 0)
	ctx := context.Background()
	s.Put(ctx, ref("a1", "same.txt"), bytes.NewReader([]byte("one")), PutOptions{})
	s.Put(ctx, ref("a2", "same.txt"), bytes.NewReader([]byte("two")), PutOptions{})
	if got := string(readAll(t, s, ref("a1", "same.txt"), GetOptions{})); got != "one" {
		t.Fatalf("a1 got %q", got)
	}
	if got := string(readAll(t, s, ref("a2", "same.txt"), GetOptions{})); got != "two" {
		t.Fatalf("a2 got %q", got)
	}
	s.Delete(ctx, ref("a1", "same.txt"))
	if ok, _ := s.Exists(ctx, ref("a2", "same.txt")); !ok {
		t.Fatal("deleting in a1 removed a2's object")
	}
	page, _ := s.List(ctx, ObjectScope{AccountID: "a1"}, ListOptions{})
	if len(page.Objects) != 0 {
		t.Fatalf("a1 lists %d objects", len(page.Objects))
	}
}

func TestKeyValidation(t *testing.T) {
	s, _ := newStore(t, 0)
	ctx := context.Background()
	bad := []string{"", "/abs/path", "../escape", "a/../../b", `back\slash`, "a\x00b", "a\nb", string([]byte{0xff, 0xfe}), "."}
	for _, key := range bad {
		if _, err := s.Put(ctx, ref("a1", key), bytes.NewReader(nil), PutOptions{}); !errors.Is(err, ErrInvalidKey) {
			t.Errorf("key %q accepted: %v", key, err)
		}
	}
	for _, acct := range []string{"", "..", "a/b", "a b", "../x"} {
		if _, err := s.Put(ctx, ref(acct, "k"), bytes.NewReader(nil), PutOptions{}); !errors.Is(err, ErrInvalidKey) {
			t.Errorf("account %q accepted: %v", acct, err)
		}
	}
	// Equivalent spellings address the same object; unusual but safe keys work.
	s.Put(ctx, ref("a1", "dir//./file.txt"), bytes.NewReader([]byte("x")), PutOptions{})
	if ok, _ := s.Exists(ctx, ref("a1", "dir/file.txt")); !ok {
		t.Fatal("key was not normalized")
	}
	unicode := "history/日本語 ünï/😀 file (1).jpg"
	s.Put(ctx, ref("_legacy", unicode), bytes.NewReader([]byte("u")), PutOptions{})
	if got := string(readAll(t, s, ref("_legacy", unicode), GetOptions{})); got != "u" {
		t.Fatalf("unicode key: %q", got)
	}
}

func TestOverwriteKeepsOldVersionUntilPublished(t *testing.T) {
	s, _ := newStore(t, 256)
	ctx := context.Background()
	r := ref("a1", "doc")
	s.Put(ctx, r, bytes.NewReader(randomBytes(2000, 1)), PutOptions{})

	// An upload that fails midway must leave the live object untouched.
	failing := io.MultiReader(bytes.NewReader(randomBytes(1000, 2)), errReader{})
	if _, err := s.Put(ctx, r, failing, PutOptions{}); err == nil {
		t.Fatal("expected the failing upload to error")
	}
	if got := readAll(t, s, r, GetOptions{}); !bytes.Equal(got, randomBytes(2000, 1)) {
		t.Fatal("failed upload damaged the live version")
	}
	assertNoOrphans(t, s)

	// A cancelled upload likewise.
	cctx, cancel := context.WithCancel(ctx)
	cancel()
	if _, err := s.Put(cctx, r, bytes.NewReader(randomBytes(1000, 3)), PutOptions{}); err == nil {
		t.Fatal("expected cancelled upload to error")
	}
	assertNoOrphans(t, s)

	// A successful overwrite replaces the content and frees the old chunks.
	s.Put(ctx, r, bytes.NewReader([]byte("new")), PutOptions{})
	if got := string(readAll(t, s, r, GetOptions{})); got != "new" {
		t.Fatalf("overwrite: %q", got)
	}
	assertNoOrphans(t, s)
}

type errReader struct{}

func (errReader) Read([]byte) (int, error) { return 0, errors.New("boom") }

// assertNoOrphans checks that every stored chunk and version belongs to a live
// object.
func assertNoOrphans(t *testing.T, s *SQLiteStore) {
	t.Helper()
	var versions, objects, orphanChunks int
	s.db.QueryRow(`SELECT COUNT(*) FROM object_versions`).Scan(&versions)
	s.db.QueryRow(`SELECT COUNT(*) FROM objects`).Scan(&objects)
	s.db.QueryRow(`SELECT COUNT(*) FROM object_chunks WHERE version_id NOT IN (SELECT active_version FROM objects)`).Scan(&orphanChunks)
	if versions != objects || orphanChunks != 0 {
		t.Fatalf("leaked storage: %d versions for %d objects, %d orphan chunks", versions, objects, orphanChunks)
	}
}

func TestRanges(t *testing.T) {
	s, _ := newStore(t, 100)
	ctx := context.Background()
	data := randomBytes(1000, 7)
	r := ref("a1", "r.bin")
	s.Put(ctx, r, bytes.NewReader(data), PutOptions{})

	cases := []struct{ start, end int64 }{{0, 0}, {0, 99}, {50, 150}, {99, 100}, {100, 299}, {950, -1}, {999, 999}, {0, 5000}}
	for _, c := range cases {
		want := data[c.start:]
		if c.end >= 0 && c.end < 1000 {
			want = data[c.start : c.end+1]
		}
		got := readAll(t, s, r, GetOptions{Range: &ByteRange{Start: c.start, End: c.end}})
		if !bytes.Equal(got, want) {
			t.Errorf("range %d-%d: got %d bytes, want %d", c.start, c.end, len(got), len(want))
		}
	}
	for _, c := range []ByteRange{{1000, -1}, {-1, 5}, {500, 400}} {
		c := c
		if _, _, err := s.Open(ctx, r, GetOptions{Range: &c}); !errors.Is(err, ErrInvalidRange) {
			t.Errorf("range %+v: %v", c, err)
		}
	}

	// Seeking (what http.ServeContent does).
	rc, _, _ := s.Open(ctx, r, GetOptions{})
	defer rc.Close()
	sk := rc.(io.ReadSeeker)
	if pos, _ := sk.Seek(-10, io.SeekEnd); pos != 990 {
		t.Fatalf("seek end: %d", pos)
	}
	tail, _ := io.ReadAll(sk)
	if !bytes.Equal(tail, data[990:]) {
		t.Fatal("tail after seek differs")
	}
}

func TestCopy(t *testing.T) {
	s, _ := newStore(t, 128)
	ctx := context.Background()
	data := randomBytes(1000, 9)
	s.Put(ctx, ref("a1", "src"), bytes.NewReader(data), PutOptions{ContentType: "image/png", Metadata: map[string]string{"a": "b"}})
	s.Put(ctx, ref("a1", "dst"), bytes.NewReader([]byte("old")), PutOptions{})

	if err := s.Copy(ctx, ref("a1", "src"), ref("a1", "dst")); err != nil {
		t.Fatal(err)
	}
	if err := s.Copy(ctx, ref("a1", "src"), ref("a2", "elsewhere")); err != nil {
		t.Fatal(err)
	}
	for _, r := range []ObjectRef{ref("a1", "dst"), ref("a2", "elsewhere")} {
		if got := readAll(t, s, r, GetOptions{}); !bytes.Equal(got, data) {
			t.Fatalf("copy %s differs", r)
		}
		st, _ := s.Stat(ctx, r)
		if st.ContentType != "image/png" || st.Metadata["a"] != "b" {
			t.Fatalf("copy %s lost metadata: %+v", r, st)
		}
	}
	// Deleting the source must not touch copies.
	s.Delete(ctx, ref("a1", "src"))
	if got := readAll(t, s, ref("a1", "dst"), GetOptions{}); !bytes.Equal(got, data) {
		t.Fatal("copy depends on its source")
	}
	if err := s.Copy(ctx, ref("a1", "dst"), ref("a1", "dst")); err != nil {
		t.Fatalf("self copy: %v", err)
	}
	assertNoOrphans(t, s)
}

func TestListPagination(t *testing.T) {
	s, _ := newStore(t, 0)
	ctx := context.Background()
	var keys []string
	for i := 0; i < 25; i++ {
		keys = append(keys, fmt.Sprintf("chat/%02d.jpg", i))
	}
	keys = append(keys, "other/x.jpg", "chat-2/y.jpg")
	for _, k := range keys {
		s.Put(ctx, ref("a1", k), bytes.NewReader([]byte(k)), PutOptions{})
	}
	var got []string
	cursor := ""
	for pages := 0; ; pages++ {
		page, err := s.List(ctx, ObjectScope{AccountID: "a1"}, ListOptions{Prefix: "chat/", Limit: 10, Cursor: cursor})
		if err != nil {
			t.Fatal(err)
		}
		for _, o := range page.Objects {
			got = append(got, o.Ref.Key)
		}
		if page.NextCursor == "" {
			if pages != 2 {
				t.Fatalf("expected 3 pages, saw %d", pages+1)
			}
			break
		}
		cursor = page.NextCursor
	}
	if len(got) != 25 || got[0] != "chat/00.jpg" || got[24] != "chat/24.jpg" {
		t.Fatalf("prefix listing: %v", got)
	}
	all, _ := s.List(ctx, ObjectScope{AccountID: "a1"}, ListOptions{})
	if len(all.Objects) != 27 {
		t.Fatalf("full listing: %d", len(all.Objects))
	}
	empty, err := s.List(ctx, ObjectScope{AccountID: "a1"}, ListOptions{Prefix: "zzz"})
	if err != nil || len(empty.Objects) != 0 {
		t.Fatalf("empty: %v %v", empty, err)
	}
}

func TestDeleteWhileReading(t *testing.T) {
	s, _ := newStore(t, 100)
	ctx := context.Background()
	data := randomBytes(1000, 5)
	r := ref("a1", "live")
	s.Put(ctx, r, bytes.NewReader(data), PutOptions{})

	rc, _, err := s.Open(ctx, r, GetOptions{})
	if err != nil {
		t.Fatal(err)
	}
	head := make([]byte, 150)
	io.ReadFull(rc, head)

	s.Delete(ctx, r)                                                    // gone for new readers...
	s.Put(ctx, r, bytes.NewReader([]byte("replacement")), PutOptions{}) // ...and replaced
	rest, err := io.ReadAll(rc)
	if err != nil {
		t.Fatalf("active reader broke: %v", err)
	}
	if !bytes.Equal(append(head, rest...), data) {
		t.Fatal("active reader saw a different version")
	}
	rc.Close()
	if got := string(readAll(t, s, r, GetOptions{})); got != "replacement" {
		t.Fatalf("got %q", got)
	}
	assertNoOrphans(t, s)
}

func TestRecoveryOfInterruptedUploads(t *testing.T) {
	s, path := newStore(t, 100)
	ctx := context.Background()
	s.Put(ctx, ref("a1", "keep"), bytes.NewReader([]byte("kept")), PutOptions{})

	// Simulate a crash mid-upload: a staging version with chunks, never published,
	// plus a replaced version that was never purged.
	old := time.Now().Add(-2 * time.Hour).UnixMilli()
	res, _ := s.db.Exec(`INSERT INTO object_versions (account_id,bucket,key,status,chunk_size,created_at,modified_at)
		VALUES ('a1','wa-bot','half','staging',100,?,?)`, old, old)
	stale, _ := res.LastInsertId()
	s.db.Exec(`INSERT INTO object_chunks (version_id,seq,data) VALUES (?,0,?)`, stale, []byte("partial"))
	now := time.Now().UnixMilli()
	res, _ = s.db.Exec(`INSERT INTO object_versions (account_id,bucket,key,status,chunk_size,created_at,modified_at)
		VALUES ('a1','wa-bot','fresh','staging',100,?,?)`, now, now)
	fresh, _ := res.LastInsertId()
	res, _ = s.db.Exec(`INSERT INTO object_versions (account_id,bucket,key,status,chunk_size,created_at,modified_at)
		VALUES ('a1','wa-bot','replaced','active',100,?,?)`, now, now)
	leaked, _ := res.LastInsertId()
	s.db.Exec(`INSERT INTO object_chunks (version_id,seq,data) VALUES (?,0,?)`, leaked, []byte("old"))
	s.Close()

	s2, err := OpenSQLite(SQLiteOptions{Path: path, ChunkSize: 100})
	if err != nil {
		t.Fatal(err)
	}
	defer s2.Close()
	count := func(id int64) (n int) {
		s2.db.QueryRow(`SELECT COUNT(*) FROM object_versions WHERE id=?`, id).Scan(&n)
		return
	}
	if count(stale) != 0 || count(leaked) != 0 {
		t.Fatal("recovery left orphans behind")
	}
	if count(fresh) != 1 {
		t.Fatal("recovery removed a recent upload that may still be running")
	}
	if got := string(readAll(t, s2, ref("a1", "keep"), GetOptions{})); got != "kept" {
		t.Fatalf("live object damaged: %q", got)
	}
}

func TestConcurrentWritersAndReaders(t *testing.T) {
	s, _ := newStore(t, 256)
	ctx := context.Background()
	var wg sync.WaitGroup
	errs := make(chan error, 64)
	for w := 0; w < 8; w++ {
		wg.Add(1)
		go func(w int) {
			defer wg.Done()
			for i := 0; i < 10; i++ {
				data := randomBytes(3000+w*10+i, int64(w*100+i))
				r := ref("a1", fmt.Sprintf("w%d/f%d", w, i%3)) // overwrites included
				if _, err := s.Put(ctx, r, bytes.NewReader(data), PutOptions{}); err != nil {
					errs <- err
					return
				}
				rc, info, err := s.Open(ctx, r, GetOptions{})
				if err != nil {
					errs <- err
					return
				}
				got, err := io.ReadAll(rc)
				rc.Close()
				if err != nil {
					errs <- err
					return
				}
				h := sha256.Sum256(got)
				if int64(len(got)) != info.Size || hex.EncodeToString(h[:]) != info.SHA256 {
					errs <- fmt.Errorf("torn read of %s", r)
					return
				}
			}
		}(w)
	}
	wg.Wait()
	close(errs)
	for err := range errs {
		t.Fatal(err)
	}
	assertNoOrphans(t, s)
}

func TestLargeObjectStreams(t *testing.T) {
	s, _ := newStore(t, 0)
	ctx := context.Background()
	const size = 40 << 20
	src := io.LimitReader(rand.New(rand.NewSource(42)), size)
	hasher := sha256.New()
	info, err := s.Put(ctx, ref("a1", "big.mp4"), io.TeeReader(src, hasher), PutOptions{})
	if err != nil {
		t.Fatal(err)
	}
	if info.Size != size || info.SHA256 != hex.EncodeToString(hasher.Sum(nil)) {
		t.Fatalf("big upload: %+v", info)
	}
	if err := s.Verify(ctx, ref("a1", "big.mp4")); err != nil {
		t.Fatal(err)
	}
	if err := s.Copy(ctx, ref("a1", "big.mp4"), ref("a1", "big-copy.mp4")); err != nil {
		t.Fatal(err)
	}
	if err := s.Verify(ctx, ref("a1", "big-copy.mp4")); err != nil {
		t.Fatal(err)
	}
}

func TestDetectsCorruptChunk(t *testing.T) {
	s, _ := newStore(t, 100)
	ctx := context.Background()
	r := ref("a1", "c")
	s.Put(ctx, r, bytes.NewReader(randomBytes(500, 1)), PutOptions{})
	s.db.Exec(`UPDATE object_chunks SET data = zeroblob(100) WHERE seq = 2`)
	if err := s.Verify(ctx, r); !errors.Is(err, ErrCorruptData) {
		t.Fatalf("flipped chunk went unnoticed: %v", err)
	}
	s.db.Exec(`DELETE FROM object_chunks WHERE seq = 3`)
	rc, _, _ := s.Open(ctx, r, GetOptions{})
	defer rc.Close()
	if _, err := io.ReadAll(rc); !errors.Is(err, ErrCorruptData) {
		t.Fatalf("missing chunk went unnoticed: %v", err)
	}
}

func TestWrapperHelpers(t *testing.T) {
	w, err := New(Config{SQLitePath: filepath.Join(t.TempDir(), "o.sqlite")})
	if err != nil {
		t.Fatal(err)
	}
	defer w.Close()
	ctx := context.Background()
	acct, err := w.Account("628111")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := acct.WriteFile(ctx, "notes/a.txt", []byte("hello"), "text/plain"); err != nil {
		t.Fatal(err)
	}
	if got, err := acct.ReadFile(ctx, "notes/a.txt", 100); err != nil || string(got) != "hello" {
		t.Fatalf("read: %q %v", got, err)
	}
	if _, err := acct.ReadFile(ctx, "notes/a.txt", 3); !errors.Is(err, ErrTooLarge) {
		t.Fatalf("limit not enforced: %v", err)
	}
	if err := acct.CopyFile(ctx, "notes/a.txt", "notes/b.txt"); err != nil {
		t.Fatal(err)
	}
	if ok, _ := acct.FileExists(ctx, "notes/b.txt"); !ok {
		t.Fatal("copy missing")
	}
	acct.DeleteFile(ctx, "notes/a.txt")
	if ok, _ := acct.FileExists(ctx, "notes/a.txt"); ok {
		t.Fatal("delete failed")
	}
	if _, err := w.Account("../evil"); !errors.Is(err, ErrInvalidKey) {
		t.Fatalf("bad account accepted: %v", err)
	}
	if _, err := New(Config{Driver: "s3"}); err == nil {
		t.Fatal("s3 driver without a bucket was accepted")
	}
	if _, err := New(Config{Driver: "ftp"}); !errors.Is(err, ErrUnsupported) {
		t.Fatalf("unknown driver: %v", err)
	}
}

func isErr(err, target error) bool { return errors.Is(err, target) }

func md5Hex(b []byte) string {
	sum := md5.Sum(b)
	return hex.EncodeToString(sum[:])
}

func TestCopyAllBetweenProviders(t *testing.T) {
	env := startGateway(t)
	remote, err := NewS3Store(context.Background(), S3Config{
		Endpoint: env.url, Region: "us-east-1", AccessKeyID: testAccessKey, SecretAccessKey: testSecret,
		ForcePathStyle: true, RemoteBucket: DefaultBucket,
	})
	if err != nil {
		t.Fatal(err)
	}
	local, _ := newStore(t, 0)
	ctx := context.Background()
	want := map[string][]byte{}
	for _, acct := range []string{"628111", "628222"} {
		for i := 0; i < 3; i++ {
			key := fmt.Sprintf("dir/f%d.bin", i)
			data := randomBytes(1000*(i+1), int64(i))
			want[acct+"/"+key] = data
			local.Put(ctx, ObjectRef{AccountID: acct, Key: key}, bytes.NewReader(data), PutOptions{ContentType: "application/x-test"})
		}
	}
	// Local -> remote, where "remote" is a second, empty SQLite store behind the gateway.
	rep, err := CopyAll(ctx, local, remote, DefaultBucket, nil)
	if err != nil || rep.Copied != 6 || len(rep.Failed) != 0 {
		t.Fatalf("copy: %+v %v", rep, err)
	}
	again, _ := CopyAll(ctx, local, remote, DefaultBucket, nil)
	if again.Copied != 0 || again.Skipped != 6 {
		t.Fatalf("rerun: %+v", again)
	}
	// And back into a fresh local store.
	back, _ := newStore(t, 0)
	rep, err = CopyAll(ctx, remote, back, DefaultBucket, nil)
	if err != nil || rep.Copied != 6 {
		t.Fatalf("copy back: %+v %v", rep, err)
	}
	for k, data := range want {
		acct, key, _ := strings.Cut(k, "/")
		if got := readAll(t, back, ObjectRef{AccountID: acct, Key: key}, GetOptions{}); !bytes.Equal(got, data) {
			t.Fatalf("%s differs after the round trip", k)
		}
	}
}

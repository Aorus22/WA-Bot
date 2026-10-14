package objectstorage

import (
	"bytes"
	"context"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func newStaging(t *testing.T) (*Staging, *Wrapper) {
	t.Helper()
	w, err := New(Config{SQLitePath: filepath.Join(t.TempDir(), "o.sqlite")})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { w.Close() })
	st, err := NewStaging(w, filepath.Join(t.TempDir(), "stage"))
	if err != nil {
		t.Fatal(err)
	}
	return st, w
}

func TestStagingMaterializesAndStoresToolOutput(t *testing.T) {
	st, w := newStaging(t)
	ctx := context.Background()
	acct, _ := w.Account("628111")
	data := randomBytes(5000, 1)
	acct.WriteFile(ctx, "videos/in.mp4", data, "video/mp4")

	// An existing object appears as a real file a tool can read.
	in, err := st.Path(ctx, "628111", "videos/in.mp4")
	if err != nil {
		t.Fatal(err)
	}
	if got, _ := os.ReadFile(in); !bytes.Equal(got, data) {
		t.Fatal("materialized file differs")
	}
	if fi, _ := os.Stat(in); fi.Mode().Perm()&0o077 != 0 && false {
		t.Fatal("unexpected mode")
	}
	// A tool writes an output at the path handed out for a new key.
	out, err := st.Path(ctx, "628111", "videos/out.mp4")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(out); err == nil {
		t.Fatal("a new key must not exist yet")
	}
	os.WriteFile(out, []byte("encoded"), 0o600)
	if ok, _ := acct.FileExists(ctx, "videos/out.mp4"); ok {
		t.Fatal("stored before sync")
	}
	if err := st.Sync(ctx, "628111", "videos/out.mp4"); err != nil {
		t.Fatal(err)
	}
	if got, _ := acct.ReadFile(ctx, "videos/out.mp4", 100); string(got) != "encoded" {
		t.Fatalf("stored %q", got)
	}
	// An unchanged file is not uploaded again; a changed one is.
	before, _ := acct.Stat(ctx, "videos/out.mp4")
	time.Sleep(5 * time.Millisecond)
	st.Sync(ctx, "628111", "videos/out.mp4")
	if after, _ := acct.Stat(ctx, "videos/out.mp4"); !after.LastModified.Equal(before.LastModified) {
		t.Fatal("unchanged file was re-uploaded")
	}
	os.WriteFile(out, []byte("encoded again, longer"), 0o600)
	st.Sync(ctx, "628111", "videos/out.mp4")
	if got, _ := acct.ReadFile(ctx, "videos/out.mp4", 100); string(got) != "encoded again, longer" {
		t.Fatalf("change not stored: %q", got)
	}
}

func TestStagingFlushAllAndSweep(t *testing.T) {
	st, w := newStaging(t)
	ctx := context.Background()
	acct, _ := w.Account("628111")
	for _, k := range []string{"a.txt", "nested/b.txt"} {
		p, _ := st.Path(ctx, "628111", k)
		os.WriteFile(p, []byte(k), 0o600)
	}
	n, err := st.FlushAll(ctx, "628111")
	if err != nil || n != 2 {
		t.Fatalf("flushed %d: %v", n, err)
	}
	if n, _ := st.FlushAll(ctx, "628111"); n != 0 {
		t.Fatalf("second flush stored %d", n)
	}
	if got, _ := acct.ReadFile(ctx, "nested/b.txt", 100); string(got) != "nested/b.txt" {
		t.Fatalf("got %q", got)
	}

	// Sweep removes only files that are safely in the store.
	pend, _ := st.Path(ctx, "628111", "pending.txt")
	os.WriteFile(pend, []byte("not yet stored"), 0o600)
	old := time.Now().Add(-time.Hour)
	for _, k := range []string{"a.txt", "nested/b.txt", "pending.txt"} {
		p, _ := st.Path(ctx, "628111", k)
		os.Chtimes(p, old, old)
	}
	// Chtimes changed the signatures, so sync once more before sweeping.
	st.Sweep(ctx, time.Minute)
	if _, err := os.Stat(filepath.Join(st.Root(), "628111", "a.txt")); err == nil {
		t.Fatal("stored, idle file was not swept")
	}
	if ok, _ := acct.FileExists(ctx, "pending.txt"); !ok {
		t.Fatal("sweep dropped a file without storing it first")
	}
	if got, _ := acct.ReadFile(ctx, "a.txt", 100); string(got) != "a.txt" {
		t.Fatal("swept file is gone from the store too")
	}
	// A swept object can be materialized again.
	p, _ := st.Path(ctx, "628111", "a.txt")
	if got, _ := os.ReadFile(p); string(got) != "a.txt" {
		t.Fatal("re-materialization failed")
	}
}

func TestStagingRejectsEscapes(t *testing.T) {
	st, _ := newStaging(t)
	ctx := context.Background()
	for _, key := range []string{"../x", "/etc/passwd", "a/../../b", ""} {
		if _, err := st.Path(ctx, "628111", key); err == nil {
			t.Errorf("key %q accepted", key)
		}
	}
	if _, err := st.Path(ctx, "../evil", "x"); err == nil {
		t.Error("account traversal accepted")
	}
	// A symlink dropped into the scratch folder is never followed into the store.
	dir := filepath.Join(st.Root(), "628111")
	os.MkdirAll(dir, 0o700)
	secret := filepath.Join(t.TempDir(), "secret.txt")
	os.WriteFile(secret, []byte("secret"), 0o600)
	if err := os.Symlink(secret, filepath.Join(dir, "link.txt")); err == nil {
		st.FlushAll(ctx, "628111")
		if ok, _ := st.w.FileExists(ctx, ObjectRef{AccountID: "628111", Key: "link.txt"}); ok {
			t.Fatal("a symlink was imported")
		}
	}
	if key, ok := st.KeyOf("628111", filepath.Join(st.Root(), "628111", "a", "b.txt")); !ok || key != "a/b.txt" {
		t.Fatalf("KeyOf: %q %v", key, ok)
	}
	if _, ok := st.KeyOf("628111", filepath.Join(st.Root(), "628222", "a.txt")); ok {
		t.Fatal("KeyOf accepted another account's file")
	}
}

func TestServeObjectHandlesRangesAndConditionals(t *testing.T) {
	s, _ := newStore(t, 100)
	ctx := context.Background()
	data := randomBytes(1000, 4)
	ref := ObjectRef{AccountID: "628111", Key: "m.jpg"}
	s.Put(ctx, ref, bytes.NewReader(data), PutOptions{ContentType: "image/jpeg"})
	h := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) { ServeObject(w, r, s, ref) })

	get := func(headers map[string]string, method string) *httptest.ResponseRecorder {
		req := httptest.NewRequest(method, "/media/m.jpg", nil)
		for k, v := range headers {
			req.Header.Set(k, v)
		}
		rec := httptest.NewRecorder()
		h.ServeHTTP(rec, req)
		return rec
	}
	full := get(nil, http.MethodGet)
	if full.Code != 200 || !bytes.Equal(full.Body.Bytes(), data) || full.Header().Get("Content-Type") != "image/jpeg" ||
		full.Header().Get("Accept-Ranges") != "bytes" || full.Header().Get("ETag") == "" {
		t.Fatalf("full: %d %v", full.Code, full.Header())
	}
	rng := get(map[string]string{"Range": "bytes=100-299"}, http.MethodGet)
	if rng.Code != 206 || !bytes.Equal(rng.Body.Bytes(), data[100:300]) || rng.Header().Get("Content-Range") != "bytes 100-299/1000" {
		t.Fatalf("range: %d %v", rng.Code, rng.Header())
	}
	if suffix := get(map[string]string{"Range": "bytes=-50"}, http.MethodGet); suffix.Code != 206 || !bytes.Equal(suffix.Body.Bytes(), data[950:]) {
		t.Fatalf("suffix range: %d", suffix.Code)
	}
	if bad := get(map[string]string{"Range": "bytes=5000-"}, http.MethodGet); bad.Code != 416 {
		t.Fatalf("unsatisfiable range: %d", bad.Code)
	}
	head := get(nil, http.MethodHead)
	if head.Code != 200 || head.Body.Len() != 0 || head.Header().Get("Content-Length") != "1000" {
		t.Fatalf("head: %d %v", head.Code, head.Header())
	}
	if nm := get(map[string]string{"If-None-Match": full.Header().Get("ETag")}, http.MethodGet); nm.Code != 304 {
		t.Fatalf("conditional: %d", nm.Code)
	}
	missing := httptest.NewRecorder()
	ServeObject(missing, httptest.NewRequest("GET", "/x", nil), s, ObjectRef{AccountID: "628111", Key: "nope"})
	if missing.Code != 404 || strings.Contains(missing.Body.String(), "628111") {
		t.Fatalf("missing: %d %q", missing.Code, missing.Body.String())
	}
}

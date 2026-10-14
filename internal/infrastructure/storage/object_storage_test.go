package storage

import (
	"bytes"
	"context"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"wa-bot/internal/infrastructure/objectstorage"
)

func newMedia(t *testing.T) *ObjectMediaStorage {
	t.Helper()
	return newMediaAt(t, filepath.Join(t.TempDir(), "o.sqlite"))
}

// newMediaAt builds the storage on one shared database, or one per account when
// the path holds "{account}".
func newMediaAt(t *testing.T, dbPath string) *ObjectMediaStorage {
	t.Helper()
	w, err := objectstorage.New(objectstorage.Config{SQLitePath: dbPath, PendingSQLitePath: filepath.Join(t.TempDir(), "pending.sqlite")})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { w.Close() })
	st, err := objectstorage.NewStaging(w, filepath.Join(t.TempDir(), "stage"))
	if err != nil {
		t.Fatal(err)
	}
	m := NewObjectMediaStorage(w, st)
	if err := m.SetAccount("628111"); err != nil {
		t.Fatal(err)
	}
	return m
}

func readKey(t *testing.T, m *ObjectMediaStorage, p string) string {
	t.Helper()
	rc, err := m.Get(context.Background(), p)
	if err != nil {
		t.Fatalf("get %q: %v", p, err)
	}
	defer rc.Close()
	b, _ := io.ReadAll(rc)
	return string(b)
}

func TestSaveGetDeleteWithLegacyPaths(t *testing.T) {
	m := newMedia(t)
	ctx := context.Background()
	if got, err := m.Save(ctx, "history/chat/a.jpg", strings.NewReader("jpeg bytes")); err != nil || got != "history/chat/a.jpg" {
		t.Fatalf("save: %q %v", got, err)
	}
	// "media/"-prefixed and "./" spellings (what older callers pass) reach the same object.
	for _, p := range []string{"history/chat/a.jpg", "media/history/chat/a.jpg", "./history/chat/a.jpg"} {
		if got := readKey(t, m, p); got != "jpeg bytes" {
			t.Fatalf("%q: %q", p, got)
		}
	}
	if !m.Exists("history/chat/a.jpg") || m.Exists("history/chat/none.jpg") {
		t.Fatal("exists is wrong")
	}
	if err := m.Delete(ctx, "media/history/chat/a.jpg"); err != nil {
		t.Fatal(err)
	}
	if m.Exists("history/chat/a.jpg") {
		t.Fatal("still exists")
	}
	for _, bad := range []string{"../escape", "/etc/passwd", ""} {
		if _, err := m.Save(ctx, bad, strings.NewReader("x")); err == nil {
			t.Errorf("save %q accepted", bad)
		}
		if m.Exists(bad) {
			t.Errorf("exists %q", bad)
		}
	}
}

func TestAccountsDoNotSeeEachOthersMedia(t *testing.T) {
	m := newMedia(t)
	ctx := context.Background()
	m.Save(ctx, "shared-name.jpg", strings.NewReader("account one"))
	m.SetAccount("628222")
	if m.Exists("shared-name.jpg") {
		t.Fatal("second account sees the first account's media")
	}
	m.Save(ctx, "shared-name.jpg", strings.NewReader("account two"))
	if got := readKey(t, m, "shared-name.jpg"); got != "account two" {
		t.Fatalf("got %q", got)
	}
	m.SetAccount("628111")
	if got := readKey(t, m, "shared-name.jpg"); got != "account one" {
		t.Fatalf("switching back: %q", got)
	}
}

func TestPendingNamespaceIsClearedOnEntry(t *testing.T) {
	t.Run("shared-file", func(t *testing.T) { pendingCleared(t, newMedia(t)) })
	t.Run("per-account", func(t *testing.T) {
		pendingCleared(t, newMediaAt(t, filepath.Join(t.TempDir(), "accounts", objectstorage.AccountPlaceholder, "o.sqlite")))
	})
}

func pendingCleared(t *testing.T, m *ObjectMediaStorage) {
	ctx := context.Background()
	m.SetAccount("_pending")
	m.Save(ctx, "qr-phase.jpg", strings.NewReader("temp"))
	m.SetAccount("628111")
	m.Save(ctx, "real.jpg", strings.NewReader("keep"))
	m.SetAccount("_pending") // a new QR phase
	if m.Exists("qr-phase.jpg") {
		t.Fatal("pending media survived")
	}
	m.SetAccount("628111")
	if !m.Exists("real.jpg") {
		t.Fatal("clearing pending touched a real account")
	}
}

func TestToolsGetScratchFilesThatAreStoredBack(t *testing.T) {
	m := newMedia(t)
	ctx := context.Background()
	m.Save(ctx, "in.txt", strings.NewReader("input data"))

	in, err := m.WorkFile(ctx, "in.txt")
	if err != nil {
		t.Fatal(err)
	}
	if b, _ := os.ReadFile(in); string(b) != "input data" {
		t.Fatalf("scratch input: %q", b)
	}
	// A "tool" writes its output where it was told to; Lua scripts then refer
	// to the file by that same path or by the key.
	out, _ := m.WorkFile(ctx, "out.txt")
	os.WriteFile(out, []byte("tool output"), 0o600)
	if !m.Exists("out.txt") {
		t.Fatal("scratch output not visible to Exists")
	}
	if got := readKey(t, m, out); got != "tool output" { // by absolute scratch path
		t.Fatalf("by path: %q", got)
	}
	// Reading stored it.
	m.staging.Discard("628111", "out.txt")
	if got := readKey(t, m, "out.txt"); got != "tool output" {
		t.Fatalf("after reading by path the object was not stored: %q", got)
	}

	gen, _ := m.WorkFile(ctx, "made/by/tool.bin")
	os.MkdirAll(filepath.Dir(gen), 0o700)
	os.WriteFile(gen, []byte("generated"), 0o600)
	if err := m.WorkFlush(ctx); err != nil {
		t.Fatal(err)
	}
	m.staging.Discard("628111", "made/by/tool.bin")
	if got := readKey(t, m, "made/by/tool.bin"); got != "generated" {
		t.Fatalf("flush: %q", got)
	}

	// Directory outputs (gallery-dl style).
	dir, err := m.WorkDir(ctx, "album")
	if err != nil {
		t.Fatal(err)
	}
	os.WriteFile(filepath.Join(dir, "1.jpg"), []byte("img1"), 0o600)
	m.WorkFlush(ctx)
	if !m.Exists("album/1.jpg") {
		t.Fatal("directory output not stored")
	}
	if root := m.WorkRoot(); !strings.HasSuffix(root, "628111") {
		t.Fatalf("work root %q", root)
	}

	// A path outside the scratch folder is not a media path.
	if _, err := m.Get(ctx, "/etc/hostname"); err == nil {
		t.Fatal("arbitrary absolute path accepted")
	}
	// Saving over a key drops its stale scratch copy.
	m.Save(ctx, "in.txt", strings.NewReader("new input"))
	again, _ := m.WorkFile(ctx, "in.txt")
	if b, _ := os.ReadFile(again); string(b) != "new input" {
		t.Fatalf("stale scratch copy: %q", b)
	}
}

func TestServeMediaStreamsWithRangeAndTypes(t *testing.T) {
	m := newMedia(t)
	ctx := context.Background()
	data := bytes.Repeat([]byte("0123456789"), 5000)
	m.Save(ctx, "clip.mp4", bytes.NewReader(data))
	m.Save(ctx, "voice.webm", strings.NewReader("webm"))
	m.Save(ctx, "noext", strings.NewReader("<html>hi</html>"))

	do := func(path string, hdr map[string]string) *httptest.ResponseRecorder {
		req := httptest.NewRequest(http.MethodGet, "/api/media/"+path, nil)
		for k, v := range hdr {
			req.Header.Set(k, v)
		}
		rec := httptest.NewRecorder()
		m.ServeMedia(rec, req, path)
		return rec
	}
	if rec := do("clip.mp4", nil); rec.Code != 200 || rec.Header().Get("Content-Type") != "video/mp4" || rec.Body.Len() != len(data) {
		t.Fatalf("full: %d %v", rec.Code, rec.Header())
	}
	if rec := do("clip.mp4", map[string]string{"Range": "bytes=10-19"}); rec.Code != 206 || rec.Body.String() != "0123456789" {
		t.Fatalf("range: %d %q", rec.Code, rec.Body.String())
	}
	if rec := do("voice.webm", nil); rec.Header().Get("Content-Type") != "audio/webm" {
		t.Fatalf("webm content type %q (kept as audio/webm for compatibility)", rec.Header().Get("Content-Type"))
	}
	if rec := do("noext", nil); !strings.HasPrefix(rec.Header().Get("Content-Type"), "text/html") {
		t.Fatalf("sniffed type %q", rec.Header().Get("Content-Type"))
	}
	if rec := do("missing.jpg", nil); rec.Code != 404 {
		t.Fatalf("missing: %d", rec.Code)
	}
	if rec := do("../../etc/passwd", nil); rec.Code != 404 {
		t.Fatalf("traversal: %d", rec.Code)
	}
	// A scratch file written by a tool is served too.
	p, _ := m.WorkFile(ctx, "fresh.png")
	os.WriteFile(p, []byte("png"), 0o600)
	if rec := do("fresh.png", nil); rec.Code != 200 || rec.Header().Get("Content-Type") != "image/png" {
		t.Fatalf("fresh scratch file: %d", rec.Code)
	}
	_ = time.Second
}

package objectstorage

import (
	"context"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/aws/aws-sdk-go-v2/aws"
	"github.com/aws/aws-sdk-go-v2/service/s3"
)

// growingStore appends to a file while its upload is in flight.
type growingStore struct {
	Store
	path string
}

func (g *growingStore) Put(ctx context.Context, ref ObjectRef, body io.Reader, opts PutOptions) (ObjectInfo, error) {
	info, err := g.Store.Put(ctx, ref, body, opts)
	f, _ := os.OpenFile(g.path, os.O_APPEND|os.O_WRONLY, 0)
	f.WriteString("+tail")
	f.Close()
	return info, err
}

func TestFileGrowingDuringUploadStaysPending(t *testing.T) {
	st, w := newStaging(t)
	ctx := context.Background()
	p, _ := st.Path(ctx, "628111", "growing.bin")
	os.WriteFile(p, []byte("head"), 0o600)
	st.w = NewWithStore(&growingStore{Store: w.Store(), path: p}, w.Bucket())
	if err := st.Sync(ctx, "628111", "growing.bin"); err != nil {
		t.Fatal(err)
	}
	// The stored copy is short, so the file must not count as stored...
	st.w = w
	if n, _ := st.FlushAll(ctx, "628111"); n != 1 {
		t.Fatalf("a file that grew during upload was treated as stored (flushed %d)", n)
	}
	// ...and the next sync stores the whole thing.
	acct, _ := w.Account("628111")
	if got, _ := acct.ReadFile(ctx, "growing.bin", 100); string(got) != "head+tail" {
		t.Fatalf("stored %q", got)
	}
}

func TestSweepKeepsFilesHandedToTools(t *testing.T) {
	st, w := newStaging(t)
	ctx := context.Background()
	acct, _ := w.Account("628111")
	acct.WriteFile(ctx, "clip.mp4", []byte("video"), "video/mp4")
	p, _ := st.Path(ctx, "628111", "clip.mp4")
	old := time.Now().Add(-time.Hour)
	os.Chtimes(p, old, old)
	// A script asks for it again just before the sweep runs.
	if _, err := st.Path(ctx, "628111", "clip.mp4"); err != nil {
		t.Fatal(err)
	}
	st.Sweep(ctx, 15*time.Minute)
	if _, err := os.Stat(p); err != nil {
		t.Fatal("a file in active use was swept")
	}
}

func TestStagingRefusesToWipeForeignFolders(t *testing.T) {
	w, _ := New(Config{SQLitePath: filepath.Join(t.TempDir(), "o.sqlite")})
	defer w.Close()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "precious.db"), []byte("data"), 0o600)
	if _, err := NewStaging(w, dir); err == nil {
		t.Fatal("cleared a folder that was not a staging folder")
	}
	if _, err := os.Stat(filepath.Join(dir, "precious.db")); err != nil {
		t.Fatal("foreign file was removed")
	}
	// An earlier staging folder is reused and cleared.
	own := filepath.Join(t.TempDir(), "stage")
	st, err := NewStaging(w, own)
	if err != nil {
		t.Fatal(err)
	}
	os.MkdirAll(filepath.Join(st.Root(), "628111"), 0o700)
	os.WriteFile(filepath.Join(st.Root(), "628111", "leftover"), []byte("x"), 0o600)
	if _, err := NewStaging(w, own); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(filepath.Join(own, "628111", "leftover")); err == nil {
		t.Fatal("leftovers survived")
	}
}

func TestGatewayListingKeepsKeysAfterACommonPrefix(t *testing.T) {
	env := startGateway(t)
	c := env.client(testAccessKey, testSecret)
	ctx := context.Background()
	for _, k := range []string{"x/1", "x0", "y"} {
		c.PutObject(ctx, &s3.PutObjectInput{Bucket: aws.String(DefaultBucket), Key: aws.String(s3Prefix + k), Body: strings.NewReader("1")})
	}
	out, err := c.ListObjectsV2(ctx, &s3.ListObjectsV2Input{Bucket: aws.String(DefaultBucket), Prefix: aws.String(s3Prefix), Delimiter: aws.String("/")})
	if err != nil {
		t.Fatal(err)
	}
	var keys []string
	for _, o := range out.Contents {
		keys = append(keys, strings.TrimPrefix(aws.ToString(o.Key), s3Prefix))
	}
	if len(keys) != 2 || keys[0] != "x0" || keys[1] != "y" || len(out.CommonPrefixes) != 1 {
		t.Fatalf("keys %v, prefixes %v", keys, out.CommonPrefixes)
	}
}

func TestGatewayRejectsUnsignedAmzHeaders(t *testing.T) {
	env := startGateway(t)
	c := env.client(testAccessKey, testSecret)
	ctx := context.Background()
	other := ObjectRef{AccountID: "999999", Key: "secret.txt"}
	env.store.Put(ctx, other, strings.NewReader("another account's data"), PutOptions{})

	signed, err := s3.NewPresignClient(c).PresignPutObject(ctx, &s3.PutObjectInput{Bucket: aws.String(DefaultBucket), Key: aws.String(s3Prefix + "slot.txt")})
	if err != nil {
		t.Fatal(err)
	}
	req, _ := http.NewRequest(http.MethodPut, signed.URL, strings.NewReader(""))
	req.Header.Set("X-Amz-Copy-Source", "/"+DefaultBucket+"/accounts/999999/media/secret.txt")
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatal(err)
	}
	resp.Body.Close()
	if resp.StatusCode != http.StatusForbidden {
		t.Fatalf("an unsigned x-amz-copy-source was accepted (%d)", resp.StatusCode)
	}
	if ok, _ := env.store.Exists(ctx, ref("628111", "slot.txt")); ok {
		t.Fatal("the other account's object was copied")
	}
}

func TestCheckDoesNotRunRecoveryOnALiveDatabase(t *testing.T) {
	s, path := newStore(t, 100)
	ctx := context.Background()
	s.Put(ctx, ref("a1", "x"), strings.NewReader("one"), PutOptions{})
	// Pretend a reader still pins a replaced version: an unreferenced active row.
	now := time.Now().UnixMilli()
	res, _ := s.db.Exec(`INSERT INTO object_versions (account_id,bucket,key,status,chunk_size,created_at,modified_at)
		VALUES ('a1','wa-bot','x','active',100,?,?)`, now, now)
	pinned, _ := res.LastInsertId()
	if _, _, err := Check(ctx, path, 5); err != nil {
		t.Fatal(err)
	}
	var n int
	s.db.QueryRow(`SELECT COUNT(*) FROM object_versions WHERE id = ?`, pinned).Scan(&n)
	if n != 1 {
		t.Fatal("check removed a version a running server may still be streaming")
	}
}

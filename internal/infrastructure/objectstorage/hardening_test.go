package objectstorage

import (
	"bytes"
	"context"
	"errors"
	"net/http"
	"strings"
	"testing"

	"github.com/aws/aws-sdk-go-v2/aws"
	"github.com/aws/aws-sdk-go-v2/service/s3"
)

// A full disk must fail the write cleanly: existing objects keep working and no
// half-written data is left behind.
func TestDiskFullLeavesExistingObjectsIntact(t *testing.T) {
	s, _ := newStore(t, 4096)
	s.db.SetMaxOpenConns(1) // PRAGMAs are per connection
	ctx := context.Background()
	keep := randomBytes(20000, 1)
	if _, err := s.Put(ctx, ref("a1", "keep.bin"), bytes.NewReader(keep), PutOptions{}); err != nil {
		t.Fatal(err)
	}
	var pages int
	s.db.QueryRow(`PRAGMA page_count`).Scan(&pages)
	if _, err := s.db.Exec(`PRAGMA max_page_count = ` + itoa(pages+8)); err != nil {
		t.Fatal(err)
	}
	_, err := s.Put(ctx, ref("a1", "too-big.bin"), bytes.NewReader(randomBytes(1<<20, 2)), PutOptions{})
	if !errors.Is(err, ErrUnavailable) {
		t.Fatalf("full disk should map to ErrUnavailable, got %v", err)
	}
	if ok, _ := s.Exists(ctx, ref("a1", "too-big.bin")); ok {
		t.Fatal("a failed upload is visible")
	}
	if got := readAll(t, s, ref("a1", "keep.bin"), GetOptions{}); !bytes.Equal(got, keep) {
		t.Fatal("existing object damaged")
	}
	assertNoOrphans(t, s)
}

func itoa(n int) string {
	var b [20]byte
	i := len(b)
	for n > 0 || i == len(b) {
		i--
		b[i] = byte('0' + n%10)
		n /= 10
	}
	return string(b[i:])
}

// The gateway must not accept a captured signed request with changed content,
// headers, or target.
func TestGatewayRejectsTamperedRequests(t *testing.T) {
	env := startGateway(t)
	c := env.client(testAccessKey, testSecret)
	ctx := context.Background()
	ps := s3.NewPresignClient(c)

	signed, err := ps.PresignPutObject(ctx, &s3.PutObjectInput{Bucket: aws.String(DefaultBucket), Key: aws.String(s3Prefix + "t/x.txt")})
	if err != nil {
		t.Fatal(err)
	}
	// Moving a presigned URL to another key.
	moved := strings.Replace(signed.URL, "t/x.txt", "t/y.txt", 1)
	req, _ := http.NewRequest(http.MethodPut, moved, strings.NewReader("evil"))
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatal(err)
	}
	resp.Body.Close()
	if resp.StatusCode != http.StatusForbidden {
		t.Fatalf("relocated presigned URL got %d", resp.StatusCode)
	}

	// A header-signed request whose declared payload hash does not match the body.
	req, _ = http.NewRequest(http.MethodPut, env.url+"/"+DefaultBucket+"/"+s3Prefix+"t/z.txt", strings.NewReader("tampered body"))
	hash, err := signRequest(req, "original body")
	if err != nil {
		t.Fatal(err)
	}
	_ = hash
	resp, err = http.DefaultClient.Do(req)
	if err != nil {
		t.Fatal(err)
	}
	resp.Body.Close()
	if resp.StatusCode == http.StatusOK {
		t.Fatal("a body that does not match the signed payload hash was accepted")
	}
	if ok, _ := env.store.Exists(ctx, ref("628111", "t/z.txt")); ok {
		t.Fatal("tampered body was stored")
	}
}

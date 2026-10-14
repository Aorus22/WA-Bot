package objectstorage

import (
	"bytes"
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/aws/aws-sdk-go-v2/aws"
	"github.com/aws/aws-sdk-go-v2/service/s3"
)

func newPerAccount(t *testing.T) (*PerAccountStore, string) {
	t.Helper()
	root := t.TempDir()
	p, err := OpenPerAccount(filepath.Join(root, "accounts", AccountPlaceholder, "database", "objectstorage.sqlite"),
		filepath.Join(root, "tmp", "pending.sqlite"), SQLiteOptions{})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { p.Close() })
	return p, root
}

func TestPerAccountContract(t *testing.T) {
	p, _ := newPerAccount(t)
	runStoreContract(t, p)
}

func TestEachAccountHasItsOwnFile(t *testing.T) {
	p, root := newPerAccount(t)
	ctx := context.Background()
	if _, err := p.Stat(ctx, ObjectRef{AccountID: "628111", Key: "x"}); err == nil {
		t.Fatal("expected not found")
	}
	if _, err := os.Stat(filepath.Join(root, "accounts", "628111")); err == nil {
		t.Fatal("a read created the account's database")
	}
	p.Put(ctx, ObjectRef{AccountID: "628111", Key: "a.jpg"}, strings.NewReader("one"), PutOptions{})
	p.Put(ctx, ObjectRef{AccountID: "628222", Key: "a.jpg"}, strings.NewReader("two"), PutOptions{})
	for _, acct := range []string{"628111", "628222"} {
		if st, err := os.Stat(filepath.Join(root, "accounts", acct, "database", "objectstorage.sqlite")); err != nil || st.Mode().Perm() != 0o600 {
			t.Fatalf("%s: %v %v", acct, st, err)
		}
	}
	got, err := p.ListAccounts(ctx)
	if err != nil || len(got) != 2 || got[0] != "628111" || got[1] != "628222" {
		t.Fatalf("accounts: %v %v", got, err)
	}
	// Cross-account copy streams between the two files.
	if err := p.Copy(ctx, ObjectRef{AccountID: "628111", Key: "a.jpg"}, ObjectRef{AccountID: "628222", Key: "from-111.jpg"}); err != nil {
		t.Fatal(err)
	}
	if got := readAll(t, p, ObjectRef{AccountID: "628222", Key: "from-111.jpg"}, GetOptions{}); string(got) != "one" {
		t.Fatalf("cross copy: %q", got)
	}
	// An account's media goes away with its file.
	if err := p.DropAccount("628111"); err != nil {
		t.Fatal(err)
	}
	if ok, _ := p.Exists(ctx, ObjectRef{AccountID: "628111", Key: "a.jpg"}); ok {
		t.Fatal("dropped account still has media")
	}
	if ok, _ := p.Exists(ctx, ObjectRef{AccountID: "628222", Key: "a.jpg"}); !ok {
		t.Fatal("dropping one account touched another")
	}
	// The pending namespace is a file of its own, outside the accounts folder.
	p.Put(ctx, ObjectRef{AccountID: "_pending", Key: "qr.jpg"}, strings.NewReader("tmp"), PutOptions{})
	if _, err := os.Stat(filepath.Join(root, "tmp", "pending.sqlite")); err != nil {
		t.Fatal("pending database not at its own path")
	}
	if accts, _ := p.ListAccounts(ctx); len(accts) != 1 || accts[0] != "628222" {
		t.Fatalf("pending must not be listed as an account: %v", accts)
	}
}

func TestPerAccountWrapperAndBackup(t *testing.T) {
	root := t.TempDir()
	w, err := New(Config{SQLitePath: filepath.Join(root, "accounts", AccountPlaceholder, "db", "o.sqlite")})
	if err != nil {
		t.Fatal(err)
	}
	defer w.Close()
	ctx := context.Background()
	a, _ := w.Account("628111")
	a.WriteFile(ctx, "doc.txt", []byte("hello"), "text/plain")
	store := w.Store().(*PerAccountStore)
	dest := filepath.Join(root, "backups", "628111.sqlite")
	res, err := store.Backup(ctx, "628111", dest)
	if err != nil || res.Objects != 1 {
		t.Fatalf("backup: %+v %v", res, err)
	}
	if _, err := store.Backup(ctx, "999", filepath.Join(root, "x.sqlite")); err == nil {
		t.Fatal("backed up an account with no database")
	}
	if _, _, err := Check(ctx, dest, 5); err != nil {
		t.Fatal(err)
	}
	dropped, err := w.DropAccount("628111")
	if !dropped || err != nil {
		t.Fatalf("drop: %v %v", dropped, err)
	}
}

func TestGatewayOverPerAccountStores(t *testing.T) {
	p, _ := newPerAccount(t)
	gw, err := StartGateway(p, DefaultBucket, GatewayConfig{Bind: "127.0.0.1:0", AccessKeyID: testAccessKey, SecretAccessKey: testSecret})
	if err != nil {
		t.Fatal(err)
	}
	defer gw.Close()
	env := &gatewayEnv{gw: gw, url: "http://" + gw.Addr()}
	c := env.client(testAccessKey, testSecret)
	ctx := context.Background()
	bucket := aws.String(DefaultBucket)
	for _, k := range []string{
		"accounts/628111/media/a.txt", "accounts/628111/media/dir/b.txt",
		"accounts/628222/media/c.txt", "accounts/62822/media/z.txt",
	} {
		if _, err := c.PutObject(ctx, &s3.PutObjectInput{Bucket: bucket, Key: aws.String(k), Body: strings.NewReader(k)}); err != nil {
			t.Fatal(err)
		}
	}
	// Full listing is in S3 key order across the two databases.
	all, err := c.ListObjectsV2(ctx, &s3.ListObjectsV2Input{Bucket: bucket})
	if err != nil {
		t.Fatal(err)
	}
	var keys []string
	for _, o := range all.Contents {
		keys = append(keys, aws.ToString(o.Key))
	}
	// Plain byte order of the S3 keys ("62811..." < "62822/" < "628222/").
	want := []string{"accounts/628111/media/a.txt", "accounts/628111/media/dir/b.txt", "accounts/62822/media/z.txt", "accounts/628222/media/c.txt"}
	if strings.Join(keys, ",") != strings.Join(want, ",") {
		t.Fatalf("order: %v", keys)
	}
	top, _ := c.ListObjectsV2(ctx, &s3.ListObjectsV2Input{Bucket: bucket, Prefix: aws.String("accounts/"), Delimiter: aws.String("/")})
	if len(top.CommonPrefixes) != 3 {
		t.Fatalf("account prefixes: %+v", top.CommonPrefixes)
	}
	// Paging through the multi-database scan.
	var seen []string
	var token *string
	for n := 0; n < 6; n++ {
		out, err := c.ListObjectsV2(ctx, &s3.ListObjectsV2Input{Bucket: bucket, MaxKeys: aws.Int32(1), ContinuationToken: token})
		if err != nil {
			t.Fatal(err)
		}
		for _, o := range out.Contents {
			seen = append(seen, aws.ToString(o.Key))
		}
		if !aws.ToBool(out.IsTruncated) {
			break
		}
		token = out.NextContinuationToken
	}
	if strings.Join(seen, ",") != strings.Join(want, ",") {
		t.Fatalf("paged order: %v", seen)
	}
	// Multipart goes to the right account's database.
	data := randomBytes(11<<20, 4)
	if _, err := c.PutObject(ctx, &s3.PutObjectInput{Bucket: bucket, Key: aws.String("accounts/628222/media/small.bin"), Body: bytes.NewReader(data[:1000])}); err != nil {
		t.Fatal(err)
	}
	if ok, _ := p.Exists(ctx, ObjectRef{AccountID: "628222", Key: "small.bin"}); !ok {
		t.Fatal("object not visible through the store")
	}
}

func TestDatabaseSizeIsTheSQLiteFileOfTheAccount(t *testing.T) {
	p, _ := newPerAccount(t)
	ctx := context.Background()
	if size, err := p.DatabaseSize(ctx, "628111"); err != nil || size != 0 {
		t.Fatalf("an account without media has no database: %d %v", size, err)
	}
	empty := strings.Repeat("x", 10)
	p.Put(ctx, ObjectRef{AccountID: "628111", Key: "small.bin"}, strings.NewReader(empty), PutOptions{})
	small, err := p.DatabaseSize(ctx, "628111")
	if err != nil || small <= 0 {
		t.Fatalf("size after one file: %d %v", small, err)
	}
	p.Put(ctx, ObjectRef{AccountID: "628111", Key: "big.bin"}, strings.NewReader(strings.Repeat("y", 3<<20)), PutOptions{})
	big, _ := p.DatabaseSize(ctx, "628111")
	if big < small+3<<20 {
		t.Fatalf("size did not grow with a 3 MB file: %d -> %d", small, big)
	}
	if other, _ := p.DatabaseSize(ctx, "628222"); other != 0 {
		t.Fatalf("another account's database is counted: %d", other)
	}
}

package objectstorage

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	v4 "github.com/aws/aws-sdk-go-v2/aws/signer/v4"
	"io"
	"net/http"
	"os/exec"
	"strings"
	"testing"
	"time"

	"github.com/aws/aws-sdk-go-v2/aws"
	"github.com/aws/aws-sdk-go-v2/credentials"
	"github.com/aws/aws-sdk-go-v2/feature/s3/manager"
	"github.com/aws/aws-sdk-go-v2/service/s3"
	"github.com/aws/aws-sdk-go-v2/service/s3/types"
)

const (
	testAccessKey = "WABOTTESTKEY"
	testSecret    = "wabot-test-secret-0123456789"
	s3Prefix      = "accounts/628111/media/"
)

type gatewayEnv struct {
	store *SQLiteStore
	gw    *Gateway
	url   string
}

func startGateway(t *testing.T) *gatewayEnv {
	t.Helper()
	store, _ := newStore(t, 0)
	gw, err := StartGateway(store, DefaultBucket, GatewayConfig{Bind: "127.0.0.1:0", AccessKeyID: testAccessKey, SecretAccessKey: testSecret})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { gw.Close() })
	return &gatewayEnv{store: store, gw: gw, url: "http://" + gw.Addr()}
}

func (e *gatewayEnv) client(key, secret string) *s3.Client {
	return s3.New(s3.Options{
		Region:       "us-east-1",
		BaseEndpoint: aws.String(e.url),
		UsePathStyle: true,
		Credentials:  credentials.NewStaticCredentialsProvider(key, secret, ""),
	})
}

func TestGatewayRefusesNetworkBindAndWeakKeys(t *testing.T) {
	store, _ := newStore(t, 0)
	for _, bind := range []string{"0.0.0.0:0", ":0", "192.168.1.5:9000", "example.com:9000"} {
		if _, err := StartGateway(store, "", GatewayConfig{Bind: bind, AccessKeyID: "k", SecretAccessKey: "longsecret"}); err == nil {
			t.Errorf("bind %q accepted", bind)
		}
	}
	if _, err := StartGateway(store, "", GatewayConfig{Bind: "127.0.0.1:0"}); err == nil {
		t.Error("gateway started without credentials")
	}
}

func TestGatewayRequiresValidSignatures(t *testing.T) {
	env := startGateway(t)
	ctx := context.Background()

	// Anonymous.
	resp, err := http.Get(env.url + "/" + DefaultBucket + "/" + s3Prefix + "x")
	if err != nil {
		t.Fatal(err)
	}
	resp.Body.Close()
	if resp.StatusCode != http.StatusForbidden {
		t.Fatalf("anonymous request got %d", resp.StatusCode)
	}
	// Wrong secret and wrong key.
	for _, c := range []*s3.Client{env.client(testAccessKey, "wrong-secret-wrong-secret"), env.client("OTHERKEY", testSecret)} {
		_, err := c.PutObject(ctx, &s3.PutObjectInput{Bucket: aws.String(DefaultBucket), Key: aws.String(s3Prefix + "x"), Body: strings.NewReader("data")})
		if err == nil {
			t.Fatal("bad credentials were accepted")
		}
	}
	if ok, _ := env.store.Exists(ctx, ObjectRef{AccountID: "628111", Key: "x"}); ok {
		t.Fatal("unauthenticated write landed")
	}
	// Right credentials work.
	good := env.client(testAccessKey, testSecret)
	if _, err := good.PutObject(ctx, &s3.PutObjectInput{Bucket: aws.String(DefaultBucket), Key: aws.String(s3Prefix + "x"), Body: strings.NewReader("data")}); err != nil {
		t.Fatalf("valid request refused: %v", err)
	}
}

func TestGatewayWithAWSSDK(t *testing.T) {
	env := startGateway(t)
	c := env.client(testAccessKey, testSecret)
	ctx := context.Background()
	bucket := aws.String(DefaultBucket)
	key := func(k string) *string { return aws.String(s3Prefix + k) }

	// Buckets: the one fixed bucket only.
	lb, err := c.ListBuckets(ctx, &s3.ListBucketsInput{})
	if err != nil || len(lb.Buckets) != 1 || aws.ToString(lb.Buckets[0].Name) != DefaultBucket {
		t.Fatalf("list buckets: %+v %v", lb, err)
	}
	if _, err := c.HeadBucket(ctx, &s3.HeadBucketInput{Bucket: bucket}); err != nil {
		t.Fatalf("head bucket: %v", err)
	}
	if _, err := c.HeadBucket(ctx, &s3.HeadBucketInput{Bucket: aws.String("other-bucket")}); err == nil {
		t.Fatal("unknown bucket exists")
	}

	// Put/Head/Get with metadata and content type.
	data := randomBytes(300000, 5)
	if _, err := c.PutObject(ctx, &s3.PutObjectInput{
		Bucket: bucket, Key: key("pics/a b+c.jpg"), Body: bytes.NewReader(data),
		ContentType: aws.String("image/jpeg"), Metadata: map[string]string{"note": "hello"},
	}); err != nil {
		t.Fatal(err)
	}
	head, err := c.HeadObject(ctx, &s3.HeadObjectInput{Bucket: bucket, Key: key("pics/a b+c.jpg")})
	if err != nil || aws.ToInt64(head.ContentLength) != 300000 || aws.ToString(head.ContentType) != "image/jpeg" || head.Metadata["note"] != "hello" {
		t.Fatalf("head: %+v %v", head, err)
	}
	sum := md5Hex(data)
	if strings.Trim(aws.ToString(head.ETag), `"`) != sum {
		t.Fatalf("etag %s, want %s", aws.ToString(head.ETag), sum)
	}
	got, err := c.GetObject(ctx, &s3.GetObjectInput{Bucket: bucket, Key: key("pics/a b+c.jpg")})
	if err != nil {
		t.Fatal(err)
	}
	body, _ := io.ReadAll(got.Body)
	got.Body.Close()
	if !bytes.Equal(body, data) {
		t.Fatal("downloaded content differs")
	}
	// The same object through the internal API.
	if info, err := env.store.Stat(ctx, ObjectRef{AccountID: "628111", Key: "pics/a b+c.jpg"}); err != nil || info.Size != 300000 {
		t.Fatalf("internal view: %+v %v", info, err)
	}

	// Range.
	rng, err := c.GetObject(ctx, &s3.GetObjectInput{Bucket: bucket, Key: key("pics/a b+c.jpg"), Range: aws.String("bytes=100-199")})
	if err != nil {
		t.Fatal(err)
	}
	part, _ := io.ReadAll(rng.Body)
	rng.Body.Close()
	if !bytes.Equal(part, data[100:200]) || aws.ToString(rng.ContentRange) != "bytes 100-199/300000" {
		t.Fatalf("range: %d bytes, %s", len(part), aws.ToString(rng.ContentRange))
	}

	// Missing object.
	if _, err := c.GetObject(ctx, &s3.GetObjectInput{Bucket: bucket, Key: key("nope")}); err == nil {
		t.Fatal("missing object returned data")
	} else if !strings.Contains(err.Error(), "NoSuchKey") {
		t.Fatalf("missing object error: %v", err)
	}

	// Keys outside the accounts/<id>/media/ layout are refused.
	if _, err := c.PutObject(ctx, &s3.PutObjectInput{Bucket: bucket, Key: aws.String("stray.txt"), Body: strings.NewReader("x")}); err == nil {
		t.Fatal("stray key accepted")
	}
	if _, err := c.PutObject(ctx, &s3.PutObjectInput{Bucket: bucket, Key: aws.String("accounts/../media/x"), Body: strings.NewReader("x")}); err == nil {
		t.Fatal("traversal key accepted")
	}

	// Listing with prefix, delimiter and pagination.
	for i := 0; i < 12; i++ {
		c.PutObject(ctx, &s3.PutObjectInput{Bucket: bucket, Key: key(fmt.Sprintf("list/%02d.txt", i)), Body: strings.NewReader("x")})
	}
	c.PutObject(ctx, &s3.PutObjectInput{Bucket: bucket, Key: aws.String("accounts/628222/media/other.txt"), Body: strings.NewReader("x")})
	var seen []string
	var token *string
	for n := 0; n < 5; n++ {
		out, err := c.ListObjectsV2(ctx, &s3.ListObjectsV2Input{Bucket: bucket, Prefix: key("list/"), MaxKeys: aws.Int32(5), ContinuationToken: token})
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
	if len(seen) != 12 || seen[0] != s3Prefix+"list/00.txt" || seen[11] != s3Prefix+"list/11.txt" {
		t.Fatalf("paginated listing: %v", seen)
	}
	top, err := c.ListObjectsV2(ctx, &s3.ListObjectsV2Input{Bucket: bucket, Prefix: aws.String("accounts/"), Delimiter: aws.String("/")})
	if err != nil {
		t.Fatal(err)
	}
	var prefixes []string
	for _, p := range top.CommonPrefixes {
		prefixes = append(prefixes, aws.ToString(p.Prefix))
	}
	if len(prefixes) != 2 || prefixes[0] != "accounts/628111/" || prefixes[1] != "accounts/628222/" {
		t.Fatalf("account prefixes: %v", prefixes)
	}
	dirs, err := c.ListObjectsV2(ctx, &s3.ListObjectsV2Input{Bucket: bucket, Prefix: key(""), Delimiter: aws.String("/")})
	if err != nil {
		t.Fatal(err)
	}
	if len(dirs.CommonPrefixes) != 2 { // list/ and pics/
		t.Fatalf("folders: %+v", dirs.CommonPrefixes)
	}

	// Copy and delete.
	if _, err := c.CopyObject(ctx, &s3.CopyObjectInput{Bucket: bucket, Key: key("pics/copy.jpg"), CopySource: aws.String(DefaultBucket + "/" + s3Prefix + "pics/a%20b%2Bc.jpg")}); err != nil {
		t.Fatalf("copy: %v", err)
	}
	if ok, _ := env.store.Exists(ctx, ObjectRef{AccountID: "628111", Key: "pics/copy.jpg"}); !ok {
		t.Fatal("copy missing")
	}
	if _, err := c.DeleteObject(ctx, &s3.DeleteObjectInput{Bucket: bucket, Key: key("pics/copy.jpg")}); err != nil {
		t.Fatal(err)
	}
	del, err := c.DeleteObjects(ctx, &s3.DeleteObjectsInput{Bucket: bucket, Delete: &types.Delete{Objects: []types.ObjectIdentifier{{Key: key("list/00.txt")}, {Key: key("list/01.txt")}}}})
	if err != nil || len(del.Deleted) != 2 {
		t.Fatalf("delete objects: %+v %v", del, err)
	}

	// Zero-byte object.
	if _, err := c.PutObject(ctx, &s3.PutObjectInput{Bucket: bucket, Key: key("empty"), Body: bytes.NewReader(nil)}); err != nil {
		t.Fatalf("zero-byte put: %v", err)
	}
	if h, err := c.HeadObject(ctx, &s3.HeadObjectInput{Bucket: bucket, Key: key("empty")}); err != nil || aws.ToInt64(h.ContentLength) != 0 {
		t.Fatalf("zero-byte head: %v", err)
	}
}

func TestGatewayMultipart(t *testing.T) {
	env := startGateway(t)
	c := env.client(testAccessKey, testSecret)
	ctx := context.Background()
	data := randomBytes(13<<20, 11)

	up := manager.NewUploader(c, func(u *manager.Uploader) { u.PartSize = 5 << 20; u.Concurrency = 2 })
	out, err := up.Upload(ctx, &s3.PutObjectInput{Bucket: aws.String(DefaultBucket), Key: aws.String(s3Prefix + "big/video.mp4"), Body: bytes.NewReader(data), ContentType: aws.String("video/mp4")})
	if err != nil {
		t.Fatal(err)
	}
	if out.UploadID == "" {
		t.Fatal("upload was not multipart")
	}
	info, err := env.store.Stat(ctx, ObjectRef{AccountID: "628111", Key: "big/video.mp4"})
	if err != nil || info.Size != int64(len(data)) || info.ContentType != "video/mp4" {
		t.Fatalf("stat: %+v %v", info, err)
	}
	if !strings.Contains(info.ETag, "-3") {
		t.Fatalf("multipart etag %q should end with the part count", info.ETag)
	}
	want := sha256.Sum256(data)
	if info.SHA256 != hex.EncodeToString(want[:]) {
		t.Fatal("assembled object has a different checksum")
	}
	if err := env.store.Verify(ctx, ObjectRef{AccountID: "628111", Key: "big/video.mp4"}); err != nil {
		t.Fatal(err)
	}
	// No leftovers once completed.
	var n int
	env.store.db.QueryRow(`SELECT COUNT(*) FROM multipart_uploads`).Scan(&n)
	var chunks int
	env.store.db.QueryRow(`SELECT COUNT(*) FROM multipart_chunks`).Scan(&chunks)
	if n != 0 || chunks != 0 {
		t.Fatalf("completed upload left %d uploads and %d chunks", n, chunks)
	}

	// Abort frees the parts; listing shows in-flight uploads.
	cm, err := c.CreateMultipartUpload(ctx, &s3.CreateMultipartUploadInput{Bucket: aws.String(DefaultBucket), Key: aws.String(s3Prefix + "abandoned.bin")})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := c.UploadPart(ctx, &s3.UploadPartInput{Bucket: aws.String(DefaultBucket), Key: aws.String(s3Prefix + "abandoned.bin"), UploadId: cm.UploadId, PartNumber: aws.Int32(1), Body: bytes.NewReader(randomBytes(1<<20, 3))}); err != nil {
		t.Fatal(err)
	}
	lu, err := c.ListMultipartUploads(ctx, &s3.ListMultipartUploadsInput{Bucket: aws.String(DefaultBucket), Prefix: aws.String(s3Prefix)})
	if err != nil || len(lu.Uploads) != 1 {
		t.Fatalf("list uploads: %+v %v", lu, err)
	}
	lp, err := c.ListParts(ctx, &s3.ListPartsInput{Bucket: aws.String(DefaultBucket), Key: aws.String(s3Prefix + "abandoned.bin"), UploadId: cm.UploadId})
	if err != nil || len(lp.Parts) != 1 || aws.ToInt64(lp.Parts[0].Size) != 1<<20 {
		t.Fatalf("list parts: %+v %v", lp, err)
	}
	if _, err := c.AbortMultipartUpload(ctx, &s3.AbortMultipartUploadInput{Bucket: aws.String(DefaultBucket), Key: aws.String(s3Prefix + "abandoned.bin"), UploadId: cm.UploadId}); err != nil {
		t.Fatal(err)
	}
	env.store.db.QueryRow(`SELECT COUNT(*) FROM multipart_chunks`).Scan(&chunks)
	if chunks != 0 {
		t.Fatalf("abort left %d chunks", chunks)
	}
	if ok, _ := env.store.Exists(ctx, ObjectRef{AccountID: "628111", Key: "abandoned.bin"}); ok {
		t.Fatal("aborted upload produced an object")
	}
}

func TestGatewayPresignedURLs(t *testing.T) {
	env := startGateway(t)
	c := env.client(testAccessKey, testSecret)
	ctx := context.Background()
	ps := s3.NewPresignClient(c)

	// Presigned PUT then GET, with plain HTTP clients.
	put, err := ps.PresignPutObject(ctx, &s3.PutObjectInput{Bucket: aws.String(DefaultBucket), Key: aws.String(s3Prefix + "presigned/file.txt")}, s3.WithPresignExpires(time.Minute))
	if err != nil {
		t.Fatal(err)
	}
	req, _ := http.NewRequest(http.MethodPut, put.URL, strings.NewReader("via presigned put"))
	resp, err := http.DefaultClient.Do(req)
	if err != nil || resp.StatusCode != http.StatusOK {
		t.Fatalf("presigned put: %v %v", resp, err)
	}
	resp.Body.Close()

	get, err := ps.PresignGetObject(ctx, &s3.GetObjectInput{Bucket: aws.String(DefaultBucket), Key: aws.String(s3Prefix + "presigned/file.txt")}, s3.WithPresignExpires(time.Minute))
	if err != nil {
		t.Fatal(err)
	}
	resp, err = http.Get(get.URL)
	if err != nil {
		t.Fatal(err)
	}
	body, _ := io.ReadAll(resp.Body)
	resp.Body.Close()
	if resp.StatusCode != http.StatusOK || string(body) != "via presigned put" {
		t.Fatalf("presigned get: %d %q", resp.StatusCode, body)
	}

	// Tampering with the URL (another key, or the signature) is refused.
	other := strings.Replace(get.URL, "presigned/file.txt", "presigned/other.txt", 1)
	resp, _ = http.Get(other)
	resp.Body.Close()
	if resp.StatusCode != http.StatusForbidden {
		t.Fatalf("tampered key got %d", resp.StatusCode)
	}
	resp, _ = http.Get(get.URL[:len(get.URL)-3] + "000")
	resp.Body.Close()
	if resp.StatusCode != http.StatusForbidden {
		t.Fatalf("tampered signature got %d", resp.StatusCode)
	}

	// Expired.
	env2 := env
	v := newSigV4Verifier(testAccessKey, testSecret, "us-east-1")
	v.now = func() time.Time { return time.Now().Add(2 * time.Minute) }
	r, _ := http.NewRequest(http.MethodGet, get.URL, nil)
	if err := v.Verify(r); err == nil {
		t.Fatal("expired presigned URL verified")
	}
	_ = env2
}

func TestGatewayBoto3(t *testing.T) {
	if _, err := exec.LookPath("python3"); err != nil {
		t.Skip("python3 not available")
	}
	if err := exec.Command("python3", "-c", "import boto3").Run(); err != nil {
		t.Skip("boto3 not installed")
	}
	env := startGateway(t)
	script := `
import sys, boto3, io, hashlib
from botocore.config import Config
from boto3.s3.transfer import TransferConfig
endpoint, ak, sk = sys.argv[1:4]
s3 = boto3.client("s3", endpoint_url=endpoint, aws_access_key_id=ak, aws_secret_access_key=sk, region_name="us-east-1",
                  config=Config(signature_version="s3v4", s3={"addressing_style": "path"}, request_checksum_calculation="when_required", response_checksum_validation="when_required"))
b = "wa-bot"; p = "accounts/628111/media/"
s3.put_object(Bucket=b, Key=p+"boto/hello.txt", Body=b"hello from boto3", ContentType="text/plain")
assert s3.get_object(Bucket=b, Key=p+"boto/hello.txt")["Body"].read() == b"hello from boto3"
assert s3.head_object(Bucket=b, Key=p+"boto/hello.txt")["ContentType"] == "text/plain"
big = bytes(range(256)) * (11 * 4096)   # ~11 MiB -> multipart
s3.upload_fileobj(io.BytesIO(big), b, p+"boto/big.bin", Config=TransferConfig(multipart_threshold=5*1024*1024, multipart_chunksize=5*1024*1024))
out = io.BytesIO(); s3.download_fileobj(b, p+"boto/big.bin", out)
assert hashlib.sha256(out.getvalue()).digest() == hashlib.sha256(big).digest()
r = s3.list_objects_v2(Bucket=b, Prefix=p+"boto/")
assert sorted(o["Key"] for o in r["Contents"]) == [p+"boto/big.bin", p+"boto/hello.txt"], r
url = s3.generate_presigned_url("get_object", Params={"Bucket": b, "Key": p+"boto/hello.txt"}, ExpiresIn=60)
import urllib.request
assert urllib.request.urlopen(url).read() == b"hello from boto3"
s3.delete_object(Bucket=b, Key=p+"boto/hello.txt")
try:
    s3.get_object(Bucket=b, Key=p+"boto/hello.txt"); raise SystemExit("deleted object still readable")
except s3.exceptions.NoSuchKey:
    pass
print("boto3 ok")
`
	out, err := exec.Command("python3", "-c", script, env.url, testAccessKey, testSecret).CombinedOutput()
	if err != nil {
		t.Fatalf("boto3 against the gateway failed: %v\n%s", err, out)
	}
	if !strings.Contains(string(out), "boto3 ok") {
		t.Fatalf("unexpected output: %s", out)
	}
}

// The remote adapter, pointed at our own gateway, must behave exactly like the
// SQLite engine.
func TestS3StoreContractAgainstGateway(t *testing.T) {
	env := startGateway(t)
	// The adapter's bucket is the gateway's single bucket.
	store, err := NewS3Store(context.Background(), S3Config{
		Endpoint: env.url, Region: "us-east-1", AccessKeyID: testAccessKey, SecretAccessKey: testSecret,
		ForcePathStyle: true, RemoteBucket: DefaultBucket,
	})
	if err != nil {
		t.Fatal(err)
	}
	runStoreContract(t, store)

	// Credentials problems map onto ErrUnauthorized.
	bad, _ := NewS3Store(context.Background(), S3Config{Endpoint: env.url, Region: "us-east-1", AccessKeyID: testAccessKey, SecretAccessKey: "wrong-secret-wrong", ForcePathStyle: true, RemoteBucket: DefaultBucket})
	if _, err := bad.Put(context.Background(), ObjectRef{AccountID: "628111", Key: "x"}, strings.NewReader("x"), PutOptions{}); !isErr(err, ErrUnauthorized) {
		t.Fatalf("bad credentials: %v", err)
	}
	// A large streamed upload goes through multipart without buffering it all.
	data := randomBytes(20<<20, 3)
	ref := ObjectRef{AccountID: "628111", Key: "large/streamed.bin"}
	if _, err := store.Put(context.Background(), ref, bytes.NewReader(data), PutOptions{}); err != nil {
		t.Fatal(err)
	}
	rc, _, _ := store.Open(context.Background(), ref, GetOptions{})
	got, _ := io.ReadAll(rc)
	rc.Close()
	if !bytes.Equal(got, data) {
		t.Fatal("large object differs")
	}
}

// signRequest signs req with SigV4 as if its body were `declared`, so a test can
// send a different body than the one the signature covers.
func signRequest(req *http.Request, declared string) (string, error) {
	sum := sha256.Sum256([]byte(declared))
	hash := hex.EncodeToString(sum[:])
	req.Header.Set("X-Amz-Content-Sha256", hash)
	req.ContentLength = int64(len(declared))
	signer := v4.NewSigner(func(o *v4.SignerOptions) { o.DisableURIPathEscaping = true })
	err := signer.SignHTTP(context.Background(), aws.Credentials{AccessKeyID: testAccessKey, SecretAccessKey: testSecret}, req, hash, "s3", "us-east-1", time.Now())
	return hash, err
}

package objectstorage

import (
	"bytes"
	"context"
	"encoding/hex"
	"errors"
	"io"
	"net/http"
	"strings"
	"time"

	"github.com/johannesboyne/gofakes3"
)

// gatewayBackend exposes a SQLiteStore through the gofakes3 frontend. The
// outside world sees one bucket whose keys are
//
//	accounts/<account-id>/media/<key>
//
// which maps one-to-one onto the store's (account, bucket, key) namespace, so
// an S3 client can reach every account's media and nothing else. The internal
// WA-Bot code does not go through this layer.
type gatewayBackend struct {
	store   gatewayStore
	bucket  string
	created time.Time
}

var (
	_ gofakes3.Backend          = (*gatewayBackend)(nil)
	_ gofakes3.MultipartBackend = (*gatewayBackend)(nil)
)

func newGatewayBackend(store gatewayStore, bucket string) *gatewayBackend {
	return &gatewayBackend{store: store, bucket: bucket, created: time.Now()}
}

// gatewayStore is what the S3 frontend needs from a provider: the plain store,
// multipart uploads, metadata replacement and the cross-account S3-ordered scan.
// Both the single-file and the per-account SQLite stores provide it.
type gatewayStore interface {
	Store
	MultipartStore
	SetMetadata(ctx context.Context, ref ObjectRef, contentType string, metadata map[string]string) error
	// scanComposed returns up to limit objects ordered by their S3 key
	// (accounts/<account>/media/<key>) starting at prefix and after `after`
	// (inclusive when asked). account, when set, narrows it to one account.
	scanComposed(ctx context.Context, bucket, account, prefix, after string, inclusive bool, limit int) ([]ObjectInfo, error)
}

const s3KeyPrefix = "accounts/"

// splitS3Key maps an S3 key to the store's namespace.
func splitS3Key(k string) (account, key string, err error) {
	parts := strings.SplitN(k, "/", 4)
	if len(parts) != 4 || parts[0] != "accounts" || parts[2] != "media" || parts[3] == "" {
		return "", "", gofakes3.ErrorMessagef(gofakes3.ErrInvalidArgument, "keys must look like accounts/<account>/media/<key>, got %q", truncate(k))
	}
	if err := ValidateAccountID(parts[1]); err != nil {
		return "", "", gofakes3.ErrorMessagef(gofakes3.ErrInvalidArgument, "bad account in key %q", truncate(k))
	}
	if _, err := NormalizeKey(parts[3]); err != nil {
		return "", "", gofakes3.ErrorMessagef(gofakes3.ErrInvalidArgument, "bad key %q", truncate(k))
	}
	return parts[1], parts[3], nil
}

func joinS3Key(account, key string) string { return s3KeyPrefix + account + "/media/" + key }

func (g *gatewayBackend) ref(s3key string) (ObjectRef, error) {
	account, key, err := splitS3Key(s3key)
	if err != nil {
		return ObjectRef{}, err
	}
	return ObjectRef{AccountID: account, Bucket: g.bucket, Key: key}, nil
}

func (g *gatewayBackend) checkBucket(name string) error {
	if name != g.bucket {
		return gofakes3.BucketNotFound(name)
	}
	return nil
}

// mapGatewayErr turns store errors into S3 error responses.
func mapGatewayErr(err error, s3key string) error {
	switch {
	case err == nil:
		return nil
	case errors.Is(err, ErrNoSuchUpload):
		return gofakes3.ErrNoSuchUpload
	case errors.Is(err, ErrNotFound):
		return gofakes3.KeyNotFound(s3key)
	case errors.Is(err, ErrInvalidKey):
		return gofakes3.ErrorMessage(gofakes3.ErrInvalidArgument, err.Error())
	case errors.Is(err, ErrInvalidRange):
		return gofakes3.ErrInvalidRange
	case errors.Is(err, ErrUnavailable):
		return gofakes3.ErrorMessage(gofakes3.ErrInternal, "storage is busy or unavailable")
	default:
		return gofakes3.ErrorMessage(gofakes3.ErrInternal, "storage error")
	}
}

func (g *gatewayBackend) ListBuckets() ([]gofakes3.BucketInfo, error) {
	return []gofakes3.BucketInfo{{Name: g.bucket, CreationDate: gofakes3.NewContentTime(g.created)}}, nil
}

func (g *gatewayBackend) CreateBucket(name string) error {
	if name == g.bucket {
		return gofakes3.ResourceError(gofakes3.ErrBucketAlreadyExists, name)
	}
	return gofakes3.ErrorMessage(gofakes3.ErrInvalidArgument, "this gateway serves a single fixed bucket")
}

func (g *gatewayBackend) BucketExists(name string) (bool, error) { return name == g.bucket, nil }

func (g *gatewayBackend) DeleteBucket(name string) error {
	if err := g.checkBucket(name); err != nil {
		return err
	}
	return gofakes3.ErrorMessage(gofakes3.ErrInvalidArgument, "the media bucket cannot be deleted through S3")
}

func (g *gatewayBackend) ForceDeleteBucket(name string) error { return g.DeleteBucket(name) }

func etagBytes(etag string) []byte {
	if i := strings.IndexByte(etag, '-'); i >= 0 {
		etag = etag[:i]
	}
	b, _ := hex.DecodeString(etag)
	return b
}

func metaToHeaders(info ObjectInfo) map[string]string {
	m := map[string]string{}
	for k, v := range info.Metadata {
		m[k] = v
	}
	if info.ContentType != "" {
		m["Content-Type"] = info.ContentType
	}
	m["Last-Modified"] = info.LastModified.UTC().Format(http.TimeFormat)
	return m
}

func (g *gatewayBackend) object(info ObjectInfo, body io.ReadCloser, rng *gofakes3.ObjectRange) *gofakes3.Object {
	return &gofakes3.Object{
		Name:     joinS3Key(info.Ref.AccountID, info.Ref.Key),
		Metadata: metaToHeaders(info),
		Size:     info.Size,
		Contents: body,
		Hash:     etagBytes(info.ETag),
		Range:    rng,
	}
}

func (g *gatewayBackend) HeadObject(bucketName, objectName string) (*gofakes3.Object, error) {
	if err := g.checkBucket(bucketName); err != nil {
		return nil, err
	}
	ref, err := g.ref(objectName)
	if err != nil {
		return nil, gofakes3.KeyNotFound(objectName)
	}
	info, err := g.store.Stat(context.Background(), ref)
	if err != nil {
		return nil, mapGatewayErr(err, objectName)
	}
	return g.object(info, io.NopCloser(bytes.NewReader(nil)), nil), nil
}

func (g *gatewayBackend) GetObject(bucketName, objectName string, rangeRequest *gofakes3.ObjectRangeRequest) (*gofakes3.Object, error) {
	if err := g.checkBucket(bucketName); err != nil {
		return nil, err
	}
	ref, err := g.ref(objectName)
	if err != nil {
		return nil, gofakes3.KeyNotFound(objectName)
	}
	ctx := context.Background()
	info, err := g.store.Stat(ctx, ref)
	if err != nil {
		return nil, mapGatewayErr(err, objectName)
	}
	rng, err := rangeRequest.Range(info.Size)
	if err != nil {
		return nil, err
	}
	opts := GetOptions{}
	if rng != nil {
		opts.Range = &ByteRange{Start: rng.Start, End: rng.Start + rng.Length - 1}
	}
	rc, got, err := g.store.Open(ctx, ref, opts)
	if err != nil {
		return nil, mapGatewayErr(err, objectName)
	}
	return g.object(got, rc, rng), nil
}

func (g *gatewayBackend) PutObject(bucketName, key string, meta map[string]string, input io.Reader, size int64, conditions *gofakes3.PutConditions) (gofakes3.PutObjectResult, error) {
	var res gofakes3.PutObjectResult
	if err := g.checkBucket(bucketName); err != nil {
		return res, err
	}
	ref, err := g.ref(key)
	if err != nil {
		return res, err
	}
	ctx := context.Background()
	if conditions != nil {
		cur := &gofakes3.ConditionalObjectInfo{}
		if info, err := g.store.Stat(ctx, ref); err == nil {
			cur.Exists, cur.Hash = true, etagBytes(info.ETag)
		}
		if err := gofakes3.CheckPutConditions(conditions, cur); err != nil {
			return res, err
		}
	}
	opts := PutOptions{ContentType: meta["Content-Type"], Metadata: userMetadata(meta)}
	if _, err := g.store.Put(ctx, ref, input, opts); err != nil {
		return res, mapGatewayErr(err, key)
	}
	return res, nil
}

// userMetadata keeps the headers worth returning on later reads and drops the
// ones that describe the request or are recomputed.
func userMetadata(meta map[string]string) map[string]string {
	out := map[string]string{}
	for k, v := range meta {
		switch k {
		case "Last-Modified", "Content-Type", "Content-Length", "Content-Md5", "Authorization", "Host", "User-Agent",
			"X-Amz-Date", "X-Amz-Content-Sha256", "X-Amz-Decoded-Content-Length", "X-Amz-Copy-Source", "Expect", "Accept-Encoding":
			continue
		}
		out[k] = v
	}
	return out
}

func (g *gatewayBackend) DeleteObject(bucketName, objectName string) (gofakes3.ObjectDeleteResult, error) {
	var res gofakes3.ObjectDeleteResult
	if err := g.checkBucket(bucketName); err != nil {
		return res, err
	}
	ref, err := g.ref(objectName)
	if err != nil {
		return res, nil // an address that cannot exist is already gone
	}
	return res, mapGatewayErr(g.store.Delete(context.Background(), ref), objectName)
}

func (g *gatewayBackend) DeleteMulti(bucketName string, objects ...string) (gofakes3.MultiDeleteResult, error) {
	var res gofakes3.MultiDeleteResult
	for _, o := range objects {
		if _, err := g.DeleteObject(bucketName, o); err != nil {
			res.Error = append(res.Error, gofakes3.ErrorResult{Key: o, Code: gofakes3.ErrInternal, Message: "delete failed"})
			continue
		}
		res.Deleted = append(res.Deleted, gofakes3.ObjectID{Key: o})
	}
	return res, nil
}

func (g *gatewayBackend) CopyObject(srcBucket, srcKey, dstBucket, dstKey string, meta map[string]string) (gofakes3.CopyObjectResult, error) {
	var res gofakes3.CopyObjectResult
	if err := g.checkBucket(srcBucket); err != nil {
		return res, err
	}
	if err := g.checkBucket(dstBucket); err != nil {
		return res, err
	}
	src, err := g.ref(srcKey)
	if err != nil {
		return res, gofakes3.KeyNotFound(srcKey)
	}
	dst, err := g.ref(dstKey)
	if err != nil {
		return res, err
	}
	ctx := context.Background()
	if err := g.store.Copy(ctx, src, dst); err != nil {
		return res, mapGatewayErr(err, srcKey)
	}
	if strings.EqualFold(meta["X-Amz-Metadata-Directive"], "REPLACE") {
		if err := g.store.SetMetadata(ctx, dst, meta["Content-Type"], userMetadata(meta)); err != nil {
			return res, mapGatewayErr(err, dstKey)
		}
	}
	info, err := g.store.Stat(ctx, dst)
	if err != nil {
		return res, mapGatewayErr(err, dstKey)
	}
	res.ETag = gofakes3.FormatETag(etagBytes(info.ETag))
	res.LastModified = gofakes3.NewContentTime(info.LastModified)
	return res, nil
}

// ListBucket lists keys in S3 order (byte order of the full key). With a prefix
// that names an account the query is the store's indexed listing; otherwise the
// composed key is scanned, which only external tooling ever does.
func (g *gatewayBackend) ListBucket(name string, prefix *gofakes3.Prefix, page gofakes3.ListBucketPage) (*gofakes3.ObjectList, error) {
	if err := g.checkBucket(name); err != nil {
		return nil, err
	}
	if prefix == nil {
		prefix = &gofakes3.Prefix{}
	}
	maxKeys := int(page.MaxKeys)
	if maxKeys <= 0 {
		maxKeys = 1000
	}
	list := gofakes3.NewObjectList()
	ctx := context.Background()

	cursor := ""
	if page.HasMarker {
		cursor = page.Marker
	}
	pfx := ""
	if prefix.HasPrefix {
		pfx = prefix.Prefix
	}
	account := accountOfPrefix(pfx)
	lastCommon := ""
	count := 0

	const batch = 200
	inclusive := false // true right after a jump: the cursor is itself a candidate key
	for {
		rows, err := g.store.scanComposed(ctx, g.bucket, account, pfx, cursor, inclusive, batch)
		if err != nil {
			return nil, mapGatewayErr(err, pfx)
		}
		jumped := false
		for _, o := range rows {
			s3key := joinS3Key(o.Ref.AccountID, o.Ref.Key)
			var match gofakes3.PrefixMatch
			if !prefix.Match(s3key, &match) {
				cursor = s3key
				continue
			}
			if match.CommonPrefix {
				if match.MatchedPart != lastCommon {
					if count >= maxKeys {
						list.IsTruncated, list.NextMarker = true, cursor
						return list, nil
					}
					list.AddPrefix(match.MatchedPart)
					lastCommon = match.MatchedPart
					count++
				}
				// Everything else under this common prefix is skipped in one jump.
				if up, ok := prefixUpperBound(match.MatchedPart); ok {
					cursor, jumped, inclusive = up, true, true
					break
				}
				cursor = s3key
				continue
			}
			if count >= maxKeys {
				list.IsTruncated, list.NextMarker = true, cursor
				return list, nil
			}
			list.Add(&gofakes3.Content{
				Key:          s3key,
				LastModified: gofakes3.NewContentTime(o.LastModified),
				ETag:         gofakes3.FormatETag(etagBytes(o.ETag)),
				Size:         o.Size,
				StorageClass: gofakes3.StorageStandard,
			})
			count++
			cursor = s3key
		}
		if !jumped && len(rows) < batch {
			return list, nil
		}
		if !jumped {
			inclusive = false
		}
	}
}

// accountOfPrefix returns the account an S3 prefix pins down, if any.
func accountOfPrefix(p string) string {
	rest, ok := strings.CutPrefix(p, s3KeyPrefix)
	if !ok {
		return ""
	}
	id, _, found := strings.Cut(rest, "/")
	if !found || ValidateAccountID(id) != nil {
		return ""
	}
	return id
}

// --- multipart --------------------------------------------------------------

func (g *gatewayBackend) CreateMultipartUpload(bucket, object string, meta map[string]string) (gofakes3.UploadID, error) {
	if err := g.checkBucket(bucket); err != nil {
		return "", err
	}
	ref, err := g.ref(object)
	if err != nil {
		return "", err
	}
	id, err := g.store.CreateMultipart(context.Background(), ref, PutOptions{ContentType: meta["Content-Type"], Metadata: userMetadata(meta)})
	return gofakes3.UploadID(id), mapGatewayErr(err, object)
}

func (g *gatewayBackend) UploadPart(bucket, object string, id gofakes3.UploadID, partNumber int, contentLength int64, input io.Reader) (string, error) {
	if err := g.checkBucket(bucket); err != nil {
		return "", err
	}
	ref, err := g.ref(object)
	if err != nil {
		return "", err
	}
	if partNumber > MaxPartNumber {
		return "", gofakes3.ErrInvalidPart
	}
	part, err := g.store.UploadPart(context.Background(), ref, string(id), partNumber, input)
	if err != nil {
		return "", mapGatewayErr(err, object)
	}
	if part.Size != contentLength {
		return "", gofakes3.ErrIncompleteBody
	}
	return `"` + part.ETag + `"`, nil
}

func (g *gatewayBackend) ListMultipartUploads(bucket string, marker *gofakes3.UploadListMarker, prefix gofakes3.Prefix, limit int64) (*gofakes3.ListMultipartUploadsResult, error) {
	if err := g.checkBucket(bucket); err != nil {
		return nil, err
	}
	res := &gofakes3.ListMultipartUploadsResult{Bucket: bucket, MaxUploads: limit, Prefix: prefix.Prefix, Delimiter: prefix.Delimiter}
	account := accountOfPrefix(prefix.Prefix)
	if account == "" {
		return res, nil // listing across accounts is not offered
	}
	keyPrefix := strings.TrimPrefix(prefix.Prefix, s3KeyPrefix+account+"/media/")
	if !strings.HasPrefix(prefix.Prefix, s3KeyPrefix+account+"/media/") {
		keyPrefix = ""
	}
	km, um := "", ""
	if marker != nil {
		if a, k, err := splitS3Key(marker.Object); err == nil && a == account {
			km, um = k, string(marker.UploadID)
		}
	}
	n := int(limit)
	if n <= 0 || n > 1000 {
		n = 1000
	}
	ups, err := g.store.ListUploads(context.Background(), ObjectScope{AccountID: account, Bucket: g.bucket}, keyPrefix, km, um, n+1)
	if err != nil {
		return nil, mapGatewayErr(err, prefix.Prefix)
	}
	if len(ups) > n {
		ups = ups[:n]
		res.IsTruncated = true
		res.NextKeyMarker = joinS3Key(account, ups[n-1].Ref.Key)
		res.NextUploadIDMarker = gofakes3.UploadID(ups[n-1].UploadID)
	}
	for _, u := range ups {
		res.Uploads = append(res.Uploads, gofakes3.ListMultipartUploadItem{
			Key: joinS3Key(account, u.Ref.Key), UploadID: gofakes3.UploadID(u.UploadID),
			StorageClass: gofakes3.StorageStandard, Initiated: gofakes3.NewContentTime(u.Initiated),
		})
	}
	return res, nil
}

func (g *gatewayBackend) ListParts(bucket, object string, uploadID gofakes3.UploadID, marker int, limit int64) (*gofakes3.ListMultipartUploadPartsResult, error) {
	if err := g.checkBucket(bucket); err != nil {
		return nil, err
	}
	ref, err := g.ref(object)
	if err != nil {
		return nil, err
	}
	n := int(limit)
	if n <= 0 || n > 1000 {
		n = 1000
	}
	parts, err := g.store.ListParts(context.Background(), ref, string(uploadID), marker, n+1)
	if err != nil {
		return nil, mapGatewayErr(err, object)
	}
	res := &gofakes3.ListMultipartUploadPartsResult{
		Bucket: bucket, Key: object, UploadID: uploadID, StorageClass: gofakes3.StorageStandard,
		PartNumberMarker: marker, MaxParts: limit,
	}
	if len(parts) > n {
		parts = parts[:n]
		res.IsTruncated = true
		res.NextPartNumberMarker = parts[n-1].PartNumber
	}
	for _, p := range parts {
		res.Parts = append(res.Parts, gofakes3.ListMultipartUploadPartItem{
			PartNumber: p.PartNumber, ETag: `"` + p.ETag + `"`, Size: p.Size, LastModified: gofakes3.NewContentTime(p.LastModified),
		})
	}
	return res, nil
}

func (g *gatewayBackend) AbortMultipartUpload(bucket, object string, id gofakes3.UploadID) error {
	if err := g.checkBucket(bucket); err != nil {
		return err
	}
	ref, err := g.ref(object)
	if err != nil {
		return err
	}
	return mapGatewayErr(g.store.AbortMultipart(context.Background(), ref, string(id)), object)
}

func (g *gatewayBackend) CompleteMultipartUpload(bucket, object string, id gofakes3.UploadID, input *gofakes3.CompleteMultipartUploadRequest) (gofakes3.VersionID, string, error) {
	if err := g.checkBucket(bucket); err != nil {
		return "", "", err
	}
	ref, err := g.ref(object)
	if err != nil {
		return "", "", err
	}
	parts := make([]CompletedPart, len(input.Parts))
	for i, p := range input.Parts {
		parts[i] = CompletedPart{PartNumber: p.PartNumber, ETag: p.ETag}
	}
	info, err := g.store.CompleteMultipart(context.Background(), ref, string(id), parts)
	if err != nil {
		if errors.Is(err, ErrInvalidRange) {
			return "", "", gofakes3.ErrorMessage(gofakes3.ErrInvalidPart, err.Error())
		}
		return "", "", mapGatewayErr(err, object)
	}
	// gofakes3 renders an object's ETag as the hex of its hash, so a later HEAD
	// cannot carry the "-<parts>" suffix; report the same value here.
	return "", gofakes3.FormatETag(etagBytes(info.ETag)), nil
}

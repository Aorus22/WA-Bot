package objectstorage

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"net/http"
	"strconv"
	"strings"
	"time"

	"github.com/aws/aws-sdk-go-v2/aws"
	awshttp "github.com/aws/aws-sdk-go-v2/aws/transport/http"
	"github.com/aws/aws-sdk-go-v2/credentials"
	"github.com/aws/aws-sdk-go-v2/feature/s3/manager"
	"github.com/aws/aws-sdk-go-v2/service/s3"
	"github.com/aws/smithy-go"
)

// S3Config configures the remote provider. Credentials come from the explicit
// keys or, when both are empty, from the AWS default chain (env, shared
// config, instance role).
type S3Config struct {
	Endpoint        string // empty = AWS; set for R2, MinIO and other compatible services
	Region          string
	AccessKeyID     string
	SecretAccessKey string
	ForcePathStyle  bool
	// RemoteBucket is the bucket that holds WA-Bot's objects. It must exist.
	RemoteBucket string
	// LogicalBucket is the WA-Bot bucket these objects belong to (default
	// DefaultBucket); other logical buckets are refused.
	LogicalBucket string
	HTTPClient    *http.Client
}

// S3Store keeps objects in an S3-compatible service under
// accounts/<account-id>/media/<key>, the same layout the local gateway shows.
type S3Store struct {
	client   *s3.Client
	uploader *manager.Uploader
	bucket   string
	logical  string
}

var _ Store = (*S3Store)(nil)

const (
	shaMetaKey = "wabot-sha256"
	// 8 MiB parts, 2 in flight: an upload holds at most about 16 MiB in memory.
	uploadPartSize    = 8 << 20
	uploadConcurrency = 2
	maxCopyObject     = 5 << 30 // single CopyObject limit
)

// NewS3Store connects to the remote provider. It does not create the bucket.
func NewS3Store(ctx context.Context, cfg S3Config) (*S3Store, error) {
	if cfg.RemoteBucket == "" {
		return nil, errors.New("objectstorage: S3 bucket name is empty")
	}
	if cfg.Region == "" {
		cfg.Region = "us-east-1"
	}
	logical := cfg.LogicalBucket
	if logical == "" {
		logical = DefaultBucket
	}
	opts := s3.Options{
		Region:       cfg.Region,
		UsePathStyle: cfg.ForcePathStyle,
		// Only send/validate checksums when an operation asks for one; several
		// S3-compatible services reject the newer default checksum headers.
		RequestChecksumCalculation: aws.RequestChecksumCalculationWhenRequired,
		ResponseChecksumValidation: aws.ResponseChecksumValidationWhenRequired,
		RetryMaxAttempts:           4,
	}
	if cfg.HTTPClient != nil {
		opts.HTTPClient = cfg.HTTPClient
	} else {
		opts.HTTPClient = awshttp.NewBuildableClient().WithTimeout(0)
	}
	if cfg.Endpoint != "" {
		opts.BaseEndpoint = aws.String(cfg.Endpoint)
	}
	switch {
	case cfg.AccessKeyID != "" || cfg.SecretAccessKey != "":
		opts.Credentials = credentials.NewStaticCredentialsProvider(cfg.AccessKeyID, cfg.SecretAccessKey, "")
	default:
		awsCfg, err := loadDefaultAWSConfig(ctx, cfg.Region)
		if err != nil {
			return nil, fmt.Errorf("%w: AWS credentials: %v", ErrUnauthorized, err)
		}
		opts.Credentials = awsCfg.Credentials
	}
	client := s3.New(opts)
	return &S3Store{
		client: client,
		uploader: manager.NewUploader(client, func(u *manager.Uploader) {
			u.PartSize = uploadPartSize
			u.Concurrency = uploadConcurrency
		}),
		bucket:  cfg.RemoteBucket,
		logical: logical,
	}, nil
}

func (s *S3Store) remoteKey(ref ObjectRef) (ObjectRef, string, error) {
	ref, err := canonical(ref)
	if err != nil {
		return ref, "", err
	}
	if ref.Bucket != s.logical {
		return ref, "", fmt.Errorf("%w: bucket %q (this provider serves %q)", ErrUnsupported, ref.Bucket, s.logical)
	}
	return ref, joinS3Key(ref.AccountID, ref.Key), nil
}

// mapS3Err maps SDK failures onto the shared error contract.
func mapS3Err(err error) error {
	if err == nil {
		return nil
	}
	if errors.Is(err, context.Canceled) || errors.Is(err, context.DeadlineExceeded) {
		return err
	}
	var apiErr smithy.APIError
	if errors.As(err, &apiErr) {
		switch apiErr.ErrorCode() {
		case "NoSuchKey", "NotFound", "NoSuchBucket", "404":
			return fmt.Errorf("%w: %s", ErrNotFound, apiErr.ErrorMessage())
		case "InvalidRange":
			return fmt.Errorf("%w: %s", ErrInvalidRange, apiErr.ErrorMessage())
		case "AccessDenied", "InvalidAccessKeyId", "SignatureDoesNotMatch", "ExpiredToken", "AllAccessDisabled":
			return fmt.Errorf("%w: %s", ErrUnauthorized, apiErr.ErrorMessage())
		case "PreconditionFailed", "BucketAlreadyExists":
			return fmt.Errorf("%w: %s", ErrConflict, apiErr.ErrorMessage())
		}
	}
	var respErr *awshttp.ResponseError
	if errors.As(err, &respErr) {
		switch respErr.HTTPStatusCode() {
		case http.StatusNotFound:
			return fmt.Errorf("%w: %v", ErrNotFound, err)
		case http.StatusRequestedRangeNotSatisfiable:
			return fmt.Errorf("%w: %v", ErrInvalidRange, err)
		case http.StatusUnauthorized, http.StatusForbidden:
			return fmt.Errorf("%w: %v", ErrUnauthorized, err)
		}
	}
	return fmt.Errorf("%w: %v", ErrUnavailable, err)
}

func asciiMetadata(in map[string]string) (map[string]string, error) {
	out := make(map[string]string, len(in)+1)
	for k, v := range in {
		for _, r := range k + v {
			if r < 0x20 || r > 0x7e {
				return nil, fmt.Errorf("%w: metadata %q must be printable ASCII for S3", ErrInvalidKey, k)
			}
		}
		out[strings.ToLower(k)] = v
	}
	return out, nil
}

func (s *S3Store) Put(ctx context.Context, ref ObjectRef, body io.Reader, opts PutOptions) (ObjectInfo, error) {
	ref, key, err := s.remoteKey(ref)
	if err != nil {
		return ObjectInfo{}, err
	}
	meta, err := asciiMetadata(opts.Metadata)
	if err != nil {
		return ObjectInfo{}, err
	}
	// The upload cannot know the checksum up front, and S3 only reads user
	// metadata at the start of the request, so the hash is attached by a second
	// (metadata-only) copy afterwards.
	sum, counted := sha256.New(), &countWriter{}
	in := &s3.PutObjectInput{
		Bucket:   aws.String(s.bucket),
		Key:      aws.String(key),
		Body:     io.TeeReader(body, io.MultiWriter(sum, counted)),
		Metadata: meta,
	}
	if opts.ContentType != "" {
		in.ContentType = aws.String(opts.ContentType)
	}
	if _, err := s.uploader.Upload(ctx, in); err != nil {
		return ObjectInfo{}, mapS3Err(err)
	}
	if counted.n > maxCopyObject {
		// Too big to rewrite its metadata in one request; stored without checksum.
		return s.Stat(ctx, ref)
	}
	// Replace the object's metadata in place (same key, same bytes) to record
	// the checksum for later Stat/Verify.
	meta[shaMetaKey] = hex.EncodeToString(sum.Sum(nil))
	// The object is already stored and complete; if recording the checksum fails
	// it simply has none, which is not worth failing the upload for.
	_, _ = s.client.CopyObject(ctx, &s3.CopyObjectInput{
		Bucket:            aws.String(s.bucket),
		Key:               aws.String(key),
		CopySource:        aws.String(s.bucket + "/" + escapeCopySource(key)),
		Metadata:          meta,
		MetadataDirective: "REPLACE",
		ContentType:       in.ContentType,
	})
	return s.Stat(ctx, ref)
}

type countWriter struct{ n int64 }

func (c *countWriter) Write(p []byte) (int, error) { c.n += int64(len(p)); return len(p), nil }

func escapeCopySource(key string) string {
	parts := strings.Split(key, "/")
	for i, p := range parts {
		parts[i] = urlPathEscape(p)
	}
	return strings.Join(parts, "/")
}

func (s *S3Store) infoFrom(ref ObjectRef, size int64, etag, contentType *string, modified *time.Time, meta map[string]string) ObjectInfo {
	info := ObjectInfo{Ref: ref, Size: size, SHA256: meta[shaMetaKey]}
	if etag != nil {
		info.ETag = strings.Trim(*etag, `"`)
	}
	if contentType != nil {
		info.ContentType = *contentType
	}
	if modified != nil {
		info.LastModified = modified.UTC()
	}
	if len(meta) > 0 {
		info.Metadata = map[string]string{}
		for k, v := range meta {
			if k != shaMetaKey {
				info.Metadata[k] = v
			}
		}
		if len(info.Metadata) == 0 {
			info.Metadata = nil
		}
	}
	return info
}

func (s *S3Store) Stat(ctx context.Context, ref ObjectRef) (ObjectInfo, error) {
	ref, key, err := s.remoteKey(ref)
	if err != nil {
		return ObjectInfo{}, err
	}
	out, err := s.client.HeadObject(ctx, &s3.HeadObjectInput{Bucket: aws.String(s.bucket), Key: aws.String(key)})
	if err != nil {
		return ObjectInfo{}, mapS3Err(err)
	}
	return s.infoFrom(ref, aws.ToInt64(out.ContentLength), out.ETag, out.ContentType, out.LastModified, out.Metadata), nil
}

func (s *S3Store) Exists(ctx context.Context, ref ObjectRef) (bool, error) {
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

func (s *S3Store) Open(ctx context.Context, ref ObjectRef, opts GetOptions) (io.ReadCloser, ObjectInfo, error) {
	ref, key, err := s.remoteKey(ref)
	if err != nil {
		return nil, ObjectInfo{}, err
	}
	// Stat first so ranges are validated against the real size exactly as the
	// SQLite engine does, whatever the remote service answers for odd ranges.
	info, err := s.Stat(ctx, ref)
	if err != nil {
		return nil, ObjectInfo{}, err
	}
	in := &s3.GetObjectInput{Bucket: aws.String(s.bucket), Key: aws.String(key)}
	if r := opts.Range; r != nil {
		last := r.End
		if last < 0 || last >= info.Size {
			last = info.Size - 1
		}
		switch {
		case info.Size == 0 && r.Start == 0:
			return io.NopCloser(strings.NewReader("")), info, nil
		case r.Start < 0 || r.Start >= info.Size || last < r.Start:
			return nil, info, fmt.Errorf("%w: %d-%d of %d", ErrInvalidRange, r.Start, r.End, info.Size)
		}
		in.Range = aws.String("bytes=" + strconv.FormatInt(r.Start, 10) + "-" + strconv.FormatInt(last, 10))
	}
	out, err := s.client.GetObject(ctx, in)
	if err != nil {
		return nil, info, mapS3Err(err)
	}
	return out.Body, info, nil
}

func (s *S3Store) Copy(ctx context.Context, src, dst ObjectRef) error {
	src, srcKey, err := s.remoteKey(src)
	if err != nil {
		return err
	}
	dst, dstKey, err := s.remoteKey(dst)
	if err != nil {
		return err
	}
	info, err := s.Stat(ctx, src)
	if err != nil {
		return err
	}
	if src == dst {
		return nil
	}
	if info.Size > maxCopyObject {
		// Beyond the single-request limit: stream it through.
		rc, _, err := s.Open(ctx, src, GetOptions{})
		if err != nil {
			return err
		}
		defer rc.Close()
		_, err = s.Put(ctx, dst, rc, PutOptions{ContentType: info.ContentType, Metadata: info.Metadata})
		return err
	}
	_, err = s.client.CopyObject(ctx, &s3.CopyObjectInput{
		Bucket:     aws.String(s.bucket),
		Key:        aws.String(dstKey),
		CopySource: aws.String(s.bucket + "/" + escapeCopySource(srcKey)),
	})
	return mapS3Err(err)
}

func (s *S3Store) Delete(ctx context.Context, ref ObjectRef) error {
	_, key, err := s.remoteKey(ref)
	if err != nil {
		return err
	}
	_, err = s.client.DeleteObject(ctx, &s3.DeleteObjectInput{Bucket: aws.String(s.bucket), Key: aws.String(key)})
	if err = mapS3Err(err); errors.Is(err, ErrNotFound) {
		return nil
	}
	return err
}

func (s *S3Store) List(ctx context.Context, scope ObjectScope, opts ListOptions) (ObjectPage, error) {
	if err := ValidateAccountID(scope.AccountID); err != nil {
		return ObjectPage{}, err
	}
	if scope.Bucket == "" {
		scope.Bucket = DefaultBucket
	}
	if scope.Bucket != s.logical {
		return ObjectPage{}, fmt.Errorf("%w: bucket %q", ErrUnsupported, scope.Bucket)
	}
	if strings.ContainsAny(opts.Prefix, "\\\x00") || strings.ContainsAny(opts.Cursor, "\\\x00") {
		return ObjectPage{}, fmt.Errorf("%w: list prefix or cursor", ErrInvalidKey)
	}
	limit := opts.Limit
	if limit <= 0 {
		limit = DefaultListLimit
	}
	if limit > MaxListLimit {
		limit = MaxListLimit
	}
	base := joinS3Key(scope.AccountID, "")
	in := &s3.ListObjectsV2Input{
		Bucket:  aws.String(s.bucket),
		Prefix:  aws.String(base + opts.Prefix),
		MaxKeys: aws.Int32(int32(limit)),
	}
	if opts.Cursor != "" {
		in.StartAfter = aws.String(base + opts.Cursor)
	}
	out, err := s.client.ListObjectsV2(ctx, in)
	if err != nil {
		return ObjectPage{}, mapS3Err(err)
	}
	var page ObjectPage
	for _, o := range out.Contents {
		key := strings.TrimPrefix(aws.ToString(o.Key), base)
		page.Objects = append(page.Objects, ObjectInfo{
			Ref:  ObjectRef{AccountID: scope.AccountID, Bucket: scope.Bucket, Key: key},
			Size: aws.ToInt64(o.Size), ETag: strings.Trim(aws.ToString(o.ETag), `"`), LastModified: aws.ToTime(o.LastModified).UTC(),
		})
	}
	if aws.ToBool(out.IsTruncated) && len(page.Objects) > 0 {
		page.NextCursor = page.Objects[len(page.Objects)-1].Ref.Key
	}
	return page, nil
}

func (s *S3Store) Close() error { return nil }

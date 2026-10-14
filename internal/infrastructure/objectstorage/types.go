// Package objectstorage is WA-Bot's only door to media storage. A Wrapper
// fronts a Store: the embedded one-file SQLite engine (s3lite.go) today, a
// remote S3 provider later. Callers address objects by account + key, never by
// filesystem path.
package objectstorage

import (
	"context"
	"errors"
	"fmt"
	"io"
	"regexp"
	"strings"
	"time"
	"unicode/utf8"
)

// DefaultBucket is the bucket WA-Bot keeps its media in.
const DefaultBucket = "wa-bot"

// Errors every provider maps its failures onto.
var (
	ErrNotFound     = errors.New("objectstorage: object not found")
	ErrConflict     = errors.New("objectstorage: conflict")
	ErrInvalidKey   = errors.New("objectstorage: invalid key")
	ErrInvalidRange = errors.New("objectstorage: invalid range")
	ErrUnauthorized = errors.New("objectstorage: unauthorized")
	ErrUnavailable  = errors.New("objectstorage: storage unavailable")
	ErrCorruptData  = errors.New("objectstorage: corrupt data")
	ErrTooLarge     = errors.New("objectstorage: object larger than the allowed size")
	ErrUnsupported  = errors.New("objectstorage: unsupported")
	ErrStoreClosed  = errors.New("objectstorage: store closed")
)

// ObjectRef names one object. AccountID is mandatory: media of different
// WhatsApp accounts never share a namespace.
type ObjectRef struct {
	AccountID string
	Bucket    string
	Key       string
}

func (r ObjectRef) String() string {
	return fmt.Sprintf("%s/%s/%s", r.AccountID, r.Bucket, r.Key)
}

// ObjectScope is an account + bucket, for listing.
type ObjectScope struct {
	AccountID string
	Bucket    string
}

// ObjectInfo is an object's metadata.
type ObjectInfo struct {
	Ref          ObjectRef
	Size         int64
	ContentType  string
	ETag         string // MD5 of the content, hex. Not meaningful for multipart objects.
	SHA256       string // hex
	LastModified time.Time
	Metadata     map[string]string
}

type PutOptions struct {
	ContentType string
	Metadata    map[string]string
}

// ByteRange is an inclusive byte range. End < 0 means "to the end".
type ByteRange struct {
	Start int64
	End   int64
}

type GetOptions struct {
	Range *ByteRange
}

type ListOptions struct {
	Prefix string
	Limit  int    // 0 = default (1000); capped at MaxListLimit
	Cursor string // NextCursor of the previous page
}

type ObjectPage struct {
	Objects    []ObjectInfo
	NextCursor string // empty on the last page
}

const (
	DefaultListLimit = 1000
	MaxListLimit     = 1000
	maxKeyBytes      = 1024
)

// Store is what a provider implements. All operations are streaming and take a
// context; none of them buffers a whole object.
type Store interface {
	Put(ctx context.Context, ref ObjectRef, body io.Reader, opts PutOptions) (ObjectInfo, error)
	// Open returns the object's bytes. The reader of the SQLite engine also
	// implements io.Seeker.
	Open(ctx context.Context, ref ObjectRef, opts GetOptions) (io.ReadCloser, ObjectInfo, error)
	Stat(ctx context.Context, ref ObjectRef) (ObjectInfo, error)
	Exists(ctx context.Context, ref ObjectRef) (bool, error)
	Copy(ctx context.Context, src, dst ObjectRef) error
	Delete(ctx context.Context, ref ObjectRef) error
	List(ctx context.Context, scope ObjectScope, opts ListOptions) (ObjectPage, error)
	Close() error
}

// CompletedPart is one entry of a CompleteMultipart request.
type CompletedPart struct {
	PartNumber int
	ETag       string
}

// PartInfo describes an uploaded part.
type PartInfo struct {
	PartNumber   int
	ETag         string
	Size         int64
	LastModified time.Time
}

// UploadInfo describes an unfinished multipart upload.
type UploadInfo struct {
	UploadID  string
	Ref       ObjectRef
	Initiated time.Time
}

// MaxPartNumber is S3's limit.
const MaxPartNumber = 10000

// MultipartStore is implemented by providers that hold multipart uploads
// themselves (the SQLite engine), so a part never has to fit in memory.
type MultipartStore interface {
	CreateMultipart(ctx context.Context, ref ObjectRef, opts PutOptions) (uploadID string, err error)
	UploadPart(ctx context.Context, ref ObjectRef, uploadID string, partNumber int, body io.Reader) (PartInfo, error)
	CompleteMultipart(ctx context.Context, ref ObjectRef, uploadID string, parts []CompletedPart) (ObjectInfo, error)
	AbortMultipart(ctx context.Context, ref ObjectRef, uploadID string) error
	// ListParts returns up to limit parts with PartNumber > after.
	ListParts(ctx context.Context, ref ObjectRef, uploadID string, after, limit int) ([]PartInfo, error)
	// ListUploads returns up to limit unfinished uploads of scope whose key
	// starts with prefix, after (keyMarker, uploadMarker).
	ListUploads(ctx context.Context, scope ObjectScope, prefix, keyMarker, uploadMarker string, limit int) ([]UploadInfo, error)
}

var accountIDPattern = regexp.MustCompile(`^[A-Za-z0-9][A-Za-z0-9_.-]{0,63}$`)
var bucketPattern = regexp.MustCompile(`^[a-z0-9][a-z0-9.-]{1,61}[a-z0-9]$`)

// ValidateAccountID accepts phone numbers and the reserved "_pending" /
// "_legacy" namespaces.
func ValidateAccountID(id string) error {
	if id == "_pending" || id == "_legacy" || accountIDPattern.MatchString(id) {
		if id == "." || id == ".." {
			return fmt.Errorf("%w: account %q", ErrInvalidKey, id)
		}
		return nil
	}
	return fmt.Errorf("%w: account %q", ErrInvalidKey, id)
}

func validateBucket(b string) error {
	if !bucketPattern.MatchString(b) {
		return fmt.Errorf("%w: bucket %q", ErrInvalidKey, b)
	}
	return nil
}

// NormalizeKey turns a caller-supplied key (possibly "./a//b.jpg") into the canonical key. It rejects anything that could
// escape a namespace or behave differently on another provider: absolute
// paths, backslashes, "." / ".." segments, control characters, invalid UTF-8.
func NormalizeKey(key string) (string, error) {
	if key == "" || len(key) > maxKeyBytes || !utf8.ValidString(key) {
		return "", fmt.Errorf("%w: %q", ErrInvalidKey, truncate(key))
	}
	if strings.HasPrefix(key, "/") || strings.ContainsRune(key, '\\') {
		return "", fmt.Errorf("%w: %q", ErrInvalidKey, truncate(key))
	}
	for _, r := range key {
		if r < 0x20 || r == 0x7f {
			return "", fmt.Errorf("%w: control character in key", ErrInvalidKey)
		}
	}
	parts := strings.Split(key, "/")
	out := parts[:0]
	for _, p := range parts {
		switch p {
		case "", ".":
			continue
		case "..":
			return "", fmt.Errorf("%w: %q", ErrInvalidKey, truncate(key))
		}
		out = append(out, p)
	}
	if len(out) == 0 {
		return "", fmt.Errorf("%w: %q", ErrInvalidKey, truncate(key))
	}
	return strings.Join(out, "/"), nil
}

func truncate(s string) string {
	if len(s) > 80 {
		return s[:80] + "..."
	}
	return s
}

// canonical validates and normalizes a reference.
func canonical(ref ObjectRef) (ObjectRef, error) {
	if err := ValidateAccountID(ref.AccountID); err != nil {
		return ref, err
	}
	if ref.Bucket == "" {
		ref.Bucket = DefaultBucket
	}
	if err := validateBucket(ref.Bucket); err != nil {
		return ref, err
	}
	key, err := NormalizeKey(ref.Key)
	if err != nil {
		return ref, err
	}
	ref.Key = key
	return ref, nil
}

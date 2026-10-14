package objectstorage

import (
	"bytes"
	"context"
	"fmt"
	"io"
	"path/filepath"
	"strings"
)

// Config selects and configures a provider.
type Config struct {
	Driver string // "sqlite" (default) or "s3"
	Bucket string // WA-Bot's logical bucket (default "wa-bot")
	// SQLitePath is the database file. If it contains "{account}" every account
	// gets its own file (e.g. "accounts/{account}/database/objectstorage.sqlite");
	// otherwise all accounts share the one file.
	SQLitePath string
	// PendingSQLitePath is where the throw-away "_pending" namespace lives in
	// per-account mode (default: tmp/pending-objectstorage.sqlite).
	PendingSQLitePath string
	S3                S3Config // used by the "s3" driver
}

// Wrapper is the single entry point WA-Bot code uses for media. It adds the
// file-like helpers on top of a Store and nothing else: the provider decides
// where bytes live.
type Wrapper struct {
	store  Store
	bucket string
}

// New opens the provider named by cfg.
func New(cfg Config) (*Wrapper, error) {
	bucket := cfg.Bucket
	if bucket == "" {
		bucket = DefaultBucket
	}
	if err := validateBucket(bucket); err != nil {
		return nil, err
	}
	switch strings.ToLower(cfg.Driver) {
	case "", "sqlite":
		if strings.Contains(cfg.SQLitePath, AccountPlaceholder) {
			pending := cfg.PendingSQLitePath
			if pending == "" {
				pending = filepath.Join("tmp", "pending-objectstorage.sqlite")
			}
			store, err := OpenPerAccount(cfg.SQLitePath, pending, SQLiteOptions{})
			if err != nil {
				return nil, err
			}
			return &Wrapper{store: store, bucket: bucket}, nil
		}
		store, err := OpenSQLite(SQLiteOptions{Path: cfg.SQLitePath})
		if err != nil {
			return nil, err
		}
		return &Wrapper{store: store, bucket: bucket}, nil
	case "s3":
		cfg.S3.LogicalBucket = bucket
		store, err := NewS3Store(context.Background(), cfg.S3)
		if err != nil {
			return nil, err
		}
		return &Wrapper{store: store, bucket: bucket}, nil
	default:
		return nil, fmt.Errorf("%w: object storage driver %q", ErrUnsupported, cfg.Driver)
	}
}

// NewWithStore wraps an already opened provider.
func NewWithStore(store Store, bucket string) *Wrapper {
	if bucket == "" {
		bucket = DefaultBucket
	}
	return &Wrapper{store: store, bucket: bucket}
}

// StartGateway serves the embedded store over the S3 protocol on a loopback
// address. Only the SQLite provider has anything to serve.
func (w *Wrapper) StartGateway(cfg GatewayConfig) (*Gateway, error) {
	store, ok := w.store.(gatewayStore)
	if !ok {
		return nil, fmt.Errorf("%w: the S3 gateway needs the sqlite driver", ErrUnsupported)
	}
	return StartGateway(store, w.bucket, cfg)
}

// DropAccount deletes everything stored for an account when the provider keeps
// one database per account (used to clear the throw-away "_pending" namespace).
// It reports false when the provider cannot do that.
func (w *Wrapper) DropAccount(account string) (bool, error) {
	d, ok := w.store.(interface{ DropAccount(string) error })
	if !ok {
		return false, nil
	}
	return true, d.DropAccount(account)
}

// Store is the underlying provider, for streaming callers and tools.
func (w *Wrapper) Store() Store { return w.store }

func (w *Wrapper) Bucket() string { return w.bucket }

func (w *Wrapper) Close() error { return w.store.Close() }

func (w *Wrapper) ref(account, key string) ObjectRef {
	return ObjectRef{AccountID: account, Bucket: w.bucket, Key: key}
}

// Account returns a view bound to one WhatsApp account. A job that captured it
// keeps writing to that account even if the active account changes meanwhile.
func (w *Wrapper) Account(id string) (*Scope, error) {
	if err := ValidateAccountID(id); err != nil {
		return nil, err
	}
	return &Scope{w: w, account: id}, nil
}

// Scope is a Wrapper bound to one account and the default bucket.
type Scope struct {
	w       *Wrapper
	account string
}

func (s *Scope) AccountID() string { return s.account }

// Put streams body into key.
func (s *Scope) Put(ctx context.Context, key string, body io.Reader, opts PutOptions) (ObjectInfo, error) {
	return s.w.store.Put(ctx, s.w.ref(s.account, key), body, opts)
}

// Open streams key (or a range of it).
func (s *Scope) Open(ctx context.Context, key string, opts GetOptions) (io.ReadCloser, ObjectInfo, error) {
	return s.w.store.Open(ctx, s.w.ref(s.account, key), opts)
}

func (s *Scope) Stat(ctx context.Context, key string) (ObjectInfo, error) {
	return s.w.store.Stat(ctx, s.w.ref(s.account, key))
}

func (s *Scope) List(ctx context.Context, opts ListOptions) (ObjectPage, error) {
	return s.w.store.List(ctx, ObjectScope{AccountID: s.account, Bucket: s.w.bucket}, opts)
}

// ReadFile loads a small object. It fails with ErrTooLarge instead of reading
// more than maxBytes; use Open for media.
func (s *Scope) ReadFile(ctx context.Context, key string, maxBytes int64) ([]byte, error) {
	return s.w.ReadFile(ctx, s.w.ref(s.account, key), maxBytes)
}

func (s *Scope) WriteFile(ctx context.Context, key string, data []byte, contentType string) (ObjectInfo, error) {
	return s.w.WriteFile(ctx, s.w.ref(s.account, key), data, contentType)
}

// CopyFile copies within this account.
func (s *Scope) CopyFile(ctx context.Context, srcKey, dstKey string) error {
	return s.w.CopyFile(ctx, s.w.ref(s.account, srcKey), s.w.ref(s.account, dstKey))
}

func (s *Scope) DeleteFile(ctx context.Context, key string) error {
	return s.w.DeleteFile(ctx, s.w.ref(s.account, key))
}

func (s *Scope) FileExists(ctx context.Context, key string) (bool, error) {
	return s.w.FileExists(ctx, s.w.ref(s.account, key))
}

// ReadFile reads a whole object into memory, up to maxBytes.
func (w *Wrapper) ReadFile(ctx context.Context, ref ObjectRef, maxBytes int64) ([]byte, error) {
	if maxBytes <= 0 {
		return nil, fmt.Errorf("%w: ReadFile needs a positive size limit", ErrInvalidRange)
	}
	rc, info, err := w.store.Open(ctx, ref, GetOptions{})
	if err != nil {
		return nil, err
	}
	defer rc.Close()
	if info.Size > maxBytes {
		return nil, fmt.Errorf("%w: %s is %d bytes, limit %d", ErrTooLarge, info.Ref, info.Size, maxBytes)
	}
	return io.ReadAll(io.LimitReader(rc, maxBytes))
}

func (w *Wrapper) WriteFile(ctx context.Context, ref ObjectRef, data []byte, contentType string) (ObjectInfo, error) {
	return w.store.Put(ctx, ref, bytes.NewReader(data), PutOptions{ContentType: contentType})
}

func (w *Wrapper) CopyFile(ctx context.Context, src, dst ObjectRef) error {
	return w.store.Copy(ctx, src, dst)
}

func (w *Wrapper) DeleteFile(ctx context.Context, ref ObjectRef) error {
	return w.store.Delete(ctx, ref)
}

func (w *Wrapper) FileExists(ctx context.Context, ref ObjectRef) (bool, error) {
	return w.store.Exists(ctx, ref)
}

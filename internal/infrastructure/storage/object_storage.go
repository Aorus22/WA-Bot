package storage

import (
	"bufio"
	"context"
	"fmt"
	"io"
	"net/http"
	"os"
	"path"
	"path/filepath"
	"strings"
	"sync/atomic"
	"time"

	"wa-bot/internal/domain/repository"
	"wa-bot/internal/infrastructure/objectstorage"
)

const pendingAccount = "_pending"

// extContentTypes keeps the content types the media endpoint has always sent
// (Go's table says video/webm and audio/x-ogg for some of these).
var extContentTypes = map[string]string{
	".jpg": "image/jpeg", ".jpeg": "image/jpeg", ".png": "image/png", ".webp": "image/webp", ".gif": "image/gif",
	".mp4": "video/mp4", ".pdf": "application/pdf", ".ogg": "audio/ogg", ".opus": "audio/ogg", ".mp3": "audio/mpeg",
	".m4a": "audio/mp4", ".wav": "audio/wav", ".webm": "audio/webm",
}

// ObjectMediaStorage serves WA-Bot's media from an object-storage Wrapper. The
// WhatsApp account whose media is addressed is the one bound last (see
// SetAccount); every operation resolves it once, at the start, so a job never
// straddles two accounts.
type ObjectMediaStorage struct {
	w       *objectstorage.Wrapper
	staging *objectstorage.Staging
	account atomic.Value // string
}

var _ repository.StorageRepository = (*ObjectMediaStorage)(nil)

func NewObjectMediaStorage(w *objectstorage.Wrapper, staging *objectstorage.Staging) *ObjectMediaStorage {
	m := &ObjectMediaStorage{w: w, staging: staging}
	m.account.Store(pendingAccount)
	return m
}

// Account is the account media currently belongs to.
func (m *ObjectMediaStorage) Account() string { return m.account.Load().(string) }

// SetAccount binds the storage to a WhatsApp account. The pending namespace is
// throw-away: binding it clears whatever an earlier QR phase left behind.
func (m *ObjectMediaStorage) SetAccount(id string) error {
	if err := objectstorage.ValidateAccountID(id); err != nil {
		return err
	}
	if id == pendingAccount {
		if err := m.purge(context.Background(), pendingAccount); err != nil {
			return fmt.Errorf("clear pending media: %w", err)
		}
	}
	m.account.Store(id)
	return nil
}

func (m *ObjectMediaStorage) purge(ctx context.Context, account string) error {
	scope, err := m.w.Account(account)
	if err != nil {
		return err
	}
	_ = os.RemoveAll(filepath.Join(m.staging.Root(), account))
	// One database per account: dropping the namespace is deleting its file.
	if dropped, err := m.w.DropAccount(account); dropped {
		return err
	}
	for {
		page, err := scope.List(ctx, objectstorage.ListOptions{})
		if err != nil {
			return err
		}
		for _, o := range page.Objects {
			if err := scope.DeleteFile(ctx, o.Ref.Key); err != nil {
				return err
			}
		}
		if page.NextCursor == "" {
			return nil
		}
	}
}

// key turns what callers pass (a key, a "media/"-prefixed key, or the scratch
// path of a file) into an object key of the given account.
func (m *ObjectMediaStorage) key(account, p string) (string, error) {
	if filepath.IsAbs(p) {
		k, ok := m.staging.KeyOf(account, p)
		if !ok {
			return "", fmt.Errorf("%w: %q is not a stored media path", objectstorage.ErrInvalidKey, p)
		}
		return k, nil
	}
	p = strings.TrimPrefix(filepath.ToSlash(p), "./")
	p = strings.TrimPrefix(p, "media/")
	return objectstorage.NormalizeKey(p)
}

func (m *ObjectMediaStorage) scope() (string, *objectstorage.Scope, error) {
	id := m.Account()
	s, err := m.w.Account(id)
	return id, s, err
}

func contentTypeOf(key string, br *bufio.Reader) string {
	if ct, ok := extContentTypes[strings.ToLower(path.Ext(key))]; ok {
		return ct
	}
	head, _ := br.Peek(512)
	return http.DetectContentType(head)
}

func (m *ObjectMediaStorage) Save(ctx context.Context, p string, reader io.Reader) (string, error) {
	account, scope, err := m.scope()
	if err != nil {
		return "", err
	}
	key, err := m.key(account, p)
	if err != nil {
		return "", err
	}
	// A scratch copy would now be stale; if a script holds its path, give it the
	// new content at the same place.
	hadScratch := m.staging.Has(account, key)
	m.staging.Discard(account, key)
	br := bufio.NewReaderSize(reader, 512)
	if _, err := scope.Put(ctx, key, br, objectstorage.PutOptions{ContentType: contentTypeOf(key, br)}); err != nil {
		return "", fmt.Errorf("failed to save file: %w", err)
	}
	if hadScratch {
		_, _ = m.staging.Path(ctx, account, key)
	}
	return p, nil
}

func (m *ObjectMediaStorage) Get(ctx context.Context, p string) (io.ReadCloser, error) {
	account, scope, err := m.scope()
	if err != nil {
		return nil, err
	}
	key, err := m.key(account, p)
	if err != nil {
		return nil, err
	}
	// Something a tool just wrote is read after being stored.
	if err := m.staging.Sync(ctx, account, key); err != nil {
		return nil, err
	}
	rc, _, err := scope.Open(ctx, key, objectstorage.GetOptions{})
	if err != nil {
		return nil, fmt.Errorf("failed to open file %s: %w", key, err)
	}
	return rc, nil
}

func (m *ObjectMediaStorage) Delete(ctx context.Context, p string) error {
	account, scope, err := m.scope()
	if err != nil {
		return err
	}
	key, err := m.key(account, p)
	if err != nil {
		return err
	}
	m.staging.Discard(account, key)
	return scope.DeleteFile(ctx, key)
}

func (m *ObjectMediaStorage) Exists(p string) bool {
	account, scope, err := m.scope()
	if err != nil {
		return false
	}
	key, err := m.key(account, p)
	if err != nil {
		return false
	}
	if m.staging.Has(account, key) {
		return true
	}
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	ok, _ := scope.FileExists(ctx, key)
	return ok
}

func (m *ObjectMediaStorage) ServeMedia(w http.ResponseWriter, r *http.Request, p string) {
	account, _, err := m.scope()
	if err != nil {
		http.NotFound(w, r)
		return
	}
	key, err := m.key(account, p)
	if err != nil {
		http.NotFound(w, r)
		return
	}
	if err := m.staging.Sync(r.Context(), account, key); err != nil {
		http.Error(w, "storage unavailable", http.StatusServiceUnavailable)
		return
	}
	objectstorage.ServeObject(w, r, m.w.Store(), objectstorage.ObjectRef{AccountID: account, Bucket: m.w.Bucket(), Key: key})
}

func (m *ObjectMediaStorage) WorkFile(ctx context.Context, p string) (string, error) {
	account := m.Account()
	key, err := m.key(account, p)
	if err != nil {
		return "", err
	}
	return m.staging.Path(ctx, account, key)
}

func (m *ObjectMediaStorage) WorkDir(ctx context.Context, p string) (string, error) {
	account := m.Account()
	if p == "" || p == "." {
		return m.WorkRoot(), nil
	}
	dir, err := m.WorkFile(ctx, p)
	if err != nil {
		return "", err
	}
	if err := os.MkdirAll(dir, 0o700); err != nil {
		return "", err
	}
	_ = account
	return dir, nil
}

func (m *ObjectMediaStorage) WorkRoot() string {
	dir := filepath.Join(m.staging.Root(), m.Account())
	_ = os.MkdirAll(dir, 0o700)
	return dir
}

func (m *ObjectMediaStorage) WorkSync(ctx context.Context, p string) error {
	account := m.Account()
	key, err := m.key(account, p)
	if err != nil {
		return err
	}
	return m.staging.Sync(ctx, account, key)
}

func (m *ObjectMediaStorage) WorkFlush(ctx context.Context) error {
	_, err := m.staging.FlushAll(ctx, m.Account())
	return err
}

func (m *ObjectMediaStorage) WorkTemp(prefix string) (string, func(), error) {
	return m.staging.TempDir(prefix)
}

// Sweep stores pending scratch files and deletes idle ones; run it periodically.
func (m *ObjectMediaStorage) Sweep(idle time.Duration) {
	m.staging.Sweep(context.Background(), idle)
}

// WorkFlushAll stores pending scratch files of every account (shutdown).
func (m *ObjectMediaStorage) WorkFlushAll(ctx context.Context) error {
	return m.staging.FlushEverything(ctx)
}

package objectstorage

import (
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"sync"
)

// AccountPlaceholder marks where the account id goes in a per-account database
// path, e.g. "accounts/{account}/database/objectstorage.sqlite".
const AccountPlaceholder = "{account}"

// PerAccountStore gives every WhatsApp account its own SQLite database, next to
// the rest of that account's data, so an account's media is backed up, moved
// and deleted together with it and two accounts never share a file. Databases
// are opened on first use. Each one is a complete SQLiteStore.
type PerAccountStore struct {
	template    string // contains AccountPlaceholder
	pendingPath string // the throw-away "_pending" namespace lives here
	opts        SQLiteOptions

	mu     sync.Mutex
	stores map[string]*SQLiteStore
	closed bool
}

var (
	_ Store         = (*PerAccountStore)(nil)
	_ gatewayStore  = (*PerAccountStore)(nil)
	_ AccountLister = (*PerAccountStore)(nil)
)

// OpenPerAccount prepares a per-account store. template must contain
// AccountPlaceholder; pendingPath is the database used for the "_pending"
// namespace (media seen before a number is linked).
func OpenPerAccount(template, pendingPath string, opts SQLiteOptions) (*PerAccountStore, error) {
	if !strings.Contains(template, AccountPlaceholder) {
		return nil, fmt.Errorf("objectstorage: %q has no %s", template, AccountPlaceholder)
	}
	if pendingPath == "" {
		return nil, fmt.Errorf("objectstorage: pending database path is empty")
	}
	return &PerAccountStore{template: template, pendingPath: pendingPath, opts: opts, stores: map[string]*SQLiteStore{}}, nil
}

// PathFor is the database file of account.
func (p *PerAccountStore) PathFor(account string) (string, error) {
	if err := ValidateAccountID(account); err != nil {
		return "", err
	}
	if account == "_pending" {
		return p.pendingPath, nil
	}
	return strings.ReplaceAll(p.template, AccountPlaceholder, account), nil
}

func (p *PerAccountStore) store(account string) (*SQLiteStore, error) {
	path, err := p.PathFor(account)
	if err != nil {
		return nil, err
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return nil, ErrStoreClosed
	}
	if s, ok := p.stores[account]; ok {
		return s, nil
	}
	o := p.opts
	o.Path = path
	s, err := OpenSQLite(o)
	if err != nil {
		return nil, err
	}
	p.stores[account] = s
	return s, nil
}

// existing opens an account's database only if its file is already there, so
// read-only questions about an unknown account do not create files.
func (p *PerAccountStore) existing(account string) (*SQLiteStore, error) {
	path, err := p.PathFor(account)
	if err != nil {
		return nil, err
	}
	p.mu.Lock()
	_, open := p.stores[account]
	p.mu.Unlock()
	if !open {
		if _, err := os.Stat(path); err != nil {
			return nil, nil
		}
	}
	return p.store(account)
}

// DropAccount closes and deletes an account's database. The "_pending"
// namespace is cleared this way on every QR phase.
func (p *PerAccountStore) DropAccount(account string) error {
	path, err := p.PathFor(account)
	if err != nil {
		return err
	}
	p.mu.Lock()
	s := p.stores[account]
	delete(p.stores, account)
	p.mu.Unlock()
	if s != nil {
		_ = s.Close()
	}
	for _, suffix := range []string{"", "-wal", "-shm"} {
		if err := os.Remove(path + suffix); err != nil && !os.IsNotExist(err) {
			return err
		}
	}
	return nil
}

func (p *PerAccountStore) Put(ctx context.Context, ref ObjectRef, body io.Reader, opts PutOptions) (ObjectInfo, error) {
	s, err := p.store(ref.AccountID)
	if err != nil {
		return ObjectInfo{}, err
	}
	return s.Put(ctx, ref, body, opts)
}

func (p *PerAccountStore) Open(ctx context.Context, ref ObjectRef, opts GetOptions) (io.ReadCloser, ObjectInfo, error) {
	s, err := p.existing(ref.AccountID)
	if err != nil {
		return nil, ObjectInfo{}, err
	}
	if s == nil {
		return nil, ObjectInfo{}, fmt.Errorf("%w: %s", ErrNotFound, ref)
	}
	return s.Open(ctx, ref, opts)
}

// DatabaseSize is the size of the database file of account in bytes (0 when the
// account has no database yet).
func (p *PerAccountStore) DatabaseSize(ctx context.Context, account string) (int64, error) {
	s, err := p.existing(account)
	if err != nil || s == nil {
		return 0, err
	}
	return s.DatabaseSize(ctx, account)
}

func (p *PerAccountStore) Stat(ctx context.Context, ref ObjectRef) (ObjectInfo, error) {
	s, err := p.existing(ref.AccountID)
	if err != nil {
		return ObjectInfo{}, err
	}
	if s == nil {
		return ObjectInfo{}, fmt.Errorf("%w: %s", ErrNotFound, ref)
	}
	return s.Stat(ctx, ref)
}

func (p *PerAccountStore) Exists(ctx context.Context, ref ObjectRef) (bool, error) {
	_, err := p.Stat(ctx, ref)
	switch {
	case err == nil:
		return true, nil
	case isNotFound(err):
		return false, nil
	default:
		return false, err
	}
}

// Copy within an account is a single SQL transaction; between accounts the
// bytes are streamed from one database into the other.
func (p *PerAccountStore) Copy(ctx context.Context, src, dst ObjectRef) error {
	if src.AccountID == dst.AccountID {
		s, err := p.existing(src.AccountID)
		if err != nil {
			return err
		}
		if s == nil {
			return fmt.Errorf("%w: %s", ErrNotFound, src)
		}
		return s.Copy(ctx, src, dst)
	}
	rc, info, err := p.Open(ctx, src, GetOptions{})
	if err != nil {
		return err
	}
	defer rc.Close()
	_, err = p.Put(ctx, dst, rc, PutOptions{ContentType: info.ContentType, Metadata: info.Metadata})
	return err
}

func (p *PerAccountStore) Delete(ctx context.Context, ref ObjectRef) error {
	s, err := p.existing(ref.AccountID)
	if err != nil || s == nil {
		return err
	}
	return s.Delete(ctx, ref)
}

func (p *PerAccountStore) List(ctx context.Context, scope ObjectScope, opts ListOptions) (ObjectPage, error) {
	if err := ValidateAccountID(scope.AccountID); err != nil {
		return ObjectPage{}, err
	}
	s, err := p.existing(scope.AccountID)
	if err != nil || s == nil {
		return ObjectPage{}, err
	}
	return s.List(ctx, scope, opts)
}

func (p *PerAccountStore) SetMetadata(ctx context.Context, ref ObjectRef, contentType string, metadata map[string]string) error {
	s, err := p.existing(ref.AccountID)
	if err != nil {
		return err
	}
	if s == nil {
		return fmt.Errorf("%w: %s", ErrNotFound, ref)
	}
	return s.SetMetadata(ctx, ref, contentType, metadata)
}

// Verify re-reads an object and checks it against its recorded checksum.
func (p *PerAccountStore) Verify(ctx context.Context, ref ObjectRef) error {
	s, err := p.existing(ref.AccountID)
	if err != nil {
		return err
	}
	if s == nil {
		return fmt.Errorf("%w: %s", ErrNotFound, ref)
	}
	return s.Verify(ctx, ref)
}

// Backup writes a verified snapshot of one account's database.
func (p *PerAccountStore) Backup(ctx context.Context, account, dest string) (BackupResult, error) {
	s, err := p.existing(account)
	if err != nil {
		return BackupResult{}, err
	}
	if s == nil {
		return BackupResult{}, fmt.Errorf("%w: account %s has no media database", ErrNotFound, account)
	}
	return s.Backup(ctx, dest)
}

// ListAccounts finds every account that has a database file.
func (p *PerAccountStore) ListAccounts(ctx context.Context) ([]string, error) {
	pattern := strings.ReplaceAll(p.template, AccountPlaceholder, "*")
	matches, err := filepath.Glob(pattern)
	if err != nil {
		return nil, err
	}
	// Recover the account id from where the placeholder sat in the path.
	idx := strings.Index(p.template, AccountPlaceholder)
	prefix, suffix := p.template[:idx], p.template[idx+len(AccountPlaceholder):]
	var out []string
	for _, m := range matches {
		id := strings.TrimSuffix(strings.TrimPrefix(filepath.ToSlash(m), filepath.ToSlash(prefix)), filepath.ToSlash(suffix))
		if ValidateAccountID(id) == nil && id != "_pending" {
			out = append(out, id)
		}
	}
	sort.Strings(out)
	return out, nil
}

func (p *PerAccountStore) scanComposed(ctx context.Context, bucket, account, prefix, after string, inclusive bool, limit int) ([]ObjectInfo, error) {
	accounts := []string{account}
	if account == "" {
		var err error
		if accounts, err = p.ListAccounts(ctx); err != nil {
			return nil, err
		}
		// Each account's keys share the prefix accounts/<id>/media/, and no such
		// prefix contains another, so ordering accounts by it orders the keys.
		sort.Slice(accounts, func(i, j int) bool { return joinS3Key(accounts[i], "") < joinS3Key(accounts[j], "") })
	}
	upper, hasUpper := prefixUpperBound(prefix)
	var out []ObjectInfo
	for _, acct := range accounts {
		base := joinS3Key(acct, "")
		// Skip accounts wholly outside [prefix, upper) or wholly before the cursor.
		if hasUpper && base >= upper {
			continue
		}
		if after != "" {
			if bu, ok := prefixUpperBound(base); ok && bu <= after {
				continue
			}
		}
		s, err := p.existing(acct)
		if err != nil {
			return nil, err
		}
		if s == nil {
			continue
		}
		rows, err := s.scanComposed(ctx, bucket, acct, prefix, after, inclusive, limit-len(out))
		if err != nil {
			return nil, err
		}
		out = append(out, rows...)
		if len(out) >= limit {
			break
		}
	}
	return out, nil
}

func (p *PerAccountStore) CreateMultipart(ctx context.Context, ref ObjectRef, opts PutOptions) (string, error) {
	s, err := p.store(ref.AccountID)
	if err != nil {
		return "", err
	}
	return s.CreateMultipart(ctx, ref, opts)
}

func (p *PerAccountStore) UploadPart(ctx context.Context, ref ObjectRef, uploadID string, partNumber int, body io.Reader) (PartInfo, error) {
	s, err := p.store(ref.AccountID)
	if err != nil {
		return PartInfo{}, err
	}
	return s.UploadPart(ctx, ref, uploadID, partNumber, body)
}

func (p *PerAccountStore) CompleteMultipart(ctx context.Context, ref ObjectRef, uploadID string, parts []CompletedPart) (ObjectInfo, error) {
	s, err := p.store(ref.AccountID)
	if err != nil {
		return ObjectInfo{}, err
	}
	return s.CompleteMultipart(ctx, ref, uploadID, parts)
}

func (p *PerAccountStore) AbortMultipart(ctx context.Context, ref ObjectRef, uploadID string) error {
	s, err := p.store(ref.AccountID)
	if err != nil {
		return err
	}
	return s.AbortMultipart(ctx, ref, uploadID)
}

func (p *PerAccountStore) ListParts(ctx context.Context, ref ObjectRef, uploadID string, after, limit int) ([]PartInfo, error) {
	s, err := p.store(ref.AccountID)
	if err != nil {
		return nil, err
	}
	return s.ListParts(ctx, ref, uploadID, after, limit)
}

func (p *PerAccountStore) ListUploads(ctx context.Context, scope ObjectScope, prefix, keyMarker, uploadMarker string, limit int) ([]UploadInfo, error) {
	s, err := p.existing(scope.AccountID)
	if err != nil || s == nil {
		return nil, err
	}
	return s.ListUploads(ctx, scope, prefix, keyMarker, uploadMarker, limit)
}

// Close closes every open database.
func (p *PerAccountStore) Close() error {
	p.mu.Lock()
	p.closed = true
	stores := p.stores
	p.stores = map[string]*SQLiteStore{}
	p.mu.Unlock()
	var first error
	for _, s := range stores {
		if err := s.Close(); err != nil && first == nil {
			first = err
		}
	}
	return first
}

func isNotFound(err error) bool { return errors.Is(err, ErrNotFound) }

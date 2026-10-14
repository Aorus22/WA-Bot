package objectstorage

import (
	"context"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"
)

// Staging gives external programs (ffmpeg, yt-dlp, gallery-dl, webpmux, Lua
// scripts) the real files they need, without making those files the storage.
//
// A staged file is a scratch copy of an object, or a file a tool created for a
// key. It is never the source of truth: Sync/FlushAll copy new or changed
// files into the store, and Sweep deletes scratch files that are in the store
// and have not been used for a while. Everything under the staging folder is
// disposable and is wiped at start-up.
type Staging struct {
	w    *Wrapper
	root string

	mu     sync.Mutex
	synced map[string]fileSig // absolute path -> state when last in step with the store
	locks  map[string]*sync.Mutex
}

// lockFor serializes the uploads of one scratch file, so a slow upload of an
// older copy can never be published after a newer one.
func (st *Staging) lockFor(path string) *sync.Mutex {
	st.mu.Lock()
	defer st.mu.Unlock()
	l, ok := st.locks[path]
	if !ok {
		l = &sync.Mutex{}
		st.locks[path] = l
	}
	return l
}

type fileSig struct {
	size    int64
	modTime time.Time
}

const stagingMarker = ".wabot-staging"

// NewStaging prepares root (owner-only) and clears whatever a previous run left.
// It refuses a non-empty folder it did not create, so a mistaken setting such as
// the data folder can never be wiped.
func NewStaging(w *Wrapper, root string) (*Staging, error) {
	root, err := filepath.Abs(root)
	if err != nil {
		return nil, err
	}
	if entries, err := os.ReadDir(root); err == nil && len(entries) > 0 {
		if _, err := os.Stat(filepath.Join(root, stagingMarker)); err != nil {
			return nil, fmt.Errorf("objectstorage: %s is not empty and was not created as a staging folder; refusing to clear it", root)
		}
		for _, e := range entries {
			if err := os.RemoveAll(filepath.Join(root, e.Name())); err != nil {
				return nil, fmt.Errorf("objectstorage: clear staging: %w", err)
			}
		}
	}
	if err := os.MkdirAll(root, 0o700); err != nil {
		return nil, fmt.Errorf("objectstorage: create staging: %w", err)
	}
	if err := os.WriteFile(filepath.Join(root, stagingMarker), nil, 0o600); err != nil {
		return nil, fmt.Errorf("objectstorage: create staging: %w", err)
	}
	return &Staging{w: w, root: root, synced: map[string]fileSig{}, locks: map[string]*sync.Mutex{}}, nil
}

// Root is the staging folder.
func (st *Staging) Root() string { return st.root }

func (st *Staging) accountDir(account string) (string, error) {
	if err := ValidateAccountID(account); err != nil {
		return "", err
	}
	return filepath.Join(st.root, account), nil
}

// pathFor is the scratch location of key; it is safe by construction because
// the key was normalized (no "..", no absolute paths).
func (st *Staging) pathFor(account, key string) (string, string, error) {
	key, err := NormalizeKey(key)
	if err != nil {
		return "", "", err
	}
	dir, err := st.accountDir(account)
	if err != nil {
		return "", "", err
	}
	return filepath.Join(dir, filepath.FromSlash(key)), key, nil
}

// KeyOf maps a path inside the account's scratch folder back to its key.
func (st *Staging) KeyOf(account, path string) (string, bool) {
	dir, err := st.accountDir(account)
	if err != nil || !filepath.IsAbs(path) {
		return "", false
	}
	rel, err := filepath.Rel(dir, filepath.Clean(path))
	if err != nil || rel == "." || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return "", false
	}
	return filepath.ToSlash(rel), true
}

func sigOf(info fs.FileInfo) fileSig { return fileSig{size: info.Size(), modTime: info.ModTime()} }

// Path returns a local file for key. When the object exists and no scratch copy
// does, it is copied out (streamed); otherwise the path is where a tool should
// write it. Call Sync (or FlushAll) afterwards to store what a tool produced.
func (st *Staging) Path(ctx context.Context, account, key string) (string, error) {
	path, key, err := st.pathFor(account, key)
	if err != nil {
		return "", err
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		return "", err
	}
	if info, err := os.Lstat(path); err == nil {
		st.touch(path, info)
		return path, nil
	}
	rc, _, err := st.w.store.Open(ctx, ObjectRef{AccountID: account, Bucket: st.w.bucket, Key: key}, GetOptions{})
	if errors.Is(err, ErrNotFound) {
		return path, nil // a file the caller is about to create
	}
	if err != nil {
		return "", err
	}
	defer rc.Close()
	tmp, err := os.CreateTemp(filepath.Dir(path), ".stage-*")
	if err != nil {
		return "", err
	}
	if _, err := io.Copy(tmp, rc); err != nil {
		tmp.Close()
		os.Remove(tmp.Name())
		return "", err
	}
	if err := tmp.Close(); err != nil {
		os.Remove(tmp.Name())
		return "", err
	}
	if err := os.Rename(tmp.Name(), path); err != nil {
		os.Remove(tmp.Name())
		return "", err
	}
	if info, err := os.Lstat(path); err == nil {
		st.mu.Lock()
		st.synced[path] = sigOf(info)
		st.mu.Unlock()
	}
	return path, nil
}

// touch marks a scratch file as used now, so an idle sweep does not remove a
// file a script is about to hand to a tool. A file that is in step with the
// store stays in step.
func (st *Staging) touch(path string, info fs.FileInfo) {
	st.mu.Lock()
	prev, known := st.synced[path]
	st.mu.Unlock()
	now := time.Now()
	if err := os.Chtimes(path, now, now); err != nil {
		return
	}
	if known && prev == sigOf(info) {
		if after, err := os.Lstat(path); err == nil {
			st.mu.Lock()
			st.synced[path] = sigOf(after)
			st.mu.Unlock()
		}
	}
}

// Has reports whether a scratch file exists for key (it may be newer than, or
// absent from, the store).
func (st *Staging) Has(account, key string) bool {
	path, _, err := st.pathFor(account, key)
	if err != nil {
		return false
	}
	info, err := os.Lstat(path)
	return err == nil && info.Mode().IsRegular()
}

// Sync stores the scratch file of key if it is new or changed since it was
// last in step with the store. A missing scratch file is not an error.
func (st *Staging) Sync(ctx context.Context, account, key string) error {
	path, key, err := st.pathFor(account, key)
	if err != nil {
		return err
	}
	return st.syncFile(ctx, account, key, path)
}

func (st *Staging) syncFile(ctx context.Context, account, key, path string) error {
	lock := st.lockFor(path)
	lock.Lock()
	defer lock.Unlock()

	before, err := os.Lstat(path)
	if errors.Is(err, fs.ErrNotExist) {
		return nil
	}
	if err != nil {
		return err
	}
	if !before.Mode().IsRegular() {
		return fmt.Errorf("%w: %s is not a regular file", ErrInvalidKey, key)
	}
	st.mu.Lock()
	prev, known := st.synced[path]
	st.mu.Unlock()
	if known && prev == sigOf(before) {
		return nil
	}
	f, err := os.Open(path)
	if err != nil {
		return err
	}
	defer f.Close()
	if _, err := st.w.store.Put(ctx, ObjectRef{AccountID: account, Bucket: st.w.bucket, Key: key}, f, PutOptions{ContentType: contentTypeFor(key, f)}); err != nil {
		return err
	}
	// Only a file that did not change while it was read counts as stored; a
	// growing one stays pending and is stored again by the next sync.
	if after, err := os.Lstat(path); err == nil && sigOf(after) == sigOf(before) {
		st.mu.Lock()
		st.synced[path] = sigOf(before)
		st.mu.Unlock()
	}
	return nil
}

// FlushAll stores every new or changed scratch file of account and returns how
// many were stored.
func (st *Staging) FlushAll(ctx context.Context, account string) (int, error) {
	return st.flush(ctx, account, 0)
}

// flush is FlushAll skipping files modified less than minAge ago: a periodic
// sweep must not store a file a program is still writing.
func (st *Staging) flush(ctx context.Context, account string, minAge time.Duration) (int, error) {
	dir, err := st.accountDir(account)
	if err != nil {
		return 0, err
	}
	stored := 0
	var firstErr error
	walkErr := filepath.WalkDir(dir, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			if errors.Is(err, fs.ErrNotExist) {
				return nil
			}
			return err
		}
		if d.IsDir() || strings.HasPrefix(d.Name(), ".stage-") {
			return nil
		}
		key, ok := st.KeyOf(account, path)
		if !ok {
			return nil
		}
		if _, err := NormalizeKey(key); err != nil {
			return nil
		}
		st.mu.Lock()
		prev, known := st.synced[path]
		st.mu.Unlock()
		if info, ierr := d.Info(); ierr == nil {
			if known && prev == sigOf(info) {
				return nil
			}
			if minAge > 0 && time.Since(info.ModTime()) < minAge {
				return nil
			}
		}
		if err := st.syncFile(ctx, account, key, path); err != nil {
			if firstErr == nil {
				firstErr = err
			}
			return nil
		}
		stored++
		return nil
	})
	if firstErr == nil {
		firstErr = walkErr
	}
	return stored, firstErr
}

// FlushEverything stores pending scratch files of every account, whatever their
// age (shutdown).
func (st *Staging) FlushEverything(ctx context.Context) error {
	entries, err := os.ReadDir(st.root)
	if err != nil {
		return err
	}
	var firstErr error
	for _, e := range entries {
		if !e.IsDir() || ValidateAccountID(e.Name()) != nil {
			continue
		}
		if _, err := st.FlushAll(ctx, e.Name()); err != nil && firstErr == nil {
			firstErr = err
		}
	}
	return firstErr
}

// Discard removes the scratch copy of key (after the object was deleted).
func (st *Staging) Discard(account, key string) {
	path, _, err := st.pathFor(account, key)
	if err != nil {
		return
	}
	_ = os.Remove(path)
	st.mu.Lock()
	delete(st.synced, path)
	st.mu.Unlock()
}

// sweepQuietPeriod is how long a changed scratch file must have been left alone
// before a background sweep stores it.
const sweepQuietPeriod = 30 * time.Second

// Sweep stores pending changes of every account, then deletes scratch files that
// are in step with the store and unused for longer than idle.
func (st *Staging) Sweep(ctx context.Context, idle time.Duration) {
	entries, _ := os.ReadDir(st.root)
	for _, e := range entries {
		if !e.IsDir() || ValidateAccountID(e.Name()) != nil {
			continue
		}
		_, _ = st.flush(ctx, e.Name(), sweepQuietPeriod)
	}
	cutoff := time.Now().Add(-idle)
	_ = filepath.WalkDir(st.root, func(path string, d fs.DirEntry, err error) error {
		if err != nil || d.IsDir() {
			return nil
		}
		info, err := d.Info()
		if err != nil || info.ModTime().After(cutoff) {
			return nil
		}
		st.mu.Lock()
		prev, known := st.synced[path]
		st.mu.Unlock()
		if known && prev == sigOf(info) {
			_ = os.Remove(path)
			st.mu.Lock()
			delete(st.synced, path)
			st.mu.Unlock()
		}
		return nil
	})
	// Drop folders that emptied out (never the root or the account folders).
	var dirs []string
	_ = filepath.WalkDir(st.root, func(path string, d fs.DirEntry, err error) error {
		if err == nil && d.IsDir() && path != st.root && filepath.Dir(path) != st.root {
			dirs = append(dirs, path)
		}
		return nil
	})
	for i := len(dirs) - 1; i >= 0; i-- {
		_ = os.Remove(dirs[i]) // only succeeds when empty
	}
}

// TempDir makes a private scratch folder (outside any account's key space) for
// a tool whose output names are not known in advance. Call cleanup when done.
func (st *Staging) TempDir(prefix string) (dir string, cleanup func(), err error) {
	base := filepath.Join(st.root, ".tmp")
	if err := os.MkdirAll(base, 0o700); err != nil {
		return "", nil, err
	}
	dir, err = os.MkdirTemp(base, prefix+"-*")
	if err != nil {
		return "", nil, err
	}
	return dir, func() { _ = os.RemoveAll(dir) }, nil
}

// Package backup copies an account's chats and media to a Target (a local
// folder today, Google Drive or GitHub later) as independent, content-addressed
// chunks plus a manifest per backup, and rebuilds the databases from them.
//
// Nothing is ever one big archive: every chunk is compressed and checksummed on
// its own, chats are split per conversation per month and media per object, so
// a damaged chunk costs only the rows or the media it holds. The manifest is
// written last and names its own checksum; a backup without one never existed.
package backup

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"os"
	"path"
	"path/filepath"
	"strings"
	"time"
)

// ErrNotExist is returned by Target.Get for a name the target does not hold.
var ErrNotExist = errors.New("backup: not in target")

// Entry describes one file in a target.
type Entry struct {
	Name    string
	Size    int64
	ModTime time.Time
	// SHA256 is the target's own checksum of the stored bytes (hex), when the
	// target computes one. Empty when it does not.
	SHA256 string
}

// Target is a place backups are written to. Names are slash-separated
// relative paths such as "chunks/ab/abcdef…".
type Target interface {
	// Put stores data under name atomically: afterwards the name either holds
	// all of data or does not exist. The returned entry carries the target's
	// checksum of what it stored, when it has one.
	Put(ctx context.Context, name string, data io.Reader, size int64) (Entry, error)
	// Get opens a stored file; ErrNotExist when it is missing.
	Get(ctx context.Context, name string) (io.ReadCloser, error)
	// List returns every file whose name starts with prefix.
	List(ctx context.Context, prefix string) ([]Entry, error)
	// Delete removes a file; a missing file is not an error.
	Delete(ctx context.Context, name string) error
}

func validName(name string) error {
	if name == "" || strings.HasPrefix(name, "/") || strings.Contains(name, "\\") || path.Clean(name) != name || strings.HasPrefix(name, "..") {
		return fmt.Errorf("backup: invalid name %q", name)
	}
	return nil
}

// LocalFolder is a Target on the local filesystem (an external disk, a NAS
// mount, or a folder for tests).
type LocalFolder struct {
	Root string
}

const localTempMark = ".partial-"

func (l LocalFolder) file(name string) (string, error) {
	if err := validName(name); err != nil {
		return "", err
	}
	return filepath.Join(l.Root, filepath.FromSlash(name)), nil
}

func (l LocalFolder) Put(ctx context.Context, name string, data io.Reader, size int64) (Entry, error) {
	dest, err := l.file(name)
	if err != nil {
		return Entry{}, err
	}
	if err := os.MkdirAll(filepath.Dir(dest), 0o700); err != nil {
		return Entry{}, err
	}
	f, err := os.CreateTemp(filepath.Dir(dest), localTempMark+"*")
	if err != nil {
		return Entry{}, err
	}
	tmp := f.Name()
	defer os.Remove(tmp)
	h := sha256.New()
	n, err := io.Copy(io.MultiWriter(f, h), ctxReader{ctx, data})
	if err == nil && size >= 0 && n != size {
		err = fmt.Errorf("backup: %s: wrote %d bytes, expected %d", name, n, size)
	}
	if err == nil {
		err = f.Sync()
	}
	if cerr := f.Close(); err == nil {
		err = cerr
	}
	if err != nil {
		return Entry{}, err
	}
	if err := os.Rename(tmp, dest); err != nil {
		return Entry{}, err
	}
	syncDir(filepath.Dir(dest))
	return Entry{Name: name, Size: n, ModTime: time.Now(), SHA256: hex.EncodeToString(h.Sum(nil))}, nil
}

func (l LocalFolder) Get(_ context.Context, name string) (io.ReadCloser, error) {
	p, err := l.file(name)
	if err != nil {
		return nil, err
	}
	f, err := os.Open(p)
	if errors.Is(err, fs.ErrNotExist) {
		return nil, fmt.Errorf("%w: %s", ErrNotExist, name)
	}
	return f, err
}

func (l LocalFolder) List(ctx context.Context, prefix string) ([]Entry, error) {
	var out []Entry
	err := filepath.WalkDir(l.Root, func(p string, d fs.DirEntry, err error) error {
		if err != nil {
			if errors.Is(err, fs.ErrNotExist) {
				return nil
			}
			return err
		}
		if ctx.Err() != nil {
			return ctx.Err()
		}
		if d.IsDir() || strings.HasPrefix(d.Name(), localTempMark) {
			return nil
		}
		rel, err := filepath.Rel(l.Root, p)
		if err != nil {
			return err
		}
		name := filepath.ToSlash(rel)
		if !strings.HasPrefix(name, prefix) {
			return nil
		}
		info, err := d.Info()
		if err != nil {
			return err
		}
		out = append(out, Entry{Name: name, Size: info.Size(), ModTime: info.ModTime()})
		return nil
	})
	return out, err
}

func (l LocalFolder) Delete(_ context.Context, name string) error {
	p, err := l.file(name)
	if err != nil {
		return err
	}
	if err := os.Remove(p); err != nil && !errors.Is(err, fs.ErrNotExist) {
		return err
	}
	return nil
}

func syncDir(dir string) {
	if d, err := os.Open(dir); err == nil {
		_ = d.Sync()
		d.Close()
	}
}

// ctxReader stops a long copy when the context is cancelled.
type ctxReader struct {
	ctx context.Context
	r   io.Reader
}

func (c ctxReader) Read(p []byte) (int, error) {
	if err := c.ctx.Err(); err != nil {
		return 0, err
	}
	return c.r.Read(p)
}

package objectstorage

import (
	"context"
	"errors"
	"fmt"
	"io"
	"mime"
	"net/http"
	"path"
	"strings"
)

// ServeObject answers a GET/HEAD for one object: Content-Type, ETag,
// Last-Modified, conditional requests, Accept-Ranges and single or multiple
// Range requests (206/416), streaming from the store. A request that goes away
// stops the read. It never loads the object into memory.
func ServeObject(w http.ResponseWriter, r *http.Request, store Store, ref ObjectRef) {
	info, err := store.Stat(r.Context(), ref)
	if err != nil {
		writeStoreError(w, err)
		return
	}
	if info.ContentType != "" {
		w.Header().Set("Content-Type", info.ContentType)
	}
	if info.ETag != "" {
		w.Header().Set("ETag", `"`+info.ETag+`"`)
	}
	w.Header().Set("Accept-Ranges", "bytes")
	rs := NewReadSeeker(r.Context(), store, ref, info)
	defer rs.Close()
	http.ServeContent(w, r, "", info.LastModified, rs)
}

func writeStoreError(w http.ResponseWriter, err error) {
	switch {
	case errors.Is(err, ErrNotFound):
		http.Error(w, "media not found", http.StatusNotFound)
	case errors.Is(err, ErrInvalidKey):
		http.Error(w, "invalid media key", http.StatusBadRequest)
	case errors.Is(err, ErrInvalidRange):
		http.Error(w, "range not satisfiable", http.StatusRequestedRangeNotSatisfiable)
	case errors.Is(err, ErrUnauthorized):
		http.Error(w, "storage access denied", http.StatusBadGateway)
	default:
		http.Error(w, "storage unavailable", http.StatusServiceUnavailable)
	}
}

// NewReadSeeker presents an object as an io.ReadSeekCloser over any provider. Each
// run of reads opens one ranged stream; Seek only moves the position, so
// http.ServeContent can answer ranges and sniff without downloading the rest.
func NewReadSeeker(ctx context.Context, store Store, ref ObjectRef, info ObjectInfo) io.ReadSeekCloser {
	return &storeSeeker{ctx: ctx, store: store, ref: ref, size: info.Size}
}

type storeSeeker struct {
	ctx   context.Context
	store Store
	ref   ObjectRef
	size  int64
	pos   int64
	cur   io.ReadCloser
}

func (s *storeSeeker) Read(p []byte) (int, error) {
	if s.pos >= s.size {
		return 0, io.EOF
	}
	if s.cur == nil {
		rc, _, err := s.store.Open(s.ctx, s.ref, GetOptions{Range: &ByteRange{Start: s.pos, End: -1}})
		if err != nil {
			return 0, err
		}
		s.cur = rc
	}
	n, err := s.cur.Read(p)
	s.pos += int64(n)
	if err == io.EOF && s.pos < s.size {
		err = io.ErrUnexpectedEOF
	}
	return n, err
}

func (s *storeSeeker) Seek(offset int64, whence int) (int64, error) {
	var abs int64
	switch whence {
	case io.SeekStart:
		abs = offset
	case io.SeekCurrent:
		abs = s.pos + offset
	case io.SeekEnd:
		abs = s.size + offset
	default:
		return 0, fmt.Errorf("%w: whence %d", ErrInvalidRange, whence)
	}
	if abs < 0 {
		return 0, fmt.Errorf("%w: negative position", ErrInvalidRange)
	}
	if abs != s.pos && s.cur != nil {
		s.cur.Close()
		s.cur = nil
	}
	s.pos = abs
	return abs, nil
}

// Close releases an open stream.
func (s *storeSeeker) Close() error {
	if s.cur != nil {
		err := s.cur.Close()
		s.cur = nil
		return err
	}
	return nil
}

// contentTypeFor guesses a content type from the key's extension, then from the
// first bytes of f (which is rewound).
func contentTypeFor(key string, f io.ReadSeeker) string {
	if ct := mime.TypeByExtension(strings.ToLower(path.Ext(key))); ct != "" {
		return ct
	}
	head := make([]byte, 512)
	n, _ := io.ReadFull(f, head)
	_, _ = f.Seek(0, io.SeekStart)
	return http.DetectContentType(head[:n])
}

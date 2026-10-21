package backup

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"sync"
	"time"

	"github.com/klauspost/compress/zstd"
)

const (
	encodingZstd  = "zstd"
	maxChunkBytes = 64 << 20 // no chunk this package writes is larger
	putAttempts   = 3
	getAttempts   = 3
)

// retryDelay is the pause before the second attempt; later ones wait longer.
var retryDelay = time.Second

var (
	zstdOnce sync.Once
	zstdEnc  *zstd.Encoder
	zstdDec  *zstd.Decoder
)

func codecs() (*zstd.Encoder, *zstd.Decoder) {
	zstdOnce.Do(func() {
		zstdEnc, _ = zstd.NewWriter(nil, zstd.WithEncoderLevel(zstd.SpeedDefault), zstd.WithEncoderConcurrency(1))
		zstdDec, _ = zstd.NewReader(nil, zstd.WithDecoderConcurrency(1), zstd.WithDecoderMaxMemory(maxChunkBytes*4))
	})
	return zstdEnc, zstdDec
}

func sum256(b []byte) string {
	s := sha256.Sum256(b)
	return hex.EncodeToString(s[:])
}

// chunkStore writes and reads chunks of one target, skipping chunks the target
// already holds.
type chunkStore struct {
	t       Target
	present map[string]int64 // chunk name -> size

	uploaded      int
	reused        int
	uploadedBytes int64
}

func newChunkStore(ctx context.Context, t Target) (*chunkStore, error) {
	entries, err := t.List(ctx, chunkPrefix)
	if err != nil {
		return nil, fmt.Errorf("backup: list chunks: %w", err)
	}
	c := &chunkStore{t: t, present: make(map[string]int64, len(entries))}
	for _, e := range entries {
		c.present[e.Name] = e.Size
	}
	return c, nil
}

func (c *chunkStore) has(sum string, size int64) bool {
	got, ok := c.present[chunkName(sum)]
	return ok && got == size
}

// putRaw stores b as is (media).
func (c *chunkStore) putRaw(ctx context.Context, b []byte) (ChunkRef, error) {
	ref := ChunkRef{SHA256: sum256(b), Size: int64(len(b))}
	return ref, c.store(ctx, ref, b)
}

// putCompressed stores b compressed (table segments).
func (c *chunkStore) putCompressed(ctx context.Context, raw []byte) (ChunkRef, error) {
	enc, _ := codecs()
	stored := enc.EncodeAll(raw, nil)
	ref := ChunkRef{
		SHA256: sum256(stored), Size: int64(len(stored)), Encoding: encodingZstd,
		RawSHA256: sum256(raw), RawSize: int64(len(raw)),
	}
	return ref, c.store(ctx, ref, stored)
}

func (c *chunkStore) store(ctx context.Context, ref ChunkRef, stored []byte) error {
	if c.has(ref.SHA256, ref.Size) {
		c.reused++
		return nil
	}
	name := chunkName(ref.SHA256)
	var err error
	for attempt := 0; attempt < putAttempts; attempt++ {
		if attempt > 0 {
			if werr := wait(ctx, time.Duration(attempt)*retryDelay); werr != nil {
				return werr
			}
		}
		var e Entry
		e, err = c.t.Put(ctx, name, bytes.NewReader(stored), ref.Size)
		if err == nil && e.SHA256 != "" && e.SHA256 != ref.SHA256 {
			// The target stored something else: never keep it under this name.
			_ = c.t.Delete(ctx, name)
			err = fmt.Errorf("backup: %s stored with checksum %s", name, e.SHA256)
		}
		if err == nil {
			c.present[name] = ref.Size
			c.uploaded++
			c.uploadedBytes += ref.Size
			return nil
		}
		if ctx.Err() != nil {
			return ctx.Err()
		}
	}
	return err
}

// get downloads a chunk, checks it against ref and returns the decoded bytes.
func (c *chunkStore) get(ctx context.Context, ref ChunkRef) ([]byte, error) {
	var err error
	for attempt := 0; attempt < getAttempts; attempt++ {
		if attempt > 0 {
			if werr := wait(ctx, time.Duration(attempt)*retryDelay); werr != nil {
				return nil, werr
			}
		}
		var b []byte
		b, err = fetchChunk(ctx, c.t, ref)
		if err == nil {
			return b, nil
		}
		if errors.Is(err, ErrNotExist) || ctx.Err() != nil {
			break
		}
	}
	return nil, err
}

func fetchChunk(ctx context.Context, t Target, ref ChunkRef) ([]byte, error) {
	if ref.Size > maxChunkBytes || len(ref.SHA256) != 64 {
		return nil, fmt.Errorf("backup: chunk %q is not valid", ref.SHA256)
	}
	rc, err := t.Get(ctx, chunkName(ref.SHA256))
	if err != nil {
		return nil, err
	}
	defer rc.Close()
	stored, err := io.ReadAll(io.LimitReader(rc, ref.Size+1))
	if err != nil {
		return nil, err
	}
	if int64(len(stored)) != ref.Size || sum256(stored) != ref.SHA256 {
		return nil, fmt.Errorf("backup: chunk %s is damaged", ref.SHA256)
	}
	switch ref.Encoding {
	case "":
		return stored, nil
	case encodingZstd:
		_, dec := codecs()
		raw, err := dec.DecodeAll(stored, nil)
		if err != nil {
			return nil, fmt.Errorf("backup: chunk %s: %w", ref.SHA256, err)
		}
		if int64(len(raw)) != ref.RawSize || sum256(raw) != ref.RawSHA256 {
			return nil, fmt.Errorf("backup: chunk %s decodes to the wrong content", ref.SHA256)
		}
		return raw, nil
	default:
		return nil, fmt.Errorf("backup: chunk %s: unknown encoding %q", ref.SHA256, ref.Encoding)
	}
}

func wait(ctx context.Context, d time.Duration) error {
	t := time.NewTimer(d)
	defer t.Stop()
	select {
	case <-ctx.Done():
		return ctx.Err()
	case <-t.C:
		return nil
	}
}

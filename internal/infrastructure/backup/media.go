package backup

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"os"

	"wa-bot/internal/infrastructure/objectstorage"
)

// mediaPartSize is the largest media chunk; bigger objects are split.
var mediaPartSize = 32 << 20

// backupMedia stores every object of store. Objects whose bytes the target
// already holds are referenced without being read again; new ones are read,
// uploaded part by part and checked against the checksum recorded when they
// were saved. An object that fails that check is left out and reported.
func backupMedia(ctx context.Context, store *objectstorage.SQLiteStore, file string, prev map[string][]ChunkRef, chunks *chunkStore, progress func(Progress)) (*Media, error) {
	if err := store.IntegrityCheck(ctx); err != nil {
		return nil, fmt.Errorf("media database is damaged, nothing was backed up: %w", err)
	}
	scopes, err := store.ListScopes(ctx)
	if err != nil {
		return nil, err
	}
	var infos []objectstorage.ObjectInfo
	for _, sc := range scopes {
		cursor := ""
		for {
			page, err := store.List(ctx, sc, objectstorage.ListOptions{Limit: objectstorage.MaxListLimit, Cursor: cursor})
			if err != nil {
				return nil, err
			}
			infos = append(infos, page.Objects...)
			if page.NextCursor == "" {
				break
			}
			cursor = page.NextCursor
		}
	}

	m := &Media{File: file}
	buf := make([]byte, mediaPartSize)
	for i, info := range infos {
		progress(Progress{Stage: "media", Item: info.Ref.Key, Done: int64(i), Total: int64(len(infos))})
		parts, err := mediaParts(ctx, store, info, prev, chunks, buf)
		if errors.Is(err, objectstorage.ErrNotFound) {
			continue // deleted while the backup ran
		}
		if errors.Is(err, objectstorage.ErrCorruptData) {
			m.Skipped = append(m.Skipped, info.Ref.String())
			continue
		}
		if err != nil {
			return nil, fmt.Errorf("media %s: %w", info.Ref, err)
		}
		m.Objects = append(m.Objects, MediaObject{
			Account: info.Ref.AccountID, Bucket: info.Ref.Bucket, Key: info.Ref.Key,
			Size: info.Size, SHA256: info.SHA256, ContentType: info.ContentType,
			Metadata: info.Metadata, LastModified: info.LastModified.UTC(), Parts: parts,
		})
		m.Bytes += info.Size
	}
	progress(Progress{Stage: "media", Done: int64(len(infos)), Total: int64(len(infos))})
	return m, nil
}

func mediaParts(ctx context.Context, store *objectstorage.SQLiteStore, info objectstorage.ObjectInfo, prev map[string][]ChunkRef, chunks *chunkStore, buf []byte) ([]ChunkRef, error) {
	if info.SHA256 != "" {
		// A one-part object's chunk is named after the object's own checksum.
		if info.Size <= int64(mediaPartSize) && chunks.has(info.SHA256, info.Size) {
			chunks.reused++
			return []ChunkRef{{SHA256: info.SHA256, Size: info.Size}}, nil
		}
		if parts, ok := prev[info.SHA256]; ok && partsPresent(parts, chunks) {
			chunks.reused += len(parts)
			return parts, nil
		}
	}
	rc, got, err := store.Open(ctx, info.Ref, objectstorage.GetOptions{})
	if err != nil {
		return nil, err
	}
	defer rc.Close()
	whole := sha256.New()
	var parts []ChunkRef
	var total int64
	for {
		n, rerr := io.ReadFull(rc, buf)
		if n > 0 {
			whole.Write(buf[:n])
			total += int64(n)
			ref, err := chunks.putRaw(ctx, buf[:n])
			if err != nil {
				return nil, err
			}
			parts = append(parts, ref)
		}
		if rerr == io.EOF || rerr == io.ErrUnexpectedEOF {
			break
		}
		if rerr != nil {
			return nil, rerr
		}
	}
	if total != got.Size || hex.EncodeToString(whole.Sum(nil)) != got.SHA256 {
		return nil, fmt.Errorf("%w: %s does not match its recorded checksum", objectstorage.ErrCorruptData, info.Ref)
	}
	if len(parts) == 0 {
		// An empty object still needs one (empty) chunk to restore from.
		ref, err := chunks.putRaw(ctx, nil)
		if err != nil {
			return nil, err
		}
		parts = []ChunkRef{ref}
	}
	return parts, nil
}

func partsPresent(parts []ChunkRef, chunks *chunkStore) bool {
	for _, p := range parts {
		if !chunks.has(p.SHA256, p.Size) {
			return false
		}
	}
	return len(parts) > 0
}

// restoreMedia builds a new media database at dest. Objects that cannot be
// read back are recorded and skipped.
func restoreMedia(ctx context.Context, m *Media, dest string, chunks *chunkStore, rep *RestoreReport, progress func(Progress)) error {
	if _, err := os.Stat(dest); err == nil {
		return fmt.Errorf("%s already exists", dest)
	}
	store, err := objectstorage.OpenSQLite(objectstorage.SQLiteOptions{Path: dest})
	if err != nil {
		return err
	}
	defer store.Close()
	for i, o := range m.Objects {
		progress(Progress{Stage: "media", Item: o.Key, Done: int64(i), Total: int64(len(m.Objects))})
		ref := objectstorage.ObjectRef{AccountID: o.Account, Bucket: o.Bucket, Key: o.Key}
		err := restoreObject(ctx, store, ref, o, chunks)
		if err != nil {
			if ctx.Err() != nil {
				return ctx.Err()
			}
			_ = store.Delete(ctx, ref)
			rep.Lost = append(rep.Lost, LostItem{What: "media", Key: ref.String(), Err: err.Error()})
			continue
		}
		rep.Media++
	}
	progress(Progress{Stage: "media", Done: int64(len(m.Objects)), Total: int64(len(m.Objects))})
	if err := store.IntegrityCheck(ctx); err != nil {
		return fmt.Errorf("restored media database failed its check: %w", err)
	}
	return store.Close()
}

func restoreObject(ctx context.Context, store *objectstorage.SQLiteStore, ref objectstorage.ObjectRef, o MediaObject, chunks *chunkStore) error {
	pr, pw := io.Pipe()
	go func() {
		// Each part is checked before any of it reaches the store, and a bad
		// part aborts the upload, so a damaged object is never published.
		for _, p := range o.Parts {
			b, err := chunks.get(ctx, p)
			if err != nil {
				pw.CloseWithError(err)
				return
			}
			if _, err := pw.Write(b); err != nil {
				return
			}
		}
		pw.Close()
	}()
	info, err := store.Put(ctx, ref, pr, objectstorage.PutOptions{ContentType: o.ContentType, Metadata: o.Metadata})
	pr.CloseWithError(errors.New("upload finished"))
	if err != nil {
		return err
	}
	if info.Size != o.Size || info.SHA256 != o.SHA256 {
		return fmt.Errorf("restored bytes do not match the recorded checksum")
	}
	return nil
}

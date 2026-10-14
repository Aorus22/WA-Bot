package objectstorage

import (
	"context"
	"crypto/md5"
	"crypto/rand"
	"database/sql"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"strings"
	"time"
)

var _ MultipartStore = (*SQLiteStore)(nil)

func newUploadID() string {
	b := make([]byte, 16)
	if _, err := rand.Read(b); err != nil {
		panic(err) // the OS random source failing is not recoverable
	}
	return hex.EncodeToString(b)
}

// ErrNoSuchUpload is returned for an unknown or finished upload id.
var ErrNoSuchUpload = fmt.Errorf("%w: no such multipart upload", ErrNotFound)

func (s *SQLiteStore) uploadExists(ctx context.Context, ref ObjectRef, id string) error {
	var one int
	err := s.db.QueryRowContext(ctx, `SELECT 1 FROM multipart_uploads
		WHERE id = ? AND account_id = ? AND bucket = ? AND key = ?`, id, ref.AccountID, ref.Bucket, ref.Key).Scan(&one)
	if errors.Is(err, sql.ErrNoRows) {
		return ErrNoSuchUpload
	}
	return mapErr(err)
}

func (s *SQLiteStore) CreateMultipart(ctx context.Context, ref ObjectRef, opts PutOptions) (string, error) {
	ref, err := canonical(ref)
	if err != nil {
		return "", err
	}
	if s.isClosed() {
		return "", ErrStoreClosed
	}
	meta, err := encodeMetadata(opts.Metadata)
	if err != nil {
		return "", err
	}
	id := newUploadID()
	if _, err := s.db.ExecContext(ctx, `INSERT INTO multipart_uploads
		(id, account_id, bucket, key, content_type, metadata, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)`,
		id, ref.AccountID, ref.Bucket, ref.Key, opts.ContentType, meta, time.Now().UnixMilli()); err != nil {
		return "", mapErr(err)
	}
	return id, nil
}

func (s *SQLiteStore) UploadPart(ctx context.Context, ref ObjectRef, uploadID string, partNumber int, body io.Reader) (PartInfo, error) {
	ref, err := canonical(ref)
	if err != nil {
		return PartInfo{}, err
	}
	if partNumber < 1 || partNumber > MaxPartNumber {
		return PartInfo{}, fmt.Errorf("%w: part number %d", ErrInvalidRange, partNumber)
	}
	if err := s.uploadExists(ctx, ref, uploadID); err != nil {
		return PartInfo{}, err
	}

	// A re-uploaded part replaces the old one; the new row starts without an
	// etag and only counts once it is complete.
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return PartInfo{}, mapErr(err)
	}
	if _, err := tx.ExecContext(ctx, `DELETE FROM multipart_parts WHERE upload_id = ? AND part_number = ?`, uploadID, partNumber); err != nil {
		tx.Rollback()
		return PartInfo{}, mapErr(err)
	}
	now := time.Now()
	if _, err := tx.ExecContext(ctx, `INSERT INTO multipart_parts (upload_id, part_number, created_at) VALUES (?, ?, ?)`,
		uploadID, partNumber, now.UnixMilli()); err != nil {
		tx.Rollback()
		return PartInfo{}, mapErr(err)
	}
	if err := tx.Commit(); err != nil {
		return PartInfo{}, mapErr(err)
	}

	done := false
	defer func() {
		if !done {
			cctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
			defer cancel()
			_, _ = s.db.ExecContext(cctx, `DELETE FROM multipart_parts WHERE upload_id = ? AND part_number = ? AND etag = ''`, uploadID, partNumber)
		}
	}()

	h := md5.New()
	buf := make([]byte, s.chunkSize)
	var size int64
	for seq := int64(0); ; seq++ {
		if err := ctx.Err(); err != nil {
			return PartInfo{}, err
		}
		n, rerr := io.ReadFull(body, buf)
		if n > 0 {
			h.Write(buf[:n])
			if _, err := s.db.ExecContext(ctx, `INSERT INTO multipart_chunks (upload_id, part_number, seq, data) VALUES (?, ?, ?, ?)`,
				uploadID, partNumber, seq, buf[:n]); err != nil {
				return PartInfo{}, mapErr(err)
			}
			size += int64(n)
		}
		if rerr == io.EOF || rerr == io.ErrUnexpectedEOF {
			break
		}
		if rerr != nil {
			return PartInfo{}, rerr
		}
	}
	etag := hex.EncodeToString(h.Sum(nil))
	if _, err := s.db.ExecContext(ctx, `UPDATE multipart_parts SET etag = ?, size = ? WHERE upload_id = ? AND part_number = ?`,
		etag, size, uploadID, partNumber); err != nil {
		return PartInfo{}, mapErr(err)
	}
	done = true
	return PartInfo{PartNumber: partNumber, ETag: etag, Size: size, LastModified: now.UTC()}, nil
}

func (s *SQLiteStore) ListParts(ctx context.Context, ref ObjectRef, uploadID string, after, limit int) ([]PartInfo, error) {
	ref, err := canonical(ref)
	if err != nil {
		return nil, err
	}
	if err := s.uploadExists(ctx, ref, uploadID); err != nil {
		return nil, err
	}
	if limit <= 0 || limit > 1000 {
		limit = 1000
	}
	rows, err := s.db.QueryContext(ctx, `SELECT part_number, etag, size, created_at FROM multipart_parts
		WHERE upload_id = ? AND part_number > ? AND etag != '' ORDER BY part_number LIMIT ?`, uploadID, after, limit)
	if err != nil {
		return nil, mapErr(err)
	}
	defer rows.Close()
	var out []PartInfo
	for rows.Next() {
		var p PartInfo
		var ts int64
		if err := rows.Scan(&p.PartNumber, &p.ETag, &p.Size, &ts); err != nil {
			return nil, mapErr(err)
		}
		p.LastModified = time.UnixMilli(ts).UTC()
		out = append(out, p)
	}
	return out, mapErr(rows.Err())
}

func (s *SQLiteStore) ListUploads(ctx context.Context, scope ObjectScope, prefix, keyMarker, uploadMarker string, limit int) ([]UploadInfo, error) {
	if err := ValidateAccountID(scope.AccountID); err != nil {
		return nil, err
	}
	if scope.Bucket == "" {
		scope.Bucket = DefaultBucket
	}
	if limit <= 0 || limit > 1000 {
		limit = 1000
	}
	rows, err := s.db.QueryContext(ctx, `SELECT id, key, created_at FROM multipart_uploads
		WHERE account_id = ? AND bucket = ? AND key >= ? AND (key > ? OR (key = ? AND id > ?))
		ORDER BY key, id LIMIT ?`, scope.AccountID, scope.Bucket, prefix, keyMarker, keyMarker, uploadMarker, limit)
	if err != nil {
		return nil, mapErr(err)
	}
	defer rows.Close()
	var out []UploadInfo
	for rows.Next() {
		var u UploadInfo
		var ts int64
		if err := rows.Scan(&u.UploadID, &u.Ref.Key, &ts); err != nil {
			return nil, mapErr(err)
		}
		if prefix != "" && !strings.HasPrefix(u.Ref.Key, prefix) {
			continue
		}
		u.Ref.AccountID, u.Ref.Bucket = scope.AccountID, scope.Bucket
		u.Initiated = time.UnixMilli(ts).UTC()
		out = append(out, u)
	}
	return out, mapErr(rows.Err())
}

func (s *SQLiteStore) AbortMultipart(ctx context.Context, ref ObjectRef, uploadID string) error {
	ref, err := canonical(ref)
	if err != nil {
		return err
	}
	if err := s.uploadExists(ctx, ref, uploadID); err != nil {
		return err
	}
	_, err = s.db.ExecContext(ctx, `DELETE FROM multipart_uploads WHERE id = ?`, uploadID)
	return mapErr(err)
}

// CompleteMultipart assembles the listed parts into one object. The parts are
// streamed through Put, so memory stays at one chunk however large they are.
// The object's ETag follows S3: md5 of the part md5s, then "-<parts>".
func (s *SQLiteStore) CompleteMultipart(ctx context.Context, ref ObjectRef, uploadID string, parts []CompletedPart) (ObjectInfo, error) {
	ref, err := canonical(ref)
	if err != nil {
		return ObjectInfo{}, err
	}
	if len(parts) == 0 {
		return ObjectInfo{}, fmt.Errorf("%w: no parts", ErrInvalidRange)
	}
	var contentType, metaJSON string
	err = s.db.QueryRowContext(ctx, `SELECT content_type, metadata FROM multipart_uploads
		WHERE id = ? AND account_id = ? AND bucket = ? AND key = ?`, uploadID, ref.AccountID, ref.Bucket, ref.Key).Scan(&contentType, &metaJSON)
	if errors.Is(err, sql.ErrNoRows) {
		return ObjectInfo{}, ErrNoSuchUpload
	}
	if err != nil {
		return ObjectInfo{}, mapErr(err)
	}

	sums := md5.New()
	last := 0
	var readers []io.Reader
	for _, want := range parts {
		if want.PartNumber <= last {
			return ObjectInfo{}, fmt.Errorf("%w: parts must be in ascending order", ErrInvalidRange)
		}
		last = want.PartNumber
		var etag string
		var size int64
		err := s.db.QueryRowContext(ctx, `SELECT etag, size FROM multipart_parts WHERE upload_id = ? AND part_number = ? AND etag != ''`,
			uploadID, want.PartNumber).Scan(&etag, &size)
		if errors.Is(err, sql.ErrNoRows) {
			return ObjectInfo{}, fmt.Errorf("%w: part %d was not uploaded", ErrInvalidRange, want.PartNumber)
		}
		if err != nil {
			return ObjectInfo{}, mapErr(err)
		}
		if strings.Trim(want.ETag, `"`) != etag {
			return ObjectInfo{}, fmt.Errorf("%w: etag of part %d does not match", ErrInvalidRange, want.PartNumber)
		}
		raw, _ := hex.DecodeString(etag)
		sums.Write(raw)
		readers = append(readers, &partReader{s: s, ctx: ctx, upload: uploadID, part: want.PartNumber, size: size})
	}
	etag := fmt.Sprintf("%s-%d", hex.EncodeToString(sums.Sum(nil)), len(parts))

	opts := PutOptions{ContentType: contentType}
	if metaJSON != "" && metaJSON != "{}" {
		if err := jsonUnmarshal(metaJSON, &opts.Metadata); err != nil {
			return ObjectInfo{}, fmt.Errorf("%w: upload metadata", ErrCorruptData)
		}
	}
	info, err := s.put(ctx, ref, io.MultiReader(readers...), opts, etag)
	if err != nil {
		return ObjectInfo{}, err
	}
	_, _ = s.db.ExecContext(ctx, `DELETE FROM multipart_uploads WHERE id = ?`, uploadID)
	return info, nil
}

// partReader streams one stored part chunk by chunk.
type partReader struct {
	s      *SQLiteStore
	ctx    context.Context
	upload string
	part   int
	seq    int64
	size   int64 // recorded size; a shorter or longer stream means damaged chunks
	read   int64
	cur    []byte
	done   bool
}

func (r *partReader) Read(p []byte) (int, error) {
	for len(r.cur) == 0 {
		if r.done {
			return 0, io.EOF
		}
		if err := r.ctx.Err(); err != nil {
			return 0, err
		}
		var data []byte
		err := r.s.db.QueryRowContext(r.ctx, `SELECT data FROM multipart_chunks WHERE upload_id = ? AND part_number = ? AND seq = ?`,
			r.upload, r.part, r.seq).Scan(&data)
		if errors.Is(err, sql.ErrNoRows) {
			r.done = true
			if r.read != r.size {
				return 0, fmt.Errorf("%w: part %d of upload %s has %d bytes, expected %d", ErrCorruptData, r.part, r.upload, r.read, r.size)
			}
			return 0, io.EOF
		}
		if err != nil {
			return 0, mapErr(err)
		}
		r.cur, r.seq = data, r.seq+1
	}
	n := copy(p, r.cur)
	r.cur = r.cur[n:]
	r.read += int64(n)
	return n, nil
}

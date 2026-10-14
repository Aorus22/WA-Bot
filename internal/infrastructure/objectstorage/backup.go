package objectstorage

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
)

// BackupResult describes a finished backup.
type BackupResult struct {
	Path       string
	Bytes      int64
	Objects    int
	VerifiedOK int
}

// Backup writes a consistent single-file snapshot of the live database to dest
// (VACUUM INTO, safe while the store is in use; never a raw copy of a file with
// an active WAL). The snapshot is checked with integrity_check and a sample of
// its objects is re-read and checksummed before it replaces anything; only then
// is it moved into place. The size of dest is the disk space the backup needs.
func (s *SQLiteStore) Backup(ctx context.Context, dest string) (BackupResult, error) {
	var res BackupResult
	dest, err := filepath.Abs(dest)
	if err != nil {
		return res, err
	}
	if dest == s.path {
		return res, fmt.Errorf("%w: backup target is the live database", ErrConflict)
	}
	if _, err := os.Stat(dest); err == nil {
		return res, fmt.Errorf("%w: %s already exists", ErrConflict, dest)
	}
	if err := os.MkdirAll(filepath.Dir(dest), 0o700); err != nil {
		return res, err
	}
	tmp := dest + ".partial"
	_ = os.Remove(tmp)
	defer os.Remove(tmp)

	if _, err := s.db.ExecContext(ctx, `VACUUM INTO ?`, tmp); err != nil {
		return res, fmt.Errorf("objectstorage: backup: %w", mapErr(err))
	}
	if err := os.Chmod(tmp, 0o600); err != nil {
		return res, err
	}
	res.Objects, res.VerifiedOK, err = checkSnapshot(ctx, tmp, 20)
	if err != nil {
		return res, fmt.Errorf("objectstorage: backup failed verification: %w", err)
	}
	if err := os.Rename(tmp, dest); err != nil {
		return res, err
	}
	st, err := os.Stat(dest)
	if err != nil {
		return res, err
	}
	res.Path, res.Bytes = dest, st.Size()
	return res, nil
}

// checkSnapshot opens the database at path on its own, runs integrity_check and
// re-hashes up to sample objects.
func checkSnapshot(ctx context.Context, path string, sample int) (objects, verified int, err error) {
	return checkDatabase(ctx, path, sample)
}

func checkDatabase(ctx context.Context, path string, sample int) (objects, verified int, err error) {
	store, err := OpenSQLite(SQLiteOptions{Path: path, SkipRecovery: true})
	if err != nil {
		return 0, 0, err
	}
	defer store.Close()

	var verdict string
	if err := store.db.QueryRowContext(ctx, `PRAGMA integrity_check`).Scan(&verdict); err != nil {
		return 0, 0, mapErr(err)
	}
	if verdict != "ok" {
		return 0, 0, fmt.Errorf("%w: integrity_check: %s", ErrCorruptData, verdict)
	}
	if err := store.db.QueryRowContext(ctx, `SELECT COUNT(*) FROM objects`).Scan(&objects); err != nil {
		return 0, 0, mapErr(err)
	}
	rows, err := store.db.QueryContext(ctx, `SELECT account_id, bucket, key FROM objects ORDER BY RANDOM() LIMIT ?`, sample)
	if err != nil {
		return objects, 0, mapErr(err)
	}
	var refs []ObjectRef
	for rows.Next() {
		var r ObjectRef
		if err := rows.Scan(&r.AccountID, &r.Bucket, &r.Key); err != nil {
			rows.Close()
			return objects, 0, mapErr(err)
		}
		refs = append(refs, r)
	}
	rows.Close()
	for _, r := range refs {
		if err := store.Verify(ctx, r); err != nil {
			return objects, verified, err
		}
		verified++
	}
	return objects, verified, nil
}

// Check verifies the database file at path without modifying it: integrity and
// a sample of object checksums.
func Check(ctx context.Context, path string, sample int) (objects, verified int, err error) {
	return checkDatabase(ctx, path, sample)
}

// RestoreBackup replaces the database at dest with the verified snapshot src.
// The store at dest must not be open: stop WA-Bot first. Stale -wal/-shm files
// of dest are removed, and the old database is kept as dest+".before-restore"
// until the new one is in place.
func RestoreBackup(ctx context.Context, src, dest string) error {
	src, err := filepath.Abs(src)
	if err != nil {
		return err
	}
	dest, err = filepath.Abs(dest)
	if err != nil {
		return err
	}
	if src == dest {
		return fmt.Errorf("%w: restore source is the destination", ErrConflict)
	}
	// Work on a copy so checking the snapshot never writes next to the original.
	if err := os.MkdirAll(filepath.Dir(dest), 0o700); err != nil {
		return err
	}
	staged := dest + ".restoring"
	_ = os.Remove(staged)
	defer os.Remove(staged)
	if err := copyFile(src, staged); err != nil {
		return err
	}
	if _, _, err := checkSnapshot(ctx, staged, 20); err != nil {
		return fmt.Errorf("objectstorage: snapshot %s is not usable: %w", src, err)
	}
	// checkSnapshot opened the copy in WAL mode: it left no -wal behind after a
	// clean close, but be explicit about the destination's.
	backup := dest + ".before-restore"
	if _, err := os.Stat(dest); err == nil {
		_ = os.Remove(backup)
		if err := os.Rename(dest, backup); err != nil {
			return err
		}
	}
	for _, suffix := range []string{"-wal", "-shm"} {
		_ = os.Remove(dest + suffix)
	}
	if err := os.Rename(staged, dest); err != nil {
		_ = os.Rename(backup, dest)
		return err
	}
	return nil
}

func copyFile(src, dst string) error {
	in, err := os.Open(src)
	if err != nil {
		return err
	}
	defer in.Close()
	out, err := os.OpenFile(dst, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, 0o600)
	if err != nil {
		return err
	}
	if _, err := out.ReadFrom(in); err != nil {
		out.Close()
		return err
	}
	return out.Close()
}

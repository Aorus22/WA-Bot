package objectstorage

import (
	"bytes"
	"context"
	"os"
	"path/filepath"
	"testing"
)

func TestBackupRestoreRoundTrip(t *testing.T) {
	dir := t.TempDir()
	livePath := filepath.Join(dir, "live.sqlite")
	s, err := OpenSQLite(SQLiteOptions{Path: livePath, ChunkSize: 256})
	if err != nil {
		t.Fatal(err)
	}
	ctx := context.Background()
	for i, name := range []string{"a", "b", "c"} {
		s.Put(ctx, ref("628111", name), bytes.NewReader(randomBytes(2000+i, int64(i))), PutOptions{})
	}
	// The database is open (WAL active) while it is backed up.
	out := filepath.Join(dir, "backups", "snap.sqlite")
	res, err := s.Backup(ctx, out)
	if err != nil {
		t.Fatal(err)
	}
	if res.Objects != 3 || res.VerifiedOK != 3 || res.Bytes == 0 {
		t.Fatalf("result: %+v", res)
	}
	if _, err := os.Stat(out + "-wal"); err == nil {
		t.Fatal("backup is not a single file")
	}
	if _, err := s.Backup(ctx, out); err == nil {
		t.Fatal("overwrote an existing backup")
	}
	s.Put(ctx, ref("628111", "d"), bytes.NewReader([]byte("after backup")), PutOptions{})
	s.Close()

	// Restore into a "new machine".
	fresh := filepath.Join(t.TempDir(), "restored.sqlite")
	if err := RestoreBackup(ctx, out, fresh); err != nil {
		t.Fatal(err)
	}
	r, err := OpenSQLite(SQLiteOptions{Path: fresh})
	if err != nil {
		t.Fatal(err)
	}
	defer r.Close()
	if got := readAll(t, r, ref("628111", "b"), GetOptions{}); !bytes.Equal(got, randomBytes(2001, 1)) {
		t.Fatal("restored content differs")
	}
	if ok, _ := r.Exists(ctx, ref("628111", "d")); ok {
		t.Fatal("restore contains data from after the snapshot")
	}

	// A damaged snapshot is refused and nothing is replaced.
	bad := filepath.Join(dir, "bad.sqlite")
	os.WriteFile(bad, bytes.Repeat([]byte("not a database"), 100), 0o600)
	if err := RestoreBackup(ctx, bad, fresh); err == nil {
		t.Fatal("restored a corrupt file")
	}
	if got := readAll(t, r, ref("628111", "a"), GetOptions{}); len(got) != 2000 {
		t.Fatal("failed restore damaged the destination")
	}
}

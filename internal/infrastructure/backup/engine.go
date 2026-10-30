package backup

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"time"

	"wa-bot/internal/infrastructure/objectstorage"
)

// Source is what one account's backup is made of. Empty paths and a nil Media
// leave that part out.
type Source struct {
	Account    string
	MessagesDB string
	AppDB      string
	Media      *objectstorage.SQLiteStore
}

// File names databases are restored under.
const (
	MessagesFile = "wa-bot-messages.db"
	AppFile      = "wa-bot-app.db"
	MediaFile    = "objectstorage.sqlite"
)

// Progress reports how far a backup or restore is.
type Progress struct {
	Stage string // "snapshot", "chat", "media", "manifest"
	Item  string
	Done  int64
	Total int64
}

// Options tune Backup and Restore.
type Options struct {
	// WorkDir holds temporary snapshots; it should be on a disk with room for
	// a copy of the chat databases. Defaults to the system temp folder.
	WorkDir    string
	AppVersion string
	Progress   func(Progress)
	Now        func() time.Time
}

func (o *Options) defaults() {
	if o.Progress == nil {
		o.Progress = func(Progress) {}
	}
	if o.Now == nil {
		o.Now = time.Now
	}
	if o.WorkDir == "" {
		o.WorkDir = os.TempDir()
	}
}

// BackupReport says what a backup did.
type BackupReport struct {
	ID             string
	ManifestSHA256 string
	Uploaded       int
	UploadedBytes  int64
	Reused         int
	Rows           int64
	MediaObjects   int
	MediaBytes     int64
	SkippedMedia   []string
	Warnings       []string
}

// Backup writes one complete backup of src to t. Databases are snapshotted and
// checked first: if any is damaged, nothing is written and the error says so.
// The manifest goes last, so an interrupted backup leaves no backup at all,
// only chunks the next prune removes.
func Backup(ctx context.Context, t Target, src Source, opt Options) (*BackupReport, error) {
	opt.defaults()
	if err := validName(manifestPrefix + src.Account); err != nil || src.Account == "" {
		return nil, fmt.Errorf("backup: invalid account %q", src.Account)
	}
	now := opt.Now().UTC()
	m := &Manifest{Format: FormatVersion, ID: now.Format(idLayout), Account: src.Account, CreatedAt: now, AppVersion: opt.AppVersion}
	rep := &BackupReport{ID: m.ID}

	work, err := os.MkdirTemp(opt.WorkDir, "wa-bot-backup-")
	if err != nil {
		return nil, err
	}
	defer os.RemoveAll(work)

	// Snapshot and check every database before anything is uploaded.
	type snap struct{ name, file, path string }
	var snaps []snap
	for _, d := range []struct{ name, file, live string }{
		{"messages", MessagesFile, src.MessagesDB},
		{"app", AppFile, src.AppDB},
	} {
		if d.live == "" {
			continue
		}
		opt.Progress(Progress{Stage: "snapshot", Item: d.name})
		p := filepath.Join(work, d.file)
		if err := snapshotDatabase(ctx, d.live, p); err != nil {
			return nil, fmt.Errorf("backup: %w", err)
		}
		snaps = append(snaps, snap{d.name, d.file, p})
	}

	chunks, err := newChunkStore(ctx, t)
	if err != nil {
		return nil, err
	}
	for _, s := range snaps {
		d, warnings, err := dumpDatabase(ctx, s.path, s.name, s.file, chunks, opt.Progress)
		if err != nil {
			return nil, fmt.Errorf("backup: %w", err)
		}
		m.Databases = append(m.Databases, d)
		rep.Warnings = append(rep.Warnings, warnings...)
		for _, tb := range d.Tables {
			rep.Rows += tb.Rows
		}
		_ = os.Remove(s.path)
	}
	if src.Media != nil {
		prev := previousParts(ctx, t, src.Account)
		media, err := backupMedia(ctx, src.Media, MediaFile, prev, chunks, opt.Progress)
		if err != nil {
			return nil, fmt.Errorf("backup: %w", err)
		}
		m.Media = media
		rep.MediaObjects, rep.MediaBytes, rep.SkippedMedia = len(media.Objects), media.Bytes, media.Skipped
	}

	opt.Progress(Progress{Stage: "manifest"})
	sum, err := writeManifest(ctx, t, m)
	if err != nil {
		return nil, err
	}
	rep.ManifestSHA256 = sum
	rep.Uploaded, rep.UploadedBytes, rep.Reused = chunks.uploaded, chunks.uploadedBytes, chunks.reused
	return rep, nil
}

// previousParts maps media checksums to the parts the newest readable backup
// stored them in, so unchanged multi-part objects are not read again.
func previousParts(ctx context.Context, t Target, account string) map[string][]ChunkRef {
	out := map[string][]ChunkRef{}
	list, err := ListBackups(ctx, t, account)
	if err != nil {
		return out
	}
	for _, mi := range list {
		m, err := LoadManifest(ctx, t, mi)
		if err != nil {
			continue
		}
		if m.Media != nil {
			for _, o := range m.Media.Objects {
				out[o.SHA256] = o.Parts
			}
		}
		break
	}
	return out
}

// RestoreReport says what a restore rebuilt and what it could not.
type RestoreReport struct {
	ID       string
	Account  string
	OutDir   string
	Files    []string
	Rows     int64
	Media    int
	Lost     []LostItem
	Mismatch []string
	Warnings []string
}

// Complete is true when everything in the backup came back intact.
func (r *RestoreReport) Complete() bool { return len(r.Lost) == 0 && len(r.Mismatch) == 0 }

// Restore rebuilds the databases of one backup into outDir (which must not
// hold them yet). id selects the backup; empty means the newest one whose
// manifest can be read. Damaged parts are skipped and listed in the report;
// the error is reserved for failures that leave nothing usable.
func Restore(ctx context.Context, t Target, account, id, outDir string, opt Options) (*RestoreReport, error) {
	opt.defaults()
	rep := &RestoreReport{Account: account, OutDir: outDir}
	m, err := pickManifest(ctx, t, account, id, &rep.Warnings)
	if err != nil {
		return rep, err
	}
	rep.ID = m.ID

	if err := os.MkdirAll(outDir, 0o700); err != nil {
		return rep, err
	}
	chunks := &chunkStore{t: t}
	for _, d := range m.Databases {
		dest := filepath.Join(outDir, filepath.Base(d.File))
		if err := restoreDatabase(ctx, d, dest, chunks, rep, opt.Progress); err != nil {
			return rep, fmt.Errorf("backup: restore %s: %w", d.Name, err)
		}
		rep.Files = append(rep.Files, dest)
	}
	if m.Media != nil {
		dest := filepath.Join(outDir, filepath.Base(m.Media.File))
		if err := restoreMedia(ctx, m.Media, dest, chunks, rep, opt.Progress); err != nil {
			return rep, fmt.Errorf("backup: restore media: %w", err)
		}
		rep.Files = append(rep.Files, dest)
	}
	return rep, nil
}

// pickManifest loads the manifest of backup id of account (empty id: the newest
// one that can be read). Manifests that fail to load add a line to warnings.
func pickManifest(ctx context.Context, t Target, account, id string, warnings *[]string) (*Manifest, error) {
	list, err := ListBackups(ctx, t, account)
	if err != nil {
		return nil, err
	}
	for _, mi := range list {
		if mi.Account != account || (id != "" && mi.ID != id) {
			continue
		}
		got, err := LoadManifest(ctx, t, mi)
		if err != nil {
			*warnings = append(*warnings, err.Error())
			continue
		}
		return got, nil
	}
	if id != "" {
		return nil, fmt.Errorf("backup: no readable backup %s for %s", id, account)
	}
	return nil, fmt.Errorf("backup: no readable backup for %s", account)
}

// CheckedBackup is the state of one backup in a target.
type CheckedBackup struct {
	Account string
	ID      string
	// Damaged is set when no copy of the manifest could be read.
	Damaged string
	Chunks  int
	Missing []string // chunks absent or of the wrong size
	Corrupt []string // chunks whose content fails the checksum (deep only)
}

// OK is true when the backup can be restored completely.
func (c CheckedBackup) OK() bool {
	return c.Damaged == "" && len(c.Missing) == 0 && len(c.Corrupt) == 0
}

// Check verifies every backup in the target (of account, when not empty):
// manifests readable, every chunk present with the right size and, when deep,
// every chunk downloaded and checksummed.
func Check(ctx context.Context, t Target, account string, deep bool) ([]CheckedBackup, error) {
	list, err := ListBackups(ctx, t, account)
	if err != nil {
		return nil, err
	}
	entries, err := t.List(ctx, chunkPrefix)
	if err != nil {
		return nil, err
	}
	present := make(map[string]int64, len(entries))
	for _, e := range entries {
		present[e.Name] = e.Size
	}
	verdict := map[string]error{} // deep results, shared by backups
	var out []CheckedBackup
	for _, mi := range list {
		cb := CheckedBackup{Account: mi.Account, ID: mi.ID}
		m, err := LoadManifest(ctx, t, mi)
		if err != nil {
			cb.Damaged = err.Error()
			out = append(out, cb)
			continue
		}
		seen := map[string]bool{}
		for _, c := range m.chunks() {
			if seen[c.SHA256] {
				continue
			}
			seen[c.SHA256] = true
			cb.Chunks++
			if size, ok := present[chunkName(c.SHA256)]; !ok || size != c.Size {
				cb.Missing = append(cb.Missing, c.SHA256)
				continue
			}
			if !deep {
				continue
			}
			err, done := verdict[c.SHA256]
			if !done {
				_, err = fetchChunk(ctx, t, c)
				if ctx.Err() != nil {
					return nil, ctx.Err()
				}
				verdict[c.SHA256] = err
			}
			if err != nil {
				cb.Corrupt = append(cb.Corrupt, c.SHA256)
			}
		}
		out = append(out, cb)
	}
	return out, nil
}

// PruneReport says what Prune removed.
type PruneReport struct {
	Kept           int
	DeletedBackups []string
	DeletedChunks  int
	FreedBytes     int64
	SkippedChunkGC string
}

// Prune keeps the newest keep readable backups of every account, deletes the
// older ones and then every chunk no kept backup refers to. Chunks younger
// than grace are left alone, since a backup running right now may have
// uploaded them without having written its manifest yet.
func Prune(ctx context.Context, t Target, keep int, grace time.Duration, now time.Time) (*PruneReport, error) {
	if keep < 1 {
		return nil, errors.New("backup: prune must keep at least one backup")
	}
	list, err := ListBackups(ctx, t, "")
	if err != nil {
		return nil, err
	}
	rep := &PruneReport{}
	referenced := map[string]bool{}
	kept := map[string]int{}
	for _, mi := range list { // newest first
		if kept[mi.Account] >= keep {
			for _, name := range mi.names {
				if err := t.Delete(ctx, name); err != nil {
					return rep, err
				}
			}
			rep.DeletedBackups = append(rep.DeletedBackups, mi.Account+"/"+mi.ID)
			continue
		}
		m, err := LoadManifest(ctx, t, mi)
		if err != nil {
			// Unreadable (damaged, or the target failed to answer): it does not
			// count as kept and stays in place, and since its chunks are
			// unknown no chunk is deleted this time.
			rep.SkippedChunkGC = err.Error()
			continue
		}
		kept[mi.Account]++
		rep.Kept++
		for _, c := range m.chunks() {
			referenced[chunkName(c.SHA256)] = true
		}
	}
	if rep.Kept == 0 && rep.SkippedChunkGC == "" {
		rep.SkippedChunkGC = "no readable backup left"
	}
	if rep.SkippedChunkGC != "" {
		return rep, nil
	}
	entries, err := t.List(ctx, chunkPrefix)
	if err != nil {
		return rep, err
	}
	sort.Slice(entries, func(i, j int) bool { return entries[i].Name < entries[j].Name })
	for _, e := range entries {
		if referenced[e.Name] || now.Sub(e.ModTime) < grace {
			continue
		}
		if err := t.Delete(ctx, e.Name); err != nil {
			return rep, err
		}
		rep.DeletedChunks++
		rep.FreedBytes += e.Size
	}
	return rep, nil
}

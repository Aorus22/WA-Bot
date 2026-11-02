package main

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"wa-bot/internal/delivery/http/handlers"
	"wa-bot/internal/infrastructure/backup"
	"wa-bot/internal/infrastructure/objectstorage"
)

// backupService runs one backup at a time into a local folder, for the app's
// Backup settings. The CLI (`wa-bot-backend backup`) does the same on demand.
type backupService struct {
	mu     sync.Mutex
	status handlers.BackupStatus
	// media and hub are what a restore merges into and announces to.
	media objectstorage.Store
	hub   interface {
		BroadcastMessage(msgType string, payload interface{})
	}
	restoreStatus handlers.BackupStatus
}

func newBackupService(media objectstorage.Store, hub interface {
	BroadcastMessage(msgType string, payload interface{})
}) *backupService {
	return &backupService{media: media, hub: hub}
}

func (b *backupService) Status() handlers.BackupStatus {
	b.mu.Lock()
	defer b.mu.Unlock()
	return b.status
}

func (b *backupService) RestoreStatus() handlers.BackupStatus {
	b.mu.Lock()
	defer b.mu.Unlock()
	return b.restoreStatus
}

// StartRestore merges one backup into the account's live data in the
// background. Nothing live is replaced, see backup.Merge.
func (b *backupService) StartRestore(target, account, id string) error {
	if target == "" || !filepath.IsAbs(target) {
		return fmt.Errorf("choose a backup folder (absolute path)")
	}
	if account == "" || id == "" {
		return fmt.Errorf("choose a backup to restore")
	}
	db := filepath.Join(accountsRoot, account, "database", backup.MessagesFile)
	if _, err := os.Stat(db); err != nil {
		return fmt.Errorf("account %s is not linked on this device; log in with it first", account)
	}
	b.mu.Lock()
	if b.restoreStatus.Running || b.status.Running {
		b.mu.Unlock()
		return fmt.Errorf("a backup or restore is already running")
	}
	b.restoreStatus = handlers.BackupStatus{Running: true, Account: account, StartedAt: time.Now().UnixMilli()}
	b.mu.Unlock()

	go func() {
		rep, err := backup.Merge(context.Background(), backup.LocalFolder{Root: target}, account, id,
			backup.MergeTarget{MessagesDB: db, Media: mergeMedia(b.media)},
			backup.Options{WorkDir: "tmp", Progress: func(p backup.Progress) {
				b.mu.Lock()
				defer b.mu.Unlock()
				b.restoreStatus.Stage, b.restoreStatus.Item = p.Stage, p.Item
				b.restoreStatus.Done, b.restoreStatus.Total = p.Done, p.Total
				b.restoreStatus.Percent = restorePercent(p.Stage, p.Done, p.Total)
			}})
		b.mu.Lock()
		b.restoreStatus.Running = false
		b.restoreStatus.FinishedAt = time.Now().UnixMilli()
		if err != nil {
			b.restoreStatus.Error = err.Error()
		} else {
			b.restoreStatus.Percent = 100
			b.restoreStatus.Summary = mergeSummary(rep)
		}
		b.mu.Unlock()
		if err == nil && b.hub != nil {
			b.hub.BroadcastMessage("chats_changed", map[string]interface{}{"reason": "restored"})
		}
	}()
	return nil
}

// restorePercent folds a merge stage into 0..99: reading the backup into
// staging, adding the rows, then the media objects.
func restorePercent(stage string, done, total int64) int {
	switch stage {
	case "chat":
		return 10
	case "merge":
		return 40
	case "media":
		if total > 0 {
			return 40 + int(float64(done)/float64(total)*59)
		}
		return 40
	}
	return 0
}

func mergeSummary(r *backup.MergeReport) string {
	var rows, present int64
	for _, n := range r.Added {
		rows += n
	}
	for _, n := range r.Present {
		present += n
	}
	out := fmt.Sprintf("%d messages, chats and contacts added (%d were already here)\n%d media added (%d were already here)",
		rows, present, r.MediaAdded, r.MediaPresent)
	if len(r.Lost) > 0 {
		out += fmt.Sprintf("\n%d parts of the backup could not be read and were skipped", len(r.Lost))
	}
	return out
}

// mergeMedia is nil-safe: an unset store leaves media out of the merge.
func mergeMedia(s objectstorage.Store) backup.MergeMedia {
	if s == nil {
		return nil
	}
	return s
}

func (b *backupService) Start(target string, keep int) error {
	if target == "" || !filepath.IsAbs(target) {
		return fmt.Errorf("choose a backup folder (absolute path)")
	}
	b.mu.Lock()
	if b.status.Running || b.restoreStatus.Running {
		b.mu.Unlock()
		return fmt.Errorf("a backup or restore is already running")
	}
	b.status = handlers.BackupStatus{Running: true, StartedAt: time.Now().UnixMilli()}
	b.mu.Unlock()

	go func() {
		summary, err := b.run(context.Background(), backup.LocalFolder{Root: target}, keep)
		b.mu.Lock()
		defer b.mu.Unlock()
		b.status.Running = false
		b.status.FinishedAt = time.Now().UnixMilli()
		if err == nil {
			b.status.Percent = 100
		}
		b.status.Summary = summary
		if err != nil {
			b.status.Error = err.Error()
		}
	}()
	return nil
}

// stageSpan is where a stage sits in one account's backup: it starts at start
// and takes up share of the whole.
var stageSpan = map[string]struct{ start, share float64 }{
	"snapshot": {0, 0.05},
	"chat":     {0.05, 0.40},
	"media":    {0.45, 0.50},
	"manifest": {0.95, 0.05},
}

// overallPercent folds the account, stage and its done/total into 0..100.
func overallPercent(index, count int, stage string, done, total int64) int {
	if count < 1 {
		return 0
	}
	within := 0.0
	if span, ok := stageSpan[stage]; ok {
		within = span.start
		if total > 0 {
			within += span.share * float64(done) / float64(total)
		}
	}
	pct := (float64(index-1) + within) / float64(count) * 100
	if pct > 99 {
		pct = 99 // 100 is the finished state
	}
	return int(pct)
}

func (b *backupService) progress(account string, index, count int) func(backup.Progress) {
	return func(p backup.Progress) {
		b.mu.Lock()
		b.status.Account, b.status.Stage, b.status.Item = account, p.Stage, p.Item
		b.status.Done, b.status.Total = p.Done, p.Total
		b.status.AccountIndex, b.status.AccountCount = index, count
		b.status.Percent = overallPercent(index, count, p.Stage, p.Done, p.Total)
		b.mu.Unlock()
	}
}

func (b *backupService) run(ctx context.Context, target backup.LocalFolder, keep int) (string, error) {
	accounts, err := backupAccounts()
	if err != nil {
		return "", err
	}
	if len(accounts) == 0 {
		return "", fmt.Errorf("no account to back up")
	}
	cfg := objectStorageConfigFromEnv()
	pa, err := objectstorage.OpenPerAccount(cfg.SQLitePath, cfg.PendingSQLitePath, objectstorage.SQLiteOptions{SkipRecovery: true})
	if err != nil {
		return "", err
	}
	defer pa.Close()

	var done []string
	var failed []string
	for i, acct := range accounts {
		media, err := pa.Existing(acct)
		if err != nil {
			return "", err
		}
		rep, err := backup.Backup(ctx, target, backup.Source{
			Account:    acct,
			MessagesDB: filepath.Join(accountsRoot, acct, "database", backup.MessagesFile),
			AppDB:      filepath.Join("database", backup.AppFile),
			Media:      media,
		}, backup.Options{WorkDir: "tmp", Progress: b.progress(acct, i+1, len(accounts))})
		if err != nil {
			failed = append(failed, fmt.Sprintf("%s: %v", acct, err))
			continue
		}
		done = append(done, fmt.Sprintf("%s: %d messages rows, %d media (%.1f MiB)", acct, rep.Rows, rep.MediaObjects, mib(rep.MediaBytes)))
	}
	if len(failed) > 0 {
		return strings.Join(done, "\n"), fmt.Errorf("%s", strings.Join(failed, "; "))
	}
	if keep > 0 {
		if _, err := backup.Prune(ctx, target, keep, pruneGrace, time.Now()); err != nil {
			return strings.Join(done, "\n"), fmt.Errorf("backup done, cleanup failed: %w", err)
		}
	}
	return strings.Join(done, "\n"), nil
}

func (b *backupService) List(ctx context.Context, target string) ([]handlers.BackupEntry, error) {
	if target == "" || !filepath.IsAbs(target) {
		return nil, fmt.Errorf("choose a backup folder (absolute path)")
	}
	list, err := backup.ListBackups(ctx, backup.LocalFolder{Root: target}, "")
	if err != nil {
		return nil, err
	}
	out := make([]handlers.BackupEntry, 0, len(list))
	for _, mi := range list {
		out = append(out, handlers.BackupEntry{Account: mi.Account, ID: mi.ID, CreatedAt: mi.CreatedAt().UnixMilli()})
	}
	return out, nil
}

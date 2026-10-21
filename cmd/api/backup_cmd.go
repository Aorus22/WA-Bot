package main

import (
	"context"
	"flag"
	"fmt"
	"os"
	"os/signal"
	"path/filepath"
	"time"

	"github.com/joho/godotenv"

	"wa-bot/internal/infrastructure/backup"
	"wa-bot/internal/infrastructure/objectstorage"
)

const backupUsage = `usage: wa-bot-backend backup <command> [flags]

Chats and media of every account, as independent checksummed chunks plus one
manifest per backup. The WhatsApp session is never included. Safe while WA-Bot
runs.

commands:
  run       back up (--account PHONE for one account; --keep N prunes afterwards)
  list      list the backups in the target
  restore   rebuild one backup into a new folder (the live data is not touched):
              --account PHONE --output DIR [--id ID]
  check     verify every backup: manifests readable, chunks present
              (--deep downloads and checksums every chunk)
  prune     keep the newest --keep N backups per account, delete the rest and
            chunks nothing refers to

common flags:
  --target DIR   the backup folder
`

// pruneGrace keeps chunks a backup that is still running may need.
const pruneGrace = 24 * time.Hour

// runBackupCommand handles `wa-bot-backend backup ...` and returns the exit code.
func runBackupCommand(args []string) int {
	_ = godotenv.Load()
	if len(args) == 0 {
		fmt.Fprint(os.Stderr, backupUsage)
		return 2
	}
	fs := flag.NewFlagSet("backup "+args[0], flag.ContinueOnError)
	targetDir := fs.String("target", "", "backup folder")
	account := fs.String("account", "", "limit to one account")
	output := fs.String("output", "", "restore destination folder")
	id := fs.String("id", "", "backup to restore (default: newest readable)")
	keep := fs.Int("keep", 0, "backups to keep per account")
	deep := fs.Bool("deep", false, "download and checksum every chunk")
	if err := fs.Parse(args[1:]); err != nil {
		return 2
	}
	if *targetDir == "" {
		fmt.Fprintln(os.Stderr, "--target is required")
		return 2
	}
	target := backup.LocalFolder{Root: *targetDir}

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt)
	defer stop()

	switch args[0] {
	case "run":
		list := []string{*account}
		if *account == "" {
			var err error
			if list, err = backupAccounts(); err != nil {
				return fail(err)
			}
		}
		if len(list) == 0 {
			fmt.Println("no account to back up")
			return 0
		}
		cfg := objectStorageConfigFromEnv()
		pa, err := objectstorage.OpenPerAccount(cfg.SQLitePath, cfg.PendingSQLitePath, objectstorage.SQLiteOptions{SkipRecovery: true})
		if err != nil {
			return fail(err)
		}
		defer pa.Close()
		failed := 0
		for _, acct := range list {
			media, err := pa.Existing(acct)
			if err != nil {
				return fail(err)
			}
			src := backup.Source{
				Account:    acct,
				MessagesDB: filepath.Join(accountsRoot, acct, "database", backup.MessagesFile),
				AppDB:      filepath.Join("database", backup.AppFile),
				Media:      media,
			}
			rep, err := backup.Backup(ctx, target, src, backup.Options{WorkDir: "tmp", Progress: printProgress})
			fmt.Println()
			if err != nil {
				fmt.Printf("FAILED %s: %v\n", acct, err)
				failed++
				continue
			}
			fmt.Printf("backup %s/%s: %d rows, %d media (%.1f MiB); uploaded %d chunks (%.1f MiB), reused %d\n",
				acct, rep.ID, rep.Rows, rep.MediaObjects, mib(rep.MediaBytes), rep.Uploaded, mib(rep.UploadedBytes), rep.Reused)
			for _, s := range rep.SkippedMedia {
				fmt.Println("  SKIPPED damaged media", s)
			}
			for _, w := range rep.Warnings {
				fmt.Println("  warning:", w)
			}
		}
		if failed > 0 {
			return 1
		}
		if *keep > 0 {
			return printPrune(backup.Prune(ctx, target, *keep, pruneGrace, time.Now()))
		}
		return 0

	case "list":
		list, err := backup.ListBackups(ctx, target, *account)
		if err != nil {
			return fail(err)
		}
		for _, mi := range list {
			fmt.Printf("%s  %s  %s\n", mi.Account, mi.ID, mi.CreatedAt().Local().Format("2006-01-02 15:04"))
		}
		return 0

	case "restore":
		if *account == "" || *output == "" {
			fmt.Fprintln(os.Stderr, "restore needs --account and --output")
			return 2
		}
		rep, err := backup.Restore(ctx, target, *account, *id, *output, backup.Options{Progress: printProgress})
		fmt.Println()
		for _, w := range rep.Warnings {
			fmt.Println("warning:", w)
		}
		if err != nil {
			return fail(err)
		}
		fmt.Printf("restored %s/%s into %s: %d rows, %d media\n", rep.Account, rep.ID, rep.OutDir, rep.Rows, rep.Media)
		for _, l := range rep.Lost {
			fmt.Printf("  LOST %s %s (%d rows): %s\n", l.What, l.Key, l.Rows, l.Err)
		}
		for _, m := range rep.Mismatch {
			fmt.Println("  MISMATCH", m)
		}
		if !rep.Complete() {
			return 1
		}
		return 0

	case "check":
		res, err := backup.Check(ctx, target, *account, *deep)
		if err != nil {
			return fail(err)
		}
		bad := 0
		for _, c := range res {
			switch {
			case c.Damaged != "":
				fmt.Printf("DAMAGED %s/%s: %s\n", c.Account, c.ID, c.Damaged)
			case !c.OK():
				fmt.Printf("INCOMPLETE %s/%s: %d chunks, %d missing, %d corrupt\n", c.Account, c.ID, c.Chunks, len(c.Missing), len(c.Corrupt))
			default:
				fmt.Printf("ok %s/%s: %d chunks\n", c.Account, c.ID, c.Chunks)
				continue
			}
			bad++
		}
		if bad > 0 {
			return 1
		}
		return 0

	case "prune":
		if *keep < 1 {
			fmt.Fprintln(os.Stderr, "prune needs --keep N (at least 1)")
			return 2
		}
		return printPrune(backup.Prune(ctx, target, *keep, pruneGrace, time.Now()))

	default:
		fmt.Fprint(os.Stderr, backupUsage)
		return 2
	}
}

// backupAccounts lists the accounts that have a chat database.
func backupAccounts() ([]string, error) {
	entries, err := os.ReadDir(accountsRoot)
	if os.IsNotExist(err) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	var out []string
	for _, e := range entries {
		if !e.IsDir() || objectstorage.ValidateAccountID(e.Name()) != nil {
			continue
		}
		if _, err := os.Stat(filepath.Join(accountsRoot, e.Name(), "database", backup.MessagesFile)); err == nil {
			out = append(out, e.Name())
		}
	}
	return out, nil
}

func printPrune(rep *backup.PruneReport, err error) int {
	if err != nil {
		return fail(err)
	}
	fmt.Printf("prune: kept %d backups, deleted %d, removed %d chunks (%.1f MiB)\n", rep.Kept, len(rep.DeletedBackups), rep.DeletedChunks, mib(rep.FreedBytes))
	if rep.SkippedChunkGC != "" {
		fmt.Println("  chunks not cleaned:", rep.SkippedChunkGC)
	}
	return 0
}

func printProgress(p backup.Progress) {
	if p.Total > 0 {
		fmt.Printf("\r%-8s %d/%d          ", p.Stage, p.Done, p.Total)
	} else {
		fmt.Printf("\r%-8s %s          ", p.Stage, p.Item)
	}
}

func mib(b int64) float64 { return float64(b) / (1 << 20) }

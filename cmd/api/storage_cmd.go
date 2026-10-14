package main

import (
	"context"
	"flag"
	"fmt"
	"os"
	"os/signal"
	"path/filepath"
	"strings"

	"github.com/joho/godotenv"

	"wa-bot/internal/infrastructure/objectstorage"
)

const storageUsage = `usage: wa-bot-backend storage <command> [flags]

Media is stored in one SQLite database per account
(accounts/<phone>/database/objectstorage.sqlite) unless OBJECT_STORAGE_SQLITE_PATH
points to a single file.

commands:
  copy      stream every object from one provider to the other (never deletes):
              --from sqlite|s3 --to sqlite|s3
            sqlite uses the database setting; s3 uses the S3_* settings from the
            environment. Switching OBJECT_STORAGE_DRIVER does not move data by
            itself; run this.
  backup    write verified single-file snapshots (safe while WA-Bot runs):
              --output DIR      one DIR/<phone>.sqlite per account
              --account PHONE   only this account (--output may then be a file)
  restore   replace an account's database with a snapshot (stop WA-Bot first;
            the old file is kept as *.before-restore):
              --input FILE --account PHONE
  check     integrity check and checksum sample of every database (--account to
            limit)

common flags:
  --db PATH   override the database path or "{account}" template
`

// runStorageCommand handles `wa-bot-backend storage ...` and returns the exit code.
func runStorageCommand(args []string) int {
	_ = godotenv.Load()
	if len(args) == 0 {
		fmt.Fprint(os.Stderr, storageUsage)
		return 2
	}
	fs := flag.NewFlagSet("storage "+args[0], flag.ContinueOnError)
	cfg := objectStorageConfigFromEnv()
	db := fs.String("db", cfg.SQLitePath, "database path or {account} template")
	from := fs.String("from", "", "source provider (sqlite|s3)")
	to := fs.String("to", "", "destination provider (sqlite|s3)")
	output := fs.String("output", "", "backup destination")
	input := fs.String("input", "", "snapshot to restore")
	account := fs.String("account", "", "limit to one account")
	if err := fs.Parse(args[1:]); err != nil {
		return 2
	}
	cfg.SQLitePath = *db

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt)
	defer stop()

	perAccount := strings.Contains(cfg.SQLitePath, objectstorage.AccountPlaceholder)
	var pa *objectstorage.PerAccountStore
	if perAccount {
		var err error
		pa, err = objectstorage.OpenPerAccount(cfg.SQLitePath, cfg.PendingSQLitePath, objectstorage.SQLiteOptions{SkipRecovery: true})
		if err != nil {
			return fail(err)
		}
		defer pa.Close()
	}
	// accounts lists what a command should act on.
	accounts := func() ([]string, error) {
		if *account != "" {
			return []string{*account}, nil
		}
		if pa == nil {
			return nil, nil
		}
		return pa.ListAccounts(ctx)
	}

	switch args[0] {
	case "copy":
		if *from == "" || *to == "" || *from == *to {
			fmt.Fprintln(os.Stderr, "copy needs --from and --to, one sqlite and one s3")
			return 2
		}
		src, err := openProvider(ctx, *from, cfg)
		if err != nil {
			return fail(err)
		}
		defer src.Close()
		dst, err := openProvider(ctx, *to, cfg)
		if err != nil {
			return fail(err)
		}
		defer dst.Close()
		rep, err := objectstorage.CopyAll(ctx, src, dst, cfg.Bucket, func(ref objectstorage.ObjectRef) { fmt.Println("copied", ref) })
		fmt.Printf("copied %d objects (%.1f MiB), skipped %d identical, failed %d\n", rep.Copied, float64(rep.Bytes)/(1<<20), rep.Skipped, len(rep.Failed))
		for _, f := range rep.Failed {
			fmt.Println("  FAILED", f)
		}
		if err != nil || len(rep.Failed) > 0 {
			if err != nil {
				fmt.Fprintln(os.Stderr, "error:", err)
			}
			return 1
		}
		return 0

	case "backup":
		if *output == "" {
			fmt.Fprintln(os.Stderr, "backup needs --output")
			return 2
		}
		if !perAccount {
			s, err := objectstorage.OpenSQLite(objectstorage.SQLiteOptions{Path: cfg.SQLitePath, SkipRecovery: true})
			if err != nil {
				return fail(err)
			}
			defer s.Close()
			res, err := s.Backup(ctx, *output)
			if err != nil {
				return fail(err)
			}
			fmt.Printf("backup %s: %.1f MiB, %d objects, %d verified by checksum\n", res.Path, float64(res.Bytes)/(1<<20), res.Objects, res.VerifiedOK)
			return 0
		}
		list, err := accounts()
		if err != nil {
			return fail(err)
		}
		if len(list) == 0 {
			fmt.Println("no account has a media database yet")
			return 0
		}
		failed := 0
		for _, acct := range list {
			dest := filepath.Join(*output, acct+".sqlite")
			if *account != "" && filepath.Ext(*output) == ".sqlite" {
				dest = *output
			}
			res, err := pa.Backup(ctx, acct, dest)
			if err != nil {
				fmt.Printf("  FAILED %s: %v\n", acct, err)
				failed++
				continue
			}
			fmt.Printf("backup %s: %.1f MiB, %d objects, %d verified by checksum\n", res.Path, float64(res.Bytes)/(1<<20), res.Objects, res.VerifiedOK)
		}
		if failed > 0 {
			return 1
		}
		return 0

	case "restore":
		if *input == "" || (perAccount && *account == "") {
			fmt.Fprintln(os.Stderr, "restore needs --input FILE (and --account PHONE for per-account storage)")
			return 2
		}
		dest := cfg.SQLitePath
		if perAccount {
			var err error
			if dest, err = pa.PathFor(*account); err != nil {
				return fail(err)
			}
		}
		if err := objectstorage.RestoreBackup(ctx, *input, dest); err != nil {
			return fail(err)
		}
		fmt.Println("restored", dest, "(previous file kept as", dest+".before-restore)")
		return 0

	case "check":
		paths := []string{cfg.SQLitePath}
		if perAccount {
			list, err := accounts()
			if err != nil {
				return fail(err)
			}
			paths = nil
			for _, acct := range list {
				p, err := pa.PathFor(acct)
				if err != nil {
					return fail(err)
				}
				paths = append(paths, p)
			}
		}
		bad := 0
		for _, p := range paths {
			objects, verified, err := objectstorage.Check(ctx, p, 50)
			if err != nil {
				fmt.Printf("  FAILED %s: %v\n", p, err)
				bad++
				continue
			}
			fmt.Printf("ok %s: %d objects, %d sampled and verified\n", p, objects, verified)
		}
		if bad > 0 {
			return 1
		}
		return 0
	}
	fmt.Fprint(os.Stderr, storageUsage)
	return 2
}

func fail(err error) int {
	fmt.Fprintln(os.Stderr, "error:", err)
	return 1
}

// openProvider opens one side of a copy.
func openProvider(ctx context.Context, name string, cfg objectstorage.Config) (objectstorage.Store, error) {
	switch name {
	case "sqlite":
		if strings.Contains(cfg.SQLitePath, objectstorage.AccountPlaceholder) {
			return objectstorage.OpenPerAccount(cfg.SQLitePath, cfg.PendingSQLitePath, objectstorage.SQLiteOptions{SkipRecovery: true})
		}
		return objectstorage.OpenSQLite(objectstorage.SQLiteOptions{Path: cfg.SQLitePath, SkipRecovery: true})
	case "s3":
		cfg.S3.LogicalBucket = cfg.Bucket
		return objectstorage.NewS3Store(ctx, cfg.S3)
	}
	return nil, fmt.Errorf("unknown provider %q (use sqlite or s3)", name)
}

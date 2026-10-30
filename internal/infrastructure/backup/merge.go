package backup

import (
	"context"
	"database/sql"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"wa-bot/internal/infrastructure/objectstorage"
)

// MergeTarget is the live data of one account that a backup is merged into.
type MergeTarget struct {
	// MessagesDB is the account's chat database. It may be in use: rows are
	// added in one transaction and nothing that is already there is touched.
	MessagesDB string
	// Media is the account's media store; nil leaves media out.
	Media MergeMedia
}

// MergeMedia is the media store a merge adds missing objects to.
type MergeMedia interface {
	mediaSink
	Exists(ctx context.Context, ref objectstorage.ObjectRef) (bool, error)
}

// mergeTables are the chat database tables a merge fills, parents first. Every
// one has a primary key, so a row that is already there is recognised by it.
// Sync bookkeeping and stories are left out: they only make sense live.
var mergeTables = []string{"contacts", "chats", "messages", "favorite_stickers", "lid_mapping", "group_cache"}

// MergeReport says what a merge added.
type MergeReport struct {
	ID      string
	Account string
	// Added counts new rows per table.
	Added map[string]int64
	// Present counts rows the live database already had.
	Present      map[string]int64
	MediaAdded   int
	MediaPresent int
	Lost         []LostItem
	Warnings     []string
}

// Merge adds what backup id of account has and the live data lacks. Live data
// always wins: a row or media object that exists is left as it is, so merging
// can neither lose nor duplicate anything. Damaged parts of the backup are
// listed in the report and skipped.
func Merge(ctx context.Context, t Target, account, id string, live MergeTarget, opt Options) (*MergeReport, error) {
	opt.defaults()
	rep := &MergeReport{Account: account, Added: map[string]int64{}, Present: map[string]int64{}}
	m, err := pickManifest(ctx, t, account, id, &rep.Warnings)
	if err != nil {
		return rep, err
	}
	rep.ID = m.ID
	chunks := &chunkStore{t: t}

	for _, d := range m.Databases {
		if filepath.Base(d.File) != MessagesFile {
			continue
		}
		if live.MessagesDB == "" {
			break
		}
		work, err := os.MkdirTemp(opt.WorkDir, "wa-bot-merge-")
		if err != nil {
			return rep, err
		}
		defer os.RemoveAll(work)
		staged := filepath.Join(work, MessagesFile)
		var rr RestoreReport
		if err := restoreDatabase(ctx, d, staged, chunks, &rr, opt.Progress); err != nil {
			return rep, fmt.Errorf("backup: read %s: %w", d.Name, err)
		}
		rep.Lost = append(rep.Lost, rr.Lost...)
		rep.Warnings = append(rep.Warnings, rr.Warnings...)
		opt.Progress(Progress{Stage: "merge", Item: "chats"})
		if err := mergeMessages(ctx, staged, live.MessagesDB, rep); err != nil {
			return rep, fmt.Errorf("backup: merge chats: %w", err)
		}
	}

	if m.Media != nil && live.Media != nil {
		for i, o := range m.Media.Objects {
			opt.Progress(Progress{Stage: "media", Item: o.Key, Done: int64(i), Total: int64(len(m.Media.Objects))})
			ref := objectstorage.ObjectRef{AccountID: o.Account, Bucket: o.Bucket, Key: o.Key}
			if have, err := live.Media.Exists(ctx, ref); err == nil && have {
				rep.MediaPresent++
				continue
			}
			if err := restoreObject(ctx, live.Media, ref, o, chunks); err != nil {
				if ctx.Err() != nil {
					return rep, ctx.Err()
				}
				if d, ok := live.Media.(interface {
					Delete(context.Context, objectstorage.ObjectRef) error
				}); ok {
					_ = d.Delete(ctx, ref)
				}
				rep.Lost = append(rep.Lost, LostItem{What: "media", Key: ref.String(), Err: err.Error()})
				continue
			}
			rep.MediaAdded++
		}
		opt.Progress(Progress{Stage: "media", Done: int64(len(m.Media.Objects)), Total: int64(len(m.Media.Objects))})
	}
	return rep, nil
}

// mergeMessages copies the rows live lacks out of the restored database at
// staged, all or nothing.
func mergeMessages(ctx context.Context, staged, live string, rep *MergeReport) error {
	db, err := sql.Open("sqlite3", fileDSN(live, "_busy_timeout=30000&_foreign_keys=off"))
	if err != nil {
		return err
	}
	// ATTACH belongs to one connection, so use only that one.
	db.SetMaxOpenConns(1)
	defer db.Close()
	conn, err := db.Conn(ctx)
	if err != nil {
		return err
	}
	defer conn.Close()
	if _, err := conn.ExecContext(ctx, `ATTACH DATABASE ? AS stg`, staged); err != nil {
		return err
	}
	defer conn.ExecContext(context.Background(), `DETACH DATABASE stg`)

	tx, err := conn.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer tx.Rollback()
	for _, table := range mergeTables {
		cols, err := sharedColumns(ctx, tx, table)
		if err != nil {
			return err
		}
		if len(cols) == 0 {
			continue
		}
		list := strings.Join(cols, ", ")
		var total int64
		if err := tx.QueryRowContext(ctx, fmt.Sprintf(`SELECT count(*) FROM stg."%s"`, table)).Scan(&total); err != nil {
			return err
		}
		res, err := tx.ExecContext(ctx, fmt.Sprintf(`INSERT OR IGNORE INTO main."%[1]s" (%[2]s) SELECT %[2]s FROM stg."%[1]s"`, table, list))
		if err != nil {
			return fmt.Errorf("%s: %w", table, err)
		}
		added, _ := res.RowsAffected()
		rep.Added[table], rep.Present[table] = added, total-added
	}
	// A chat that live has but the backup holds newer messages for shows them.
	chatCols, err := sharedColumns(ctx, tx, "chats")
	if err != nil {
		return err
	}
	set := ""
	for _, c := range chatCols {
		if c == `"last_msg"` || c == `"last_time"` {
			set += fmt.Sprintf("%[1]s = s.%[1]s, ", c)
		}
	}
	if strings.Contains(set, `"last_time"`) {
		if _, err := tx.ExecContext(ctx, `UPDATE main.chats SET `+strings.TrimSuffix(set, ", ")+` FROM stg.chats AS s
			WHERE s.id = main.chats.id AND s.last_time > COALESCE(main.chats.last_time, 0)`); err != nil {
			return fmt.Errorf("chats: %w", err)
		}
	}
	return tx.Commit()
}

// sharedColumns lists the quoted columns table has in both live and staged
// databases, so a backup from another app version still merges.
func sharedColumns(ctx context.Context, tx *sql.Tx, table string) ([]string, error) {
	names := func(schema string) ([]string, error) {
		rows, err := tx.QueryContext(ctx, `SELECT name FROM pragma_table_info(?, ?)`, table, schema)
		if err != nil {
			return nil, err
		}
		defer rows.Close()
		var out []string
		for rows.Next() {
			var n string
			if err := rows.Scan(&n); err != nil {
				return nil, err
			}
			out = append(out, n)
		}
		return out, rows.Err()
	}
	live, err := names("main")
	if err != nil {
		return nil, err
	}
	staged, err := names("stg")
	if err != nil {
		return nil, err
	}
	have := map[string]bool{}
	for _, n := range staged {
		have[n] = true
	}
	var out []string
	for _, n := range live {
		if have[n] {
			out = append(out, `"`+n+`"`)
		}
	}
	if len(live) == 0 || len(staged) == 0 {
		return nil, nil
	}
	return out, nil
}

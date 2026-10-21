package backup

import (
	"bufio"
	"bytes"
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"slices"
	"sort"
	"strconv"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/mattn/go-sqlite3"
)

const (
	segmentMaxRows  = 5000
	segmentMaxBytes = 16 << 20
	sequenceTable   = "sqlite_sequence"
	restoreDriver   = "sqlite3_backup_restore"
)

func init() {
	// Restored schemas come from a manifest: the connection that runs them may
	// not reach any file but the database being built.
	sql.Register(restoreDriver, &sqlite3.SQLiteDriver{
		ConnectHook: func(c *sqlite3.SQLiteConn) error {
			c.RegisterAuthorizer(func(op int, _, _, _ string) int {
				if op == sqlite3.SQLITE_ATTACH || op == sqlite3.SQLITE_DETACH {
					return sqlite3.SQLITE_DENY
				}
				return sqlite3.SQLITE_OK
			})
			return nil
		},
	})
}

func quoteIdent(s string) string { return `"` + strings.ReplaceAll(s, `"`, `""`) + `"` }

func fileDSN(path, params string) string {
	escaped := strings.NewReplacer("%", "%25", "?", "%3f", "#", "%23").Replace(path)
	return "file:" + escaped + "?" + params
}

func integrityCheck(ctx context.Context, db *sql.DB) error {
	rows, err := db.QueryContext(ctx, `PRAGMA integrity_check`)
	if err != nil {
		return err
	}
	defer rows.Close()
	var problems []string
	for rows.Next() {
		var v string
		if err := rows.Scan(&v); err != nil {
			return err
		}
		if v != "ok" {
			problems = append(problems, v)
		}
	}
	if err := rows.Err(); err != nil {
		return err
	}
	if len(problems) > 0 {
		if len(problems) > 5 {
			problems = append(problems[:5], fmt.Sprintf("… %d more", len(problems)-5))
		}
		return fmt.Errorf("integrity_check: %s", strings.Join(problems, "; "))
	}
	return nil
}

func foreignKeyViolations(ctx context.Context, db *sql.DB) (int, error) {
	rows, err := db.QueryContext(ctx, `PRAGMA foreign_key_check`)
	if err != nil {
		return 0, err
	}
	defer rows.Close()
	n := 0
	for rows.Next() {
		n++
	}
	return n, rows.Err()
}

// snapshotDatabase checks the live database at live and writes a consistent
// copy of it to dest with VACUUM INTO. The live file is only read.
func snapshotDatabase(ctx context.Context, live, dest string) error {
	if _, err := os.Stat(live); err != nil {
		return err
	}
	db, err := sql.Open("sqlite3", fileDSN(live, "mode=ro&_busy_timeout=15000"))
	if err != nil {
		return err
	}
	defer db.Close()
	db.SetMaxOpenConns(1)
	if err := integrityCheck(ctx, db); err != nil {
		return fmt.Errorf("%s is damaged, nothing was backed up: %w", live, err)
	}
	if _, err := db.ExecContext(ctx, `VACUUM INTO ?`, dest); err != nil {
		return fmt.Errorf("snapshot %s: %w", live, err)
	}
	return nil
}

// tablePlan is how one table is read.
type tablePlan struct {
	name    string
	columns []string
	order   []string
	// byChat splits segments per chat per month (columns chat_id + timestamp).
	byChat bool
}

func (p tablePlan) selectSQL() string {
	cols := make([]string, len(p.columns))
	for i, c := range p.columns {
		// The unary + drops the declared type, so the driver hands back the
		// stored value untouched (no DATETIME parsing).
		cols[i] = "+" + quoteIdent(c)
	}
	return "SELECT " + strings.Join(cols, ", ") + " FROM " + quoteIdent(p.name) + " ORDER BY " + strings.Join(p.order, ", ")
}

func planTable(ctx context.Context, db *sql.DB, name, createSQL string) (tablePlan, error) {
	p := tablePlan{name: name}
	rows, err := db.QueryContext(ctx, `SELECT name, pk FROM pragma_table_info(?) ORDER BY cid`, name)
	if err != nil {
		return p, err
	}
	type pkCol struct {
		name string
		pos  int
	}
	var pks []pkCol
	for rows.Next() {
		var col string
		var pk int
		if err := rows.Scan(&col, &pk); err != nil {
			rows.Close()
			return p, err
		}
		p.columns = append(p.columns, col)
		if pk > 0 {
			pks = append(pks, pkCol{col, pk})
		}
	}
	rows.Close()
	if err := rows.Err(); err != nil {
		return p, err
	}
	if len(p.columns) == 0 {
		return p, fmt.Errorf("table %s has no columns", name)
	}
	sort.Slice(pks, func(i, j int) bool { return pks[i].pos < pks[j].pos })
	var key []string
	for _, c := range pks {
		key = append(key, quoteIdent(c.name))
	}
	if len(key) == 0 {
		if strings.Contains(strings.ToUpper(createSQL), "WITHOUT ROWID") {
			for _, c := range p.columns {
				key = append(key, quoteIdent(c))
			}
		} else {
			key = []string{"rowid"}
		}
	}
	p.byChat = slices.Contains(p.columns, "chat_id") && slices.Contains(p.columns, "timestamp")
	if p.byChat {
		p.order = append([]string{`"chat_id"`, `"timestamp"`}, key...)
	} else {
		p.order = key
	}
	return p, nil
}

// encodeRow writes one row as a JSON array that keeps SQLite's storage class:
// integers are numbers, reals {"r":"…"}, text strings (or {"x":base64} when not
// UTF-8), blobs {"b":base64}, NULL null.
func encodeRow(buf *bytes.Buffer, vals []any) error {
	buf.WriteByte('[')
	for i, v := range vals {
		if i > 0 {
			buf.WriteByte(',')
		}
		switch x := v.(type) {
		case nil:
			buf.WriteString("null")
		case int64:
			buf.WriteString(strconv.FormatInt(x, 10))
		case float64:
			buf.WriteString(`{"r":"` + strconv.FormatFloat(x, 'g', -1, 64) + `"}`)
		case string:
			if !utf8.ValidString(x) {
				buf.WriteString(`{"x":"` + base64.StdEncoding.EncodeToString([]byte(x)) + `"}`)
				continue
			}
			b, err := json.Marshal(x)
			if err != nil {
				return err
			}
			buf.Write(b)
		case []byte:
			buf.WriteString(`{"b":"` + base64.StdEncoding.EncodeToString(x) + `"}`)
		case bool:
			if x {
				buf.WriteString("1")
			} else {
				buf.WriteString("0")
			}
		case time.Time:
			b, _ := json.Marshal(x.Format(time.RFC3339Nano))
			buf.Write(b)
		default:
			return fmt.Errorf("unsupported value type %T", v)
		}
	}
	buf.WriteString("]\n")
	return nil
}

func decodeRow(line []byte, n int) ([]any, error) {
	dec := json.NewDecoder(bytes.NewReader(line))
	dec.UseNumber()
	var raw []any
	if err := dec.Decode(&raw); err != nil {
		return nil, err
	}
	if len(raw) != n {
		return nil, fmt.Errorf("row has %d values, expected %d", len(raw), n)
	}
	out := make([]any, n)
	for i, v := range raw {
		switch x := v.(type) {
		case nil:
		case json.Number:
			iv, err := strconv.ParseInt(string(x), 10, 64)
			if err != nil {
				return nil, err
			}
			out[i] = iv
		case string:
			out[i] = x
		case map[string]any:
			var err error
			switch {
			case x["r"] != nil:
				s, _ := x["r"].(string)
				out[i], err = strconv.ParseFloat(s, 64)
			case x["b"] != nil:
				s, _ := x["b"].(string)
				out[i], err = base64.StdEncoding.DecodeString(s)
			case x["x"] != nil:
				s, _ := x["x"].(string)
				var b []byte
				b, err = base64.StdEncoding.DecodeString(s)
				out[i] = string(b)
			default:
				err = errors.New("unknown value")
			}
			if err != nil {
				return nil, err
			}
		default:
			return nil, fmt.Errorf("unexpected value %T", v)
		}
	}
	return out, nil
}

// scanTable streams every row of the table in plan order, handing each row's
// values and its encoded line to fn.
func scanTable(ctx context.Context, db *sql.DB, p tablePlan, fn func(vals []any, line []byte) error) error {
	rows, err := db.QueryContext(ctx, p.selectSQL())
	if err != nil {
		return err
	}
	defer rows.Close()
	vals := make([]any, len(p.columns))
	ptrs := make([]any, len(p.columns))
	for i := range vals {
		ptrs[i] = &vals[i]
	}
	var line bytes.Buffer
	for rows.Next() {
		if err := rows.Scan(ptrs...); err != nil {
			return err
		}
		line.Reset()
		if err := encodeRow(&line, vals); err != nil {
			return fmt.Errorf("table %s: %w", p.name, err)
		}
		if err := fn(vals, line.Bytes()); err != nil {
			return err
		}
	}
	return rows.Err()
}

// digest folds row hashes into one value that does not depend on row order.
type digest struct{ sums [][32]byte }

func (d *digest) add(line []byte) { d.sums = append(d.sums, sha256.Sum256(line)) }

func (d *digest) hex() string {
	sort.Slice(d.sums, func(i, j int) bool { return bytes.Compare(d.sums[i][:], d.sums[j][:]) < 0 })
	h := sha256.New()
	for _, s := range d.sums {
		h.Write(s[:])
	}
	return hex.EncodeToString(h.Sum(nil))
}

type segmentHeader struct {
	Table   string   `json:"table"`
	Columns []string `json:"columns"`
}

func monthOf(v any) string {
	ts, ok := v.(int64)
	if !ok {
		return "none"
	}
	if ts > 100_000_000_000 { // milliseconds
		ts /= 1000
	}
	return time.Unix(ts, 0).UTC().Format("2006-01")
}

// dumpDatabase stores the snapshot at path as schema + table segments.
func dumpDatabase(ctx context.Context, path, name, file string, chunks *chunkStore, progress func(Progress)) (Database, []string, error) {
	out := Database{Name: name, File: file}
	var warnings []string
	db, err := sql.Open("sqlite3", fileDSN(path, "mode=ro"))
	if err != nil {
		return out, nil, err
	}
	defer db.Close()
	db.SetMaxOpenConns(1)

	if err := integrityCheck(ctx, db); err != nil {
		return out, nil, fmt.Errorf("snapshot of %s failed its check: %w", name, err)
	}
	if n, err := foreignKeyViolations(ctx, db); err != nil {
		return out, nil, err
	} else if n > 0 {
		warnings = append(warnings, fmt.Sprintf("%s: %d foreign key violations (backed up as they are)", name, n))
	}
	if err := db.QueryRowContext(ctx, `PRAGMA user_version`).Scan(&out.UserVersion); err != nil {
		return out, nil, err
	}

	rows, err := db.QueryContext(ctx, `SELECT type, name, tbl_name, sql FROM sqlite_master WHERE sql IS NOT NULL ORDER BY rowid`)
	if err != nil {
		return out, nil, err
	}
	hasSequence := false
	for rows.Next() {
		var o SchemaObject
		if err := rows.Scan(&o.Type, &o.Name, &o.Table, &o.SQL); err != nil {
			rows.Close()
			return out, nil, err
		}
		if strings.HasPrefix(strings.ToUpper(strings.TrimSpace(o.SQL)), "CREATE VIRTUAL TABLE") {
			rows.Close()
			return out, nil, fmt.Errorf("%s: virtual table %s is not supported", name, o.Name)
		}
		if o.Name == sequenceTable {
			hasSequence = true
			continue
		}
		if strings.HasPrefix(o.Name, "sqlite_") {
			continue
		}
		out.Schema = append(out.Schema, o)
	}
	rows.Close()
	if err := rows.Err(); err != nil {
		return out, nil, err
	}

	var tables []SchemaObject
	for _, o := range out.Schema {
		if o.Type == "table" {
			tables = append(tables, o)
		}
	}
	if hasSequence {
		tables = append(tables, SchemaObject{Type: "table", Name: sequenceTable})
	}
	for i, o := range tables {
		p, err := planTable(ctx, db, o.Name, o.SQL)
		if err != nil {
			return out, nil, err
		}
		t, err := dumpTable(ctx, db, p, chunks)
		if err != nil {
			return out, nil, fmt.Errorf("%s.%s: %w", name, o.Name, err)
		}
		out.Tables = append(out.Tables, t)
		progress(Progress{Stage: "chat", Item: name + "." + o.Name, Done: int64(i + 1), Total: int64(len(tables))})
	}
	return out, warnings, nil
}

func dumpTable(ctx context.Context, db *sql.DB, p tablePlan, chunks *chunkStore) (Table, error) {
	t := Table{Name: p.name, Columns: p.columns}
	header, err := json.Marshal(segmentHeader{Table: p.name, Columns: p.columns})
	if err != nil {
		return t, err
	}
	var (
		d       digest
		seg     bytes.Buffer
		segRows int64
		segKey  string
		part    int
		lastKey string
	)
	flush := func() error {
		if segRows == 0 {
			return nil
		}
		ref, err := chunks.putCompressed(ctx, seg.Bytes())
		if err != nil {
			return err
		}
		t.Segments = append(t.Segments, Segment{Key: segKey, Rows: segRows, Chunk: ref})
		seg.Reset()
		segRows = 0
		return nil
	}
	chatIdx, tsIdx := slices.Index(p.columns, "chat_id"), slices.Index(p.columns, "timestamp")
	err = scanTable(ctx, db, p, func(vals []any, line []byte) error {
		d.add(line)
		t.Rows++
		key := ""
		if p.byChat {
			chat, _ := vals[chatIdx].(string)
			key = "chat=" + chat + " month=" + monthOf(vals[tsIdx])
		}
		if segRows > 0 && (key != lastKey || segRows >= segmentMaxRows || seg.Len() >= segmentMaxBytes) {
			if err := flush(); err != nil {
				return err
			}
			if key == lastKey {
				part++
			} else {
				part = 0
			}
		}
		if segRows == 0 {
			if p.byChat {
				segKey = fmt.Sprintf("%s part=%d", key, part)
			} else {
				segKey = fmt.Sprintf("from=%d", t.Rows-1)
			}
			seg.Write(header)
			seg.WriteByte('\n')
		}
		lastKey = key
		seg.Write(line)
		segRows++
		return nil
	})
	if err != nil {
		return t, err
	}
	if err := flush(); err != nil {
		return t, err
	}
	t.Digest = d.hex()
	return t, nil
}

// LostItem is part of a backup that could not be restored.
type LostItem struct {
	What string `json:"what"`
	Key  string `json:"key"`
	Rows int64  `json:"rows,omitempty"`
	Err  string `json:"error"`
}

// restoreDatabase builds a new database file at dest from db. Segments that
// cannot be read are recorded in lost and skipped; the rest is restored.
func restoreDatabase(ctx context.Context, d Database, dest string, chunks *chunkStore, rep *RestoreReport, progress func(Progress)) error {
	if _, err := os.Stat(dest); err == nil {
		return fmt.Errorf("%s already exists", dest)
	}
	db, err := sql.Open(restoreDriver, fileDSN(dest, "_journal_mode=OFF&_synchronous=OFF"))
	if err != nil {
		return err
	}
	db.SetMaxOpenConns(1)
	defer db.Close()

	for _, o := range d.Schema {
		if o.Type != "table" {
			continue
		}
		if !strings.HasPrefix(strings.ToUpper(strings.TrimSpace(o.SQL)), "CREATE TABLE") {
			return fmt.Errorf("%s: unexpected schema for table %s", d.Name, o.Name)
		}
		if _, err := db.ExecContext(ctx, o.SQL); err != nil {
			return fmt.Errorf("%s: create table %s: %w", d.Name, o.Name, err)
		}
	}

	lostTables := map[string]bool{}
	for i, t := range d.Tables {
		if err := restoreTable(ctx, db, d.Name, t, chunks, rep, lostTables); err != nil {
			return err
		}
		progress(Progress{Stage: "chat", Item: d.Name + "." + t.Name, Done: int64(i + 1), Total: int64(len(d.Tables))})
	}

	for _, o := range d.Schema {
		if o.Type == "table" {
			continue
		}
		if !strings.HasPrefix(strings.ToUpper(strings.TrimSpace(o.SQL)), "CREATE ") {
			return fmt.Errorf("%s: unexpected schema for %s", d.Name, o.Name)
		}
		if _, err := db.ExecContext(ctx, o.SQL); err != nil {
			return fmt.Errorf("%s: create %s %s: %w", d.Name, o.Type, o.Name, err)
		}
	}
	if _, err := db.ExecContext(ctx, fmt.Sprintf("PRAGMA user_version = %d", d.UserVersion)); err != nil {
		return err
	}

	// Prove what came back is what was backed up.
	if err := integrityCheck(ctx, db); err != nil {
		return fmt.Errorf("%s: restored database failed its check: %w", d.Name, err)
	}
	for _, t := range d.Tables {
		p := tablePlan{name: t.Name, columns: t.Columns, order: []string{"rowid"}}
		if t.Name == sequenceTable {
			p.order = []string{`"name"`}
		}
		var dg digest
		var n int64
		if err := scanTable(ctx, db, p, func(_ []any, line []byte) error {
			dg.add(line)
			n++
			return nil
		}); err != nil {
			return fmt.Errorf("%s.%s: %w", d.Name, t.Name, err)
		}
		rep.Rows += n
		if lostTables[t.Name] {
			continue // already reported as lost
		}
		if n != t.Rows || dg.hex() != t.Digest {
			rep.Mismatch = append(rep.Mismatch, fmt.Sprintf("%s.%s: %d rows restored, %d backed up, content differs", d.Name, t.Name, n, t.Rows))
		}
	}
	if err := db.Close(); err != nil {
		return err
	}
	if f, err := os.Open(dest); err == nil {
		_ = f.Sync()
		f.Close()
	}
	return nil
}

func restoreTable(ctx context.Context, db *sql.DB, dbName string, t Table, chunks *chunkStore, rep *RestoreReport, lostTables map[string]bool) error {
	cols := make([]string, len(t.Columns))
	marks := make([]string, len(t.Columns))
	for i, c := range t.Columns {
		cols[i] = quoteIdent(c)
		marks[i] = "?"
	}
	insert := "INSERT INTO " + quoteIdent(t.Name) + " (" + strings.Join(cols, ", ") + ") VALUES (" + strings.Join(marks, ", ") + ")"
	if t.Name == sequenceTable {
		if _, err := db.ExecContext(ctx, "DELETE FROM "+sequenceTable); err != nil {
			// No AUTOINCREMENT table was created, so there is nothing to restore.
			return nil
		}
	}
	for _, s := range t.Segments {
		lose := func(err error) {
			lostTables[t.Name] = true
			rep.Lost = append(rep.Lost, LostItem{What: dbName + "." + t.Name, Key: s.Key, Rows: s.Rows, Err: err.Error()})
		}
		raw, err := chunks.get(ctx, s.Chunk)
		if err != nil {
			if ctx.Err() != nil {
				return ctx.Err()
			}
			lose(err)
			continue
		}
		if err := insertSegment(ctx, db, insert, t, raw, s.Rows); err != nil {
			if ctx.Err() != nil {
				return ctx.Err()
			}
			lose(err)
		}
	}
	return nil
}

// insertSegment adds a segment's rows in one transaction: all of them or none.
func insertSegment(ctx context.Context, db *sql.DB, insert string, t Table, raw []byte, want int64) error {
	sc := bufio.NewScanner(bytes.NewReader(raw))
	sc.Buffer(make([]byte, 0, 1<<20), maxChunkBytes)
	if !sc.Scan() {
		return errors.New("segment is empty")
	}
	var h segmentHeader
	if err := json.Unmarshal(sc.Bytes(), &h); err != nil {
		return fmt.Errorf("segment header: %w", err)
	}
	if h.Table != t.Name || !slices.Equal(h.Columns, t.Columns) {
		return errors.New("segment belongs to another table")
	}
	tx, err := db.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer tx.Rollback()
	stmt, err := tx.PrepareContext(ctx, insert)
	if err != nil {
		return err
	}
	defer stmt.Close()
	var n int64
	for sc.Scan() {
		vals, err := decodeRow(sc.Bytes(), len(t.Columns))
		if err != nil {
			return fmt.Errorf("row %d: %w", n, err)
		}
		if _, err := stmt.ExecContext(ctx, vals...); err != nil {
			return fmt.Errorf("row %d: %w", n, err)
		}
		n++
	}
	if err := sc.Err(); err != nil {
		return err
	}
	if n != want {
		return fmt.Errorf("segment holds %d rows, manifest says %d", n, want)
	}
	return tx.Commit()
}

package backup

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"sort"
	"strings"
	"time"
)

// FormatVersion is the manifest layout this package writes and reads.
const FormatVersion = 1

const (
	chunkPrefix        = "chunks/"
	manifestPrefix     = "manifests/"
	manifestCopyPrefix = "manifests-copy/"
	idLayout           = "20060102T150405Z"
)

// Manifest lists everything one backup of one account holds. It is complete on
// its own: restoring never needs an older manifest.
type Manifest struct {
	Format     int        `json:"format"`
	ID         string     `json:"id"`
	Account    string     `json:"account"`
	CreatedAt  time.Time  `json:"created_at"`
	AppVersion string     `json:"app_version,omitempty"`
	Databases  []Database `json:"databases"`
	Media      *Media     `json:"media,omitempty"`
}

// Database is one SQLite file stored as schema + table segments.
type Database struct {
	Name        string         `json:"name"`
	File        string         `json:"file"`
	UserVersion int64          `json:"user_version"`
	Schema      []SchemaObject `json:"schema"`
	Tables      []Table        `json:"tables"`
}

// SchemaObject is one row of sqlite_master.
type SchemaObject struct {
	Type  string `json:"type"`
	Name  string `json:"name"`
	Table string `json:"table"`
	SQL   string `json:"sql"`
}

// Table is one table's rows, split into independent segments.
type Table struct {
	Name    string   `json:"name"`
	Columns []string `json:"columns"`
	Rows    int64    `json:"rows"`
	// Digest identifies the table's content regardless of row order or
	// segmentation; restore recomputes it to prove the rows came back intact.
	Digest   string    `json:"digest"`
	Segments []Segment `json:"segments"`
}

// Segment is a run of rows stored in one chunk.
type Segment struct {
	Key   string   `json:"key"`
	Rows  int64    `json:"rows"`
	Chunk ChunkRef `json:"chunk"`
}

// ChunkRef points at stored bytes. The chunk is named after the SHA-256 of
// the bytes as stored; RawSHA256 is the checksum after decoding.
type ChunkRef struct {
	SHA256    string `json:"sha256"`
	Size      int64  `json:"size"`
	Encoding  string `json:"encoding,omitempty"`
	RawSHA256 string `json:"raw_sha256,omitempty"`
	RawSize   int64  `json:"raw_size,omitempty"`
}

// Media is the account's object storage, one entry per object.
type Media struct {
	File    string        `json:"file"`
	Objects []MediaObject `json:"objects"`
	Bytes   int64         `json:"bytes"`
	// Skipped lists objects that failed their checksum on this machine and
	// were therefore left out.
	Skipped []string `json:"skipped,omitempty"`
}

// MediaObject is one stored media file; Parts are its bytes in order.
type MediaObject struct {
	Account      string            `json:"account"`
	Bucket       string            `json:"bucket"`
	Key          string            `json:"key"`
	Size         int64             `json:"size"`
	SHA256       string            `json:"sha256"`
	ContentType  string            `json:"content_type,omitempty"`
	Metadata     map[string]string `json:"metadata,omitempty"`
	LastModified time.Time         `json:"last_modified"`
	Parts        []ChunkRef        `json:"parts"`
}

func chunkName(sum string) string { return chunkPrefix + sum[:2] + "/" + sum }

func manifestName(prefix, account, id, sum string) string {
	return prefix + account + "/" + id + "-" + sum + ".json"
}

// chunks returns every chunk the manifest refers to.
func (m *Manifest) chunks() []ChunkRef {
	var out []ChunkRef
	for _, db := range m.Databases {
		for _, t := range db.Tables {
			for _, s := range t.Segments {
				out = append(out, s.Chunk)
			}
		}
	}
	if m.Media != nil {
		for _, o := range m.Media.Objects {
			out = append(out, o.Parts...)
		}
	}
	return out
}

func encodeManifest(m *Manifest) ([]byte, string, error) {
	data, err := json.MarshalIndent(m, "", " ")
	if err != nil {
		return nil, "", err
	}
	sum := sha256.Sum256(data)
	return data, hex.EncodeToString(sum[:]), nil
}

// writeManifest stores the manifest twice. It is the commit point of a backup.
func writeManifest(ctx context.Context, t Target, m *Manifest) (string, error) {
	data, sum, err := encodeManifest(m)
	if err != nil {
		return "", err
	}
	for _, prefix := range []string{manifestPrefix, manifestCopyPrefix} {
		name := manifestName(prefix, m.Account, m.ID, sum)
		e, err := t.Put(ctx, name, bytes.NewReader(data), int64(len(data)))
		if err != nil {
			return "", fmt.Errorf("backup: write manifest: %w", err)
		}
		if e.SHA256 != "" && e.SHA256 != sum {
			return "", fmt.Errorf("backup: manifest %s stored with checksum %s", name, e.SHA256)
		}
	}
	return sum, nil
}

// ManifestInfo is a backup found in a target.
type ManifestInfo struct {
	Account string
	ID      string
	SHA256  string
	// names are the stored copies, primary first.
	names []string
}

// CreatedAt is when the backup was taken, from its ID.
func (mi ManifestInfo) CreatedAt() time.Time {
	t, _ := time.Parse(idLayout, mi.ID)
	return t
}

// parseManifestName splits "manifests/<acct>/<id>-<sum>.json".
func parseManifestName(prefix, name string) (account, id, sum string, ok bool) {
	rest, found := strings.CutPrefix(name, prefix)
	if !found {
		return "", "", "", false
	}
	account, file, found := strings.Cut(rest, "/")
	if !found || strings.Contains(file, "/") {
		return "", "", "", false
	}
	base, found := strings.CutSuffix(file, ".json")
	if !found {
		return "", "", "", false
	}
	i := strings.LastIndex(base, "-")
	if i < 0 || len(base)-i-1 != 64 {
		return "", "", "", false
	}
	return account, base[:i], base[i+1:], true
}

// ListBackups returns the backups in the target, newest first. account limits
// the result to one account when not empty.
func ListBackups(ctx context.Context, t Target, account string) ([]ManifestInfo, error) {
	byKey := map[string]*ManifestInfo{}
	var order []string
	for _, prefix := range []string{manifestPrefix, manifestCopyPrefix} {
		p := prefix
		if account != "" {
			p += account + "/"
		}
		entries, err := t.List(ctx, p)
		if err != nil {
			return nil, err
		}
		for _, e := range entries {
			acct, id, sum, ok := parseManifestName(prefix, e.Name)
			if !ok {
				continue
			}
			k := acct + "/" + id + "/" + sum
			mi := byKey[k]
			if mi == nil {
				mi = &ManifestInfo{Account: acct, ID: id, SHA256: sum}
				byKey[k] = mi
				order = append(order, k)
			}
			mi.names = append(mi.names, e.Name)
		}
	}
	out := make([]ManifestInfo, 0, len(order))
	for _, k := range order {
		out = append(out, *byKey[k])
	}
	sort.SliceStable(out, func(i, j int) bool {
		if out[i].ID != out[j].ID {
			return out[i].ID > out[j].ID
		}
		return out[i].Account < out[j].Account
	})
	return out, nil
}

// errBadManifest marks a manifest whose every copy is missing or damaged.
var errBadManifest = errors.New("backup: manifest damaged")

// LoadManifest reads a backup's manifest, trying each stored copy until one
// matches the checksum in its name.
func LoadManifest(ctx context.Context, t Target, mi ManifestInfo) (*Manifest, error) {
	var lastErr error
	for _, name := range mi.names {
		m, err := loadManifestCopy(ctx, t, name, mi.SHA256)
		if err == nil {
			return m, nil
		}
		lastErr = err
	}
	return nil, fmt.Errorf("%w: %s %s: %v", errBadManifest, mi.Account, mi.ID, lastErr)
}

func loadManifestCopy(ctx context.Context, t Target, name, sum string) (*Manifest, error) {
	rc, err := t.Get(ctx, name)
	if err != nil {
		return nil, err
	}
	defer rc.Close()
	data, err := io.ReadAll(io.LimitReader(rc, 256<<20))
	if err != nil {
		return nil, err
	}
	got := sha256.Sum256(data)
	if hex.EncodeToString(got[:]) != sum {
		return nil, fmt.Errorf("%s: checksum mismatch", name)
	}
	var m Manifest
	if err := json.Unmarshal(data, &m); err != nil {
		return nil, fmt.Errorf("%s: %w", name, err)
	}
	if m.Format != FormatVersion {
		return nil, fmt.Errorf("%s: unsupported format %d", name, m.Format)
	}
	return &m, nil
}

package repository

import (
	"context"
	"io"
	"net/http"
)

// StorageRepository is how WA-Bot reads and writes media. Paths are object keys
// ("history/chat/a.jpg"), not filesystem locations: where the bytes live is the
// provider's business. The legacy "media/" prefix on a key is ignored.
//
// Programs that insist on real files (ffmpeg, yt-dlp, gallery-dl, Lua scripts)
// use the Work* methods: they get scratch files that are copied out of, and
// back into, the storage. A Work path is disposable, never a permanent location.
type StorageRepository interface {
	// Save stores reader under path and returns the key it was stored as.
	Save(ctx context.Context, path string, reader io.Reader) (string, error)
	// Get streams an object. The caller must Close it.
	Get(ctx context.Context, path string) (io.ReadCloser, error)
	Delete(ctx context.Context, path string) error
	Exists(path string) bool
	// ServeMedia answers an HTTP GET/HEAD for path with streaming, Range,
	// ETag and conditional-request support.
	ServeMedia(w http.ResponseWriter, r *http.Request, path string)

	// WorkFile returns a scratch file holding the object at path (or, for a key
	// that does not exist yet, the place a tool should create it). Pass the
	// result to WorkSync once the tool is done to keep what it wrote.
	WorkFile(ctx context.Context, path string) (string, error)
	// WorkDir is WorkFile for a directory a tool fills with files; the
	// directory exists on return. WorkFlush stores whatever ended up in it.
	WorkDir(ctx context.Context, path string) (string, error)
	// WorkRoot is the current account's scratch folder.
	WorkRoot() string
	// WorkSync stores the scratch file of path if it is new or changed.
	WorkSync(ctx context.Context, path string) error
	// WorkFlush stores every new or changed scratch file of the current account.
	WorkFlush(ctx context.Context) error
	// WorkTemp is a private throw-away folder for a tool with unpredictable
	// output names. The caller removes it with cleanup.
	WorkTemp(prefix string) (dir string, cleanup func(), err error)
}

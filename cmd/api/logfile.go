package main

import (
	"io"
	"os"
	"path/filepath"
)

const maxLogBytes = 5 << 20

// teeOutputToFile mirrors stdout and stderr into backend.log beside the
// database folder (when DB_PATH says where that is), so a backend started by
// the desktop app, whose output goes to a pipe, still leaves a log to read.
// The previous log is kept as backend.log.old once it grows past 5 MB.
func teeOutputToFile() {
	dbPath := os.Getenv("DB_PATH")
	if dbPath == "" {
		return
	}
	path := filepath.Join(filepath.Dir(dbPath), "backend.log")
	if info, err := os.Stat(path); err == nil && info.Size() > maxLogBytes {
		_ = os.Rename(path, path+".old")
	}
	file, err := os.OpenFile(path, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o600)
	if err != nil {
		return
	}
	for _, stream := range []**os.File{&os.Stdout, &os.Stderr} {
		original := *stream
		reader, writer, err := os.Pipe()
		if err != nil {
			continue
		}
		*stream = writer
		go func() { _, _ = io.Copy(io.MultiWriter(original, file), reader) }()
	}
}

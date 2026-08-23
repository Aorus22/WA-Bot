package media

import (
	"os"
	"path/filepath"
	"runtime"
	"sync"
)

var (
	gstOnce   sync.Once
	gstAvail  bool
	gstProbed bool
)

// GStreamerAvailable reports whether the GTK4 media backend's GStreamer
// runtime library can be found. Inline audio/video players are only created
// when this is true; otherwise bubbles fall back to opening files with an
// external player.
func GStreamerAvailable() bool {
	gstOnce.Do(func() {
		gstAvail = probeGStreamer()
		gstProbed = true
	})
	return gstAvail
}

func probeGStreamer() bool {
	switch runtime.GOOS {
	case "windows":
		for _, dir := range candidateWindowsDirs() {
			if _, err := os.Stat(filepath.Join(dir, "libgstreamer-1.0-0.dll")); err == nil {
				return true
			}
			if _, err := os.Stat(filepath.Join(dir, "gstreamer-1.0")); err == nil {
				return true
			}
		}
		return false
	default:
		for _, pattern := range []string{
			"/usr/lib/*/libgstreamer-1.0.so*",
			"/usr/lib64/libgstreamer-1.0.so*",
			"/usr/local/lib/libgstreamer-1.0.so*",
			"/opt/homebrew/lib/libgstreamer-1.0.dylib",
		} {
			if hits, _ := filepath.Glob(pattern); len(hits) > 0 {
				return true
			}
		}
		return false
	}
}

func candidateWindowsDirs() []string {
	var dirs []string
	if exe, err := os.Executable(); err == nil {
		dirs = append(dirs, filepath.Dir(exe))
	}
	dirs = append(dirs, `C:\msys64\mingw64\bin`)
	for _, pathDir := range filepath.SplitList(os.Getenv("PATH")) {
		dirs = append(dirs, pathDir)
	}
	return dirs
}

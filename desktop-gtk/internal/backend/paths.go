// Package backend: per-user data directory resolution + ring buffer for stdout.
package backend

import (
	"os"
	"path/filepath"
)

// UserDataDir returns the per-user data directory for wa-bot-desktop.
// Layout matches Electron's app.getPath('userData') for parity:
//   - Windows: %APPDATA%\wa-bot-desktop
//   - macOS:   ~/Library/Application Support/wa-bot-desktop
//   - Linux:   $XDG_DATA_HOME/wa-bot-desktop  (default ~/.local/share/wa-bot-desktop)
//
// The directory is created if it does not exist.
func UserDataDir() string {
	var base string
	if v := os.Getenv("WA_BOT_DESKTOP_DATA"); v != "" {
		base = v
	} else {
		switch {
		case isWindows():
			base = filepath.Join(os.Getenv("APPDATA"), "wa-bot-desktop")
		case isDarwin():
			base = filepath.Join(homeDir(), "Library", "Application Support", "wa-bot-desktop")
		default:
			xdg := os.Getenv("XDG_DATA_HOME")
			if xdg == "" {
				xdg = filepath.Join(homeDir(), ".local", "share")
			}
			base = filepath.Join(xdg, "wa-bot-desktop")
		}
	}
	_ = os.MkdirAll(base, 0o755)
	return base
}

func homeDir() string {
	h, _ := os.UserHomeDir()
	return h
}

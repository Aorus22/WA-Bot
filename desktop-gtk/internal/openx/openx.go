// Package openx opens files and URLs with the platform's default handler.
package openx

import (
	"os/exec"
	"runtime"
)

// File opens a local file with the OS default application.
func File(path string) error {
	return open(path)
}

// URL opens an http(s) URL in the default browser.
func URL(rawURL string) error {
	return open(rawURL)
}

func open(target string) error {
	switch runtime.GOOS {
	case "windows":
		// rundll32 avoids the cmd.exe quoting pitfalls of `start`.
		return exec.Command("rundll32", "url.dll,FileProtocolHandler", target).Start()
	case "darwin":
		return exec.Command("open", target).Start()
	default:
		return exec.Command("xdg-open", target).Start()
	}
}

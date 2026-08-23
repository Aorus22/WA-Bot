//go:build windows
// +build windows

package backend

import "os"

// findProcess is os.FindProcess on Windows.
func findProcess(pid int) (*os.Process, error) {
	return os.FindProcess(pid)
}

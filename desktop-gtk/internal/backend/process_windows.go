//go:build windows
// +build windows

package backend

import "os/exec"

// applyPlatformSysProcAttr is a no-op on Windows: Setpgid is not supported,
// and we use Process.Kill() directly in Stop().
func applyPlatformSysProcAttr(cmd *exec.Cmd) {}

// isWindows is true on Windows builds.
func isWindows() bool { return true }

// signalStop kills the process on Windows. SIGINT is not supported for child
// processes on Windows.
func signalStop(pid int) error {
	// Process.Kill sends a TerminateProcess; safe to call from any goroutine.
	// We return nil because the caller has no recovery path on Windows anyway.
	proc, err := findProcess(pid)
	if err != nil {
		return err
	}
	return proc.Kill()
}

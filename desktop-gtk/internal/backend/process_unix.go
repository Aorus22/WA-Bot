//go:build !windows
// +build !windows

package backend

import (
	"os/exec"
	"syscall"
)

// applyPlatformSysProcAttr configures POSIX-specific process attributes:
// run the child in its own process group so a Ctrl+C in the desktop app
// can signal the whole tree.
func applyPlatformSysProcAttr(cmd *exec.Cmd) {
	cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
}

// signalProcessGroup sends the given signal to the child's process group
// (POSIX). Returns an error if the group cannot be determined.
func signalProcessGroup(pid int, sig syscall.Signal) error {
	pgid, err := syscall.Getpgid(pid)
	if err != nil {
		return err
	}
	return syscall.Kill(-pgid, sig)
}

// isWindows is false on POSIX builds (it's true only on Windows).
func isWindows() bool { return false }

// signalStop sends SIGINT to the child's process group (POSIX).
func signalStop(pid int) error {
	return signalProcessGroup(pid, syscall.SIGINT)
}

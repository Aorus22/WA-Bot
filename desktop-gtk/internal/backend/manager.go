// Package backend manages the wa-bot-backend subprocess: lifecycle, port discovery,
// log capture, and graceful shutdown.
package backend

import (
	"bufio"
	"context"
	"errors"
	"fmt"
	"io"
	"log"
	"os"
	"os/exec"
	"runtime"
	"strconv"
	"strings"
	"sync"
	"time"
)

// Manager owns the wa-bot-backend subprocess and exposes:
//   - a channel that delivers the bound port once the backend prints `BACKEND_PORT:<n>`
//   - a channel for process exit errors
//   - a thread-safe log ring buffer of the backend's stdout+stderr
type Manager struct {
	path      string
	userData  string
	cmd       *exec.Cmd
	portCh    chan int
	errCh     chan error
	stoppedCh chan struct{}

	logBuf  *RingBuffer
	mu      sync.Mutex
	stopped bool
}

// NewManager creates a Manager. userData is the per-user data dir; the manager
// will set DB_PATH=<userData>/database and MEDIA_PATH=<userData>/media for the
// backend.
func NewManager(path, userData string) *Manager {
	return &Manager{
		path:      path,
		userData:  userData,
		portCh:    make(chan int, 1),
		errCh:     make(chan error, 1),
		stoppedCh: make(chan struct{}),
		logBuf:    NewRingBuffer(500),
	}
}

// Start launches the backend subprocess and begins reading stdout for the
// BACKEND_PORT: handshake. It returns once the process has been started; the
// port is delivered asynchronously on Port().
func (m *Manager) Start(ctx context.Context) error {
	// Ensure DB and media directories exist
	dbPath := m.databasePath()
	mediaPath := m.mediaPath()
	for _, p := range []string{dbPath, mediaPath} {
		if err := os.MkdirAll(p, 0o755); err != nil {
			return fmt.Errorf("create %s: %w", p, err)
		}
	}

	cmd := exec.CommandContext(ctx, m.path)
	cmd.Env = buildEnv(m.userData)

	// Run the backend in the user-data directory so its hardcoded relative
	// paths (e.g. "database/wa-bot-app.db") resolve under per-user storage.
	// This mirrors the Electron app's cwd=userDataPath behavior.
	cmd.Dir = m.userData

	// Per-OS process attributes (Setpgid on POSIX, no-op on Windows).
	applyPlatformSysProcAttr(cmd)

	// Capture stdout. We use cmd.StdoutPipe() so the OS pipe is owned by
	// os/exec (avoids io.Pipe buffering pitfalls). All of stdout goes to
	// the ring buffer; the line scanner looks for BACKEND_PORT: inside
	// the same goroutine, before appending to the ring buffer.
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return fmt.Errorf("stdout pipe: %w", err)
	}
	// Send stderr to the ring buffer AND to a log file under userData for debugging.
	stderrFile, err := os.OpenFile(m.userData+"/backend.stderr.log", os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o644)
	if err != nil {
		// Fall back: stderr goes to ring buffer only.
		cmd.Stderr = m.logBuf
	} else {
		cmd.Stderr = io.MultiWriter(m.logBuf, stderrFile)
	}

	if err := cmd.Start(); err != nil {
		stdout.Close()
		return fmt.Errorf("start backend: %w", err)
	}
	m.cmd = cmd
	log.Printf("backend process started, pid=%d", cmd.Process.Pid)
	m.logBuf.Append("[desktop-gtk] backend process started, pid=" + strconv.Itoa(cmd.Process.Pid))

	// Reader goroutine: write to ring buffer AND look for BACKEND_PORT: lines
	go m.scanStdout(stdout)

	// Waiter goroutine: surface exit errors
	go func() {
		err := cmd.Wait()
		log.Printf("backend process exited: %v", err)
		m.logBuf.Append("[desktop-gtk] backend process exited: " + errString(err))
		m.mu.Lock()
		alreadyStopped := m.stopped
		m.mu.Unlock()
		if !alreadyStopped {
			m.errCh <- err
		}
		close(m.stoppedCh)
	}()

	return nil
}

// Port returns the channel that delivers the bound port once known.
// Closes when the backend process exits.
func (m *Manager) Port() <-chan int { return m.portCh }

// Errors returns the channel that surfaces unexpected backend exit errors.
// Only receives if the backend crashes; not used during normal Stop().
func (m *Manager) Errors() <-chan error { return m.errCh }

// Log returns the ring buffer of backend stdout+stderr.
func (m *Manager) Log() *RingBuffer { return m.logBuf }

// Stop gracefully terminates the backend. Sends SIGINT (or Kill on Windows),
// waits up to 3s, then forces Kill. Safe to call multiple times.
func (m *Manager) Stop(ctx context.Context) error {
	m.mu.Lock()
	if m.stopped {
		m.mu.Unlock()
		return nil
	}
	m.stopped = true
	cmd := m.cmd
	m.mu.Unlock()

	if cmd == nil || cmd.Process == nil {
		return nil
	}

	if err := signalStop(cmd.Process.Pid); err != nil {
		m.logBuf.Append("[desktop-gtk] signal error: " + err.Error())
	}

	// Wait for the process to exit, or force-kill after 3s
	done := make(chan struct{})
	go func() {
		<-m.stoppedCh
		close(done)
	}()

	timeout := time.After(3 * time.Second)
	select {
	case <-done:
		return nil
	case <-timeout:
		_ = cmd.Process.Kill()
		<-done
		return nil
	case <-ctx.Done():
		_ = cmd.Process.Kill()
		return ctx.Err()
	}
}

// scanStdout reads lines from the backend's stdout, writes each to the ring
// buffer, and forwards `BACKEND_PORT:<n>` lines to the port channel.
func (m *Manager) scanStdout(r io.Reader) {
	log.Printf("scanStdout: starting")
	scanner := bufio.NewScanner(r)
	// Allow long lines (e.g. multipart base64)
	scanner.Buffer(make([]byte, 0, 64*1024), 1024*1024)
	for scanner.Scan() {
		line := scanner.Text()
		log.Printf("backend stdout: %s", line)
		m.logBuf.Append(line)
		if strings.HasPrefix(line, "BACKEND_PORT:") {
			portStr := strings.TrimSpace(strings.TrimPrefix(line, "BACKEND_PORT:"))
			port, err := strconv.Atoi(portStr)
			if err == nil {
				log.Printf("backend port discovered (in scanStdout): %d", port)
				select {
				case m.portCh <- port:
				default:
				}
			}
		}
	}
	if err := scanner.Err(); err != nil && !errors.Is(err, io.EOF) {
		log.Printf("stdout scanner error: %v", err)
		m.logBuf.Append("[desktop-gtk] stdout scanner error: " + err.Error())
	}
}

func (m *Manager) databasePath() string {
	return m.userData + "/database"
}

func (m *Manager) mediaPath() string {
	return m.userData + "/media"
}

// buildEnv composes the env vars passed to the backend.
func buildEnv(userData string) []string {
	env := append(os.Environ(),
		"PORT=:0",           // bind on a free port
		"ALLOWED_ORIGINS=*", // desktop app is local; allow all
		"DB_PATH="+userData+"/database",
		"MEDIA_PATH="+userData+"/media",
		"TZ=Asia/Jakarta",
	)
	return env
}

func errString(err error) string {
	if err == nil {
		return "exit code 0"
	}
	return err.Error()
}

// keep runtime referenced for platform-specific code paths
var _ = runtime.GOOS

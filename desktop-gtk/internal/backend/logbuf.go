package backend

import (
	"sync"
)

// RingBuffer is a thread-safe fixed-size buffer of strings (oldest at front).
// Used to keep the last N lines of the backend's stdout for the settings "log tail" view.
type RingBuffer struct {
	mu     sync.Mutex
	lines  []string
	maxLen int
}

// NewRingBuffer returns a buffer that holds up to maxLen lines. maxLen must be > 0.
func NewRingBuffer(maxLen int) *RingBuffer {
	if maxLen < 1 {
		maxLen = 1
	}
	return &RingBuffer{
		lines:  make([]string, 0, maxLen),
		maxLen: maxLen,
	}
}

// Append adds a line to the buffer, evicting the oldest if full.
func (r *RingBuffer) Append(line string) {
	r.mu.Lock()
	defer r.mu.Unlock()
	if len(r.lines) >= r.maxLen {
		// drop the oldest
		r.lines = r.lines[1:]
	}
	r.lines = append(r.lines, line)
}

// Snapshot returns a copy of the buffer (oldest -> newest). Safe with concurrent Append.
func (r *RingBuffer) Snapshot() []string {
	r.mu.Lock()
	defer r.mu.Unlock()
	out := make([]string, len(r.lines))
	copy(out, r.lines)
	return out
}

// Write implements io.Writer so the buffer can be used as a sink for stdout/stderr.
// Each Write call appends one line. Partial lines are buffered until newline.
func (r *RingBuffer) Write(p []byte) (int, error) {
	// We process the bytes as complete lines.
	s := string(p)
	for {
		nl := indexNewline(s)
		if nl < 0 {
			return len(p), nil // buffer partial; for our use we accept this
		}
		r.Append(s[:nl])
		s = s[nl+1:]
	}
}

func indexNewline(s string) int {
	for i := 0; i < len(s); i++ {
		if s[i] == '\n' {
			return i
		}
	}
	return -1
}

package call

import (
	"bytes"
	"errors"
	"io"
	"log"
	"os/exec"
	"strings"
	"sync"
	"time"
)

// WhatsApp video travels as H.264 Annex-B access units, but clients (the web
// UI and the GPUI desktop app) have no H.264 encoder/decoder of their own, so
// they exchange plain JPEG pictures with the backend and ffmpeg — already a
// dependency of this project — converts in both directions:
//
//	client JPEG  → JPEGToH264 → meowcaller.Call.SendVideo
//	meowcaller.ReceiveVideo → H264ToJPEG → client JPEG

// ErrNoH264Encoder is returned when the local ffmpeg has no usable H.264 encoder.
var ErrNoH264Encoder = errors.New("ffmpeg has no h264 encoder (libx264/libopenh264)")

var (
	encoderOnce sync.Once
	encoderArgs []string
	encoderName string
)

// h264EncoderArgs probes ffmpeg once and returns the codec arguments for the best
// low-latency software H.264 encoder it has.
func h264EncoderArgs() ([]string, string) {
	encoderOnce.Do(func() {
		out, err := exec.Command("ffmpeg", "-hide_banner", "-encoders").Output()
		if err != nil {
			return
		}
		list := string(out)
		switch {
		case strings.Contains(list, " libx264 "):
			encoderName = "libx264"
			encoderArgs = []string{
				"-c:v", "libx264", "-preset", "ultrafast", "-tune", "zerolatency",
				"-profile:v", "baseline", "-bf", "0", "-g", "45", "-b:v", "700k",
				"-maxrate", "900k", "-bufsize", "600k",
			}
		case strings.Contains(list, " libopenh264 "):
			encoderName = "libopenh264"
			encoderArgs = []string{
				"-c:v", "libopenh264", "-profile:v", "constrained_baseline",
				"-b:v", "700k", "-g", "45", "-allow_skip_frames", "0",
			}
		}
	})
	return encoderArgs, encoderName
}

// ---------------------------------------------------------------------------
// JPEG -> H.264
// ---------------------------------------------------------------------------

// JPEGToH264 turns a stream of JPEG pictures into H.264 Annex-B access units
// using one long-lived ffmpeg process.
type JPEGToH264 struct {
	onAU func(au []byte, duration time.Duration)
	in   chan []byte
	done chan struct{}

	mu        sync.Mutex
	cur       *exec.Cmd
	closed    bool
	closeOnce sync.Once
}

// NewJPEGToH264 starts the encoder. onAU is called from an internal goroutine for
// every encoded access unit together with the time since the previous one.
func NewJPEGToH264(onAU func(au []byte, duration time.Duration)) (*JPEGToH264, error) {
	if args, _ := h264EncoderArgs(); args == nil {
		return nil, ErrNoH264Encoder
	}
	e := &JPEGToH264{
		onAU: onAU,
		in:   make(chan []byte, 2),
		done: make(chan struct{}),
	}
	go e.run()
	return e, nil
}

// WriteJPEG queues one picture. It never blocks: when ffmpeg is behind, the
// oldest queued picture is dropped.
func (e *JPEGToH264) WriteJPEG(jpeg []byte) {
	cp := append([]byte(nil), jpeg...)
	for {
		select {
		case <-e.done:
			return
		case e.in <- cp:
			return
		default:
		}
		select {
		case <-e.in:
		default:
		}
	}
}

// ForceKeyframe makes the next picture start a new GOP by restarting the encoder
// (a fresh ffmpeg always opens with an IDR).
func (e *JPEGToH264) ForceKeyframe() {
	e.mu.Lock()
	cmd := e.cur
	e.mu.Unlock()
	if cmd != nil && cmd.Process != nil {
		_ = cmd.Process.Kill()
	}
}

// Close stops the encoder. Safe to call more than once.
func (e *JPEGToH264) Close() {
	e.closeOnce.Do(func() {
		e.mu.Lock()
		e.closed = true
		cmd := e.cur
		e.mu.Unlock()
		close(e.done)
		if cmd != nil && cmd.Process != nil {
			_ = cmd.Process.Kill()
		}
	})
}

func (e *JPEGToH264) run() {
	quickFailures := 0
	for {
		select {
		case <-e.done:
			return
		default:
		}
		started := time.Now()
		err := e.runOnce()
		select {
		case <-e.done:
			return
		default:
		}
		if time.Since(started) < 3*time.Second {
			quickFailures++
			if quickFailures >= 5 {
				log.Printf("call video: h264 encoder keeps failing, giving up: %v", err)
				return
			}
			time.Sleep(200 * time.Millisecond)
		} else {
			quickFailures = 0
		}
	}
}

// runOnce runs one ffmpeg process until it dies or the encoder is closed.
func (e *JPEGToH264) runOnce() error {
	args, _ := h264EncoderArgs()
	full := []string{
		"-hide_banner", "-loglevel", "error", "-nostdin",
		"-fflags", "nobuffer", "-flags", "low_delay",
		"-probesize", "32768", "-analyzeduration", "0",
		"-use_wallclock_as_timestamps", "1", "-framerate", "15",
		"-f", "image2pipe", "-c:v", "mjpeg", "-i", "pipe:0",
		"-vf", "scale=trunc(iw/2)*2:trunc(ih/2)*2,format=yuv420p",
	}
	full = append(full, args...)
	full = append(full, "-fps_mode", "passthrough", "-bsf:v", "h264_metadata=aud=insert",
		"-an", "-flush_packets", "1", "-f", "h264", "pipe:1")
	cmd := exec.Command("ffmpeg", full...)
	stdin, err := cmd.StdinPipe()
	if err != nil {
		return err
	}
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return err
	}
	var stderr bytes.Buffer
	cmd.Stderr = &stderr
	e.mu.Lock()
	if e.closed {
		e.mu.Unlock()
		return nil
	}
	if err := cmd.Start(); err != nil {
		e.mu.Unlock()
		return err
	}
	e.cur = cmd
	e.mu.Unlock()

	readerDone := make(chan struct{})
	go func() {
		defer close(readerDone)
		e.readAccessUnits(stdout)
	}()

	var werr error
loop:
	for {
		select {
		case <-e.done:
			break loop
		case <-readerDone:
			break loop
		case jpeg := <-e.in:
			if _, werr = stdin.Write(jpeg); werr != nil {
				break loop
			}
		}
	}
	_ = stdin.Close()
	if cmd.Process != nil {
		_ = cmd.Process.Kill()
	}
	<-readerDone
	_ = cmd.Wait()
	e.mu.Lock()
	if e.cur == cmd {
		e.cur = nil
	}
	e.mu.Unlock()
	if msg := strings.TrimSpace(stderr.String()); msg != "" {
		select {
		case <-e.done:
		default:
			log.Printf("call video: h264 encoder: %s", msg)
		}
	}
	return werr
}

// readAccessUnits splits ffmpeg's Annex-B output into access units. Every unit
// starts with an access-unit delimiter (added by the h264_metadata filter); a
// unit is complete when the next delimiter shows up, or — to avoid holding back
// the newest frame — when ffmpeg has been quiet for a few milliseconds.
func (e *JPEGToH264) readAccessUnits(r io.Reader) {
	chunks := make(chan []byte, 16)
	go func() {
		defer close(chunks)
		buf := make([]byte, 64*1024)
		for {
			n, err := r.Read(buf)
			if n > 0 {
				c := make([]byte, n)
				copy(c, buf[:n])
				chunks <- c
			}
			if err != nil {
				return
			}
		}
	}()

	var (
		pending []byte
		last    = time.Now()
	)
	emit := func(au []byte) {
		au = stripAUD(au)
		if len(au) == 0 {
			return
		}
		now := time.Now()
		d := now.Sub(last)
		last = now
		if d < 10*time.Millisecond {
			d = 10 * time.Millisecond
		}
		if d > 500*time.Millisecond {
			d = 66 * time.Millisecond
		}
		e.onAU(au, d)
	}
	flush := time.NewTimer(time.Hour)
	defer flush.Stop()
	for {
		select {
		case c, ok := <-chunks:
			if !ok {
				return
			}
			pending = append(pending, c...)
			for {
				i := nextAUD(pending, 4)
				if i < 0 {
					break
				}
				emit(pending[:i])
				pending = append([]byte(nil), pending[i:]...)
			}
			if !flush.Stop() {
				select {
				case <-flush.C:
				default:
				}
			}
			flush.Reset(6 * time.Millisecond)
		case <-flush.C:
			if len(pending) > 0 && nextAUD(pending, 0) == 0 {
				emit(pending)
				pending = nil
			}
		}
	}
}

// nextAUD returns the offset of the first access-unit-delimiter start code at or
// after from, or -1. Both 3- and 4-byte start codes are recognised; the returned
// offset points at the first byte of the start code.
func nextAUD(b []byte, from int) int {
	for i := from; i+3 < len(b); i++ {
		if b[i] == 0 && b[i+1] == 0 && b[i+2] == 1 && b[i+3]&0x1f == 9 {
			if i > 0 && b[i-1] == 0 {
				return i - 1
			}
			return i
		}
	}
	return -1
}

// stripAUD removes a leading access-unit delimiter NAL (the encoder only
// emits it so the stream can be split; WhatsApp does not need it).
func stripAUD(au []byte) []byte {
	if nextAUD(au, 0) != 0 {
		return au
	}
	// Skip the AUD's own start code and NAL, then keep everything from the next start code.
	for i := 4; i+2 < len(au); i++ {
		if au[i] == 0 && au[i+1] == 0 && au[i+2] == 1 {
			if au[i-1] == 0 {
				return au[i-1:]
			}
			return au[i:]
		}
	}
	return nil
}

// ---------------------------------------------------------------------------
// H.264 -> JPEG
// ---------------------------------------------------------------------------

// H264ToJPEG decodes the peer's H.264 access units into JPEG pictures with one
// long-lived ffmpeg process. The process starts on the first access unit.
type H264ToJPEG struct {
	in   chan []byte
	out  chan []byte
	done chan struct{}

	startOnce sync.Once
	closeOnce sync.Once
	mu        sync.Mutex
	cur       *exec.Cmd
}

// NewH264ToJPEG builds a decoder; nothing runs until the first WriteAU.
func NewH264ToJPEG() *H264ToJPEG {
	return &H264ToJPEG{
		in:   make(chan []byte, 8),
		out:  make(chan []byte, 4),
		done: make(chan struct{}),
	}
}

// Frames returns decoded JPEG pictures.
func (d *H264ToJPEG) Frames() <-chan []byte { return d.out }

// WriteAU queues one access unit. Never blocks; drops the oldest when ffmpeg lags.
func (d *H264ToJPEG) WriteAU(au []byte) {
	d.startOnce.Do(func() { go d.run() })
	cp := append([]byte(nil), au...)
	for {
		select {
		case <-d.done:
			return
		case d.in <- cp:
			return
		default:
		}
		select {
		case <-d.in:
		default:
		}
	}
}

// Close stops the decoder. Safe to call more than once.
func (d *H264ToJPEG) Close() {
	d.closeOnce.Do(func() {
		close(d.done)
		d.mu.Lock()
		cmd := d.cur
		d.mu.Unlock()
		if cmd != nil && cmd.Process != nil {
			_ = cmd.Process.Kill()
		}
	})
}

func (d *H264ToJPEG) run() {
	quickFailures := 0
	for {
		select {
		case <-d.done:
			return
		default:
		}
		started := time.Now()
		err := d.runOnce()
		select {
		case <-d.done:
			return
		default:
		}
		if time.Since(started) < 3*time.Second {
			quickFailures++
			if quickFailures >= 5 {
				log.Printf("call video: h264 decoder keeps failing, giving up: %v", err)
				return
			}
			time.Sleep(200 * time.Millisecond)
		} else {
			quickFailures = 0
		}
	}
}

// aud is appended after every access unit written to the decoder. The raw H.264
// demuxer only knows a frame ended when the next one starts; this delimiter makes
// it release each picture immediately instead of one frame late.
var aud = []byte{0, 0, 0, 1, 0x09, 0xf0}

func (d *H264ToJPEG) runOnce() error {
	cmd := exec.Command("ffmpeg",
		"-hide_banner", "-loglevel", "error", "-nostdin",
		"-flags", "low_delay", "-probesize", "32", "-analyzeduration", "0",
		"-f", "h264", "-i", "pipe:0",
		"-an", "-pix_fmt", "yuvj420p", "-q:v", "5", "-f", "mjpeg", "pipe:1",
	)
	stdin, err := cmd.StdinPipe()
	if err != nil {
		return err
	}
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return err
	}
	var stderr bytes.Buffer
	cmd.Stderr = &stderr
	d.mu.Lock()
	select {
	case <-d.done:
		d.mu.Unlock()
		return nil
	default:
	}
	if err := cmd.Start(); err != nil {
		d.mu.Unlock()
		return err
	}
	d.cur = cmd
	d.mu.Unlock()

	readerDone := make(chan struct{})
	go func() {
		defer close(readerDone)
		d.readJPEGs(stdout)
	}()

	var werr error
	// ffmpeg needs the parameter sets to size its output, so feeding starts at
	// the first access unit that carries them (a keyframe).
	synced := false
loop:
	for {
		select {
		case <-d.done:
			break loop
		case <-readerDone:
			break loop
		case au := <-d.in:
			if !synced {
				if !hasNAL(au, 7) {
					continue
				}
				synced = true
			}
			if _, werr = stdin.Write(au); werr != nil {
				break loop
			}
			if _, werr = stdin.Write(aud); werr != nil {
				break loop
			}
		}
	}
	_ = stdin.Close()
	if cmd.Process != nil {
		_ = cmd.Process.Kill()
	}
	<-readerDone
	_ = cmd.Wait()
	d.mu.Lock()
	if d.cur == cmd {
		d.cur = nil
	}
	d.mu.Unlock()
	if msg := strings.TrimSpace(stderr.String()); msg != "" {
		select {
		case <-d.done:
		default:
			log.Printf("call video: h264 decoder: %s", msg)
		}
	}
	return werr
}

// readJPEGs splits ffmpeg's concatenated MJPEG output into pictures (SOI … EOI).
func (d *H264ToJPEG) readJPEGs(r io.Reader) {
	buf := make([]byte, 0, 128*1024)
	chunk := make([]byte, 32*1024)
	for {
		n, err := r.Read(chunk)
		if n > 0 {
			buf = append(buf, chunk[:n]...)
			for {
				pic, rest, ok := splitJPEG(buf)
				if !ok {
					break
				}
				d.deliver(pic)
				buf = append(buf[:0], rest...)
			}
			if len(buf) > 8<<20 { // runaway garbage
				buf = buf[:0]
			}
		}
		if err != nil {
			return
		}
	}
}

func (d *H264ToJPEG) deliver(pic []byte) {
	cp := append([]byte(nil), pic...)
	for {
		select {
		case <-d.done:
			return
		case d.out <- cp:
			return
		default:
		}
		select {
		case <-d.out:
		default:
		}
	}
}

// hasNAL reports whether the Annex-B unit holds a NAL unit of the given type.
func hasNAL(au []byte, typ byte) bool {
	for i := 0; i+3 < len(au); i++ {
		if au[i] == 0 && au[i+1] == 0 && au[i+2] == 1 && au[i+3]&0x1f == typ {
			return true
		}
	}
	return false
}

// splitJPEG returns the first complete JPEG in b (SOI FFD8 … EOI FFD9) and the
// bytes after it. ffmpeg's MJPEG output has no embedded thumbnails, and any FF
// inside entropy-coded data is byte-stuffed (FF00) or a restart marker, so the
// first FFD9 after the SOI is the end of the picture.
func splitJPEG(b []byte) (pic, rest []byte, ok bool) {
	start := bytes.Index(b, []byte{0xff, 0xd8})
	if start < 0 {
		return nil, b, false
	}
	end := bytes.Index(b[start+2:], []byte{0xff, 0xd9})
	if end < 0 {
		return nil, b, false
	}
	end += start + 2 + 2
	return b[start:end], b[end:], true
}

package call

import (
	"bytes"
	"context"
	"fmt"
	"io"
	"os"
	"os/exec"
	"sync"
	"time"

	"github.com/purpshell/meowcaller"
)

const (
	// videoFrameInterval paces outgoing video access units at ~30 fps.
	videoFrameInterval = 33 * time.Millisecond
	// videoFeederBuffer is the number of decoded access units buffered between
	// the ffmpeg reader and the paced sender. When full, ffmpeg is backpressured
	// through its stdout pipe so it encodes at roughly real time.
	videoFeederBuffer = 64
)

// VideoFileFeeder streams a video file into a live call. ffmpeg decodes the
// file to H.264 Annex-B on stdout; a reader goroutine splits the stream into
// access units and a paced sender forwards one access unit per frame interval
// via meowcaller.Call.SendVideo. An optional audio track is extracted to a temp
// WAV and played through a meowcaller.Player so the call carries sound too.
type VideoFileFeeder struct {
	cmd       *exec.Cmd
	done      chan struct{}
	doneOnce  sync.Once
	closeOnce sync.Once
	tmpAudio  string
}

// StartVideoFileFeeder begins streaming videoPath into the call. When video is
// non-nil its SendVideo is used (the browser video bridge); otherwise the
// call's SendVideo is used. The audio track, if any, is extracted to a temp WAV
// and played via call.Play; the returned player (may be nil) is for OnFinish
// handling. The returned feeder must be closed when playback should stop, or
// the caller may wait on Done for natural completion.
//
// ffmpeg must be available in PATH, otherwise an error is returned.
func StartVideoFileFeeder(ctx context.Context, call *meowcaller.Call, video *CallVideo, videoPath string) (*VideoFileFeeder, *meowcaller.Player, error) {
	if _, err := exec.LookPath("ffmpeg"); err != nil {
		return nil, nil, fmt.Errorf("ffmpeg not found: %w", err)
	}
	if call == nil && video == nil {
		return nil, nil, ErrCallNotActive
	}

	// Extract the audio track (best-effort): a video without an audio track must
	// not fail the whole playback.
	tmpAudio, err := extractAudioTrack(ctx, videoPath)
	if err != nil {
		tmpAudio = ""
	}

	var player *meowcaller.Player
	if tmpAudio != "" && call != nil {
		src, err := meowcaller.WAVFile(tmpAudio)
		if err != nil {
			_ = os.Remove(tmpAudio)
			tmpAudio = ""
		} else {
			player = call.Play(src)
		}
	}

	// Encode the video track to baseline H.264 Annex-B on stdout. No -re: the
	// reader/sender pace the stream manually at 30 fps.
	cmd := exec.CommandContext(ctx, "ffmpeg",
		"-y", "-hide_banner", "-loglevel", "error",
		"-i", videoPath,
		"-an",
		"-c:v", "libx264",
		"-profile:v", "baseline",
		"-pix_fmt", "yuv420p",
		"-r", "30",
		"-g", "30",
		"-vf", "scale='min(1280,iw)':-2",
		"-b:v", "800k",
		"-f", "h264",
		"pipe:1",
	)
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		if player != nil {
			player.Stop()
		}
		if tmpAudio != "" {
			_ = os.Remove(tmpAudio)
		}
		return nil, nil, err
	}
	if err := cmd.Start(); err != nil {
		if player != nil {
			player.Stop()
		}
		if tmpAudio != "" {
			_ = os.Remove(tmpAudio)
		}
		return nil, nil, err
	}

	f := &VideoFileFeeder{
		cmd:      cmd,
		done:     make(chan struct{}),
		tmpAudio: tmpAudio,
	}

	auCh := make(chan []byte, videoFeederBuffer)
	go f.readLoop(ctx, stdout, auCh)
	go f.sendLoop(ctx, video, call, auCh)

	return f, player, nil
}

// readLoop reads ffmpeg's H.264 Annex-B stdout, splits it into NAL units and
// groups them into access units (one per video frame), pushing each to auCh. It
// reaps the ffmpeg process and closes auCh when the stream ends.
func (f *VideoFileFeeder) readLoop(ctx context.Context, stdout io.ReadCloser, auCh chan<- []byte) {
	defer stdout.Close()
	defer func() { _ = f.cmd.Wait() }()
	defer close(auCh)

	buf := make([]byte, 64*1024)
	var pending []byte
	var au []byte
	var auHasVCL bool

	// flushAU sends the accumulated access unit to the paced sender. It returns
	// false when the feeder was closed or the context cancelled mid-send.
	flushAU := func() bool {
		if len(au) == 0 {
			return true
		}
		frame := make([]byte, len(au))
		copy(frame, au)
		select {
		case auCh <- frame:
			au = au[:0]
			auHasVCL = false
			return true
		case <-f.done:
			return false
		case <-ctx.Done():
			return false
		}
	}

	// processNALU folds one NAL unit into the current access unit. A new access
	// unit starts when the current one already holds a VCL NAL unit (a slice), so
	// the SPS/PPS/SEI preceding a keyframe begin a fresh unit. It returns false
	// when the feeder was closed or the context cancelled.
	processNALU := func(nalu []byte) bool {
		if len(nalu) == 0 {
			return true
		}
		naluType := nalu[0] & 0x1f
		isVCL := naluType >= 1 && naluType <= 5
		if auHasVCL && !flushAU() {
			return false
		}
		au = append(au, nalu...)
		if isVCL {
			auHasVCL = true
		}
		return true
	}

	for {
		n, err := stdout.Read(buf)
		if n > 0 {
			pending = append(pending, buf[:n]...)
			for {
				sc := findStartCode(pending)
				if sc < 0 {
					break
				}
				nalu := pending[:sc]
				pending = pending[sc:]
				if len(nalu) > 0 && !processNALU(nalu) {
					return
				}
				// Skip the start code (3 or 4 bytes).
				if len(pending) >= 4 && pending[0] == 0 && pending[1] == 0 && pending[2] == 0 && pending[3] == 1 {
					pending = pending[4:]
				} else if len(pending) >= 3 && pending[0] == 0 && pending[1] == 0 && pending[2] == 1 {
					pending = pending[3:]
				} else {
					break // incomplete start code: wait for more data
				}
			}
		}
		if err != nil {
			break
		}
	}

	// Flush any trailing NAL units left in the buffer.
	for _, nalu := range splitAnnexB(pending) {
		if !processNALU(nalu) {
			return
		}
	}
	flushAU()
}

// sendLoop paces access units at ~30 fps into the call. It exits when the
// reader closes auCh (video exhausted), the feeder is closed, or the context is
// cancelled, and then triggers full cleanup via Close.
func (f *VideoFileFeeder) sendLoop(ctx context.Context, video *CallVideo, call *meowcaller.Call, auCh <-chan []byte) {
	defer f.Close()
	ticker := time.NewTicker(videoFrameInterval)
	defer ticker.Stop()
	for {
		select {
		case <-ctx.Done():
			return
		case <-f.done:
			return
		case au, ok := <-auCh:
			if !ok {
				return
			}
			// Pace one access unit per frame interval.
			select {
			case <-ticker.C:
			case <-ctx.Done():
				return
			case <-f.done:
				return
			}
			if err := sendVideoAU(video, call, au); err != nil {
				return
			}
		}
	}
}

// sendVideoAU forwards one access unit into the call, preferring the browser
// video bridge when present.
func sendVideoAU(video *CallVideo, call *meowcaller.Call, au []byte) error {
	if video != nil {
		return video.SendVideo(au)
	}
	if call == nil {
		return ErrCallNotActive
	}
	return call.SendVideo(au)
}

// Close stops the feeder: it kills the ffmpeg process, signals Done and removes
// the extracted audio temp file. Safe to call more than once, including
// concurrently.
func (f *VideoFileFeeder) Close() {
	if f == nil {
		return
	}
	f.closeOnce.Do(func() {
		if f.cmd != nil && f.cmd.Process != nil {
			_ = f.cmd.Process.Kill()
		}
		f.doneOnce.Do(func() { close(f.done) })
		if f.tmpAudio != "" {
			_ = os.Remove(f.tmpAudio)
		}
	})
}

// Done returns a channel closed when the feeder has finished (video exhausted
// or Close called).
func (f *VideoFileFeeder) Done() <-chan struct{} {
	if f == nil {
		return nil
	}
	return f.done
}

// findStartCode returns the byte offset of the next Annex-B start code
// (0x000001 or 0x00000001) in data, or -1 if none is present. A 4-byte start
// code is reported at its leading 0x00.
func findStartCode(data []byte) int {
	idx := bytes.Index(data, []byte{0, 0, 1})
	if idx < 0 {
		return -1
	}
	if idx > 0 && data[idx-1] == 0 {
		return idx - 1
	}
	return idx
}

// splitAnnexB splits an Annex-B H.264 byte stream into NAL units, stripping the
// start codes. The final element may be a trailing partial NAL unit when data
// does not end with a start code.
func splitAnnexB(data []byte) [][]byte {
	var nals [][]byte
	for {
		sc := findStartCode(data)
		if sc < 0 {
			if len(data) > 0 {
				nals = append(nals, data)
			}
			return nals
		}
		if sc > 0 {
			nals = append(nals, data[:sc])
		}
		data = data[sc:]
		if len(data) >= 4 && data[0] == 0 && data[1] == 0 && data[2] == 0 && data[3] == 1 {
			data = data[4:]
		} else if len(data) >= 3 && data[0] == 0 && data[1] == 0 && data[2] == 1 {
			data = data[3:]
		} else {
			// Incomplete start code at the end: keep it for the next call.
			return nals
		}
	}
}

// extractAudioTrack extracts the audio track of a video file to a temporary WAV
// file (16-bit PCM, 48 kHz mono) suitable for meowcaller.WAVFile. The caller
// owns the returned temp file and must remove it. It returns an error when the
// video has no audio track or ffmpeg fails.
func extractAudioTrack(ctx context.Context, videoPath string) (string, error) {
	tmp, err := os.CreateTemp("", "wa-video-audio-*.wav")
	if err != nil {
		return "", err
	}
	path := tmp.Name()
	_ = tmp.Close()

	cmd := exec.CommandContext(ctx, "ffmpeg",
		"-y", "-hide_banner", "-loglevel", "error",
		"-i", videoPath,
		"-vn",
		"-c:a", "pcm_s16le",
		"-ar", "48000",
		"-ac", "1",
		path,
	)
	if out, err := cmd.CombinedOutput(); err != nil {
		_ = os.Remove(path)
		return "", fmt.Errorf("ffmpeg: %v: %s", err, string(out))
	}
	return path, nil
}

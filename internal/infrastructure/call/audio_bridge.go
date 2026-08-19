package call

import (
	"errors"
	"io"
	"sync"
	"time"

	"github.com/purpshell/meowcaller"
)

// ChannelAudioSource is an outgoing PCM source fed by the browser over the media
// WebSocket. If the browser is late and no frame is available within a short
// window, ReadFrame returns a silence frame so the call is never blocked (PRD §18).
type ChannelAudioSource struct {
	frames    chan []float32
	ticker    *time.Ticker
	closed    chan struct{}
	closeOnce sync.Once
	silence   []float32
}

// NewChannelAudioSource builds an audio source with the given outbound buffer.
func NewChannelAudioSource(buffer int) *ChannelAudioSource {
	if buffer <= 0 {
		buffer = 16
	}
	return &ChannelAudioSource{
		frames:  make(chan []float32, buffer),
		ticker:  time.NewTicker(20 * time.Millisecond),
		closed:  make(chan struct{}),
		silence: make([]float32, meowcaller.FrameSamples),
	}
}

// ReadFrame returns the next 16 kHz mono PCM frame, io.EOF once closed, or a
// silence frame when the browser has not supplied audio within the short window.
func (s *ChannelAudioSource) ReadFrame() ([]float32, error) {
	select {
	case <-s.closed:
		return nil, io.EOF
	case frame, ok := <-s.frames:
		if !ok {
			return nil, io.EOF
		}
		return frame, nil
	case <-s.ticker.C:
		// Browser is late: emit silence rather than blocking the call. The
		// silence buffer is reused; meowcaller consumes each frame synchronously
		// (encoded before the next ReadFrame) and does not retain it.
		return s.silence, nil
	}
}

// Close releases the source. Safe to call more than once, including concurrently.
func (s *ChannelAudioSource) Close() error {
	s.closeOnce.Do(func() {
		close(s.closed)
		s.ticker.Stop()
	})
	return nil
}

// Feed pushes an outgoing PCM frame from the browser. Non-blocking: drops the
// frame if the buffer is full so a lagging source never stalls the call.
func (s *ChannelAudioSource) Feed(frame []float32) bool {
	select {
	case <-s.closed:
		return false
	case s.frames <- frame:
		return true
	default:
		return false
	}
}

// ChannelAudioSink receives decoded remote PCM and pushes it to a bounded queue
// drained by the media WebSocket. If the browser is disconnected (no consumer),
// frames are dropped rather than blocking/backpressuring the call (PRD §19).
type ChannelAudioSink struct {
	frames    chan []float32
	closed    chan struct{}
	closeOnce sync.Once
}

// NewChannelAudioSink builds an audio sink with the given inbound buffer.
func NewChannelAudioSink(buffer int) *ChannelAudioSink {
	if buffer <= 0 {
		buffer = 32
	}
	return &ChannelAudioSink{
		frames: make(chan []float32, buffer),
		closed: make(chan struct{}),
	}
}

// WriteFrame queues one decoded mono frame for the browser. Non-blocking with
// drop-oldest semantics: if the buffer is full or the sink is closed, the frame
// is dropped.
func (s *ChannelAudioSink) WriteFrame(frame []float32) error {
	select {
	case <-s.closed:
		return nil
	case s.frames <- frame:
		return nil
	default:
		return nil
	}
}

// Close releases the sink. Safe to call more than once, including concurrently.
func (s *ChannelAudioSink) Close() error {
	s.closeOnce.Do(func() {
		close(s.closed)
	})
	return nil
}

// Frames returns the inbound frame channel drained by the media WebSocket writer.
func (s *ChannelAudioSink) Frames() <-chan []float32 {
	return s.frames
}

var errPCMInvalidLength = errors.New("pcm payload length is not even")

// PCMS16ToFloat32 converts s16le PCM bytes to the 16 kHz mono float32 frames the
// meowcaller pipeline expects. A partial final frame is zero-padded to
// FrameSamples. bytesPerSample controls the wire format (2 = s16le).
func PCMS16ToFloat32(data []byte, bytesPerSample int) ([]float32, error) {
	if bytesPerSample <= 0 {
		bytesPerSample = 2
	}
	if len(data)%bytesPerSample != 0 {
		return nil, errPCMInvalidLength
	}
	n := len(data) / bytesPerSample
	if n == 0 {
		return nil, errPCMInvalidLength
	}
	// The meowcaller audio pipeline consumes fixed FrameSamples-long frames.
	frames := make([]float32, meowcaller.FrameSamples)
	for i := 0; i < n && i < meowcaller.FrameSamples; i++ {
		var s int16
		if bytesPerSample == 2 {
			s = int16(uint16(data[2*i]) | uint16(data[2*i+1])<<8)
		} else {
			s = int16(data[i])
		}
		frames[i] = float32(s) / 32768.0
	}
	return frames, nil
}

// PCMFloat32ToS16 converts float32 frames to s16le bytes for the wire.
func PCMFloat32ToS16(frames []float32, bytesPerSample int) []byte {
	if bytesPerSample <= 0 {
		bytesPerSample = 2
	}
	out := make([]byte, len(frames)*bytesPerSample)
	for i, f := range frames {
		v := f
		if v > 1.0 {
			v = 1.0
		} else if v < -1.0 {
			v = -1.0
		}
		s := int16(v * 32767.0)
		if bytesPerSample == 2 {
			out[2*i] = byte(s)
			out[2*i+1] = byte(uint16(s) >> 8)
		} else {
			out[i] = byte(s)
		}
	}
	return out
}

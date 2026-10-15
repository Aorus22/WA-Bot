package call

import (
	"errors"
	"io"
	"sync"

	"github.com/purpshell/meowcaller"
)

// ChannelAudioSource is an outgoing PCM source fed by a client over the media
// WebSocket. meowcaller pulls one 60 ms frame on its own send clock, so the
// source never blocks: when the client has not delivered a frame in time it
// returns silence. The queue is short and drops its oldest frame when full, so
// a client running slightly fast (or a stall followed by a burst) cannot build
// up a lasting delay.
type ChannelAudioSource struct {
	mu      sync.Mutex
	queue   [][]float32
	max     int
	closed  bool
	silence []float32
}

// NewChannelAudioSource builds an audio source holding at most `buffer` frames
// (each 60 ms).
func NewChannelAudioSource(buffer int) *ChannelAudioSource {
	if buffer <= 0 {
		buffer = 3
	}
	return &ChannelAudioSource{
		max:     buffer,
		silence: make([]float32, meowcaller.FrameSamples),
	}
}

// ReadFrame returns the oldest queued frame, a silence frame when none is
// waiting, or io.EOF once closed. The silence buffer is reused; meowcaller
// encodes each frame before the next ReadFrame and does not retain it.
func (s *ChannelAudioSource) ReadFrame() ([]float32, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.closed {
		return nil, io.EOF
	}
	if len(s.queue) == 0 {
		return s.silence, nil
	}
	frame := s.queue[0]
	s.queue[0] = nil
	s.queue = s.queue[1:]
	return frame, nil
}

// Close releases the source. Safe to call more than once, including concurrently.
func (s *ChannelAudioSource) Close() error {
	s.mu.Lock()
	s.closed = true
	s.queue = nil
	s.mu.Unlock()
	return nil
}

// Feed queues an outgoing PCM frame from the client. It never blocks: when the
// queue is full the oldest frame is discarded to keep latency bounded. Returns
// false only if the source is closed.
func (s *ChannelAudioSource) Feed(frame []float32) bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.closed {
		return false
	}
	if len(s.queue) >= s.max {
		s.queue[0] = nil
		s.queue = s.queue[1:]
	}
	s.queue = append(s.queue, frame)
	return true
}

// ChannelAudioSink receives decoded remote PCM and queues it for the media
// WebSocket writer. When the client is not consuming (or is slow), the oldest
// frames are dropped rather than back-pressuring the call.
type ChannelAudioSink struct {
	frames    chan []float32
	closed    chan struct{}
	closeOnce sync.Once
}

// NewChannelAudioSink builds an audio sink with the given inbound buffer.
func NewChannelAudioSink(buffer int) *ChannelAudioSink {
	if buffer <= 0 {
		buffer = 8
	}
	return &ChannelAudioSink{
		frames: make(chan []float32, buffer),
		closed: make(chan struct{}),
	}
}

// WriteFrame queues one decoded mono frame for the client, discarding the
// oldest queued frame when the buffer is full. meowcaller may reuse the frame
// slice after returning, so it is copied.
func (s *ChannelAudioSink) WriteFrame(frame []float32) error {
	select {
	case <-s.closed:
		return nil
	default:
	}
	cp := make([]float32, len(frame))
	copy(cp, frame)
	for {
		select {
		case s.frames <- cp:
			return nil
		default:
		}
		select {
		case <-s.frames:
		default:
		}
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

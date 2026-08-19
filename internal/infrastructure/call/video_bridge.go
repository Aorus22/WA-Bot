package call

import (
	"sync"

	"github.com/purpshell/meowcaller"
)

// VideoFrame is one H.264 Annex-B access unit destined for the browser.
// ParticipantJID is reserved for group-video attribution (Phase 5); for a 1:1
// call it is empty.
type VideoFrame struct {
	ParticipantJID string
	AccessUnit     []byte
}

// CallVideo is the inbound/outbound H.264 bridge between the browser and a call.
//   - Inbound: meowcaller delivers peer access units which are queued for the
//     media WebSocket writer (frame kind 0x04).
//   - Outbound: the browser sends pre-encoded H.264 access units; SendVideo
//     forwards them into the call via meowcaller.Call.SendVideo.
type CallVideo struct {
	frames    chan VideoFrame
	keyframe  chan struct{}
	closed    chan struct{}
	closeOnce sync.Once
	call      *meowcaller.Call
}

// NewCallVideo builds the video bridge for the given call.
func NewCallVideo(call *meowcaller.Call, buffer int) *CallVideo {
	if buffer <= 0 {
		buffer = 32
	}
	v := &CallVideo{
		frames:   make(chan VideoFrame, buffer),
		keyframe: make(chan struct{}, 1),
		closed:   make(chan struct{}),
		call:     call,
	}
	if call != nil {
		call.ReceiveVideo(v.Sink())
		call.OnVideoKeyframeRequest(func() {
			select {
			case v.keyframe <- struct{}{}:
			default:
				// coalesce: only one pending keyframe request
			}
		})
	}
	return v
}

// Sink returns the meowcaller.VideoSink adapter forwarding inbound access units
// to the bridge queue. Frames are dropped when the queue is full (no consumer).
func (v *CallVideo) Sink() meowcaller.VideoSink {
	return meowcaller.VideoSinkFunc(func(accessUnit []byte) {
		select {
		case <-v.closed:
			return
		case v.frames <- VideoFrame{AccessUnit: accessUnit}:
		default:
			// no consumer: drop
		}
	})
}

// Frames returns the inbound video queue drained by the media WebSocket writer.
func (v *CallVideo) Frames() <-chan VideoFrame {
	return v.frames
}

// KeyframeRequests returns a channel signalled when the call requests an IDR
// from the browser (meowcaller PLI/FIR).
func (v *CallVideo) KeyframeRequests() <-chan struct{} {
	return v.keyframe
}

// SendVideo forwards one pre-encoded H.264 access unit (Annex-B) into the call.
func (v *CallVideo) SendVideo(accessUnit []byte) error {
	if v.call == nil {
		return ErrCallNotActive
	}
	return v.call.SendVideo(accessUnit)
}

// Close releases the video bridge. Safe to call more than once, including
// concurrently.
func (v *CallVideo) Close() {
	v.closeOnce.Do(func() {
		close(v.closed)
	})
}

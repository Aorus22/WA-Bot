package whatsapp

import (
	"sync"

	"go.mau.fi/whatsmeow/types/events"
)

// MediaRetryHub hands the phone's answer to a "upload this media again" request
// to whoever asked. The answer arrives as an event, long after the request.
type MediaRetryHub struct {
	mu      sync.Mutex
	waiters map[string][]chan *events.MediaRetry
}

func NewMediaRetryHub() *MediaRetryHub {
	return &MediaRetryHub{waiters: map[string][]chan *events.MediaRetry{}}
}

// Expect registers for the answer about messageID. Call it before sending the
// request, so a fast answer is not missed, and call cancel when done waiting.
func (h *MediaRetryHub) Expect(messageID string) (answer <-chan *events.MediaRetry, cancel func()) {
	ch := make(chan *events.MediaRetry, 1)
	h.mu.Lock()
	h.waiters[messageID] = append(h.waiters[messageID], ch)
	h.mu.Unlock()
	return ch, func() {
		h.mu.Lock()
		defer h.mu.Unlock()
		rest := h.waiters[messageID][:0]
		for _, w := range h.waiters[messageID] {
			if w != ch {
				rest = append(rest, w)
			}
		}
		if len(rest) == 0 {
			delete(h.waiters, messageID)
		} else {
			h.waiters[messageID] = rest
		}
	}
}

// Deliver passes an answer from the phone to those waiting for its message.
func (h *MediaRetryHub) Deliver(evt *events.MediaRetry) {
	if evt == nil {
		return
	}
	h.mu.Lock()
	waiters := append([]chan *events.MediaRetry(nil), h.waiters[evt.MessageID]...)
	h.mu.Unlock()
	for _, w := range waiters {
		select {
		case w <- evt:
		default: // already has an answer
		}
	}
}

// pending is how many requests are waiting for an answer (for tests).
func (h *MediaRetryHub) pending() int {
	h.mu.Lock()
	defer h.mu.Unlock()
	n := 0
	for _, w := range h.waiters {
		n += len(w)
	}
	return n
}

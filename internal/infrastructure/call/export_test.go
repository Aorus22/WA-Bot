package call

import "time"

// Test hooks for the media WebSocket tests in other packages.

// SetActiveForTest installs a live media bridge (no meowcaller call) as the active call.
func (s *CallService) SetActiveForTest(id string) *CallMedia {
	m := NewCallMedia(nil)
	s.mu.Lock()
	s.active = &CallSession{ID: id, Media: m}
	s.mu.Unlock()
	return m
}

// FeedIncomingAudioForTest plays the role of the peer's decoded audio.
func (m *CallMedia) FeedIncomingAudioForTest(frame []float32) { _ = m.sink.WriteFrame(frame) }

// NextOutgoingAudioForTest is what meowcaller would pull for the peer.
func (m *CallMedia) NextOutgoingAudioForTest() []float32 {
	f, _ := m.source.ReadFrame()
	return append([]float32(nil), f...)
}

// FeedIncomingVideoForTest plays the role of an H.264 access unit from the peer.
func (m *CallMedia) FeedIncomingVideoForTest(au []byte) { _ = m.video.Sink().WriteVideo(au) }

// WaitForForTest is a tiny polling helper.
func WaitForForTest(timeout time.Duration, cond func() bool) bool {
	deadline := time.Now().Add(timeout)
	for time.Now().Before(deadline) {
		if cond() {
			return true
		}
		time.Sleep(5 * time.Millisecond)
	}
	return cond()
}

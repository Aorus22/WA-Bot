package call

import (
	"crypto/rand"
	"crypto/subtle"
	"encoding/base64"
	"errors"
	"sync"
	"time"

	"github.com/purpshell/meowcaller"
)

const (
	// MediaSessionTTL is how long a media-session token remains valid before it
	// expires and a browser must obtain a fresh one (PRD §16).
	MediaSessionTTL = 120 * time.Second
)

var (
	// ErrMediaSessionInvalid indicates the media-session token does not match
	// the active call's session, or the session has already been consumed.
	ErrMediaSessionInvalid = errors.New("media_session_invalid")
	// ErrMediaSessionExpired indicates the media-session token has expired.
	ErrMediaSessionExpired = errors.New("media_session_expired")
)

// MediaSession is a one-shot bearer credential authorizing a browser to attach
// its media to a call over the media WebSocket. It is consumed on first use so
// a second connection to the same call is rejected.
type MediaSession struct {
	Token     string
	CallID    string
	ExpiresAt time.Time
}

// CallMedia ties the audio/video bridges to a single live call and exposes the
// small WS-facing API the media WebSocket handler consumes. Creating it wires
// the meowcaller pipeline: browser audio is played into the call, remote audio
// is received and queued, and video access units flow both ways.
type CallMedia struct {
	source    *ChannelAudioSource
	sink      *ChannelAudioSink
	video     *CallVideo
	done      chan struct{}
	closeOnce sync.Once
}

// NewCallMedia builds the media bridges for a call and attaches them to the
// meowcaller pipeline. It is safe to call with a nil call (e.g. before the
// meowcaller call exists); the bridges simply stay unwired until then.
func NewCallMedia(call *meowcaller.Call) *CallMedia {
	m := &CallMedia{
		source: NewChannelAudioSource(16),
		sink:   NewChannelAudioSink(32),
		video:  NewCallVideo(call, 32),
		done:   make(chan struct{}),
	}
	if call != nil {
		call.Play(m.source)
		call.Receive(m.sink)
		// NewCallVideo already attaches ReceiveVideo + OnVideoKeyframeRequest.
	}
	return m
}

// WriteOutgoingAudio feeds one outgoing PCM frame from the browser into the
// call. Returns false if the frame was dropped (source closed or full).
func (m *CallMedia) WriteOutgoingAudio(frame []float32) bool {
	if m == nil || m.source == nil {
		return false
	}
	return m.source.Feed(frame)
}

// WriteOutgoingVideo forwards one pre-encoded H.264 access unit into the call.
func (m *CallMedia) WriteOutgoingVideo(accessUnit []byte) error {
	if m == nil || m.video == nil {
		return ErrCallNotActive
	}
	return m.video.SendVideo(accessUnit)
}

// IncomingAudio returns the inbound remote-audio frame channel drained by the
// media WebSocket writer.
func (m *CallMedia) IncomingAudio() <-chan []float32 {
	if m == nil || m.sink == nil {
		return nil
	}
	return m.sink.Frames()
}

// IncomingVideo returns the inbound remote-video access-unit channel drained by
// the media WebSocket writer.
func (m *CallMedia) IncomingVideo() <-chan VideoFrame {
	if m == nil || m.video == nil {
		return nil
	}
	return m.video.Frames()
}

// KeyframeRequests returns a channel signalled when the call requests an IDR
// from the browser.
func (m *CallMedia) KeyframeRequests() <-chan struct{} {
	if m == nil || m.video == nil {
		return nil
	}
	return m.video.KeyframeRequests()
}

// Ended returns a channel closed when the call ends and media is released. The
// media WebSocket writer selects on it so a call finalization tears down the
// connection even if the drain channels never close.
func (m *CallMedia) Ended() <-chan struct{} {
	if m == nil {
		return nil
	}
	return m.done
}

// Close releases all media bridges and signals the media WebSocket that the call
// has ended. Safe to call more than once, including concurrently.
func (m *CallMedia) Close() {
	if m == nil {
		return
	}
	m.closeOnce.Do(func() {
		close(m.done)
		if m.source != nil {
			_ = m.source.Close()
		}
		if m.sink != nil {
			_ = m.sink.Close()
		}
		if m.video != nil {
			m.video.Close()
		}
	})
}

// CreateMediaSession issues (or reuses) a media-session token for a live call.
// Ownership: if a valid, unexpired session already exists for this call it is
// returned as-is; otherwise a fresh cryptographically-random token is created.
func (s *CallService) CreateMediaSession(callID string) (*MediaSession, error) {
	s.mu.Lock()
	active := s.active
	s.mu.Unlock()
	if active == nil || active.ID != callID {
		return nil, ErrCallNotActive
	}

	s.mediaMu.Lock()
	defer s.mediaMu.Unlock()

	if sess, ok := s.mediaSessions[callID]; ok && time.Now().Before(sess.ExpiresAt) {
		return sess, nil
	}

	token, err := newMediaToken()
	if err != nil {
		return nil, err
	}
	sess := &MediaSession{
		Token:     token,
		CallID:    callID,
		ExpiresAt: time.Now().Add(MediaSessionTTL),
	}
	s.mediaSessions[callID] = sess
	return sess, nil
}

// ValidateMediaSession checks that a media-session token is valid for the given
// call and consumes it on success, so a second connection to the same call is
// rejected. The token is compared in constant time.
func (s *CallService) ValidateMediaSession(callID string, token string) error {
	s.mediaMu.Lock()
	defer s.mediaMu.Unlock()

	sess, ok := s.mediaSessions[callID]
	if !ok {
		return ErrMediaSessionInvalid
	}
	if time.Now().After(sess.ExpiresAt) {
		delete(s.mediaSessions, callID)
		return ErrMediaSessionExpired
	}
	if subtle.ConstantTimeCompare([]byte(sess.Token), []byte(token)) != 1 {
		return ErrMediaSessionInvalid
	}
	// Consume on use: only one media connection per call.
	delete(s.mediaSessions, callID)
	return nil
}

// MediaForCall returns the live CallMedia bridge for the given call, or nil if
// the call is not active or has no media. Used by the media WebSocket handler.
func (s *CallService) MediaForCall(callID string) *CallMedia {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.active == nil || s.active.ID != callID {
		return nil
	}
	return s.active.Media
}

// dropMediaSession removes any stored media session for a call. Called when the
// call finalizes/ends.
func (s *CallService) dropMediaSession(callID string) {
	s.mediaMu.Lock()
	delete(s.mediaSessions, callID)
	s.mediaMu.Unlock()
}

// newMediaToken generates a cryptographically-secure URL-safe token.
func newMediaToken() (string, error) {
	b := make([]byte, 32)
	if _, err := rand.Read(b); err != nil {
		return "", err
	}
	return base64.RawURLEncoding.EncodeToString(b), nil
}

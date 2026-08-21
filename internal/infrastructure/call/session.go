package call

import (
	"time"

	"github.com/purpshell/meowcaller"

	"wa-bot/internal/domain/entity"
)

// CallSession is the runtime, in-memory state of a single active call.
type CallSession struct {
	ID           string
	MeowCallID   string
	Call         *meowcaller.Call
	Direction    entity.CallDirection
	Type         entity.CallType
	Source       entity.CallSource
	MediaMode    entity.MediaMode
	Target       string
	GroupJID     string
	Participants []string
	Status       entity.CallStatus
	StartedAt    time.Time
	AnsweredAt   *time.Time
	EndedAt      *time.Time
	// Media is the bridge between the browser media WebSocket and the call. It
	// is built when the call is created and closed when the call ends.
	Media *CallMedia
	// APIKeyID is the API key that initiated this call (external calls only).
	APIKeyID string
	// AudioResult is the pre-resolved TTS/audio-file source played after the
	// call is answered (media-mode external calls only). Its Cleanup is invoked
	// when the call finalizes.
	AudioResult *AudioResult
	// VideoResult is the pre-resolved video-file source played after the
	// call is answered (video media-mode). Its Cleanup is invoked when the call finalizes.
	VideoResult *VideoResult
	// VideoFeeder streams the video file into the call (video_file mode only).
	VideoFeeder *VideoFileFeeder
	// HangupAfterPlayback ends the call once the audio source finishes.
	HangupAfterPlayback bool
	// RingTimeout is the configured ring timeout for this call.
	RingTimeout time.Duration
	// ringTimer aborts the call if it is not answered in time.
	ringTimer *time.Timer
	// VideoEnabled reports whether this client's outbound camera is active.
	VideoEnabled bool
	// RemoteVideoEnabled reports whether the peer's camera is on.
	RemoteVideoEnabled bool
}

// NewCallSession builds a new runtime call session.
func NewCallSession(
	id string,
	meowCallID string,
	call *meowcaller.Call,
	direction entity.CallDirection,
	callType entity.CallType,
	source entity.CallSource,
	mediaMode entity.MediaMode,
	target string,
	groupJID string,
	participants []string,
) *CallSession {
	return &CallSession{
		ID:           id,
		MeowCallID:   meowCallID,
		Call:         call,
		Direction:    direction,
		Type:         callType,
		Source:       source,
		MediaMode:    mediaMode,
		Target:       target,
		GroupJID:     groupJID,
		Participants: participants,
		Status:       entity.CallStatusPreparing,
		StartedAt:    time.Now(),
	}
}

// View returns a serializable snapshot of the session.
func (s *CallSession) View() entity.CallStateResponse {
	var answeredAt *int64
	if s.AnsweredAt != nil {
		ms := s.AnsweredAt.UnixMilli()
		answeredAt = &ms
	}
	return entity.CallStateResponse{
		ID:                 s.ID,
		Status:             s.Status,
		Type:               s.Type,
		Direction:          s.Direction,
		Source:             s.Source,
		MediaMode:          s.MediaMode,
		Target:             s.Target,
		GroupJID:           s.GroupJID,
		Participants:       s.Participants,
		StartedAt:          s.StartedAt.UnixMilli(),
		AnsweredAt:         answeredAt,
		VideoEnabled:       s.VideoEnabled,
		RemoteVideoEnabled: s.RemoteVideoEnabled,
	}
}

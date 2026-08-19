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
	// MediaClient reserved for Phase 2.
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
		ID:           s.ID,
		Status:       s.Status,
		Type:         s.Type,
		Direction:    s.Direction,
		Source:       s.Source,
		MediaMode:    s.MediaMode,
		Target:       s.Target,
		GroupJID:     s.GroupJID,
		Participants: s.Participants,
		StartedAt:    s.StartedAt.UnixMilli(),
		AnsweredAt:   answeredAt,
	}
}

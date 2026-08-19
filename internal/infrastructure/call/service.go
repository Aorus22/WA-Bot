package call

import (
	"context"
	"errors"
	"fmt"
	"sync"
	"time"

	"github.com/purpshell/meowcaller"

	"wa-bot/internal/domain/entity"
	"wa-bot/internal/domain/repository"
)

var (
	// ErrWhatsAppNotConnected indicates the WhatsApp client is not connected.
	ErrWhatsAppNotConnected = errors.New("whatsapp_not_connected")
	// ErrCallAlreadyActive indicates a call is already active.
	ErrCallAlreadyActive = errors.New("call_already_active")
	// ErrCallNotFound indicates the call does not exist.
	ErrCallNotFound = errors.New("call_not_found")
	// ErrCallNotActive indicates the call exists but is not active.
	ErrCallNotActive = errors.New("call_not_active")
	// ErrInvalidTarget indicates the call target is empty/invalid.
	ErrInvalidTarget = errors.New("invalid_target")
	// ErrNotImplemented indicates the operation is not yet implemented.
	ErrNotImplemented = errors.New("not_implemented")
	// ErrGroupNotSupported indicates group calls are not supported yet.
	ErrGroupNotSupported = errors.New("group_not_supported")
	// ErrCallNotOwned indicates an external API key tried to act on a call it
	// does not own without the calls:write scope (PRD §37).
	ErrCallNotOwned = errors.New("call_not_owned")
)

// Ring timeout bounds (PRD §29). The service default is configurable via the
// CALL_RING_TIMEOUT_SECONDS env; external API calls are clamped to this range.
const (
	DefaultRingTimeout = 45 * time.Second
	MinRingTimeout     = 10 * time.Second
	MaxRingTimeout     = 120 * time.Second
)

// EventPublisher is the minimal broadcast contract used by CallService. It is
// satisfied by the HTTP/WS layer so the service stays decoupled from it.
type EventPublisher interface {
	BroadcastMessage(msgType string, payload interface{})
}

// CallService manages the single active call lifecycle on top of meowcaller.
type CallService struct {
	mu        sync.Mutex
	active    *CallSession
	client    *meowcaller.Client
	connected func() bool
	logs      repository.CallRepository
	hub       EventPublisher

	mediaMu       sync.Mutex
	mediaSessions map[string]*MediaSession

	// ringTimeout is the default ring timeout for outgoing calls (PRD §29).
	ringTimeout time.Duration
}

// NewCallService builds the call service and wires the incoming-call handler.
func NewCallService(client *meowcaller.Client, connected func() bool, logs repository.CallRepository, hub EventPublisher) *CallService {
	svc := &CallService{
		client:        client,
		connected:     connected,
		logs:          logs,
		hub:           hub,
		mediaSessions: make(map[string]*MediaSession),
	}
	if client != nil {
		client.OnIncomingCall(svc.onIncomingCall)
	}
	return svc
}

// MarkInterruptedOnStartup marks any unfinished call logs as interrupted.
func (s *CallService) MarkInterruptedOnStartup(ctx context.Context) error {
	if s.logs == nil {
		return nil
	}
	_, err := s.logs.MarkInterruptedCalls(ctx)
	return err
}

// SetRingTimeout sets the default ring timeout used for outgoing calls.
func (s *CallService) SetRingTimeout(d time.Duration) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.ringTimeout = d
}

// RingTimeout returns the configured default ring timeout.
func (s *CallService) RingTimeout() time.Duration {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.ringTimeout <= 0 {
		return DefaultRingTimeout
	}
	return s.ringTimeout
}

// ExternalCallRequest describes an outgoing external API call carrying a
// media-mode audio source (TTS or audio file) to play after the peer answers.
type ExternalCallRequest struct {
	Target              string
	Type                entity.CallType
	MediaMode           entity.MediaMode
	Audio               *AudioResult
	HangupAfterPlayback bool
	RingTimeout         time.Duration
	APIKeyID            string
}

// StartCall places an outgoing direct audio/video call. It does not block
// waiting for an answer; it returns once the call is initiated.
func (s *CallService) StartCall(ctx context.Context, req entity.CreateCallRequest, source entity.CallSource) (*entity.CallStateResponse, error) {
	if s.connected == nil || !s.connected() {
		return nil, ErrWhatsAppNotConnected
	}
	if req.Target == "" {
		return nil, ErrInvalidTarget
	}

	callType := req.Type
	if callType == "" {
		callType = entity.CallTypeAudio
	}
	if callType != entity.CallTypeAudio && callType != entity.CallTypeVideo {
		return nil, ErrInvalidTarget
	}

	s.mu.Lock()
	if s.active != nil {
		s.mu.Unlock()
		return nil, ErrCallAlreadyActive
	}

	id := fmt.Sprintf("call_%d", time.Now().UnixMilli())
	session := NewCallSession(
		id,
		"",
		nil,
		entity.CallDirectionOutgoing,
		callType,
		source,
		entity.MediaModeLive,
		req.Target,
		"",
		nil,
	)
	session.Status = entity.CallStatusPreparing
	s.active = session
	s.mu.Unlock()

	// Create the call log first.
	now := time.Now().UnixMilli()
	callLog := &entity.CallLog{
		ID:         id,
		MeowCallID: "",
		Direction:  session.Direction,
		CallType:   session.Type,
		Target:     session.Target,
		Source:     session.Source,
		MediaMode:  session.MediaMode,
		Status:     entity.CallStatusPreparing,
		StartedAt:  now,
		CreatedAt:  now,
	}
	if err := s.logs.CreateCallLog(ctx, callLog); err != nil {
		s.clearActive(id)
		return nil, err
	}

	// Place the call.
	var (
		call *meowcaller.Call
		err  error
	)
	if callType == entity.CallTypeVideo {
		call, err = s.client.CallWithOptions(ctx, req.Target, meowcaller.CallOptions{Video: true})
	} else {
		call, err = s.client.Call(ctx, req.Target)
	}
	if err != nil {
		// Best-effort finalize the log as failed.
		now := time.Now().UnixMilli()
		_ = s.logs.UpdateCallStatus(ctx, id, entity.CallStatusFailed, nil, &now, nil, "")
		s.clearActive(id)
		return nil, err
	}

	s.mu.Lock()
	if s.active != nil && s.active.ID == id {
		session.MeowCallID = call.ID()
		session.Call = call
		session.Media = NewCallMedia(call)
		session.Status = entity.CallStatusInitiating
	}
	s.mu.Unlock()

	s.wireCallbacks(session, call)

	// Push initial state.
	s.mu.Lock()
	state := session.View()
	s.mu.Unlock()
	s.broadcast("call.state", state)
	return &state, nil
}

// StartExternalCall reserves the single active slot for an external API call,
// persists the call log, and returns its state immediately (HTTP 202). The
// actual dial happens asynchronously in a goroutine so the caller does not wait.
// The audio source must be resolved (TTS/audio file) BEFORE calling so the peer
// does not wait for synthesis after answering.
func (s *CallService) StartExternalCall(ctx context.Context, req ExternalCallRequest) (*entity.CallStateResponse, error) {
	if s.connected == nil || !s.connected() {
		return nil, ErrWhatsAppNotConnected
	}
	if req.Target == "" {
		return nil, ErrInvalidTarget
	}
	callType := req.Type
	if callType == "" {
		callType = entity.CallTypeAudio
	}
	if callType != entity.CallTypeAudio {
		return nil, ErrInvalidTarget
	}
	// External calls only support tts / audio_file media modes (video is Phase 4).
	if req.MediaMode != entity.MediaModeTTS && req.MediaMode != entity.MediaModeAudioFile {
		return nil, ErrInvalidTarget
	}
	if req.Audio == nil {
		return nil, ErrInvalidTarget
	}

	s.mu.Lock()
	if s.active != nil {
		s.mu.Unlock()
		return nil, ErrCallAlreadyActive
	}

	id := fmt.Sprintf("call_%d", time.Now().UnixMilli())
	session := NewCallSession(
		id,
		"",
		nil,
		entity.CallDirectionOutgoing,
		callType,
		entity.CallSourceExternalAPI,
		req.MediaMode,
		req.Target,
		"",
		nil,
	)
	session.Status = entity.CallStatusPreparing
	session.APIKeyID = req.APIKeyID
	session.AudioResult = req.Audio
	session.HangupAfterPlayback = req.HangupAfterPlayback
	session.RingTimeout = req.RingTimeout
	s.active = session
	s.mu.Unlock()

	// Create the call log first.
	now := time.Now().UnixMilli()
	callLog := &entity.CallLog{
		ID:         id,
		MeowCallID: "",
		Direction:  session.Direction,
		CallType:   session.Type,
		Target:     session.Target,
		Source:     session.Source,
		MediaMode:  session.MediaMode,
		Status:     entity.CallStatusPreparing,
		APIKeyID:   session.APIKeyID,
		StartedAt:  now,
		CreatedAt:  now,
	}
	if err := s.logs.CreateCallLog(ctx, callLog); err != nil {
		s.clearActive(id)
		s.cleanupAudio(session)
		return nil, err
	}

	// Return the state immediately; dial happens in the background.
	s.mu.Lock()
	state := session.View()
	s.mu.Unlock()
	go s.dialExternal(session)
	return &state, nil
}

// dialExternal places the call asynchronously and wires its lifecycle callbacks.
// Errors are handled here by marking the log failed and broadcasting call.state.
func (s *CallService) dialExternal(session *CallSession) {
	ctx := context.Background()

	s.mu.Lock()
	if s.active == nil || s.active.ID != session.ID {
		s.mu.Unlock()
		return
	}
	target := session.Target
	s.mu.Unlock()

	call, err := s.client.Call(ctx, target)
	if err != nil {
		now := time.Now().UnixMilli()
		_ = s.logs.UpdateCallStatus(ctx, session.ID, entity.CallStatusFailed, nil, &now, nil, "")
		s.cleanupAudio(session)
		s.mu.Lock()
		if s.active != nil && s.active.ID == session.ID {
			session.Status = entity.CallStatusFailed
			state := session.View()
			s.active = nil
			s.mu.Unlock()
			s.broadcast("call.state", state)
			s.broadcast("call.ended", map[string]interface{}{
				"id":     session.ID,
				"status": entity.CallStatusFailed,
				"reason": err.Error(),
				"state":  state,
			})
			return
		}
		s.mu.Unlock()
		return
	}

	s.mu.Lock()
	if s.active != nil && s.active.ID == session.ID {
		session.MeowCallID = call.ID()
		session.Call = call
		session.Status = entity.CallStatusInitiating
	}
	s.mu.Unlock()

	s.wireExternalCallbacks(session, call)
	s.startRingTimeout(session, call)

	s.mu.Lock()
	state := session.View()
	s.mu.Unlock()
	s.broadcast("call.state", state)
}

// wireExternalCallbacks registers the lifecycle callbacks for an external call.
// On answer it stops the ring timer and plays the pre-resolved audio source.
func (s *CallService) wireExternalCallbacks(session *CallSession, call *meowcaller.Call) {
	if call == nil {
		return
	}

	call.OnStateChange(func(phase meowcaller.CallPhase) {
		s.onStateChange(session, phase)
	})
	call.OnPeerAccept(func() {
		s.mu.Lock()
		var state entity.CallStateResponse
		if s.active != nil && s.active.ID == session.ID {
			session.Status = entity.CallStatusConnecting
			state = session.View()
		}
		s.mu.Unlock()
		s.broadcast("call.peer_accepted", state)
	})
	call.OnReady(func() {
		s.mu.Lock()
		var state entity.CallStateResponse
		if s.active != nil && s.active.ID == session.ID {
			if session.ringTimer != nil {
				session.ringTimer.Stop()
				session.ringTimer = nil
			}
			now := time.Now()
			session.Status = entity.CallStatusConnected
			session.AnsweredAt = &now
			answeredAt := now.UnixMilli()
			_ = s.logs.UpdateCallStatus(context.Background(), session.ID, entity.CallStatusConnected, &answeredAt, nil, nil, session.MeowCallID)
			state = session.View()
			s.playSessionAudio(session, call)
		}
		s.mu.Unlock()
		s.broadcast("call.ready", state)
	})
	call.OnEnd(func(reason string) {
		s.onEnd(session, reason)
	})
}

// playSessionAudio streams the pre-resolved audio file into the call. If
// HangupAfterPlayback is set the call is ended when the source is exhausted.
func (s *CallService) playSessionAudio(session *CallSession, call *meowcaller.Call) {
	res := session.AudioResult
	if res == nil || call == nil {
		return
	}
	var (
		src meowcaller.AudioSource
		err error
	)
	switch res.Format {
	case "wav":
		src, err = meowcaller.WAVFile(res.Path)
	default:
		src, err = meowcaller.MP3File(res.Path)
	}
	if err != nil {
		return
	}
	player := call.Play(src)
	if session.HangupAfterPlayback {
		// OnFinish fires when the source reaches EOF (see meowcaller.Player).
		player.OnFinish(func() {
			s.hangupAfterPlayback(session)
		})
	}
}

// hangupAfterPlayback ends an external call once its audio has finished playing.
func (s *CallService) hangupAfterPlayback(session *CallSession) {
	s.mu.Lock()
	if s.active == nil || s.active.ID != session.ID {
		s.mu.Unlock()
		return
	}
	call := session.Call
	s.mu.Unlock()
	if call == nil {
		return
	}
	_ = call.Hangup()
	s.finalize(session, entity.CallStatusEnded, "playback_finished")
}

// startRingTimeout arms the ring timer for the call. It is cancelled when the
// call is answered; otherwise it hangs up and finalizes the call as missed.
func (s *CallService) startRingTimeout(session *CallSession, call *meowcaller.Call) {
	timeout := session.RingTimeout
	if timeout <= 0 {
		timeout = s.RingTimeout()
	}
	if timeout < MinRingTimeout {
		timeout = MinRingTimeout
	}
	if timeout > MaxRingTimeout {
		timeout = MaxRingTimeout
	}

	timer := time.AfterFunc(timeout, func() {
		s.ringTimeoutFired(session, call)
	})
	s.mu.Lock()
	if s.active != nil && s.active.ID == session.ID {
		session.ringTimer = timer
	}
	s.mu.Unlock()
}

// ringTimeoutFired is invoked by the ring timer: hang up if the call is still
// ringing/unanswered, and finalize it as missed or failed.
func (s *CallService) ringTimeoutFired(session *CallSession, call *meowcaller.Call) {
	s.mu.Lock()
	if s.active == nil || s.active.ID != session.ID || session.AnsweredAt != nil {
		s.mu.Unlock()
		return
	}
	status := entity.CallStatusMissed
	switch session.Status {
	case entity.CallStatusPreparing, entity.CallStatusInitiating:
		status = entity.CallStatusFailed
	}
	s.mu.Unlock()

	_ = call.Hangup()
	s.finalize(session, status, "ring_timeout")
}

// cleanupAudio releases the temp file backing an audio-source result (TTS/audio
// file) once the call finalizes.
func (s *CallService) cleanupAudio(session *CallSession) {
	if session == nil || session.AudioResult == nil || session.AudioResult.Cleanup == nil {
		return
	}
	session.AudioResult.Cleanup()
	session.AudioResult = nil
}

// StartGroupCall is reserved for Phase 5.
func (s *CallService) StartGroupCall(ctx context.Context, req entity.CreateGroupCallRequest, source entity.CallSource) (*entity.CallStateResponse, error) {
	return nil, ErrGroupNotSupported
}

// AnswerCall answers the active call.
func (s *CallService) AnswerCall(ctx context.Context, id string) error {
	s.mu.Lock()
	session := s.active
	if session == nil || session.ID != id {
		s.mu.Unlock()
		return ErrCallNotFound
	}
	if session.Call == nil {
		s.mu.Unlock()
		return ErrCallNotActive
	}
	call := session.Call
	s.mu.Unlock()
	if err := call.Answer(); err != nil {
		return err
	}
	now := time.Now()
	s.mu.Lock()
	if s.active != nil && s.active.ID == id {
		session.AnsweredAt = &now
	}
	s.mu.Unlock()
	return nil
}

// RejectCall rejects the active call.
func (s *CallService) RejectCall(ctx context.Context, id string) error {
	s.mu.Lock()
	session := s.active
	if session == nil || session.ID != id {
		s.mu.Unlock()
		return ErrCallNotFound
	}
	if session.Call == nil {
		s.mu.Unlock()
		return ErrCallNotActive
	}
	call := session.Call
	s.mu.Unlock()
	if err := call.Reject(); err != nil {
		return err
	}
	s.finalize(session, entity.CallStatusRejected, "rejected")
	return nil
}

// HangupCall ends the active call. When invoked from the external API the
// caller's APIKeyID and whether they hold calls:write scope are threaded in to
// enforce ownership (PRD §37): a key may hang up a call it created, or any call
// if it has calls:write. An empty apiKeyID (internal/UI path) skips the check.
func (s *CallService) HangupCall(ctx context.Context, id string, apiKeyID string, hasWriteScope bool) error {
	s.mu.Lock()
	session := s.active
	if session == nil || session.ID != id {
		s.mu.Unlock()
		return ErrCallNotFound
	}
	if session.Call == nil {
		s.mu.Unlock()
		return ErrCallNotActive
	}
	// Ownership: an external key may only hang up a call it created unless it
	// holds calls:write. Internal (UI) callers pass an empty apiKeyID.
	if apiKeyID != "" && !hasWriteScope && session.APIKeyID != "" && session.APIKeyID != apiKeyID {
		s.mu.Unlock()
		return ErrCallNotOwned
	}
	call := session.Call
	s.mu.Unlock()
	if err := call.Hangup(); err != nil {
		return err
	}
	s.finalize(session, entity.CallStatusEnded, "hangup")
	return nil
}

// GetActiveCall returns the active call state, or nil if none is active.
func (s *CallService) GetActiveCall() *entity.CallStateResponse {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.active == nil {
		return nil
	}
	state := s.active.View()
	return &state
}

// GetCallStatus returns the state for a call, from the active session or history.
// When called from the external API (apiKeyID != ""), ownership is enforced
// (PRD §37): a key without calls:write may only read its own calls, and its
// target is never leaked to a non-owner. A calls:write key may read any call.
func (s *CallService) GetCallStatus(ctx context.Context, id string, apiKeyID string, hasWriteScope bool) (*entity.CallStateResponse, error) {
	s.mu.Lock()
	if s.active != nil && s.active.ID == id {
		state := s.active.View()
		owner := s.active.APIKeyID
		s.mu.Unlock()
		if apiKeyID != "" && !hasWriteScope && owner != "" && owner != apiKeyID {
			return nil, ErrCallNotOwned
		}
		return &state, nil
	}
	s.mu.Unlock()

	if s.logs == nil {
		return nil, ErrCallNotFound
	}
	log, err := s.logs.GetCallLog(ctx, id)
	if err != nil {
		return nil, err
	}
	if log == nil {
		return nil, ErrCallNotFound
	}
	// Ownership: non-write keys may only read their own calls. Returning
	// ErrCallNotOwned (mapped to 404) avoids leaking a call's existence.
	if apiKeyID != "" && !hasWriteScope && log.APIKeyID != "" && log.APIKeyID != apiKeyID {
		return nil, ErrCallNotOwned
	}
	return logToState(log), nil
}

// GetHistory returns the call history.
func (s *CallService) GetHistory(ctx context.Context, opts repository.CallHistoryFilter) ([]*entity.CallLog, error) {
	if s.logs == nil {
		return []*entity.CallLog{}, nil
	}
	return s.logs.GetCallHistory(ctx, opts)
}

// GracefulShutdown hangs up any active call and marks its log interrupted.
func (s *CallService) GracefulShutdown(ctx context.Context) {
	s.mu.Lock()
	session := s.active
	s.mu.Unlock()
	if session == nil {
		return
	}
	if session.Call != nil {
		_ = session.Call.Hangup()
	}
	s.finalize(session, entity.CallStatusInterrupted, "shutdown")
}

// onIncomingCall handles an inbound call offer.
func (s *CallService) onIncomingCall(call *meowcaller.Call) {
	if call == nil {
		return
	}
	callType := entity.CallTypeAudio
	if call.IsVideo() {
		callType = entity.CallTypeVideo
	}

	target := call.Peer().String()

	if s.connected == nil || !s.connected() {
		// Not connected: reject the incoming call and log it.
		_ = call.Reject()
		now := time.Now().UnixMilli()
		rejectedLog := &entity.CallLog{
			ID:         fmt.Sprintf("call_%d", now),
			MeowCallID: call.ID(),
			Direction:  entity.CallDirectionIncoming,
			CallType:   callType,
			Target:     target,
			Source:     entity.CallSourceIncoming,
			MediaMode:  entity.MediaModeLive,
			Status:     entity.CallStatusFailed,
			StartedAt:  now,
			CreatedAt:  now,
		}
		_ = s.logs.CreateCallLog(context.Background(), rejectedLog)
		return
	}

	s.mu.Lock()
	if s.active != nil {
		// Busy: reject the incoming call and log it.
		_ = call.Reject()
		now := time.Now().UnixMilli()
		busyLog := &entity.CallLog{
			ID:         fmt.Sprintf("call_%d", now),
			MeowCallID: call.ID(),
			Direction:  entity.CallDirectionIncoming,
			CallType:   callType,
			Target:     target,
			Source:     entity.CallSourceIncoming,
			MediaMode:  entity.MediaModeLive,
			Status:     entity.CallStatusBusy,
			StartedAt:  now,
			CreatedAt:  now,
		}
		_ = s.logs.CreateCallLog(context.Background(), busyLog)
		activeState := s.active.View()
		s.mu.Unlock()
		s.broadcast("call.state", activeState)
		return
	}

	id := fmt.Sprintf("call_%d", time.Now().UnixMilli())
	session := NewCallSession(
		id,
		call.ID(),
		call,
		entity.CallDirectionIncoming,
		callType,
		entity.CallSourceIncoming,
		entity.MediaModeLive,
		target,
		"",
		nil,
	)
	session.Status = entity.CallStatusRinging
	session.Media = NewCallMedia(call)
	s.active = session
	s.mu.Unlock()

	now := time.Now().UnixMilli()
	incomingLog := &entity.CallLog{
		ID:         id,
		MeowCallID: call.ID(),
		Direction:  entity.CallDirectionIncoming,
		CallType:   callType,
		Target:     target,
		Source:     entity.CallSourceIncoming,
		MediaMode:  entity.MediaModeLive,
		Status:     entity.CallStatusRinging,
		StartedAt:  now,
		CreatedAt:  now,
	}
	if err := s.logs.CreateCallLog(context.Background(), incomingLog); err != nil {
		s.clearActive(id)
		return
	}

	s.wireCallbacks(session, call)

	state := session.View()
	s.broadcast("call.incoming", state)
	s.broadcast("call.state", state)
}

// wireCallbacks registers the lifecycle callbacks for a live call.
func (s *CallService) wireCallbacks(session *CallSession, call *meowcaller.Call) {
	if call == nil {
		return
	}

	call.OnStateChange(func(phase meowcaller.CallPhase) {
		s.onStateChange(session, phase)
	})
	call.OnPeerAccept(func() {
		s.mu.Lock()
		var state entity.CallStateResponse
		if s.active != nil && s.active.ID == session.ID {
			session.Status = entity.CallStatusConnecting
			state = session.View()
		}
		s.mu.Unlock()
		s.broadcast("call.peer_accepted", state)
	})
	call.OnReady(func() {
		now := time.Now()
		s.mu.Lock()
		var state entity.CallStateResponse
		if s.active != nil && s.active.ID == session.ID {
			session.Status = entity.CallStatusConnected
			session.AnsweredAt = &now
			state = session.View()
			// Persist the answer timestamp (written once via COALESCE).
			answeredAt := now.UnixMilli()
			_ = s.logs.UpdateCallStatus(context.Background(), session.ID, entity.CallStatusConnected, &answeredAt, nil, nil, session.MeowCallID)
		}
		s.mu.Unlock()
		s.broadcast("call.ready", state)
	})
	call.OnEnd(func(reason string) {
		s.onEnd(session, reason)
	})
}

// onStateChange maps a meowcaller CallPhase to the domain status and broadcasts.
func (s *CallService) onStateChange(session *CallSession, phase meowcaller.CallPhase) {
	status := phaseToStatus(phase)
	s.mu.Lock()
	var state entity.CallStateResponse
	if s.active != nil && s.active.ID == session.ID {
		session.Status = status
		state = session.View()
	}
	s.mu.Unlock()
	s.broadcast("call.state", state)
}

// onEnd finalizes the session and persists the terminal log state.
func (s *CallService) onEnd(session *CallSession, reason string) {
	status := entity.CallStatusEnded
	switch reason {
	case "rejected":
		status = entity.CallStatusRejected
	case "missed", "timeout", "unanswered":
		status = entity.CallStatusMissed
	case "failed", "error":
		status = entity.CallStatusFailed
	case "busy":
		status = entity.CallStatusBusy
	case "interrupted":
		status = entity.CallStatusInterrupted
	}
	s.finalize(session, status, reason)
}

// finalize persists the terminal state and clears the active session.
func (s *CallService) finalize(session *CallSession, status entity.CallStatus, reason string) {
	s.mu.Lock()
	if s.active == nil || s.active.ID != session.ID {
		s.mu.Unlock()
		return
	}
	now := time.Now()
	session.EndedAt = &now
	session.Status = status
	s.active = nil
	media := session.Media
	session.Media = nil
	s.mu.Unlock()

	// Release the media bridges and any pending media session for this call.
	if media != nil {
		media.Close()
	}
	s.dropMediaSession(session.ID)
	// Remove the temp TTS/audio-file source now that playback is done.
	s.cleanupAudio(session)

	endedMs := now.UnixMilli()
	var durationMS *int64
	if session.AnsweredAt != nil {
		d := now.Sub(*session.AnsweredAt).Milliseconds()
		if d < 0 {
			d = 0
		}
		durationMS = &d
	}
	var answeredAt *int64
	if session.AnsweredAt != nil {
		ms := session.AnsweredAt.UnixMilli()
		answeredAt = &ms
	}
	_ = s.logs.UpdateCallStatus(context.Background(), session.ID, status, answeredAt, &endedMs, durationMS, session.MeowCallID)

	s.broadcast("call.ended", map[string]interface{}{
		"id":     session.ID,
		"status": status,
		"reason": reason,
		"state":  session.View(),
	})
}

// clearActive removes the active session if it matches the given id.
func (s *CallService) clearActive(id string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.active != nil && s.active.ID == id {
		s.active = nil
	}
}

func (s *CallService) broadcast(msgType string, payload interface{}) {
	if s.hub != nil {
		s.hub.BroadcastMessage(msgType, payload)
	}
}

// phaseToStatus maps a meowcaller CallPhase to the domain status.
func phaseToStatus(phase meowcaller.CallPhase) entity.CallStatus {
	switch phase {
	case meowcaller.CallPhaseCalling:
		return entity.CallStatusInitiating
	case meowcaller.CallPhaseRinging:
		return entity.CallStatusRinging
	case meowcaller.CallPhaseConnecting:
		return entity.CallStatusConnecting
	case meowcaller.CallPhaseActive:
		return entity.CallStatusConnected
	case meowcaller.CallPhaseEnded:
		return entity.CallStatusEnded
	case meowcaller.CallPhaseWaitingRoom:
		return entity.CallStatusConnecting
	default:
		return entity.CallStatusPreparing
	}
}

// logToState converts a persisted call log into a serializable state view.
func logToState(log *entity.CallLog) *entity.CallStateResponse {
	return &entity.CallStateResponse{
		ID:           log.ID,
		Status:       log.Status,
		Type:         log.CallType,
		Direction:    log.Direction,
		Source:       log.Source,
		MediaMode:    log.MediaMode,
		Target:       log.Target,
		GroupJID:     log.GroupJID,
		Participants: log.Participants,
		StartedAt:    log.StartedAt,
		AnsweredAt:   log.AnsweredAt,
	}
}

package entity

// CallType describes the media type of a call.
type CallType string

const (
	CallTypeAudio      CallType = "audio"
	CallTypeVideo      CallType = "video"
	CallTypeGroupAudio CallType = "group_audio"
	CallTypeGroupVideo CallType = "group_video"
)

// CallDirection describes whether the call was placed or received.
type CallDirection string

const (
	CallDirectionIncoming CallDirection = "incoming"
	CallDirectionOutgoing CallDirection = "outgoing"
)

// CallSource describes how the call was initiated.
type CallSource string

const (
	CallSourceUI          CallSource = "ui"
	CallSourceExternalAPI CallSource = "external_api"
	CallSourceIncoming    CallSource = "incoming"
)

// MediaMode describes the media input mode of a call.
type MediaMode string

const (
	MediaModeLive      MediaMode = "live"
	MediaModeTTS       MediaMode = "tts"
	MediaModeAudioFile MediaMode = "audio_file"
)

// CallStatus is the lifecycle status of a call.
type CallStatus string

const (
	CallStatusPreparing   CallStatus = "preparing"
	CallStatusInitiating  CallStatus = "initiating"
	CallStatusRinging     CallStatus = "ringing"
	CallStatusConnecting  CallStatus = "connecting"
	CallStatusConnected   CallStatus = "connected"
	CallStatusEnding      CallStatus = "ending"
	CallStatusEnded       CallStatus = "ended"
	CallStatusRejected    CallStatus = "rejected"
	CallStatusMissed      CallStatus = "missed"
	CallStatusBusy        CallStatus = "busy"
	CallStatusFailed      CallStatus = "failed"
	CallStatusInterrupted CallStatus = "interrupted"
)

// CallLog is the persisted record of a call.
type CallLog struct {
	ID           string        `json:"id"`
	MeowCallID   string        `json:"meow_call_id"`
	Direction    CallDirection `json:"direction"`
	CallType     CallType      `json:"call_type"`
	Target       string        `json:"target"`
	GroupJID     string        `json:"group_jid,omitempty"`
	Participants []string      `json:"participants,omitempty"`
	Source       CallSource    `json:"source"`
	MediaMode    MediaMode     `json:"media_mode"`
	Status       CallStatus    `json:"status"`
	ErrorMessage string        `json:"error_message,omitempty"`
	APIKeyID     string        `json:"api_key_id,omitempty"`
	StartedAt    int64         `json:"started_at"`
	AnsweredAt   *int64        `json:"answered_at,omitempty"`
	EndedAt      *int64        `json:"ended_at,omitempty"`
	DurationMS   *int64        `json:"duration_ms,omitempty"`
	CreatedAt    int64         `json:"created_at"`
}

// CreateCallRequest is the body for creating a 1:1 call.
type CreateCallRequest struct {
	Target string   `json:"target"`
	Type   CallType `json:"type"`
}

// CreateGroupCallRequest is the body for creating a group call.
type CreateGroupCallRequest struct {
	GroupJID     string   `json:"group_jid"`
	Participants []string `json:"participants"`
	Type         CallType `json:"type"`
}

// AnswerCallRequest is the (empty for now) body for answering a call.
type AnswerCallRequest struct{}

// AddParticipantRequest is the body for adding a participant to a group call.
type AddParticipantRequest struct {
	Targets []string `json:"targets"`
}

// CallStateResponse is a serializable view of an active call.
type CallStateResponse struct {
	ID           string        `json:"id"`
	Status       CallStatus    `json:"status"`
	Type         CallType      `json:"type"`
	Direction    CallDirection `json:"direction"`
	Source       CallSource    `json:"source"`
	MediaMode    MediaMode     `json:"media_mode"`
	Target       string        `json:"target"`
	GroupJID     string        `json:"group_jid,omitempty"`
	Participants []string      `json:"participants,omitempty"`
	StartedAt    int64         `json:"started_at"`
	AnsweredAt   *int64        `json:"answered_at,omitempty"`
}

package handlers

import (
	"errors"
	"net/http"
	"time"

	"wa-bot/internal/delivery/http/middleware"
	"wa-bot/internal/domain/entity"
	"wa-bot/internal/infrastructure/call"
)

// maxExternalBodyBytes bounds the size of external API request bodies.
const maxExternalBodyBytes = 2 << 20 // 2MB

// externalCallMedia carries the media-mode source for an external call.
type externalCallMedia struct {
	Mode        string `json:"mode"`
	Text        string `json:"text"`
	Voice       string `json:"voice"`
	ReferenceID string `json:"reference_id"`
	AudioURL    string `json:"audio_url"`
}

// externalCallRequest is the body of POST /api/external/v1/calls.
type externalCallRequest struct {
	Target              string            `json:"target"`
	Type                entity.CallType   `json:"type"`
	Media               externalCallMedia `json:"media"`
	HangupAfterPlayback bool              `json:"hangup_after_playback"`
	// RingTimeoutSeconds optionally overrides the ring timeout (clamped 10-120s).
	RingTimeoutSeconds *int `json:"ring_timeout_seconds,omitempty"`
}

// ExternalCallHandler exposes the authenticated external call API (PRD §33-37).
type ExternalCallHandler struct {
	handler *Handler
	callSvc *call.CallService
	tts     call.TTSProvider
}

// NewExternalCallHandler builds the external call handler from the shared
// container. The TTS provider is read from the handler container.
func NewExternalCallHandler(h *Handler) *ExternalCallHandler {
	return &ExternalCallHandler{
		handler: h,
		callSvc: h.GetCallService(),
		tts:     h.GetTTSProvider(),
	}
}

// CreateCall initiates an external media-mode call. It resolves the TTS/audio
// source BEFORE dialing so the peer does not wait, then returns 202 Accepted
// immediately; the dial itself runs asynchronously in the service.
func (eh *ExternalCallHandler) CreateCall(w http.ResponseWriter, r *http.Request) {
	r.Body = http.MaxBytesReader(w, r.Body, maxExternalBodyBytes)
	var req externalCallRequest
	if err := eh.handler.readJSON(r, &req); err != nil {
		eh.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]string{
			"error": "invalid_request", "message": "invalid or too large request body",
		})
		return
	}
	if req.Target == "" {
		eh.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]string{
			"error": "invalid_target", "message": "target is required",
		})
		return
	}
	callType := req.Type
	if callType == "" {
		callType = entity.CallTypeAudio
	}
	if callType != entity.CallTypeAudio {
		// Video is out of scope for Phase 3.
		eh.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]string{
			"error": "video_not_ready", "message": "only audio calls are supported",
		})
		return
	}

	// Resolve the audio source BEFORE dialing (PRD §26).
	mediaMode := entity.MediaMode(req.Media.Mode)
	var audio *call.AudioResult
	switch mediaMode {
	case entity.MediaModeTTS:
		if req.Media.Text == "" {
			eh.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]string{
				"error": "invalid_media", "message": "tts media requires text",
			})
			return
		}
		if eh.tts == nil {
			eh.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]string{
				"error": "tts_unavailable", "message": "tts provider is not configured",
			})
			return
		}
		res, err := eh.tts.Synthesize(r.Context(), call.TTSRequest{Text: req.Media.Text, Voice: req.Media.Voice, ReferenceID: req.Media.ReferenceID})
		if err != nil {
			eh.handler.sendJSONWithStatus(w, http.StatusBadGateway, map[string]string{
				"error": "tts_failed", "message": "failed to synthesize speech",
			})
			return
		}
		audio = res
	case entity.MediaModeAudioFile:
		if req.Media.AudioURL == "" {
			eh.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]string{
				"error": "invalid_media", "message": "audio_file media requires audio_url",
			})
			return
		}
		res, err := call.PrepareAudioFile(r.Context(), req.Media.AudioURL)
		if err != nil {
			if errors.Is(err, call.ErrUnsupportedAudio) {
				eh.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]string{
					"error": "unsupported_audio", "message": "unsupported audio format",
				})
				return
			}
			if errors.Is(err, call.ErrBlockedAddress) {
				eh.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]string{
					"error": "audio_download_failed", "message": "audio source address is blocked",
				})
				return
			}
			eh.handler.sendJSONWithStatus(w, http.StatusBadGateway, map[string]string{
				"error": "audio_download_failed", "message": "failed to fetch audio file",
			})
			return
		}
		audio = res
	default:
		eh.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]string{
			"error": "invalid_media", "message": "unsupported media mode",
		})
		return
	}

	// Ring timeout: optional override, otherwise the service/env default.
	var ringTimeout time.Duration
	if req.RingTimeoutSeconds != nil {
		ringTimeout = time.Duration(*req.RingTimeoutSeconds) * time.Second
	}

	state, err := eh.callSvc.StartExternalCall(r.Context(), call.ExternalCallRequest{
		Target:              req.Target,
		Type:                callType,
		MediaMode:           mediaMode,
		Audio:               audio,
		HangupAfterPlayback: req.HangupAfterPlayback,
		RingTimeout:         ringTimeout,
		APIKeyID:            middleware.APIKeyIDFromContext(r.Context()),
	})
	if err != nil {
		// The source was resolved but the call could not start: release it.
		if audio != nil && audio.Cleanup != nil {
			audio.Cleanup()
		}
		writeExternalCallError(eh.handler, w, err)
		return
	}

	eh.handler.sendJSONWithStatus(w, http.StatusAccepted, map[string]interface{}{
		"id":     state.ID,
		"status": "preparing",
	})
}

// GetCallStatus returns the status of an external call (calls:read). Ownership
// is enforced in the service using the caller's API key ID and scope.
func (eh *ExternalCallHandler) GetCallStatus(w http.ResponseWriter, r *http.Request) {
	id := eh.handler.getJID(r, "id")
	apiKeyID := middleware.APIKeyIDFromContext(r.Context())
	hasWrite := middleware.HasScopeFromContext(r.Context(), "calls:write")
	state, err := eh.callSvc.GetCallStatus(r.Context(), id, apiKeyID, hasWrite)
	if err != nil {
		writeExternalCallError(eh.handler, w, err)
		return
	}
	eh.handler.sendJSON(w, state)
}

// HangupCall ends an external call (calls:write). Ownership is enforced in the
// service: a non-write key may only hang up a call it created.
func (eh *ExternalCallHandler) HangupCall(w http.ResponseWriter, r *http.Request) {
	id := eh.handler.getJID(r, "id")
	apiKeyID := middleware.APIKeyIDFromContext(r.Context())
	hasWrite := middleware.HasScopeFromContext(r.Context(), "calls:write")
	if err := eh.callSvc.HangupCall(r.Context(), id, apiKeyID, hasWrite); err != nil {
		writeExternalCallError(eh.handler, w, err)
		return
	}
	eh.handler.sendJSON(w, map[string]string{"status": "ended"})
}

// writeExternalCallError maps call service sentinel errors to HTTP responses for
// the untrusted external API. Unlike the internal writeCallError, unknown errors
// return a static message rather than leaking the internal error string.
func writeExternalCallError(h *Handler, w http.ResponseWriter, err error) {
	switch {
	case errors.Is(err, call.ErrCallAlreadyActive):
		h.sendJSONWithStatus(w, http.StatusConflict, map[string]string{"error": "call_already_active", "message": "a call is already active"})
	case errors.Is(err, call.ErrCallNotFound), errors.Is(err, call.ErrCallNotOwned):
		// ErrCallNotOwned maps to 404 so a caller cannot distinguish a call it
		// does not own from one that does not exist (no existence leak).
		h.sendJSONWithStatus(w, http.StatusNotFound, map[string]string{"error": "call_not_found", "message": "call not found"})
	case errors.Is(err, call.ErrCallNotActive):
		h.sendJSONWithStatus(w, http.StatusConflict, map[string]string{"error": "call_not_active", "message": "call is not active"})
	case errors.Is(err, call.ErrWhatsAppNotConnected):
		h.sendJSONWithStatus(w, http.StatusServiceUnavailable, map[string]string{"error": "whatsapp_not_connected", "message": "whatsapp client is not connected"})
	case errors.Is(err, call.ErrInvalidTarget):
		h.sendJSONWithStatus(w, http.StatusBadRequest, map[string]string{"error": "invalid_target", "message": "invalid target"})
	case errors.Is(err, call.ErrNotImplemented), errors.Is(err, call.ErrGroupNotSupported):
		h.sendJSONWithStatus(w, http.StatusNotImplemented, map[string]string{"error": "not_implemented", "message": "not implemented"})
	default:
		h.sendJSONWithStatus(w, http.StatusInternalServerError, map[string]string{"error": "internal_error", "message": "internal error"})
	}
}

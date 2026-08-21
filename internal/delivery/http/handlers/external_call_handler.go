package handlers

import (
	"errors"
	"io"
	"net/http"
	"os"
	"strconv"
	"strings"
	"time"

	"wa-bot/internal/delivery/http/middleware"
	"wa-bot/internal/domain/entity"
	"wa-bot/internal/infrastructure/call"
)

// maxExternalBodyBytes bounds the size of external API request bodies.
const maxExternalBodyBytes = 2 << 20 // 2MB
// maxExternalMultipartBytes bounds multipart uploads (video file).
const maxExternalMultipartBytes = 55 << 20 // 55MB (50MB file + overhead)

// externalCallMedia carries the media-mode source for an external call.
type externalCallMedia struct {
	Mode        string `json:"mode"`
	Text        string `json:"text"`
	Voice       string `json:"voice"`
	ReferenceID string `json:"reference_id"`
	AudioURL    string `json:"audio_url"`
	VideoURL    string `json:"video_url"`
}

// externalCallRequest is the body of POST /api/external/v1/calls (JSON mode).
type externalCallRequest struct {
	Target              string            `json:"target"`
	Type                entity.CallType   `json:"type"`
	Media               externalCallMedia `json:"media"`
	HangupAfterPlayback *bool             `json:"hangup_after_playback"`
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
//
// Two input modes are supported (content negotiation):
//   - application/json: media.mode = tts|audio_file|video_file with text/audio_url/video_url
//   - multipart/form-data: fields target, type, media_mode, video_url OR file field "file"
func (eh *ExternalCallHandler) CreateCall(w http.ResponseWriter, r *http.Request) {
	ct := r.Header.Get("Content-Type")
	if strings.HasPrefix(ct, "multipart/form-data") {
		eh.createCallMultipart(w, r)
		return
	}
	eh.createCallJSON(w, r)
}

func (eh *ExternalCallHandler) createCallJSON(w http.ResponseWriter, r *http.Request) {
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
	mediaMode := entity.MediaMode(req.Media.Mode)

	// Default call type inference for video_file.
	if callType == "" {
		if mediaMode == entity.MediaModeVideoFile {
			callType = entity.CallTypeVideo
		} else {
			callType = entity.CallTypeAudio
		}
	}
	// Validate call type vs media mode.
	if mediaMode == entity.MediaModeVideoFile {
		if callType != entity.CallTypeVideo && callType != entity.CallTypeGroupVideo {
			// Allow audio -> coerce to video for convenience.
			if callType == entity.CallTypeAudio {
				callType = entity.CallTypeVideo
			} else {
				eh.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]string{
					"error": "invalid_target", "message": "video_file requires type video or group_video",
				})
				return
			}
		}
	} else {
		if callType != entity.CallTypeAudio && callType != entity.CallTypeVideo && callType != entity.CallTypeGroupAudio && callType != entity.CallTypeGroupVideo {
			eh.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]string{
				"error": "invalid_target", "message": "invalid call type",
			})
			return
		}
	}

	// Default hangup_after_playback: true for video_file when not explicitly set.
	hangup := false
	if req.HangupAfterPlayback != nil {
		hangup = *req.HangupAfterPlayback
	} else if mediaMode == entity.MediaModeVideoFile {
		hangup = true
	}

	// Resolve the media source BEFORE dialing (PRD §26).
	var audio *call.AudioResult
	var video *call.VideoResult
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
	case entity.MediaModeVideoFile:
		if req.Media.VideoURL == "" {
			eh.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]string{
				"error": "invalid_media", "message": "video_file media requires video_url",
			})
			return
		}
		res, err := call.PrepareVideoFile(r.Context(), req.Media.VideoURL)
		if err != nil {
			if errors.Is(err, call.ErrUnsupportedVideo) {
				eh.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]string{
					"error": "unsupported_video", "message": "unsupported video format (mp4/webm/avi)",
				})
				return
			}
			if errors.Is(err, call.ErrBlockedAddress) {
				eh.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]string{
					"error": "video_download_failed", "message": "video source address is blocked",
				})
				return
			}
			eh.handler.sendJSONWithStatus(w, http.StatusBadGateway, map[string]string{
				"error": "video_download_failed", "message": "failed to fetch video file",
			})
			return
		}
		video = res
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
		Video:               video,
		HangupAfterPlayback: hangup,
		RingTimeout:         ringTimeout,
		APIKeyID:            middleware.APIKeyIDFromContext(r.Context()),
	})
	if err != nil {
		// The source was resolved but the call could not start: release it.
		if audio != nil && audio.Cleanup != nil {
			audio.Cleanup()
		}
		if video != nil && video.Cleanup != nil {
			video.Cleanup()
		}
		writeExternalCallError(eh.handler, w, err)
		return
	}

	eh.handler.sendJSONWithStatus(w, http.StatusAccepted, map[string]interface{}{
		"id":     state.ID,
		"status": "preparing",
	})
}

func (eh *ExternalCallHandler) createCallMultipart(w http.ResponseWriter, r *http.Request) {
	// Limit multipart body to 55MB.
	r.Body = http.MaxBytesReader(w, r.Body, maxExternalMultipartBytes)
	if err := r.ParseMultipartForm(maxExternalMultipartBytes); err != nil {
		eh.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]string{
			"error": "invalid_request", "message": "multipart body too large or invalid",
		})
		return
	}

	target := strings.TrimSpace(r.FormValue("target"))
	if target == "" {
		target = strings.TrimSpace(r.FormValue("Target"))
	}
	if target == "" {
		eh.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]string{
			"error": "invalid_target", "message": "target is required",
		})
		return
	}

	typeStr := strings.TrimSpace(r.FormValue("type"))
	if typeStr == "" {
		typeStr = strings.TrimSpace(r.FormValue("Type"))
	}
	mediaModeStr := strings.TrimSpace(r.FormValue("media_mode"))
	if mediaModeStr == "" {
		mediaModeStr = strings.TrimSpace(r.FormValue("mode"))
		if mediaModeStr == "" {
			mediaModeStr = strings.TrimSpace(r.FormValue("media.mode"))
		}
	}
	// Default media mode to video_file for multipart (that's the upload use-case).
	if mediaModeStr == "" {
		mediaModeStr = string(entity.MediaModeVideoFile)
	}
	mediaMode := entity.MediaMode(mediaModeStr)
	if mediaMode != entity.MediaModeVideoFile && mediaMode != entity.MediaModeAudioFile && mediaMode != entity.MediaModeTTS {
		// For multipart, only video_file and audio_file make sense; but allow video_file default.
		if mediaModeStr != "" {
			eh.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]string{
				"error": "invalid_media", "message": "unsupported media mode for upload",
			})
			return
		}
		mediaMode = entity.MediaModeVideoFile
	}

	callType := entity.CallType(typeStr)
	if callType == "" {
		if mediaMode == entity.MediaModeVideoFile {
			callType = entity.CallTypeVideo
		} else {
			callType = entity.CallTypeAudio
		}
	}
	if mediaMode == entity.MediaModeVideoFile && callType == entity.CallTypeAudio {
		callType = entity.CallTypeVideo
	}

	// hangup_after_playback: default true for video_file.
	hangup := true
	if v := r.FormValue("hangup_after_playback"); v != "" {
		b, err := strconv.ParseBool(v)
		if err == nil {
			hangup = b
		}
	} else if mediaMode != entity.MediaModeVideoFile {
		hangup = false
	}

	videoURL := strings.TrimSpace(r.FormValue("video_url"))
	if videoURL == "" {
		videoURL = strings.TrimSpace(r.FormValue("videoUrl"))
	}

	var audio *call.AudioResult
	var video *call.VideoResult

	// Try file upload first (field "file" consistent with message_handler.go:90).
	file, header, err := r.FormFile("file")
	hasFile := err == nil && header != nil
	if hasFile {
		defer file.Close()
		if header.Size > call.MaxVideoFileSize && header.Size != 0 {
			eh.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]string{
				"error": "video_too_large", "message": "video file too large (max 50MB)",
			})
			return
		}
		// Enforce max via LimitReader as well.
		tmp, tmpErr := os.CreateTemp("", "wa-video-upload-*")
		if tmpErr != nil {
			eh.handler.sendJSONWithStatus(w, http.StatusInternalServerError, map[string]string{
				"error": "internal_error", "message": "failed to store upload",
			})
			return
		}
		tmpPath := tmp.Name()
		// Copy with size cap.
		limited := io.LimitReader(file, call.MaxVideoFileSize+1)
		n, copyErr := io.Copy(tmp, limited)
		tmp.Close()
		if copyErr != nil {
			_ = os.Remove(tmpPath)
			eh.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]string{
				"error": "video_download_failed", "message": "failed to read upload",
			})
			return
		}
		if n > call.MaxVideoFileSize {
			_ = os.Remove(tmpPath)
			eh.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]string{
				"error": "video_too_large", "message": "video file too large (max 50MB)",
			})
			return
		}
		res, vErr := call.ValidateAndWrapVideoFile(tmpPath)
		if vErr != nil {
			_ = os.Remove(tmpPath)
			if errors.Is(vErr, call.ErrUnsupportedVideo) {
				eh.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]string{
					"error": "unsupported_video", "message": "unsupported video format (mp4/webm/avi)",
				})
				return
			}
			eh.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]string{
				"error": "invalid_media", "message": "invalid video file",
			})
			return
		}
		if mediaMode == entity.MediaModeVideoFile {
			video = res
		} else if mediaMode == entity.MediaModeAudioFile {
			// For audio_file multipart, reuse ValidateAndWrapVideoFile? No, need audio.
			_ = os.Remove(tmpPath)
			eh.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]string{
				"error": "invalid_media", "message": "use video_file mode for video uploads",
			})
			res.Cleanup()
			return
		} else {
			video = res
		}
	} else if videoURL != "" && mediaMode == entity.MediaModeVideoFile {
		res, vErr := call.PrepareVideoFile(r.Context(), videoURL)
		if vErr != nil {
			if errors.Is(vErr, call.ErrUnsupportedVideo) {
				eh.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]string{
					"error": "unsupported_video", "message": "unsupported video format (mp4/webm/avi)",
				})
				return
			}
			if errors.Is(vErr, call.ErrBlockedAddress) {
				eh.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]string{
					"error": "video_download_failed", "message": "video source address is blocked",
				})
				return
			}
			eh.handler.sendJSONWithStatus(w, http.StatusBadGateway, map[string]string{
				"error": "video_download_failed", "message": "failed to fetch video file",
			})
			return
		}
		video = res
	} else if mediaMode == entity.MediaModeVideoFile {
		eh.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]string{
			"error": "invalid_media", "message": "video_file requires file upload (field 'file') or video_url",
		})
		return
	} else {
		eh.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]string{
			"error": "invalid_media", "message": "unsupported media mode for multipart",
		})
		return
	}

	var ringTimeout time.Duration
	if v := r.FormValue("ring_timeout_seconds"); v != "" {
		if n, perr := strconv.Atoi(v); perr == nil {
			ringTimeout = time.Duration(n) * time.Second
		}
	}

	state, svcErr := eh.callSvc.StartExternalCall(r.Context(), call.ExternalCallRequest{
		Target:              target,
		Type:                callType,
		MediaMode:           mediaMode,
		Audio:               audio,
		Video:               video,
		HangupAfterPlayback: hangup,
		RingTimeout:         ringTimeout,
		APIKeyID:            middleware.APIKeyIDFromContext(r.Context()),
	})
	if svcErr != nil {
		if audio != nil && audio.Cleanup != nil {
			audio.Cleanup()
		}
		if video != nil && video.Cleanup != nil {
			video.Cleanup()
		}
		writeExternalCallError(eh.handler, w, svcErr)
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

package handlers

import (
	"errors"
	"net/http"
	"strconv"

	"wa-bot/internal/domain/entity"
	"wa-bot/internal/domain/repository"
	"wa-bot/internal/infrastructure/call"
)

// CallHandler exposes the internal call REST API.
type CallHandler struct {
	handler *Handler
	callSvc *call.CallService
}

// NewCallHandler builds the call handler from the shared container.
func NewCallHandler(h *Handler) *CallHandler {
	return &CallHandler{
		handler: h,
		callSvc: h.GetCallService(),
	}
}

// GetActiveCall returns the current active call state (200 with null if none).
func (ch *CallHandler) GetActiveCall(w http.ResponseWriter, r *http.Request) {
	state := ch.callSvc.GetActiveCall()
	ch.handler.sendJSON(w, state)
}

// CreateCall places an outgoing direct call.
func (ch *CallHandler) CreateCall(w http.ResponseWriter, r *http.Request) {
	var req entity.CreateCallRequest
	if err := ch.handler.readJSON(r, &req); err != nil {
		ch.handler.sendError(w, http.StatusBadRequest, "invalid request body")
		return
	}

	state, err := ch.callSvc.StartCall(r.Context(), req, entity.CallSourceUI)
	if err != nil {
		writeCallError(ch.handler, w, err)
		return
	}
	ch.handler.sendJSONWithStatus(w, http.StatusCreated, state)
}

// CreateGroupCall places an outgoing group call (Phase 5).
func (ch *CallHandler) CreateGroupCall(w http.ResponseWriter, r *http.Request) {
	var req entity.CreateGroupCallRequest
	if err := ch.handler.readJSON(r, &req); err != nil {
		ch.handler.sendError(w, http.StatusBadRequest, "invalid request body")
		return
	}
	state, err := ch.callSvc.StartGroupCall(r.Context(), req, entity.CallSourceUI)
	if err != nil {
		writeCallError(ch.handler, w, err)
		return
	}
	ch.handler.sendJSONWithStatus(w, http.StatusCreated, state)
}

// AddCallParticipants adds one or more participants to an active group call.
func (ch *CallHandler) AddCallParticipants(w http.ResponseWriter, r *http.Request) {
	id := ch.handler.getJID(r, "id")
	var req entity.AddParticipantRequest
	if err := ch.handler.readJSON(r, &req); err != nil {
		ch.handler.sendError(w, http.StatusBadRequest, "invalid request body")
		return
	}
	if len(req.Targets) == 0 {
		ch.handler.sendError(w, http.StatusBadRequest, "targets required")
		return
	}
	if errs := ch.callSvc.AddParticipants(r.Context(), id, req.Targets); len(errs) > 0 {
		writeCallError(ch.handler, w, errs[0])
		return
	}
	ch.handler.sendJSON(w, map[string]string{"status": "participants_added"})
}

// RingCallParticipant rings a specific participant in an active group call.
func (ch *CallHandler) RingCallParticipant(w http.ResponseWriter, r *http.Request) {
	id := ch.handler.getJID(r, "id")
	target := ch.handler.getQueryParam(r, "target")
	if target == "" {
		ch.handler.sendError(w, http.StatusBadRequest, "target required")
		return
	}
	if err := ch.callSvc.RingParticipant(r.Context(), id, target); err != nil {
		writeCallError(ch.handler, w, err)
		return
	}
	ch.handler.sendJSON(w, map[string]string{"status": "ringing"})
}

// AnswerCall answers the active call.
func (ch *CallHandler) AnswerCall(w http.ResponseWriter, r *http.Request) {
	id := ch.handler.getJID(r, "id")
	if err := ch.callSvc.AnswerCall(r.Context(), id); err != nil {
		writeCallError(ch.handler, w, err)
		return
	}
	ch.handler.sendJSON(w, map[string]string{"status": "answered"})
}

// RejectCall rejects the active call.
func (ch *CallHandler) RejectCall(w http.ResponseWriter, r *http.Request) {
	id := ch.handler.getJID(r, "id")
	if err := ch.callSvc.RejectCall(r.Context(), id); err != nil {
		writeCallError(ch.handler, w, err)
		return
	}
	ch.handler.sendJSON(w, map[string]string{"status": "rejected"})
}

// HangupCall ends the active call.
func (ch *CallHandler) HangupCall(w http.ResponseWriter, r *http.Request) {
	id := ch.handler.getJID(r, "id")
	// Internal/UI path: no API key ownership applies.
	if err := ch.callSvc.HangupCall(r.Context(), id, "", false); err != nil {
		writeCallError(ch.handler, w, err)
		return
	}
	ch.handler.sendJSON(w, map[string]string{"status": "ended"})
}

// StartVideo requests an audio→video upgrade on the active call.
func (ch *CallHandler) StartVideo(w http.ResponseWriter, r *http.Request) {
	id := ch.handler.getJID(r, "id")
	if err := ch.callSvc.StartVideo(r.Context(), id); err != nil {
		writeCallError(ch.handler, w, err)
		return
	}
	ch.handler.sendJSON(w, map[string]string{"status": "video_started"})
}

// AcceptVideo accepts a pending peer video upgrade request.
func (ch *CallHandler) AcceptVideo(w http.ResponseWriter, r *http.Request) {
	id := ch.handler.getJID(r, "id")
	if err := ch.callSvc.AcceptVideo(r.Context(), id); err != nil {
		writeCallError(ch.handler, w, err)
		return
	}
	ch.handler.sendJSON(w, map[string]string{"status": "video_accepted"})
}

// StopVideo stops this client's outbound video without ending the audio call.
func (ch *CallHandler) StopVideo(w http.ResponseWriter, r *http.Request) {
	id := ch.handler.getJID(r, "id")
	if err := ch.callSvc.StopVideo(r.Context(), id); err != nil {
		writeCallError(ch.handler, w, err)
		return
	}
	ch.handler.sendJSON(w, map[string]string{"status": "video_stopped"})
}

// RejectVideo declines a pending peer video upgrade request.
func (ch *CallHandler) RejectVideo(w http.ResponseWriter, r *http.Request) {
	id := ch.handler.getJID(r, "id")
	if err := ch.callSvc.RejectVideo(r.Context(), id); err != nil {
		writeCallError(ch.handler, w, err)
		return
	}
	ch.handler.sendJSON(w, map[string]string{"status": "video_rejected"})
}

// GetHistory returns the call history.
func (ch *CallHandler) GetHistory(w http.ResponseWriter, r *http.Request) {
	filter := repository.CallHistoryFilter{}

	if limit, err := strconv.Atoi(ch.handler.getQueryParam(r, "limit")); err == nil && limit > 0 {
		filter.Limit = limit
	}
	if before, err := strconv.ParseInt(ch.handler.getQueryParam(r, "before"), 10, 64); err == nil {
		filter.Before = &before
	}
	filter.Direction = entity.CallDirection(ch.handler.getQueryParam(r, "direction"))
	filter.Type = entity.CallType(ch.handler.getQueryParam(r, "type"))
	filter.Status = entity.CallStatus(ch.handler.getQueryParam(r, "status"))
	filter.Target = ch.handler.getQueryParam(r, "target")

	logs, err := ch.callSvc.GetHistory(r.Context(), filter)
	if err != nil {
		ch.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}
	ch.handler.sendJSON(w, map[string]interface{}{
		"logs": logs,
	})
}

// writeCallError maps call service sentinel errors to HTTP responses.
func writeCallError(h *Handler, w http.ResponseWriter, err error) {
	switch {
	case errors.Is(err, call.ErrCallAlreadyActive):
		h.sendJSONWithStatus(w, http.StatusConflict, map[string]string{"error": "call_already_active", "message": "a call is already active"})
	case errors.Is(err, call.ErrCallNotFound):
		h.sendJSONWithStatus(w, http.StatusNotFound, map[string]string{"error": "call_not_found", "message": "call not found"})
	case errors.Is(err, call.ErrCallNotActive):
		h.sendJSONWithStatus(w, http.StatusConflict, map[string]string{"error": "call_not_active", "message": "call is not active"})
	case errors.Is(err, call.ErrWhatsAppNotConnected):
		h.sendJSONWithStatus(w, http.StatusServiceUnavailable, map[string]string{"error": "whatsapp_not_connected", "message": "whatsapp client is not connected"})
	case errors.Is(err, call.ErrInvalidTarget):
		h.sendJSONWithStatus(w, http.StatusBadRequest, map[string]string{"error": "invalid_target", "message": "invalid target"})
	case errors.Is(err, call.ErrCallNotOwned):
		h.sendJSONWithStatus(w, http.StatusForbidden, map[string]string{"error": "call_not_owned", "message": "you do not own this call"})
	case errors.Is(err, call.ErrNotImplemented), errors.Is(err, call.ErrGroupNotSupported):
		h.sendJSONWithStatus(w, http.StatusNotImplemented, map[string]string{"error": "not_implemented", "message": "not implemented"})
	default:
		h.sendError(w, http.StatusInternalServerError, err.Error())
	}
}

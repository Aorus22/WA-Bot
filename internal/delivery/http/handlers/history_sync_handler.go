package handlers

import (
	"encoding/json"
	"io"
	"net/http"
	"time"

	"wa-bot/internal/domain/repository"
)

type HistorySyncHandler struct{ handler *Handler }

func NewHistorySyncHandler(handler *Handler) *HistorySyncHandler {
	return &HistorySyncHandler{handler: handler}
}

// historySyncRequest is the optional body of POST /api/history-sync. An empty
// body keeps the original behaviour: up to 50 older messages per chat.
type historySyncRequest struct {
	// PerChat caps the older messages added per chat (0 = no cap).
	PerChat int `json:"perChat"`
	// SinceDays only goes back this many days (0 = no time limit).
	SinceDays int `json:"sinceDays"`
	// All syncs everything the phone will give: no cap and no time limit.
	All bool `json:"all"`
	// RetryFinished also asks again for chats an earlier sync found to be at
	// the start of their history (or that the phone never answered for).
	RetryFinished bool `json:"retryFinished"`
	// Resume continues the stopped or interrupted sync instead of starting
	// one; the other fields are then ignored.
	Resume bool `json:"resume"`
}

const maxHistoryPerChat = 100000
const maxHistorySinceDays = 36500

func (h *HistorySyncHandler) Start(w http.ResponseWriter, r *http.Request) {
	if h.handler.historySync == nil {
		h.handler.sendError(w, http.StatusServiceUnavailable, "history sync is not configured")
		return
	}
	var req historySyncRequest
	if r.Body != nil {
		if err := json.NewDecoder(io.LimitReader(r.Body, 4096)).Decode(&req); err != nil && err != io.EOF {
			h.handler.sendError(w, http.StatusBadRequest, "invalid request body")
			return
		}
	}
	if req.Resume {
		h.resume(w)
		return
	}
	if req.PerChat < 0 || req.PerChat > maxHistoryPerChat || req.SinceDays < 0 || req.SinceDays > maxHistorySinceDays {
		h.handler.sendError(w, http.StatusBadRequest, "perChat or sinceDays out of range")
		return
	}
	opts := repository.HistorySyncOptions{PerChat: req.PerChat, All: req.All, IncludeExhausted: req.RetryFinished}
	if req.SinceDays > 0 {
		opts.SinceMs = time.Now().Add(-time.Duration(req.SinceDays) * 24 * time.Hour).UnixMilli()
	}
	status, err := h.handler.historySync.Start(opts)
	if err != nil {
		if status.State == "running" || status.State == "cancelling" {
			h.handler.sendJSONWithStatus(w, http.StatusConflict, status)
		} else {
			h.handler.sendJSONWithStatus(w, http.StatusServiceUnavailable, map[string]interface{}{
				"error":  err.Error(),
				"status": status,
			})
		}
		return
	}
	h.handler.sendJSONWithStatus(w, http.StatusAccepted, status)
}

func (h *HistorySyncHandler) resume(w http.ResponseWriter) {
	status, err := h.handler.historySync.Resume()
	switch {
	case err == nil:
		h.handler.sendJSONWithStatus(w, http.StatusAccepted, status)
	case status.State == "running" || status.State == "cancelling":
		h.handler.sendJSONWithStatus(w, http.StatusConflict, status)
	case status.Resumable:
		// There is something to resume but it cannot run now (e.g. not connected).
		h.handler.sendJSONWithStatus(w, http.StatusServiceUnavailable, map[string]interface{}{"error": err.Error(), "status": status})
	default:
		h.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]interface{}{"error": err.Error(), "status": status})
	}
}

// Cancel stops the running sync after the batch it is importing.
func (h *HistorySyncHandler) Cancel(w http.ResponseWriter, _ *http.Request) {
	if h.handler.historySync == nil {
		h.handler.sendError(w, http.StatusServiceUnavailable, "history sync is not configured")
		return
	}
	status, err := h.handler.historySync.Cancel()
	if err != nil {
		h.handler.sendJSONWithStatus(w, http.StatusConflict, status)
		return
	}
	h.handler.sendJSONWithStatus(w, http.StatusAccepted, status)
}

func (h *HistorySyncHandler) Status(w http.ResponseWriter, _ *http.Request) {
	if h.handler.historySync == nil {
		h.handler.sendError(w, http.StatusServiceUnavailable, "history sync is not configured")
		return
	}
	h.handler.sendJSON(w, h.handler.historySync.Status())
}

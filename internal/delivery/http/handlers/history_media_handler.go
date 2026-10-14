package handlers

import (
	"encoding/json"
	"io"
	"net/http"

	"wa-bot/internal/domain/repository"
)

type HistoryMediaHandler struct{ handler *Handler }

func NewHistoryMediaHandler(handler *Handler) *HistoryMediaHandler {
	return &HistoryMediaHandler{handler: handler}
}

type historyMediaRequest struct {
	// Types are the kinds of media to download: image, sticker, audio, video, document.
	Types []string `json:"types"`
	// RetryUnavailable also tries media an earlier download found to be gone.
	RetryUnavailable bool `json:"retryUnavailable"`
	// Resume continues the stopped or interrupted download instead of starting
	// one; the other fields are then ignored.
	Resume bool `json:"resume"`
}

func (h *HistoryMediaHandler) Start(w http.ResponseWriter, r *http.Request) {
	if h.handler.historyMedia == nil {
		h.handler.sendError(w, http.StatusServiceUnavailable, "media download is not configured")
		return
	}
	var req historyMediaRequest
	if r.Body != nil {
		if err := json.NewDecoder(io.LimitReader(r.Body, 4096)).Decode(&req); err != nil && err != io.EOF {
			h.handler.sendError(w, http.StatusBadRequest, "invalid request body")
			return
		}
	}
	var (
		status repository.HistoryMediaStatus
		err    error
	)
	if req.Resume {
		status, err = h.handler.historyMedia.Resume()
	} else {
		status, err = h.handler.historyMedia.Start(repository.HistoryMediaOptions{Types: req.Types, RetryUnavailable: req.RetryUnavailable})
	}
	switch {
	case err == nil:
		h.handler.sendJSONWithStatus(w, http.StatusAccepted, status)
	case status.State == "running" || status.State == "cancelling":
		h.handler.sendJSONWithStatus(w, http.StatusConflict, status)
	case req.Resume && !status.Resumable:
		h.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]interface{}{"error": err.Error(), "status": status})
	case !req.Resume && h.handler.historyMediaOptionsInvalid(req.Types):
		h.handler.sendJSONWithStatus(w, http.StatusBadRequest, map[string]interface{}{"error": err.Error(), "status": status})
	default:
		// Something that cannot run now, e.g. WhatsApp is not connected.
		h.handler.sendJSONWithStatus(w, http.StatusServiceUnavailable, map[string]interface{}{"error": err.Error(), "status": status})
	}
}

// historyMediaOptionsInvalid: the request names no kind of media, or one that does not exist.
func (h *Handler) historyMediaOptionsInvalid(types []string) bool {
	if len(types) == 0 {
		return true
	}
	for _, kind := range types {
		if _, ok := repository.HistoryMediaGroups[kind]; !ok {
			return true
		}
	}
	return false
}

// Cancel stops the download after the files being fetched.
func (h *HistoryMediaHandler) Cancel(w http.ResponseWriter, _ *http.Request) {
	if h.handler.historyMedia == nil {
		h.handler.sendError(w, http.StatusServiceUnavailable, "media download is not configured")
		return
	}
	status, err := h.handler.historyMedia.Cancel()
	if err != nil {
		h.handler.sendJSONWithStatus(w, http.StatusConflict, status)
		return
	}
	h.handler.sendJSONWithStatus(w, http.StatusAccepted, status)
}

func (h *HistoryMediaHandler) Status(w http.ResponseWriter, _ *http.Request) {
	if h.handler.historyMedia == nil {
		h.handler.sendError(w, http.StatusServiceUnavailable, "media download is not configured")
		return
	}
	h.handler.sendJSON(w, h.handler.historyMedia.Status())
}

package handlers

import "net/http"

type HistorySyncHandler struct{ handler *Handler }

func NewHistorySyncHandler(handler *Handler) *HistorySyncHandler {
	return &HistorySyncHandler{handler: handler}
}

func (h *HistorySyncHandler) Start(w http.ResponseWriter, r *http.Request) {
	if h.handler.historySync == nil {
		h.handler.sendError(w, http.StatusServiceUnavailable, "history sync is not configured")
		return
	}
	status, err := h.handler.historySync.Start()
	if err != nil {
		if status.State == "running" {
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

func (h *HistorySyncHandler) Status(w http.ResponseWriter, _ *http.Request) {
	if h.handler.historySync == nil {
		h.handler.sendError(w, http.StatusServiceUnavailable, "history sync is not configured")
		return
	}
	h.handler.sendJSON(w, h.handler.historySync.Status())
}

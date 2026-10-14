package handlers

import (
	"net/http"

	"wa-bot/internal/domain/repository"
)

type StorageUsageHandler struct{ handler *Handler }

func NewStorageUsageHandler(handler *Handler) *StorageUsageHandler {
	return &StorageUsageHandler{handler: handler}
}

// storageUsage is how big the two SQLite databases of the account are. A size
// is null when it is not known (no database yet, or media kept in S3).
type storageUsage struct {
	// ChatBytes is the message database: chats, messages, staged history.
	ChatBytes *int64 `json:"chatBytes"`
	// MediaBytes is the database holding the media files.
	MediaBytes *int64 `json:"mediaBytes"`
}

// Get answers GET /api/storage/usage.
func (h *StorageUsageHandler) Get(w http.ResponseWriter, r *http.Request) {
	var usage storageUsage
	if h.handler.msgRepo != nil {
		if size, err := h.handler.msgRepo.DatabaseSize(); err == nil {
			usage.ChatBytes = &size
		}
	}
	if sizer, ok := h.handler.storage.(repository.MediaSizer); ok {
		if size, known, err := sizer.MediaSize(r.Context()); err == nil && known {
			usage.MediaBytes = &size
		}
	}
	h.handler.sendJSON(w, usage)
}

package handlers

import (
	"encoding/json"
	"net/http"
	"os"

	"wa-bot/internal/delivery/http/dto"
)

type StickerHandler struct {
	handler *Handler
}

func NewStickerHandler(h *Handler) *StickerHandler {
	return &StickerHandler{handler: h}
}

func (sh *StickerHandler) GetFavorites(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if sh.handler.msgRepo == nil {
		sh.handler.sendError(w, http.StatusInternalServerError, "Message repository not configured")
		return
	}

	stickers, err := sh.handler.msgRepo.GetFavoriteStickers()
	if err != nil {
		sh.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	if stickers == nil {
		stickers = []map[string]interface{}{}
	}

	sh.handler.sendJSON(w, stickers)
}

func (sh *StickerHandler) FavoriteSticker(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	var req dto.FavoriteStickerRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		sh.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}

	SECRET := os.Getenv("API_SECRET")
	if SECRET == "" {
		SECRET = "default-secret"
	}

	if req.Secret != SECRET {
		sh.handler.sendError(w, http.StatusUnauthorized, "Unauthorized")
		return
	}

	if sh.handler.msgRepo != nil {
		err := sh.handler.msgRepo.SaveFavoriteSticker(req.MessageID, req.MediaURL, req.IsAnimated)
		if err != nil {
			sh.handler.sendError(w, http.StatusInternalServerError, err.Error())
			return
		}
	}

	w.WriteHeader(http.StatusOK)
	sh.handler.sendSuccess(w, nil)
}

func (sh *StickerHandler) DeleteFavorite(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	id := sh.handler.getJID(r, "id")

	if sh.handler.msgRepo != nil {
		err := sh.handler.msgRepo.DeleteFavoriteSticker(id)
		if err != nil {
			sh.handler.sendError(w, http.StatusInternalServerError, err.Error())
			return
		}
	}

	w.WriteHeader(http.StatusOK)
	sh.handler.sendSuccess(w, nil)
}

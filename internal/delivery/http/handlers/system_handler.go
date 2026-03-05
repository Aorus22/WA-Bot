package handlers

import (
	"context"
	"net/http"
)

type SystemHandler struct {
	handler *Handler
}

func NewSystemHandler(h *Handler) *SystemHandler {
	return &SystemHandler{handler: h}
}

func (sh *SystemHandler) GetStatus(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	isLoggedIn := false
	if sh.handler.client != nil {
		isLoggedIn = sh.handler.client.IsLoggedIn()
	}

	sh.handler.sendJSON(w, map[string]interface{}{
		"isLoggedIn": isLoggedIn,
	})
}

func (sh *SystemHandler) Logout(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	if sh.handler.client == nil {
		sh.handler.sendError(w, http.StatusInternalServerError, "WhatsApp client not initialized")
		return
	}

	err := sh.handler.client.Logout()
	if err != nil {
		sh.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	sh.handler.sendSuccess(w, nil)
}

func (sh *SystemHandler) AvatarProxy(w http.ResponseWriter, r *http.Request) {
	jid := sh.handler.getJID(r, "jid")

	avatarURL, err := sh.handler.client.GetProfilePictureInfo(context.Background(), jid)
	if err != nil {
		http.Error(w, "Avatar not found", http.StatusNotFound)
		return
	}

	http.Redirect(w, r, avatarURL, http.StatusFound)
}

func (sh *SystemHandler) HealthCheck(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	health := map[string]interface{}{
		"status": "healthy",
	}

	if sh.handler.client != nil {
		health["whatsapp"] = map[string]interface{}{
			"connected": sh.handler.client.IsLoggedIn(),
		}
	} else {
		health["whatsapp"] = map[string]interface{}{
			"connected": false,
			"error":     "client not initialized",
		}
	}

	if sh.handler.msgRepo != nil {
		health["messages"] = map[string]interface{}{
			"available": true,
		}
	} else {
		health["messages"] = map[string]interface{}{
			"available": false,
		}
	}

	if sh.handler.triggerRepo != nil {
		health["triggers"] = map[string]interface{}{
			"available": true,
		}
	} else {
		health["triggers"] = map[string]interface{}{
			"available": false,
		}
	}

	sh.handler.sendJSON(w, health)
}

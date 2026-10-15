package handlers

import (
	"encoding/json"
	"fmt"
	"net/http"
	"time"

	"github.com/gorilla/mux"

	"wa-bot/internal/delivery/http/dto"
)

type MessageManagementHandler struct {
	handler *Handler
}

func NewMessageManagementHandler(h *Handler) *MessageManagementHandler {
	return &MessageManagementHandler{handler: h}
}

func (mh *MessageManagementHandler) DeleteMessage(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	vars := mux.Vars(r)
	chatID := vars["chatId"]
	msgID := vars["id"]

	err := mh.handler.client.DeleteMessage(r.Context(), chatID, msgID)
	if err != nil {
		mh.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	// Deleted for everyone: the message stays in our database, flagged.
	revokedAt := time.Now().UnixMilli()
	if mh.handler.msgRepo != nil {
		if _, err := mh.handler.msgRepo.MarkMessageRevoked(msgID, revokedAt); err != nil {
			fmt.Printf("[REVOKE] Failed to flag %s: %v\n", msgID, err)
		}
	}

	mh.handler.BroadcastMessage("message_revoked", map[string]interface{}{
		"chatId":    chatID,
		"id":        msgID,
		"revokedAt": revokedAt,
	})

	mh.handler.sendSuccess(w, nil)
}

func (mh *MessageManagementHandler) EditMessage(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	vars := mux.Vars(r)
	chatID := vars["chatId"]
	msgID := vars["id"]

	var req dto.EditMessageRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		mh.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}

	err := mh.handler.client.EditMessage(r.Context(), chatID, msgID, req.Content)
	if err != nil {
		mh.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	payload := map[string]interface{}{
		"chatId":  chatID,
		"id":      msgID,
		"content": req.Content,
	}
	if mh.handler.msgRepo != nil {
		if extra, err := mh.handler.msgRepo.ApplyEdit(msgID, req.Content, 0); err == nil {
			payload["extra"] = extra
			if chat, changed := mh.handler.msgRepo.SyncChatPreview(msgID, req.Content); changed {
				mh.handler.BroadcastMessage("chat_update", map[string]interface{}{
					"chatId":  mh.handler.msgRepo.ResolveChatID(chat),
					"lastMsg": req.Content,
				})
			}
		} else {
			mh.handler.msgRepo.UpdateMessageContent(msgID, req.Content)
		}
	}

	mh.handler.BroadcastMessage("message_edited", payload)

	mh.handler.sendSuccess(w, nil)
}

func (mh *MessageManagementHandler) ReplyMessage(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	vars := mux.Vars(r)
	chatID := vars["chatId"]
	msgID := vars["id"]

	var req dto.ReplyMessageRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		mh.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}

	participant := ""
	if mh.handler.msgRepo != nil {
		if original, err := mh.handler.msgRepo.GetMessageByID(msgID); err == nil && original.ChatID == chatID {
			participant = original.From
		}
	}
	err := mh.handler.client.ReplyMessageWithMentions(r.Context(), chatID, msgID, participant, req.Content, req.Mentions)
	if err != nil {
		mh.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	mh.handler.sendSuccess(w, nil)
}

package handlers

import (
	"encoding/json"
	"net/http"

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

	if mh.handler.msgRepo != nil {
		mh.handler.msgRepo.DeleteMessage(msgID)
	}

	mh.handler.BroadcastMessage("message_deleted", map[string]string{
		"chatId": chatID,
		"id":     msgID,
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

	if mh.handler.msgRepo != nil {
		mh.handler.msgRepo.UpdateMessageContent(msgID, req.Content)
	}

	mh.handler.BroadcastMessage("message_edited", map[string]string{
		"chatId":  chatID,
		"id":      msgID,
		"content": req.Content,
	})

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

	err := mh.handler.client.ReplyMessage(r.Context(), chatID, msgID, req.Content)
	if err != nil {
		mh.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	mh.handler.sendSuccess(w, nil)
}

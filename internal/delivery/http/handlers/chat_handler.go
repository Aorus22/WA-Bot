package handlers

import (
	"fmt"
	"net/http"

	"github.com/gorilla/mux"

	"wa-bot/internal/domain/repository"
)

type ChatHandler struct {
	handler *Handler
}

func NewChatHandler(h *Handler) *ChatHandler {
	return &ChatHandler{handler: h}
}

func (ch *ChatHandler) GetChats(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if ch.handler.msgRepo == nil {
		ch.handler.sendError(w, http.StatusInternalServerError, "Message repository not configured")
		return
	}

	chats, err := ch.handler.msgRepo.GetChats()
	if err != nil {
		fmt.Printf("Error getting chats: %v\n", err)
		ch.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	if chats == nil {
		chats = []repository.Chat{}
	}

	ch.handler.sendJSON(w, chats)
}

func (ch *ChatHandler) GetMessages(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if ch.handler.msgRepo == nil {
		ch.handler.sendError(w, http.StatusNotImplemented, "Message repository not configured")
		return
	}

	vars := mux.Vars(r)
	chatID := vars["id"]

	limit := 100
	limitStr := r.URL.Query().Get("limit")
	if limitStr != "" {
		fmt.Sscanf(limitStr, "%d", &limit)
	}

	var before int64
	beforeStr := r.URL.Query().Get("before")
	if beforeStr != "" {
		fmt.Sscanf(beforeStr, "%d", &before)
	}

	var after int64
	afterStr := r.URL.Query().Get("after")
	if afterStr != "" {
		fmt.Sscanf(afterStr, "%d", &after)
	}

	messages, err := ch.handler.msgRepo.GetMessages(chatID, limit, before, after)
	if err != nil {
		ch.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	ch.handler.sendJSON(w, messages)
}

func (ch *ChatHandler) SearchMessages(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if ch.handler.msgRepo == nil {
		ch.handler.sendError(w, http.StatusNotImplemented, "Message repository not configured")
		return
	}

	vars := mux.Vars(r)
	chatID := vars["id"]
	query := r.URL.Query().Get("q")

	limit := 50
	limitStr := r.URL.Query().Get("limit")
	if limitStr != "" {
		fmt.Sscanf(limitStr, "%d", &limit)
	}

	messages, err := ch.handler.msgRepo.SearchMessages(chatID, query, limit)
	if err != nil {
		ch.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	ch.handler.sendJSON(w, messages)
}

func (ch *ChatHandler) GetMessageContext(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if ch.handler.msgRepo == nil {
		ch.handler.sendError(w, http.StatusNotImplemented, "Message repository not configured")
		return
	}

	vars := mux.Vars(r)
	chatID := vars["id"]
	msgID := vars["msgId"]

	limit := 50
	limitStr := r.URL.Query().Get("limit")
	if limitStr != "" {
		fmt.Sscanf(limitStr, "%d", &limit)
	}

	messages, err := ch.handler.msgRepo.GetMessageContext(chatID, msgID, limit)
	if err != nil {
		ch.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	ch.handler.sendJSON(w, messages)
}
func (ch *ChatHandler) MarkAsRead(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	if ch.handler.msgRepo == nil {
		ch.handler.sendError(w, http.StatusInternalServerError, "Message repository not configured")
		return
	}

	vars := mux.Vars(r)
	chatID := vars["id"]

	err := ch.handler.msgRepo.MarkAsRead(chatID)
	if err != nil {
		ch.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	ch.handler.sendSuccess(w, nil)
}

func (ch *ChatHandler) GetContacts(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if ch.handler.msgRepo == nil {
		ch.handler.sendError(w, http.StatusNotImplemented, "Message repository not configured")
		return
	}

	contacts, err := ch.handler.msgRepo.GetContacts()
	if err != nil {
		ch.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	ch.handler.sendJSON(w, contacts)
}

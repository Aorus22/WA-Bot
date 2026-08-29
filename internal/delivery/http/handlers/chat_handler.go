package handlers

import (
	"context"
	"fmt"
	"net/http"
	"time"

	"github.com/gorilla/mux"

	"wa-bot/internal/domain/repository"
	whatsappInfra "wa-bot/internal/infrastructure/whatsapp"
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

	// When the read-receipts toggle is on, tell WhatsApp we read the chat.
	if ch.handler.ReadReceiptsEnabled(r.Context()) {
		if targets, terr := ch.handler.msgRepo.GetRecentIncoming(chatID, 100); terr == nil && len(targets) > 0 {
			go func() {
				ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
				defer cancel()
				readTargets := make([]whatsappInfra.ReadTarget, len(targets))
				for i, t := range targets {
					readTargets[i] = whatsappInfra.ReadTarget{ID: t.ID, Sender: t.Sender}
				}
				if serr := ch.handler.client.SendReadReceipts(ctx, chatID, readTargets); serr != nil {
					fmt.Printf("[READ_RECEIPT] %s: %v\n", chatID, serr)
				}
			}()
		}
	}

	ch.handler.sendSuccess(w, nil)
}

// SubscribePresence subscribes to a 1:1 contact's availability updates so
// presence events start flowing for that chat.
func (ch *ChatHandler) SubscribePresence(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	vars := mux.Vars(r)
	chatID := vars["id"]
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()
	if err := ch.handler.client.SubscribeUserPresence(ctx, chatID); err != nil {
		ch.handler.sendError(w, http.StatusBadGateway, err.Error())
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

func (ch *ChatHandler) GetChatMedia(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if ch.handler.msgRepo == nil {
		ch.handler.sendError(w, http.StatusNotImplemented, "Message repository not configured")
		return
	}

	vars := mux.Vars(r)
	chatID := vars["id"]

	limit := 30
	limitStr := r.URL.Query().Get("limit")
	if limitStr != "" {
		fmt.Sscanf(limitStr, "%d", &limit)
	}

	var before int64
	beforeStr := r.URL.Query().Get("before")
	if beforeStr != "" {
		fmt.Sscanf(beforeStr, "%d", &before)
	}

	messages, err := ch.handler.msgRepo.GetChatMedia(chatID, limit, before)
	if err != nil {
		ch.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	ch.handler.sendJSON(w, messages)
}

func (ch *ChatHandler) GetChatDocs(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if ch.handler.msgRepo == nil {
		ch.handler.sendError(w, http.StatusNotImplemented, "Message repository not configured")
		return
	}

	vars := mux.Vars(r)
	chatID := vars["id"]

	limit := 30
	limitStr := r.URL.Query().Get("limit")
	if limitStr != "" {
		fmt.Sscanf(limitStr, "%d", &limit)
	}

	var before int64
	beforeStr := r.URL.Query().Get("before")
	if beforeStr != "" {
		fmt.Sscanf(beforeStr, "%d", &before)
	}

	messages, err := ch.handler.msgRepo.GetChatDocs(chatID, limit, before)
	if err != nil {
		ch.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	ch.handler.sendJSON(w, messages)
}

func (ch *ChatHandler) GetChatLinks(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if ch.handler.msgRepo == nil {
		ch.handler.sendError(w, http.StatusNotImplemented, "Message repository not configured")
		return
	}

	vars := mux.Vars(r)
	chatID := vars["id"]

	limit := 30
	limitStr := r.URL.Query().Get("limit")
	if limitStr != "" {
		fmt.Sscanf(limitStr, "%d", &limit)
	}

	var before int64
	beforeStr := r.URL.Query().Get("before")
	if beforeStr != "" {
		fmt.Sscanf(beforeStr, "%d", &before)
	}

	messages, err := ch.handler.msgRepo.GetChatLinks(chatID, limit, before)
	if err != nil {
		ch.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	ch.handler.sendJSON(w, messages)
}

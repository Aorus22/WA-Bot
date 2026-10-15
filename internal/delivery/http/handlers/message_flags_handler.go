package handlers

import (
	"bytes"
	"context"
	"crypto/sha1"
	"encoding/hex"
	"fmt"
	"net/http"
	"strconv"
	"time"

	"github.com/gorilla/mux"
	waTypes "go.mau.fi/whatsmeow/types"

	"wa-bot/internal/domain/repository"
	whatsappInfra "wa-bot/internal/infrastructure/whatsapp"
)

// MessageFlagsHandler serves starred messages, in-chat pinned messages,
// per-chat disappearing timers and link previews.
type MessageFlagsHandler struct {
	handler *Handler
}

func NewMessageFlagsHandler(h *Handler) *MessageFlagsHandler {
	return &MessageFlagsHandler{handler: h}
}

// storedMessage loads the message a flag action targets and parses its chat.
// The chat is the one WhatsApp knows the message under, which can be a LID
// alias of the chat id used in the URL.
func (fh *MessageFlagsHandler) storedMessage(w http.ResponseWriter, chatID, msgID string) (*repository.Message, waTypes.JID, bool) {
	if fh.handler.msgRepo == nil {
		fh.handler.sendError(w, http.StatusInternalServerError, "Message repository not configured")
		return nil, waTypes.EmptyJID, false
	}
	msg, err := fh.handler.msgRepo.GetMessageByID(msgID)
	if err != nil {
		fh.handler.sendError(w, http.StatusNotFound, "message not found")
		return nil, waTypes.EmptyJID, false
	}
	jid, err := waTypes.ParseJID(msg.ChatID)
	if err != nil || jid.IsEmpty() {
		jid, err = waTypes.ParseJID(chatID)
		if err != nil {
			fh.handler.sendError(w, http.StatusBadRequest, "invalid chat JID")
			return nil, waTypes.EmptyJID, false
		}
	}
	return msg, jid, true
}

func participantOf(msg *repository.Message) string {
	if msg.From == "me" {
		return ""
	}
	return msg.From
}

// Star flags or unflags a message and syncs the star to the phone.
func (fh *MessageFlagsHandler) Star(w http.ResponseWriter, r *http.Request) {
	if r.Method == http.MethodOptions {
		w.WriteHeader(http.StatusOK)
		return
	}
	vars := mux.Vars(r)
	var body struct {
		Starred bool `json:"starred"`
	}
	if err := fh.handler.readJSON(r, &body); err != nil {
		fh.handler.sendError(w, http.StatusBadRequest, "invalid request body")
		return
	}
	msg, jid, ok := fh.storedMessage(w, vars["chatId"], vars["id"])
	if !ok {
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 90*time.Second)
	defer cancel()
	if err := fh.handler.client.SetMessageStarred(ctx, jid, participantOf(msg), msg.ID, msg.From == "me", body.Starred); err != nil {
		fh.handler.sendError(w, http.StatusBadGateway, err.Error())
		return
	}
	if err := fh.handler.msgRepo.SetStarred(msg.ChatID, msg.ID, body.Starred, 0); err != nil {
		fh.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}
	fh.handler.BroadcastMessage("message_starred", map[string]interface{}{
		"chatId":  fh.handler.msgRepo.ResolveChatID(msg.ChatID),
		"id":      msg.ID,
		"starred": body.Starred,
	})
	fh.handler.sendSuccess(w, map[string]interface{}{"starred": body.Starred})
}

// Starred lists starred messages of one chat or of all chats.
func (fh *MessageFlagsHandler) Starred(w http.ResponseWriter, r *http.Request) {
	if fh.handler.msgRepo == nil {
		fh.handler.sendError(w, http.StatusInternalServerError, "Message repository not configured")
		return
	}
	q := r.URL.Query()
	limit, _ := strconv.Atoi(q.Get("limit"))
	before, _ := strconv.ParseInt(q.Get("before"), 10, 64)

	starred, err := fh.handler.msgRepo.GetStarredMessages(q.Get("chatId"), limit, before)
	if err != nil {
		fh.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	messages := make([]repository.Message, len(starred))
	for i := range starred {
		messages[i] = starred[i].Message
	}
	fh.handler.enrichMessagesMentions(messages)

	chatNames := map[string]string{}
	if chats, err := fh.handler.msgRepo.GetChats(); err == nil {
		fh.handler.applyChatNames(chats)
		for _, c := range chats {
			chatNames[c.ID] = c.Name
		}
	}
	for i := range starred {
		starred[i].Message = messages[i]
		starred[i].ChatID = fh.handler.msgRepo.ResolveChatID(starred[i].ChatID)
		starred[i].ChatName = chatNames[starred[i].ChatID]
	}
	fh.handler.sendJSON(w, starred)
}

// Pin pins or unpins a message for everyone in the chat.
func (fh *MessageFlagsHandler) Pin(w http.ResponseWriter, r *http.Request) {
	if r.Method == http.MethodOptions {
		w.WriteHeader(http.StatusOK)
		return
	}
	vars := mux.Vars(r)
	var body struct {
		Pinned   bool  `json:"pinned"`
		Duration int64 `json:"duration"`
	}
	if err := fh.handler.readJSON(r, &body); err != nil {
		fh.handler.sendError(w, http.StatusBadRequest, "invalid request body")
		return
	}
	msg, jid, ok := fh.storedMessage(w, vars["chatId"], vars["id"])
	if !ok {
		return
	}
	duration := repository.NormalizePinDuration(body.Duration)

	ctx, cancel := context.WithTimeout(r.Context(), 60*time.Second)
	defer cancel()
	if err := fh.handler.client.PinInChat(ctx, jid, participantOf(msg), msg.ID, msg.From == "me", body.Pinned, uint32(duration)); err != nil {
		fh.handler.sendError(w, http.StatusBadGateway, err.Error())
		return
	}

	now := time.Now().UnixMilli()
	var err error
	if body.Pinned {
		err = fh.handler.msgRepo.PinMessage(msg.ChatID, msg.ID, "me", now, now+duration*1000)
	} else {
		err = fh.handler.msgRepo.UnpinMessage(msg.ID, now)
	}
	if err != nil {
		fh.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}
	fh.handler.BroadcastMessage("chat_pins_changed", map[string]string{
		"chatId": fh.handler.msgRepo.ResolveChatID(msg.ChatID),
	})
	fh.handler.sendSuccess(w, map[string]interface{}{"pinned": body.Pinned})
}

// Pins lists the chat's active pinned messages, newest first.
func (fh *MessageFlagsHandler) Pins(w http.ResponseWriter, r *http.Request) {
	if fh.handler.msgRepo == nil {
		fh.handler.sendError(w, http.StatusInternalServerError, "Message repository not configured")
		return
	}
	pins, err := fh.handler.msgRepo.GetActivePins(mux.Vars(r)["id"])
	if err != nil {
		fh.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}
	messages := make([]repository.Message, 0, len(pins))
	for _, p := range pins {
		if p.Message != nil {
			messages = append(messages, *p.Message)
		}
	}
	fh.handler.enrichMessagesMentions(messages)
	next := 0
	for i := range pins {
		pins[i].ChatID = fh.handler.msgRepo.ResolveChatID(pins[i].ChatID)
		if pins[i].Message != nil {
			m := messages[next]
			next++
			pins[i].Message = &m
		}
	}
	fh.handler.sendJSON(w, pins)
}

var allowedDisappearing = map[uint32]time.Duration{
	0:       0,
	86400:   24 * time.Hour,
	604800:  7 * 24 * time.Hour,
	7776000: 90 * 24 * time.Hour,
}

// Disappearing changes the chat's disappearing-messages timer. Messages stay
// in our database regardless; the timer only changes what phones do.
func (fh *MessageFlagsHandler) Disappearing(w http.ResponseWriter, r *http.Request) {
	if r.Method == http.MethodOptions {
		w.WriteHeader(http.StatusOK)
		return
	}
	chatID := mux.Vars(r)["chatId"]
	var body struct {
		Seconds uint32 `json:"seconds"`
	}
	if err := fh.handler.readJSON(r, &body); err != nil {
		fh.handler.sendError(w, http.StatusBadRequest, "invalid request body")
		return
	}
	timer, ok := allowedDisappearing[body.Seconds]
	if !ok {
		fh.handler.sendError(w, http.StatusBadRequest, "seconds must be 0, 86400, 604800 or 7776000")
		return
	}
	if fh.handler.msgRepo == nil {
		fh.handler.sendError(w, http.StatusInternalServerError, "Message repository not configured")
		return
	}
	jid, err := waTypes.ParseJID(chatID)
	if err != nil {
		fh.handler.sendError(w, http.StatusBadRequest, "invalid chat JID")
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 60*time.Second)
	defer cancel()
	if err := fh.handler.client.SetDisappearingTimer(ctx, jid, timer); err != nil {
		fh.handler.sendError(w, http.StatusBadGateway, err.Error())
		return
	}
	if _, err := fh.handler.msgRepo.SetChatDisappearing(chatID, body.Seconds); err != nil {
		fh.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}
	fh.handler.BroadcastMessage("chat_update", map[string]interface{}{
		"chatId":           fh.handler.msgRepo.ResolveChatID(chatID),
		"disappearingSecs": body.Seconds,
	})
	fh.handler.sendSuccess(w, map[string]interface{}{"seconds": body.Seconds})
}

// linkPreviewThumbName is a stable per-URL file name so repeated previews of
// the same link reuse one stored thumbnail.
func linkPreviewThumbName(url string) string {
	sum := sha1.Sum([]byte(url))
	return fmt.Sprintf("lp_%s.jpg", hex.EncodeToString(sum[:8]))
}

// saveLinkPreviewThumb stores a scraped thumbnail and returns its /media URL.
func (h *Handler) saveLinkPreviewThumb(ctx context.Context, url string, thumb []byte) string {
	if len(thumb) == 0 || h.storage == nil {
		return ""
	}
	name := linkPreviewThumbName(url)
	if _, err := h.storage.Save(ctx, name, bytes.NewReader(thumb)); err != nil {
		return ""
	}
	return "/media/" + name
}

// LinkPreview scrapes a URL for the composer's preview card.
func (fh *MessageFlagsHandler) LinkPreview(w http.ResponseWriter, r *http.Request) {
	raw := r.URL.Query().Get("url")
	if !whatsappInfra.IsPublicHTTPURL(raw) {
		fh.handler.sendError(w, http.StatusBadRequest, "url must be a public http(s) address")
		return
	}
	ctx, cancel := context.WithTimeout(r.Context(), 8*time.Second)
	defer cancel()
	preview := whatsappInfra.ScrapeLinkPreview(ctx, raw)
	if preview == nil {
		fh.handler.sendError(w, http.StatusNotFound, "no preview available")
		return
	}
	fh.handler.sendJSON(w, repository.LinkPreviewMeta{
		URL:          preview.URL,
		Title:        preview.Title,
		Description:  preview.Description,
		ThumbnailURL: fh.handler.saveLinkPreviewThumb(ctx, preview.URL, preview.JPEGThumbnail),
	})
}

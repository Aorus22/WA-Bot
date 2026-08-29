package handlers

import (
	"context"
	"net/http"
	"strings"
	"time"

	"github.com/gorilla/mux"
	waTypes "go.mau.fi/whatsmeow/types"

	"wa-bot/internal/delivery/http/dto"
	"wa-bot/internal/domain/repository"
	whatsappInfra "wa-bot/internal/infrastructure/whatsapp"
)

// MessageTypeHandler covers the phase-1 parity message types: polls,
// locations, contacts, and forwarding.
type MessageTypeHandler struct {
	handler *Handler
}

func NewMessageTypeHandler(h *Handler) *MessageTypeHandler {
	return &MessageTypeHandler{handler: h}
}

// saveSentRow stores and broadcasts an outgoing message with full metadata
// (extra/forwarded), which LogSentMessage cannot carry.
func (mth *MessageTypeHandler) saveSentRow(chatID, id, content, msgType string, extra *repository.MessageExtra, forwarded bool) {
	mth.handler.SaveAndBroadcastMessage(&repository.Message{
		ID:        id,
		ChatID:    chatID,
		From:      "me",
		To:        chatID,
		Content:   content,
		Timestamp: time.Now().UnixMilli(),
		Status:    "sent",
		Type:      msgType,
		Extra:     extra,
		Forwarded: forwarded,
	})
}

func (mth *MessageTypeHandler) SendPoll(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	chatID := mux.Vars(r)["chatId"]
	var req dto.SendPollRequest
	if err := mth.handler.readJSON(r, &req); err != nil {
		mth.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}
	if req.Secret != "" && !mth.handler.validateSecretValue(req.Secret) {
		mth.handler.sendError(w, http.StatusUnauthorized, "Unauthorized")
		return
	}

	count := 1
	if req.MultiSelect {
		count = len(req.Options)
	}
	id, err := mth.handler.client.SendPoll(r.Context(), chatID, req.Question, req.Options, count)
	if err != nil {
		mth.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	options := make([]repository.PollOption, len(req.Options))
	for i, o := range req.Options {
		options[i] = repository.PollOption{Name: o}
	}
	mth.saveSentRow(chatID, id, "📊 "+req.Question, "poll", &repository.MessageExtra{
		Poll: &repository.PollMeta{
			Question:    req.Question,
			Options:     options,
			MultiSelect: req.MultiSelect,
			Votes:       map[string][]string{},
		},
	}, false)

	mth.handler.sendSuccess(w, map[string]interface{}{"id": id})
}

func (mth *MessageTypeHandler) SendPollVote(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	vars := mux.Vars(r)
	chatID := vars["chatId"]
	msgID := vars["id"]

	var req dto.PollVoteRequest
	if err := mth.handler.readJSON(r, &req); err != nil {
		mth.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}
	if mth.handler.msgRepo == nil {
		mth.handler.sendError(w, http.StatusInternalServerError, "Message repository not configured")
		return
	}

	stored, err := mth.handler.msgRepo.GetMessageByID(msgID)
	if err != nil || stored.Extra == nil || stored.Extra.Poll == nil {
		mth.handler.sendError(w, http.StatusNotFound, "poll message not found")
		return
	}

	chatJID, err := waTypes.ParseJID(chatID)
	if err != nil {
		mth.handler.sendError(w, http.StatusBadRequest, "invalid chat JID")
		return
	}
	pollInfo := waTypes.MessageInfo{
		ID: msgID,
		MessageSource: waTypes.MessageSource{
			Chat:     chatJID,
			IsGroup:  strings.HasSuffix(chatID, "@g.us"),
			IsFromMe: stored.From == "me",
		},
	}
	if !pollInfo.IsFromMe && stored.From != "" {
		if sender, serr := waTypes.ParseJID(stored.From); serr == nil {
			pollInfo.Sender = sender
		}
	}

	if err := mth.handler.client.SendPollVote(r.Context(), pollInfo, req.Options); err != nil {
		mth.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	// Apply our own vote locally; the encrypted vote echo does not come back
	// to the sending device.
	if stored.Extra.Poll.Votes == nil {
		stored.Extra.Poll.Votes = map[string][]string{}
	}
	if len(req.Options) == 0 {
		delete(stored.Extra.Poll.Votes, "me")
	} else {
		stored.Extra.Poll.Votes["me"] = req.Options
	}
	if err := mth.handler.msgRepo.UpdateMessageExtra(msgID, stored.Extra); err == nil {
		mth.handler.BroadcastMessage("poll_update", map[string]interface{}{
			"chatId": chatID,
			"id":     msgID,
			"extra":  stored.Extra,
		})
	}

	mth.handler.sendSuccess(w, nil)
}

func (mth *MessageTypeHandler) SendLocation(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	chatID := mux.Vars(r)["chatId"]
	var req dto.SendLocationRequest
	if err := mth.handler.readJSON(r, &req); err != nil {
		mth.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}
	if req.Secret != "" && !mth.handler.validateSecretValue(req.Secret) {
		mth.handler.sendError(w, http.StatusUnauthorized, "Unauthorized")
		return
	}

	var id string
	var err error
	if req.Live {
		id, err = mth.handler.client.SendLiveLocation(r.Context(), chatID, req.Latitude, req.Longitude, req.Caption, 1)
	} else {
		id, err = mth.handler.client.SendLocation(r.Context(), chatID, req.Latitude, req.Longitude, req.Name, req.Address)
	}
	if err != nil {
		mth.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	content := strings.TrimSpace(req.Name + " " + req.Address)
	if content == "" {
		if req.Live {
			content = "[Lokasi Langsung]"
		} else {
			content = "[Lokasi]"
		}
	}
	mth.saveSentRow(chatID, id, content, "location", &repository.MessageExtra{
		Location: &repository.LocationMeta{
			Latitude:  req.Latitude,
			Longitude: req.Longitude,
			Name:      req.Name,
			Address:   req.Address,
			Live:      req.Live,
		},
	}, false)

	mth.handler.sendSuccess(w, map[string]interface{}{"id": id})
}

func (mth *MessageTypeHandler) SendContact(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	chatID := mux.Vars(r)["chatId"]
	var req dto.SendContactRequest
	if err := mth.handler.readJSON(r, &req); err != nil {
		mth.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}
	if req.Secret != "" && !mth.handler.validateSecretValue(req.Secret) {
		mth.handler.sendError(w, http.StatusUnauthorized, "Unauthorized")
		return
	}

	var id string
	var err error
	var extra repository.MessageExtra

	if len(req.Contacts) > 1 {
		pairs := make([]whatsappInfra.ContactPair, 0, len(req.Contacts))
		entries := make([]repository.ContactEntry, 0, len(req.Contacts))
		display := req.DisplayName
		for _, c := range req.Contacts {
			pairs = append(pairs, whatsappInfra.ContactPair{DisplayName: c.DisplayName, Phone: c.Phone, VCard: c.VCard})
			entries = append(entries, repository.ContactEntry{DisplayName: c.DisplayName, VCard: c.VCard})
		}
		if display == "" {
			display = req.Contacts[0].DisplayName
		}
		id, err = mth.handler.client.SendContacts(r.Context(), chatID, display, pairs)
		extra = repository.MessageExtra{Contact: &repository.ContactMeta{DisplayName: display, Contacts: entries}}
	} else {
		single := req
		if len(req.Contacts) == 1 {
			single.DisplayName = req.Contacts[0].DisplayName
			single.Phone = req.Contacts[0].Phone
			single.VCard = req.Contacts[0].VCard
		}
		if single.DisplayName == "" && single.VCard != "" {
			single.DisplayName = vcardDisplayName(single.VCard)
		}
		id, err = mth.handler.client.SendContact(r.Context(), chatID, single.DisplayName, single.Phone, single.VCard)
		extra = repository.MessageExtra{Contact: &repository.ContactMeta{
			DisplayName: single.DisplayName,
			Contacts:    []repository.ContactEntry{{DisplayName: single.DisplayName, VCard: single.VCard}},
		}}
	}
	if err != nil {
		mth.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	display := extra.Contact.DisplayName
	if display == "" {
		display = "[Kontak]"
	}
	mth.saveSentRow(chatID, id, display, "contact", &extra, false)
	mth.handler.sendSuccess(w, map[string]interface{}{"id": id})
}

func (mth *MessageTypeHandler) ForwardMessage(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	vars := mux.Vars(r)
	msgID := vars["id"]

	var req dto.ForwardMessageRequest
	if err := mth.handler.readJSON(r, &req); err != nil {
		mth.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}
	if mth.handler.msgRepo == nil {
		mth.handler.sendError(w, http.StatusInternalServerError, "Message repository not configured")
		return
	}

	raw, err := mth.handler.msgRepo.GetMessageRawProto(msgID)
	if err != nil || len(raw) == 0 {
		mth.handler.sendError(w, http.StatusNotFound, "message proto not available")
		return
	}
	stored, err := mth.handler.msgRepo.GetMessageByID(msgID)
	if err != nil {
		mth.handler.sendError(w, http.StatusNotFound, "message not found")
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 60*time.Second)
	defer cancel()

	forwardedIDs := make(map[string]string, len(req.Targets))
	for _, target := range req.Targets {
		fwdID, ferr := mth.handler.client.ForwardMessage(ctx, target, raw)
		if ferr != nil {
			mth.handler.sendError(w, http.StatusInternalServerError, ferr.Error())
			return
		}
		forwardedIDs[target] = fwdID
		mth.saveSentRow(target, fwdID, stored.Content, stored.Type, stored.Extra, true)
	}

	mth.handler.sendSuccess(w, map[string]interface{}{"ids": forwardedIDs})
}

func vcardDisplayName(vcard string) string {
	for _, line := range strings.Split(vcard, "\n") {
		line = strings.TrimRight(line, "\r")
		if strings.HasPrefix(line, "FN:") {
			return strings.TrimPrefix(line, "FN:")
		}
	}
	return ""
}

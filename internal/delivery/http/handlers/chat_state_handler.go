package handlers

import (
	"bytes"
	"context"
	"fmt"
	"io"
	"mime"
	"net/http"
	"path/filepath"
	"strings"
	"time"

	"github.com/gorilla/mux"
	waCommon "go.mau.fi/whatsmeow/proto/waCommon"
	waWeb "go.mau.fi/whatsmeow/proto/waWeb"
	waTypes "go.mau.fi/whatsmeow/types"
	"google.golang.org/protobuf/proto"

	"wa-bot/internal/domain/repository"
	whatsappInfra "wa-bot/internal/infrastructure/whatsapp"
)

func (ch *ChatHandler) PinChat(w http.ResponseWriter, r *http.Request) {
	chatID := mux.Vars(r)["id"]
	var body struct {
		Pinned bool `json:"pinned"`
	}
	if err := ch.handler.readJSON(r, &body); err != nil {
		ch.handler.sendError(w, http.StatusBadRequest, "invalid request body")
		return
	}
	jid, state, ok := ch.chatMutationState(w, chatID)
	if !ok {
		return
	}
	ctx, cancel := context.WithTimeout(r.Context(), 90*time.Second)
	defer cancel()
	if err := applyToTargets(ch.mutationTargets(chatID, jid), func(target waTypes.JID) error {
		return ch.handler.client.SetChatPinned(ctx, target, body.Pinned)
	}); err != nil {
		ch.handler.sendError(w, http.StatusBadGateway, err.Error())
		return
	}
	if body.Pinned {
		now := time.Now().UnixMilli()
		state.PinnedAt = &now
	} else {
		state.PinnedAt = nil
	}
	ch.commitChatState(w, state)
}

func (ch *ChatHandler) ArchiveChat(w http.ResponseWriter, r *http.Request) {
	chatID := mux.Vars(r)["id"]
	var body struct {
		Archived bool `json:"archived"`
	}
	if err := ch.handler.readJSON(r, &body); err != nil {
		ch.handler.sendError(w, http.StatusBadRequest, "invalid request body")
		return
	}
	jid, state, ok := ch.chatMutationState(w, chatID)
	if !ok {
		return
	}
	targets := ch.mutationTargets(chatID, jid)
	aliases := make([]string, 0, len(targets))
	for _, target := range targets {
		aliases = append(aliases, target.String())
	}
	var timestamp time.Time
	var anchor *repository.MessageAnchor
	if found, err := ch.handler.msgRepo.LatestMessageAnchorAmong(aliases); err == nil {
		anchor = found
		timestamp = time.UnixMilli(anchor.Timestamp)
	}
	// App-state conflicts may require a full sync and a recovery snapshot from
	// the primary device before this mutation can be retried.
	ctx, cancel := context.WithTimeout(r.Context(), 90*time.Second)
	defer cancel()
	if err := applyToTargets(targets, func(target waTypes.JID) error {
		var key *waCommon.MessageKey
		if anchor != nil {
			key = &waCommon.MessageKey{
				RemoteJID: proto.String(target.String()),
				FromMe:    proto.Bool(anchor.IsFromMe),
				ID:        proto.String(anchor.MessageID),
			}
			if !anchor.IsFromMe && target.Server == waTypes.GroupServer && anchor.SenderID != "" {
				key.Participant = proto.String(anchor.SenderID)
			}
		}
		return ch.handler.client.SetChatArchived(ctx, target, body.Archived, timestamp, key)
	}); err != nil {
		ch.handler.sendError(w, http.StatusBadGateway, err.Error())
		return
	}
	state.Archived = body.Archived
	if body.Archived {
		state.PinnedAt = nil
	}
	ch.commitChatState(w, state)
}

func (ch *ChatHandler) MuteChat(w http.ResponseWriter, r *http.Request) {
	chatID := mux.Vars(r)["id"]
	var body struct {
		Mode string `json:"mode"`
	}
	if err := ch.handler.readJSON(r, &body); err != nil {
		ch.handler.sendError(w, http.StatusBadRequest, "invalid request body")
		return
	}
	jid, state, ok := ch.chatMutationState(w, chatID)
	if !ok {
		return
	}
	var duration time.Duration
	muted := true
	switch body.Mode {
	case "off":
		muted = false
	case "8h":
		duration = 8 * time.Hour
	case "1w":
		duration = 7 * 24 * time.Hour
	case "forever":
	default:
		ch.handler.sendError(w, http.StatusBadRequest, "mode must be off, 8h, 1w, or forever")
		return
	}
	ctx, cancel := context.WithTimeout(r.Context(), 90*time.Second)
	defer cancel()
	if err := applyToTargets(ch.mutationTargets(chatID, jid), func(target waTypes.JID) error {
		return ch.handler.client.SetChatMuted(ctx, target, muted, duration)
	}); err != nil {
		ch.handler.sendError(w, http.StatusBadGateway, err.Error())
		return
	}
	state.MuteMode = "off"
	state.MutedUntil = nil
	if muted && duration == 0 {
		state.MuteMode = "forever"
	} else if muted {
		until := time.Now().Add(duration).UnixMilli()
		state.MuteMode = "until"
		state.MutedUntil = &until
	}
	ch.commitChatState(w, state)
}

// mutationTargets lists the JIDs an app-state change must be addressed to: the
// stored forms of the chat (the phone only honours the one it knows), or just
// the requested one when nothing is stored under an alias.
func (ch *ChatHandler) mutationTargets(chatID string, requested waTypes.JID) []waTypes.JID {
	var targets []waTypes.JID
	for _, id := range ch.handler.msgRepo.ChatAliases(chatID) {
		if jid, err := waTypes.ParseJID(id); err == nil {
			targets = append(targets, jid)
		}
	}
	if len(targets) == 0 {
		targets = append(targets, requested)
	}
	return targets
}

// applyToTargets runs change for every target and succeeds when at least one
// was accepted: a stale alias the phone never heard of is not an error.
func applyToTargets(targets []waTypes.JID, change func(waTypes.JID) error) error {
	var firstErr error
	accepted := false
	for _, jid := range targets {
		if err := change(jid); err != nil {
			if firstErr == nil {
				firstErr = err
			}
			continue
		}
		accepted = true
	}
	if accepted {
		return nil
	}
	return firstErr
}

func (ch *ChatHandler) chatMutationState(w http.ResponseWriter, chatID string) (waTypes.JID, repository.ChatState, bool) {
	if ch.handler.msgRepo == nil {
		ch.handler.sendError(w, http.StatusInternalServerError, "Message repository not configured")
		return waTypes.EmptyJID, repository.ChatState{}, false
	}
	jid, err := waTypes.ParseJID(chatID)
	if err != nil {
		ch.handler.sendError(w, http.StatusBadRequest, "invalid chat JID")
		return waTypes.EmptyJID, repository.ChatState{}, false
	}
	state, err := ch.handler.msgRepo.GetChatState(chatID)
	if err != nil {
		ch.handler.sendError(w, http.StatusNotFound, "chat not found")
		return waTypes.EmptyJID, repository.ChatState{}, false
	}
	return jid, state, true
}

func (ch *ChatHandler) commitChatState(w http.ResponseWriter, state repository.ChatState) {
	if err := ch.handler.msgRepo.UpdateChatState(state); err != nil {
		ch.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}
	ch.handler.BroadcastMessage("chat_state", state)
	ch.handler.sendJSON(w, state)
}

func (ch *ChatHandler) GetHistoricalMedia(w http.ResponseWriter, r *http.Request) {
	messageID := mux.Vars(r)["messageId"]
	chatID := mux.Vars(r)["id"]
	raw, cachedURL, err := ch.handler.msgRepo.GetRawMessage(messageID)
	if err != nil || len(raw) == 0 {
		ch.handler.sendError(w, http.StatusGone, "historical media is no longer available")
		return
	}
	if cachedURL != "" && !strings.Contains(cachedURL, "/messages/") {
		// Range requests work here, which a video player needs to seek.
		path := strings.TrimPrefix(strings.TrimPrefix(cachedURL, "/api/media/"), "/media/")
		if ch.handler.storage.Exists(path) {
			ch.handler.storage.ServeMedia(w, r, path)
			return
		}
		if err := ch.serveStoredMedia(w, r, cachedURL, ""); err == nil {
			return
		}
	}

	var webMsg waWeb.WebMessageInfo
	if err := proto.Unmarshal(raw, &webMsg); err != nil {
		ch.handler.sendError(w, http.StatusGone, "historical media descriptor is invalid")
		return
	}
	jid, err := waTypes.ParseJID(chatID)
	if err != nil {
		ch.handler.sendError(w, http.StatusBadRequest, "invalid chat JID")
		return
	}
	evt, err := ch.handler.client.CoreClient().ParseWebMessage(jid, &webMsg)
	if err != nil || evt == nil {
		ch.handler.sendError(w, http.StatusGone, "historical media descriptor is invalid")
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 90*time.Second)
	defer cancel()
	media, ok := whatsappInfra.ResolveHistoricalMedia(evt.Message)
	if !ok {
		ch.handler.sendError(w, http.StatusGone, "historical media is no longer available")
		return
	}
	data, err := ch.handler.client.CoreClient().Download(ctx, media.Message)
	mimeType, fileName := media.Mime, media.FileName
	if err != nil || len(data) == 0 {
		ch.handler.sendError(w, http.StatusGone, "historical media is no longer available")
		return
	}
	cacheName := whatsappInfra.HistoricalMediaKey(chatID, messageID, fileName, mimeType)
	if _, err := ch.handler.storage.Save(ctx, cacheName, bytes.NewReader(data)); err != nil {
		ch.handler.sendError(w, http.StatusInternalServerError, "failed to cache historical media")
		return
	}
	mediaURL := "/media/" + cacheName
	_ = ch.handler.msgRepo.UpdateHistoricalMediaURL(messageID, mediaURL)
	w.Header().Set("Content-Type", mimeType)
	if fileName != "" {
		w.Header().Set("Content-Disposition", fmt.Sprintf("inline; filename=%q", filepath.Base(fileName)))
	}
	_, _ = w.Write(data)
}

// serveStoredMedia returns an error only so the caller can fall through to a
// fresh WhatsApp download when a stale cache path was recorded.
func (ch *ChatHandler) serveStoredMedia(w http.ResponseWriter, r *http.Request, mediaURL, mimeType string) error {
	path := strings.TrimPrefix(mediaURL, "/api/media/")
	path = strings.TrimPrefix(path, "/media/")
	reader, err := ch.handler.storage.Get(r.Context(), path)
	if err != nil {
		return err
	}
	defer reader.Close()
	if mimeType == "" {
		mimeType = mime.TypeByExtension(filepath.Ext(path))
	}
	if mimeType != "" {
		w.Header().Set("Content-Type", mimeType)
	}
	_, err = io.Copy(w, reader)
	return err
}

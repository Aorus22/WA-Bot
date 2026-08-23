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
	waE2E "go.mau.fi/whatsmeow/proto/waE2E"
	waWeb "go.mau.fi/whatsmeow/proto/waWeb"
	waTypes "go.mau.fi/whatsmeow/types"
	"google.golang.org/protobuf/proto"

	"wa-bot/internal/domain/repository"
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
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()
	if err := ch.handler.client.SetChatPinned(ctx, jid, body.Pinned); err != nil {
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
	var timestamp time.Time
	var key *waCommon.MessageKey
	if anchor, err := ch.handler.msgRepo.LatestMessageAnchor(chatID); err == nil {
		timestamp = time.UnixMilli(anchor.Timestamp)
		key = &waCommon.MessageKey{
			RemoteJID: proto.String(chatID),
			FromMe:    proto.Bool(anchor.IsFromMe),
			ID:        proto.String(anchor.MessageID),
		}
		if !anchor.IsFromMe && strings.HasSuffix(chatID, "@g.us") && anchor.SenderID != "" {
			key.Participant = proto.String(anchor.SenderID)
		}
	}
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()
	if err := ch.handler.client.SetChatArchived(ctx, jid, body.Archived, timestamp, key); err != nil {
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
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()
	if err := ch.handler.client.SetChatMuted(ctx, jid, muted, duration); err != nil {
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
	data, mimeType, fileName, err := downloadHistoricalMedia(ctx, ch.handler, evt.Message)
	if err != nil || len(data) == 0 {
		ch.handler.sendError(w, http.StatusGone, "historical media is no longer available")
		return
	}
	ext := filepath.Ext(fileName)
	if ext == "" {
		if extensions, _ := mime.ExtensionsByType(mimeType); len(extensions) > 0 {
			ext = extensions[0]
		} else {
			ext = ".bin"
		}
	}
	cacheName := filepath.ToSlash(filepath.Join("history", safePathPart(chatID), safePathPart(messageID)+ext))
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

func downloadHistoricalMedia(ctx context.Context, h *Handler, msg *waE2E.Message) ([]byte, string, string, error) {
	client := h.client.CoreClient()
	if media := msg.GetImageMessage(); media != nil {
		data, err := client.Download(ctx, media)
		return data, fallbackMime(media.GetMimetype(), "image/jpeg"), "image.jpg", err
	}
	if media := msg.GetVideoMessage(); media != nil {
		data, err := client.Download(ctx, media)
		return data, fallbackMime(media.GetMimetype(), "video/mp4"), "video.mp4", err
	}
	if media := msg.GetDocumentMessage(); media != nil {
		data, err := client.Download(ctx, media)
		return data, fallbackMime(media.GetMimetype(), "application/octet-stream"), media.GetFileName(), err
	}
	if media := msg.GetAudioMessage(); media != nil {
		data, err := client.Download(ctx, media)
		return data, fallbackMime(media.GetMimetype(), "audio/ogg"), "audio.ogg", err
	}
	if media := msg.GetStickerMessage(); media != nil {
		data, err := client.Download(ctx, media)
		return data, fallbackMime(media.GetMimetype(), "image/webp"), "sticker.webp", err
	}
	return nil, "", "", fmt.Errorf("unsupported historical media")
}

func fallbackMime(value, fallbackValue string) string {
	if value == "" {
		return fallbackValue
	}
	return value
}

func safePathPart(value string) string {
	value = strings.NewReplacer("@", "_", ".", "_", "/", "_", "\\", "_").Replace(value)
	if value == "" {
		return "unknown"
	}
	return value
}

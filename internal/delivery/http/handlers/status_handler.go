package handlers

import (
	"bytes"
	"context"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"

	"github.com/gorilla/mux"
	waProto "go.mau.fi/whatsmeow/proto/waE2E"
	"google.golang.org/protobuf/proto"

	"wa-bot/internal/domain/repository"
)

// StatusHandler exposes status (story) listing, posting, viewing, and
// on-demand media download.
type StatusHandler struct {
	handler *Handler
}

func NewStatusHandler(h *Handler) *StatusHandler {
	return &StatusHandler{handler: h}
}

func (sh *StatusHandler) ownJID() string {
	if id := sh.handler.client.CoreClient().Store.ID; id != nil {
		return id.ToNonAD().String()
	}
	return ""
}

// List returns active statuses grouped per sender.
func (sh *StatusHandler) List(w http.ResponseWriter, r *http.Request) {
	if sh.handler.msgRepo == nil {
		sh.handler.sendError(w, http.StatusInternalServerError, "Message repository not configured")
		return
	}
	sh.handler.msgRepo.ExpireStatuses()
	groups := sh.handler.msgRepo.GetStatusGroups(sh.ownJID())
	if groups == nil {
		groups = []repository.StatusGroup{}
	}
	sh.handler.sendJSON(w, groups)
}

// PostText publishes a text status.
func (sh *StatusHandler) PostText(w http.ResponseWriter, r *http.Request) {
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}
	var req struct {
		Text       string `json:"text"`
		Background uint32 `json:"background"`
	}
	if err := sh.handler.readJSON(r, &req); err != nil || strings.TrimSpace(req.Text) == "" {
		sh.handler.sendError(w, http.StatusBadRequest, "text is required")
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 30*time.Second)
	defer cancel()
	id, err := sh.handler.client.SendStatusText(ctx, req.Text, req.Background)
	if err != nil {
		sh.handler.sendError(w, http.StatusBadGateway, err.Error())
		return
	}

	// Store our own post so "Status Saya" shows it immediately; the phone
	// echo never reaches the sending device.
	if sh.handler.msgRepo != nil {
		sh.handler.msgRepo.SaveStatus(&repository.StatusEntry{
			ID:        id,
			Sender:    sh.ownJID(),
			Content:   req.Text,
			Type:      "text",
			Timestamp: time.Now().UnixMilli(),
			ExpiresAt: time.Now().Add(24 * time.Hour).UnixMilli(),
			Viewed:    true,
		}, nil)
	}
	sh.handler.BroadcastMessage("status_new", map[string]string{"sender": sh.ownJID(), "id": id})
	sh.handler.sendSuccess(w, map[string]interface{}{"id": id})
}

// PostMedia publishes an image/video status (multipart: file, caption, type).
func (sh *StatusHandler) PostMedia(w http.ResponseWriter, r *http.Request) {
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}
	if err := r.ParseMultipartForm(50 << 20); err != nil {
		sh.handler.sendError(w, http.StatusBadRequest, "failed to parse form: "+err.Error())
		return
	}
	kind := strings.ToLower(r.FormValue("type"))
	if kind != "image" && kind != "video" {
		kind = "image"
	}
	caption := r.FormValue("caption")

	file, _, err := r.FormFile("file")
	if err != nil {
		sh.handler.sendError(w, http.StatusBadRequest, "file is required")
		return
	}
	defer file.Close()
	data, err := io.ReadAll(file)
	if err != nil || len(data) == 0 {
		sh.handler.sendError(w, http.StatusBadRequest, "empty file")
		return
	}

	mediaURL := ""
	ext := ".jpg"
	if kind == "video" {
		ext = ".mp4"
	}
	filename := fmt.Sprintf("status_sent_%d%s", time.Now().UnixMilli(), ext)
	if _, serr := sh.handler.storage.Save(r.Context(), filename, bytes.NewReader(data)); serr == nil {
		mediaURL = "/media/" + filename
	}

	ctx, cancel := context.WithTimeout(r.Context(), 2*time.Minute)
	defer cancel()
	id, err := sh.handler.client.SendStatusMedia(ctx, kind, data, caption, mediaURL)
	if err != nil {
		sh.handler.sendError(w, http.StatusBadGateway, err.Error())
		return
	}

	if sh.handler.msgRepo != nil {
		content := caption
		if content == "" {
			content = "[Media]"
		}
		sh.handler.msgRepo.SaveStatus(&repository.StatusEntry{
			ID:        id,
			Sender:    sh.ownJID(),
			Content:   content,
			MediaURL:  mediaURL,
			Type:      kind,
			Timestamp: time.Now().UnixMilli(),
			ExpiresAt: time.Now().Add(24 * time.Hour).UnixMilli(),
			Viewed:    true,
		}, nil)
	}
	sh.handler.BroadcastMessage("status_new", map[string]string{"sender": sh.ownJID(), "id": id})
	sh.handler.sendSuccess(w, map[string]interface{}{"id": id})
}

// Viewed marks a status viewed locally and sends the read receipt.
func (sh *StatusHandler) Viewed(w http.ResponseWriter, r *http.Request) {
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}
	vars := mux.Vars(r)
	id := vars["id"]
	if sh.handler.msgRepo == nil {
		sh.handler.sendError(w, http.StatusInternalServerError, "Message repository not configured")
		return
	}
	entry, err := sh.handler.msgRepo.GetStatusByID(id)
	if err != nil {
		sh.handler.sendError(w, http.StatusNotFound, "status not found")
		return
	}
	own := sh.ownJID()
	if entry.Sender == own {
		sh.handler.sendSuccess(w, nil) // viewing own status sends no receipt
		return
	}
	if err := sh.handler.msgRepo.MarkStatusViewed(id); err != nil {
		sh.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}
	go func(sender string) {
		ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
		defer cancel()
		_ = sh.handler.client.MarkStatusViewed(ctx, []string{id}, sender)
	}(entry.Sender)
	sh.handler.sendSuccess(w, nil)
}

// Media serves (and caches) a status' media, downloading it from WhatsApp on
// first request.
func (sh *StatusHandler) Media(w http.ResponseWriter, r *http.Request) {
	id := mux.Vars(r)["id"]
	if sh.handler.msgRepo == nil {
		sh.handler.sendError(w, http.StatusInternalServerError, "Message repository not configured")
		return
	}
	entry, err := sh.handler.msgRepo.GetStatusByID(id)
	if err != nil {
		sh.handler.sendError(w, http.StatusNotFound, "status not found")
		return
	}
	if entry.MediaURL != "" {
		path := strings.TrimPrefix(entry.MediaURL, "/media/")
		if reader, gerr := sh.handler.storage.Get(r.Context(), path); gerr == nil {
			defer reader.Close()
			switch {
			case strings.HasSuffix(path, ".mp4"):
				w.Header().Set("Content-Type", "video/mp4")
			case strings.HasSuffix(path, ".webp"):
				w.Header().Set("Content-Type", "image/webp")
			default:
				w.Header().Set("Content-Type", "image/jpeg")
			}
			io.Copy(w, reader)
			return
		}
	}

	raw, err := sh.handler.msgRepo.GetStatusRawProto(id)
	if err != nil || len(raw) == 0 {
		sh.handler.sendError(w, http.StatusGone, "status media no longer available")
		return
	}
	var msg waProto.Message
	if err := proto.Unmarshal(raw, &msg); err != nil {
		sh.handler.sendError(w, http.StatusGone, "status media descriptor invalid")
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 90*time.Second)
	defer cancel()

	core := sh.handler.client.CoreClient()
	var data []byte
	var mediaType string
	switch {
	case msg.GetImageMessage() != nil:
		data, err = core.Download(ctx, msg.GetImageMessage())
		mediaType = "image/jpeg"
	case msg.GetVideoMessage() != nil:
		data, err = core.Download(ctx, msg.GetVideoMessage())
		mediaType = "video/mp4"
	case msg.GetAudioMessage() != nil:
		data, err = core.Download(ctx, msg.GetAudioMessage())
		mediaType = "audio/ogg"
	default:
		sh.handler.sendError(w, http.StatusGone, "status has no downloadable media")
		return
	}
	if err != nil || len(data) == 0 {
		sh.handler.sendError(w, http.StatusGone, "status media no longer available")
		return
	}

	cacheName := fmt.Sprintf("status_cache_%s", id)
	if strings.HasPrefix(mediaType, "video/") {
		cacheName += ".mp4"
	} else if strings.HasPrefix(mediaType, "audio/") {
		cacheName += ".ogg"
	} else {
		cacheName += ".jpg"
	}
	if _, serr := sh.handler.storage.Save(ctx, cacheName, bytes.NewReader(data)); serr == nil {
		entry.MediaURL = "/media/" + cacheName
		_ = sh.handler.msgRepo.SaveStatus(entry, raw)
	}
	w.Header().Set("Content-Type", mediaType)
	_, _ = w.Write(data)
}

// Privacy returns the status privacy settings.
func (sh *StatusHandler) Privacy(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()
	list, err := sh.handler.client.GetStatusPrivacy(ctx)
	if err != nil {
		sh.handler.sendError(w, http.StatusBadGateway, err.Error())
		return
	}
	out := make([]map[string]interface{}, 0, len(list))
	for _, p := range list {
		jids := make([]string, 0, len(p.List))
		for _, j := range p.List {
			jids = append(jids, j.String())
		}
		out = append(out, map[string]interface{}{
			"type":      string(p.Type),
			"list":      jids,
			"isDefault": p.IsDefault,
		})
	}
	sh.handler.sendJSON(w, out)
}

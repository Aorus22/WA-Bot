package handlers

import (
	"context"
	"net/http"
	"strconv"
	"strings"
	"time"

	"github.com/gorilla/mux"

	whatsappInfra "wa-bot/internal/infrastructure/whatsapp"
)

// ChannelHandler exposes WhatsApp channels (newsletters): follow/unfollow,
// message feed, reactions, and mute.
type ChannelHandler struct {
	handler *Handler
}

func NewChannelHandler(h *Handler) *ChannelHandler {
	return &ChannelHandler{handler: h}
}

// List returns the followed channels.
func (ch *ChannelHandler) List(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 20*time.Second)
	defer cancel()
	channels, err := ch.handler.client.ListChannels(ctx)
	if err != nil {
		ch.handler.sendError(w, http.StatusBadGateway, err.Error())
		return
	}
	if channels == nil {
		channels = []*whatsappInfra.ChannelInfo{}
	}
	ch.handler.sendJSON(w, channels)
}

// Preview peeks at a channel via invite link without following.
func (ch *ChannelHandler) Preview(w http.ResponseWriter, r *http.Request) {
	link := r.URL.Query().Get("url")
	if link == "" {
		ch.handler.sendError(w, http.StatusBadRequest, "url query parameter is required")
		return
	}
	ctx, cancel := context.WithTimeout(r.Context(), 20*time.Second)
	defer cancel()
	info, err := ch.handler.client.PreviewChannel(ctx, link)
	if err != nil {
		ch.handler.sendError(w, http.StatusBadGateway, err.Error())
		return
	}
	ch.handler.sendJSON(w, info)
}

// Follow joins a channel via invite link.
func (ch *ChannelHandler) Follow(w http.ResponseWriter, r *http.Request) {
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}
	var req struct {
		URL string `json:"url"`
	}
	if err := ch.handler.readJSON(r, &req); err != nil || strings.TrimSpace(req.URL) == "" {
		ch.handler.sendError(w, http.StatusBadRequest, "url is required")
		return
	}
	ctx, cancel := context.WithTimeout(r.Context(), 30*time.Second)
	defer cancel()
	info, err := ch.handler.client.FollowChannel(ctx, req.URL)
	if err != nil {
		ch.handler.sendError(w, http.StatusBadGateway, err.Error())
		return
	}
	ch.handler.BroadcastMessage("channels_changed", map[string]string{"reason": "followed"})
	ch.handler.sendSuccess(w, map[string]interface{}{"channel": info})
}

// Unfollow leaves a channel.
func (ch *ChannelHandler) Unfollow(w http.ResponseWriter, r *http.Request) {
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}
	channelID := mux.Vars(r)["id"]
	ctx, cancel := context.WithTimeout(r.Context(), 20*time.Second)
	defer cancel()
	if err := ch.handler.client.UnfollowChannel(ctx, channelID); err != nil {
		ch.handler.sendError(w, http.StatusBadGateway, err.Error())
		return
	}
	ch.handler.BroadcastMessage("channels_changed", map[string]string{"reason": "unfollowed"})
	ch.handler.sendSuccess(w, nil)
}

// Mute toggles channel notifications.
func (ch *ChannelHandler) Mute(w http.ResponseWriter, r *http.Request) {
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}
	channelID := mux.Vars(r)["id"]
	var req struct {
		Muted bool `json:"muted"`
	}
	if err := ch.handler.readJSON(r, &req); err != nil {
		ch.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}
	ctx, cancel := context.WithTimeout(r.Context(), 20*time.Second)
	defer cancel()
	if err := ch.handler.client.SetChannelMuted(ctx, channelID, req.Muted); err != nil {
		ch.handler.sendError(w, http.StatusBadGateway, err.Error())
		return
	}
	ch.handler.sendSuccess(w, nil)
}

// Messages proxies the channel post feed.
func (ch *ChannelHandler) Messages(w http.ResponseWriter, r *http.Request) {
	channelID := mux.Vars(r)["id"]
	count := 30
	if c := r.URL.Query().Get("count"); c != "" {
		if parsed, err := strconv.Atoi(c); err == nil && parsed > 0 {
			count = parsed
		}
	}
	var before int64
	if b := r.URL.Query().Get("before"); b != "" {
		before, _ = strconv.ParseInt(b, 10, 64)
	}
	ctx, cancel := context.WithTimeout(r.Context(), 30*time.Second)
	defer cancel()
	msgs, _, err := ch.handler.client.GetChannelMessages(ctx, channelID, count, before)
	if err != nil {
		ch.handler.sendError(w, http.StatusBadGateway, err.Error())
		return
	}
	if msgs == nil {
		msgs = []*whatsappInfra.ChannelMessage{}
	}
	ch.handler.sendJSON(w, msgs)
}

// React reacts to a channel post.
func (ch *ChannelHandler) React(w http.ResponseWriter, r *http.Request) {
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}
	vars := mux.Vars(r)
	channelID := vars["id"]
	var req struct {
		Emoji     string `json:"emoji"`
		MessageID string `json:"messageId"`
	}
	if err := ch.handler.readJSON(r, &req); err != nil {
		ch.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}
	serverID, _ := strconv.ParseInt(vars["serverId"], 10, 64)
	ctx, cancel := context.WithTimeout(r.Context(), 20*time.Second)
	defer cancel()
	if err := ch.handler.client.ReactChannelMessage(ctx, channelID, serverID, req.MessageID, req.Emoji); err != nil {
		ch.handler.sendError(w, http.StatusBadGateway, err.Error())
		return
	}
	ch.handler.sendSuccess(w, nil)
}

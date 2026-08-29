package handlers

import (
	"context"
	"io"
	"net/http"
	"time"

	"github.com/gorilla/mux"
	"go.mau.fi/whatsmeow"
	waTypes "go.mau.fi/whatsmeow/types"

	whatsappInfra "wa-bot/internal/infrastructure/whatsapp"
)

// GroupHandler exposes full group management over REST. Every mutating
// endpoint refreshes the cached group snapshot afterwards so clients get the
// authoritative state back.
type GroupHandler struct {
	handler *Handler
}

func NewGroupHandler(h *Handler) *GroupHandler {
	return &GroupHandler{handler: h}
}

func (gh *GroupHandler) groupCtx(r *http.Request) (context.Context, context.CancelFunc) {
	return context.WithTimeout(r.Context(), 30*time.Second)
}

// GetGroup returns the cached group snapshot, refreshing it from the server
// when there is no cache yet.
func (gh *GroupHandler) GetGroup(w http.ResponseWriter, r *http.Request) {
	groupID := mux.Vars(r)["id"]
	if gh.handler.msgRepo == nil {
		gh.handler.sendError(w, http.StatusInternalServerError, "Message repository not configured")
		return
	}

	cache, err := gh.handler.msgRepo.GetGroupCache(groupID)
	if err != nil {
		ctx, cancel := gh.groupCtx(r)
		defer cancel()
		info, ferr := gh.handler.client.FetchGroupInfo(ctx, groupID)
		if ferr != nil {
			gh.handler.sendError(w, http.StatusBadGateway, ferr.Error())
			return
		}
		cache = whatsappInfra.BuildGroupCache(info, ownJIDOf(gh))
		if serr := gh.handler.msgRepo.SaveGroupCache(cache); serr != nil {
			gh.handler.sendError(w, http.StatusInternalServerError, serr.Error())
			return
		}
	}
	gh.handler.sendJSON(w, cache)
}

func ownJIDOf(gh *GroupHandler) waTypes.JID {
	if id := gh.handler.client.CoreClient().Store.ID; id != nil {
		return id.ToNonAD()
	}
	return waTypes.EmptyJID
}

// CreateGroup creates a group with the given name and participant JIDs.
func (gh *GroupHandler) CreateGroup(w http.ResponseWriter, r *http.Request) {
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}
	var req struct {
		Name         string   `json:"name"`
		Participants []string `json:"participants"`
	}
	if err := gh.handler.readJSON(r, &req); err != nil {
		gh.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}
	if req.Name == "" || len(req.Participants) == 0 {
		gh.handler.sendError(w, http.StatusBadRequest, "name and at least one participant are required")
		return
	}

	ctx, cancel := gh.groupCtx(r)
	defer cancel()
	info, err := gh.handler.client.CreateGroup(ctx, req.Name, req.Participants)
	if err != nil {
		gh.handler.sendError(w, http.StatusBadGateway, err.Error())
		return
	}

	cache := whatsappInfra.BuildGroupCache(info, ownJIDOf(gh))
	if gh.handler.msgRepo != nil {
		_ = gh.handler.msgRepo.SaveGroupCache(cache)
	}
	gh.handler.BroadcastMessage("chats_changed", map[string]interface{}{"reason": "group_created"})
	gh.handler.sendSuccess(w, map[string]interface{}{"group": cache})
}

// UpdateGroup applies name/description/settings changes.
func (gh *GroupHandler) UpdateGroup(w http.ResponseWriter, r *http.Request) {
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}
	groupID := mux.Vars(r)["id"]
	var req struct {
		Name          *string `json:"name"`
		Description   *string `json:"description"`
		Locked        *bool   `json:"locked"`
		Announce      *bool   `json:"announce"`
		JoinApproval  *bool   `json:"joinApproval"`
		MemberAddMode *string `json:"memberAddMode"`
	}
	if err := gh.handler.readJSON(r, &req); err != nil {
		gh.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}

	ctx, cancel := gh.groupCtx(r)
	defer cancel()
	if req.Name != nil && *req.Name != "" {
		if err := gh.handler.client.SetGroupName(ctx, groupID, *req.Name); err != nil {
			gh.handler.sendError(w, http.StatusBadGateway, err.Error())
			return
		}
	}
	if req.Description != nil {
		if err := gh.handler.client.SetGroupDescription(ctx, groupID, *req.Description); err != nil {
			gh.handler.sendError(w, http.StatusBadGateway, err.Error())
			return
		}
	}
	if req.Locked != nil {
		if err := gh.handler.client.SetGroupLocked(ctx, groupID, *req.Locked); err != nil {
			gh.handler.sendError(w, http.StatusBadGateway, err.Error())
			return
		}
	}
	if req.Announce != nil {
		if err := gh.handler.client.SetGroupAnnounce(ctx, groupID, *req.Announce); err != nil {
			gh.handler.sendError(w, http.StatusBadGateway, err.Error())
			return
		}
	}
	if req.JoinApproval != nil {
		if err := gh.handler.client.SetGroupJoinApproval(ctx, groupID, *req.JoinApproval); err != nil {
			gh.handler.sendError(w, http.StatusBadGateway, err.Error())
			return
		}
	}
	if req.MemberAddMode != nil {
		if err := gh.handler.client.SetGroupMemberAddMode(ctx, groupID, *req.MemberAddMode); err != nil {
			gh.handler.sendError(w, http.StatusBadGateway, err.Error())
			return
		}
	}

	gh.respondWithRefreshedGroup(w, r, groupID)
}

// SetGroupPhoto uploads a new group avatar (multipart "file"; empty body
// removes the photo).
func (gh *GroupHandler) SetGroupPhoto(w http.ResponseWriter, r *http.Request) {
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}
	groupID := mux.Vars(r)["id"]

	var photo []byte
	if err := r.ParseMultipartForm(10 << 20); err == nil {
		if file, _, ferr := r.FormFile("file"); ferr == nil {
			defer file.Close()
			photo, _ = io.ReadAll(file)
		}
	}

	ctx, cancel := gh.groupCtx(r)
	defer cancel()
	if _, err := gh.handler.client.SetGroupPhoto(ctx, groupID, photo); err != nil {
		gh.handler.sendError(w, http.StatusBadGateway, err.Error())
		return
	}
	gh.respondWithRefreshedGroup(w, r, groupID)
}

// UpdateParticipants adds/removes/promotes/demotes members.
func (gh *GroupHandler) UpdateParticipants(w http.ResponseWriter, r *http.Request) {
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}
	groupID := mux.Vars(r)["id"]
	var req struct {
		Action string   `json:"action"` // add | remove | promote | demote
		JIDs   []string `json:"jids"`
	}
	if err := gh.handler.readJSON(r, &req); err != nil {
		gh.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}

	ctx, cancel := gh.groupCtx(r)
	defer cancel()
	if _, err := gh.handler.client.UpdateGroupParticipants(ctx, groupID, req.JIDs, participantAction(req.Action)); err != nil {
		gh.handler.sendError(w, http.StatusBadGateway, err.Error())
		return
	}
	gh.respondWithRefreshedGroup(w, r, groupID)
}

func participantAction(action string) whatsmeow.ParticipantChange {
	switch action {
	case "add":
		return whatsmeow.ParticipantChangeAdd
	case "remove":
		return whatsmeow.ParticipantChangeRemove
	case "promote":
		return whatsmeow.ParticipantChangePromote
	case "demote":
		return whatsmeow.ParticipantChangeDemote
	default:
		return ""
	}
}

// InviteLink returns (and optionally resets) the group invite link.
func (gh *GroupHandler) InviteLink(w http.ResponseWriter, r *http.Request) {
	groupID := mux.Vars(r)["id"]
	reset := r.URL.Query().Get("reset") == "true" || r.Method == "POST"
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	ctx, cancel := gh.groupCtx(r)
	defer cancel()
	link, err := gh.handler.client.GetGroupInviteLink(ctx, groupID, reset)
	if err != nil {
		gh.handler.sendError(w, http.StatusBadGateway, err.Error())
		return
	}
	gh.handler.sendSuccess(w, map[string]interface{}{"link": link})
}

// PreviewLink peeks at a group via invite link without joining.
func (gh *GroupHandler) PreviewLink(w http.ResponseWriter, r *http.Request) {
	link := r.URL.Query().Get("url")
	if link == "" {
		gh.handler.sendError(w, http.StatusBadRequest, "url query parameter is required")
		return
	}

	ctx, cancel := gh.groupCtx(r)
	defer cancel()
	info, err := gh.handler.client.GetGroupInfoFromLink(ctx, link)
	if err != nil {
		gh.handler.sendError(w, http.StatusBadGateway, err.Error())
		return
	}
	gh.handler.sendSuccess(w, map[string]interface{}{
		"jid":              info.JID.String(),
		"name":             info.Name,
		"participantCount": info.ParticipantCount,
		"description":      info.Topic,
	})
}

// JoinLink joins a group via invite link.
func (gh *GroupHandler) JoinLink(w http.ResponseWriter, r *http.Request) {
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}
	var req struct {
		URL string `json:"url"`
	}
	if err := gh.handler.readJSON(r, &req); err != nil || req.URL == "" {
		gh.handler.sendError(w, http.StatusBadRequest, "url is required")
		return
	}

	ctx, cancel := gh.groupCtx(r)
	defer cancel()
	jid, err := gh.handler.client.JoinGroupWithLink(ctx, req.URL)
	if err != nil {
		gh.handler.sendError(w, http.StatusBadGateway, err.Error())
		return
	}
	gh.handler.BroadcastMessage("chats_changed", map[string]interface{}{"reason": "joined_group"})
	gh.handler.sendSuccess(w, map[string]interface{}{"jid": jid})
}

// JoinRequests lists pending join requests.
func (gh *GroupHandler) JoinRequests(w http.ResponseWriter, r *http.Request) {
	groupID := mux.Vars(r)["id"]
	ctx, cancel := gh.groupCtx(r)
	defer cancel()
	requests, err := gh.handler.client.GetGroupJoinRequests(ctx, groupID)
	if err != nil {
		gh.handler.sendError(w, http.StatusBadGateway, err.Error())
		return
	}
	out := make([]map[string]interface{}, 0, len(requests))
	for _, req := range requests {
		out = append(out, map[string]interface{}{
			"jid":  req.JID.String(),
			"name": "",
		})
	}
	gh.handler.sendJSON(w, out)
}

// UpdateJoinRequests approves or rejects pending join requests.
func (gh *GroupHandler) UpdateJoinRequests(w http.ResponseWriter, r *http.Request) {
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}
	groupID := mux.Vars(r)["id"]
	var req struct {
		Action string   `json:"action"` // approve | reject
		JIDs   []string `json:"jids"`
	}
	if err := gh.handler.readJSON(r, &req); err != nil {
		gh.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}

	ctx, cancel := gh.groupCtx(r)
	defer cancel()
	var err error
	if req.Action == "approve" {
		err = gh.handler.client.UpdateGroupJoinRequests(ctx, groupID, req.JIDs, whatsmeow.ParticipantChangeApprove)
	} else if req.Action == "reject" {
		err = gh.handler.client.UpdateGroupJoinRequests(ctx, groupID, req.JIDs, whatsmeow.ParticipantChangeReject)
	} else {
		gh.handler.sendError(w, http.StatusBadRequest, "action must be approve or reject")
		return
	}
	if err != nil {
		gh.handler.sendError(w, http.StatusBadGateway, err.Error())
		return
	}
	gh.respondWithRefreshedGroup(w, r, groupID)
}

// LeaveGroup removes this account from the group.
func (gh *GroupHandler) LeaveGroup(w http.ResponseWriter, r *http.Request) {
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}
	groupID := mux.Vars(r)["id"]
	ctx, cancel := gh.groupCtx(r)
	defer cancel()
	if err := gh.handler.client.LeaveGroup(ctx, groupID); err != nil {
		gh.handler.sendError(w, http.StatusBadGateway, err.Error())
		return
	}
	gh.handler.BroadcastMessage("chats_changed", map[string]interface{}{"reason": "left_group"})
	gh.handler.sendSuccess(w, nil)
}

// respondWithRefreshedGroup refetches the group and returns the new snapshot.
func (gh *GroupHandler) respondWithRefreshedGroup(w http.ResponseWriter, r *http.Request, groupID string) {
	ctx, cancel := gh.groupCtx(r)
	defer cancel()
	info, err := gh.handler.client.FetchGroupInfo(ctx, groupID)
	if err != nil {
		gh.handler.sendError(w, http.StatusBadGateway, err.Error())
		return
	}
	cache := whatsappInfra.BuildGroupCache(info, ownJIDOf(gh))
	if gh.handler.msgRepo != nil {
		_ = gh.handler.msgRepo.SaveGroupCache(cache)
		_ = gh.handler.msgRepo.UpdateChatName(groupID, cache.Name)
	}
	gh.handler.BroadcastMessage("group_updated", map[string]interface{}{
		"chatId": groupID,
		"group":  cache,
	})
	gh.handler.sendJSON(w, cache)
}

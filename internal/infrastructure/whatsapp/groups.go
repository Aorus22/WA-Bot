package whatsapp

import (
	"context"
	"fmt"
	"strings"

	"go.mau.fi/whatsmeow"
	waTypes "go.mau.fi/whatsmeow/types"

	"wa-bot/internal/domain/repository"
)

func parseJIDs(ids []string) []waTypes.JID {
	jids := make([]waTypes.JID, 0, len(ids))
	for _, id := range ids {
		if jid := parseTargetJID(id); !jid.IsEmpty() {
			jids = append(jids, jid)
		}
	}
	return jids
}

func (w *WhatsAppClient) CreateGroup(ctx context.Context, name string, participants []string) (*waTypes.GroupInfo, error) {
	return w.client.CreateGroup(ctx, whatsmeow.ReqCreateGroup{
		Name:         name,
		Participants: parseJIDs(participants),
	})
}

func (w *WhatsAppClient) SetGroupName(ctx context.Context, groupID, name string) error {
	jid, err := waTypes.ParseJID(groupID)
	if err != nil {
		return err
	}
	return w.client.SetGroupName(ctx, jid, name)
}

func (w *WhatsAppClient) SetGroupDescription(ctx context.Context, groupID, description string) error {
	jid, err := waTypes.ParseJID(groupID)
	if err != nil {
		return err
	}
	return w.client.SetGroupDescription(ctx, jid, description)
}

// SetGroupPhoto sets the group avatar (nil removes it). Returns the new
// picture ID.
func (w *WhatsAppClient) SetGroupPhoto(ctx context.Context, groupID string, photo []byte) (string, error) {
	jid, err := waTypes.ParseJID(groupID)
	if err != nil {
		return "", err
	}
	return w.client.SetGroupPhoto(ctx, jid, photo)
}

// UpdateGroupParticipants applies an action (add/remove/promote/demote) to a
// set of participants.
func (w *WhatsAppClient) UpdateGroupParticipants(ctx context.Context, groupID string, participants []string, action whatsmeow.ParticipantChange) ([]waTypes.GroupParticipant, error) {
	jid, err := waTypes.ParseJID(groupID)
	if err != nil {
		return nil, err
	}
	return w.client.UpdateGroupParticipants(ctx, jid, parseJIDs(participants), action)
}

func (w *WhatsAppClient) GetGroupInviteLink(ctx context.Context, groupID string, reset bool) (string, error) {
	jid, err := waTypes.ParseJID(groupID)
	if err != nil {
		return "", err
	}
	return w.client.GetGroupInviteLink(ctx, jid, reset)
}

func (w *WhatsAppClient) GetGroupInfoFromLink(ctx context.Context, link string) (*waTypes.GroupInfo, error) {
	return w.client.GetGroupInfoFromLink(ctx, link)
}

func (w *WhatsAppClient) JoinGroupWithLink(ctx context.Context, link string) (string, error) {
	jid, err := w.client.JoinGroupWithLink(ctx, link)
	if err != nil {
		return "", err
	}
	return jid.String(), nil
}

func (w *WhatsAppClient) GetGroupJoinRequests(ctx context.Context, groupID string) ([]waTypes.GroupParticipantRequest, error) {
	jid, err := waTypes.ParseJID(groupID)
	if err != nil {
		return nil, err
	}
	return w.client.GetGroupRequestParticipants(ctx, jid)
}

func (w *WhatsAppClient) UpdateGroupJoinRequests(ctx context.Context, groupID string, participants []string, action whatsmeow.ParticipantRequestChange) error {
	jid, err := waTypes.ParseJID(groupID)
	if err != nil {
		return err
	}
	_, err = w.client.UpdateGroupRequestParticipants(ctx, jid, parseJIDs(participants), action)
	return err
}

func (w *WhatsAppClient) SetGroupLocked(ctx context.Context, groupID string, locked bool) error {
	jid, err := waTypes.ParseJID(groupID)
	if err != nil {
		return err
	}
	return w.client.SetGroupLocked(ctx, jid, locked)
}

func (w *WhatsAppClient) SetGroupAnnounce(ctx context.Context, groupID string, announce bool) error {
	jid, err := waTypes.ParseJID(groupID)
	if err != nil {
		return err
	}
	return w.client.SetGroupAnnounce(ctx, jid, announce)
}

func (w *WhatsAppClient) SetGroupJoinApproval(ctx context.Context, groupID string, approval bool) error {
	jid, err := waTypes.ParseJID(groupID)
	if err != nil {
		return err
	}
	return w.client.SetGroupJoinApprovalMode(ctx, jid, approval)
}

// SetGroupMemberAddMode accepts "admin_add" or "all_member_add".
func (w *WhatsAppClient) SetGroupMemberAddMode(ctx context.Context, groupID, mode string) error {
	jid, err := waTypes.ParseJID(groupID)
	if err != nil {
		return err
	}
	switch mode {
	case "admin_add":
		return w.client.SetGroupMemberAddMode(ctx, jid, waTypes.GroupMemberAddModeAdmin)
	case "all_member_add":
		return w.client.SetGroupMemberAddMode(ctx, jid, waTypes.GroupMemberAddModeAllMember)
	default:
		return fmt.Errorf("invalid member add mode %q", mode)
	}
}

func (w *WhatsAppClient) LeaveGroup(ctx context.Context, groupID string) error {
	jid, err := waTypes.ParseJID(groupID)
	if err != nil {
		return err
	}
	return w.client.LeaveGroup(ctx, jid)
}

func (w *WhatsAppClient) FetchGroupInfo(ctx context.Context, groupID string) (*waTypes.GroupInfo, error) {
	jid, err := waTypes.ParseJID(groupID)
	if err != nil {
		return nil, err
	}
	return w.client.GetGroupInfo(ctx, jid)
}

// NormalizeInviteLink ensures a bare invite code becomes a full link, and
// strips whitespace around pasted links.
func NormalizeInviteLink(link string) string {
	link = strings.TrimSpace(link)
	if link == "" {
		return ""
	}
	if strings.HasPrefix(link, "http://") || strings.HasPrefix(link, "https://") {
		return link
	}
	return "https://chat.whatsapp.com/" + strings.TrimPrefix(link, "/")
}

// BuildGroupCache converts a whatsmeow GroupInfo into the cached snapshot,
// resolving our own role from the participant list.
func BuildGroupCache(info *waTypes.GroupInfo, ownJID waTypes.JID) *repository.GroupCache {
	cache := &repository.GroupCache{
		JID:          info.JID.String(),
		Name:         info.Name,
		Description:  info.Topic,
		Owner:        info.OwnerJID.String(),
		Locked:       info.IsLocked,
		Announce:     info.IsAnnounce,
		JoinApproval: info.GroupMembershipApprovalMode.IsJoinApprovalRequired,
	}
	if info.MemberAddMode == waTypes.GroupMemberAddModeAdmin {
		cache.MemberAddMode = "admin_add"
	} else if info.MemberAddMode != "" {
		cache.MemberAddMode = "all_member_add"
	}

	for _, p := range info.Participants {
		entry := repository.GroupParticipantInfo{
			JID:          p.JID.String(),
			IsAdmin:      p.IsAdmin,
			IsSuperAdmin: p.IsSuperAdmin,
		}
		// whatsmeow reports the primary JID; prefer the phone-number JID for
		// contacts we may already have stored.
		if !p.PhoneNumber.IsEmpty() {
			entry.JID = p.PhoneNumber.String()
		}
		if p.JID.User == ownJID.User || p.LID.User == ownJID.User || p.PhoneNumber.User == ownJID.User {
			switch {
			case p.IsSuperAdmin:
				cache.OwnRole = "superadmin"
			case p.IsAdmin:
				cache.OwnRole = "admin"
			default:
				cache.OwnRole = "member"
			}
		}
		cache.Participants = append(cache.Participants, entry)
	}
	cache.ParticipantCount = len(cache.Participants)
	return cache
}

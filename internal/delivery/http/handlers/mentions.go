package handlers

import (
	"context"
	"regexp"
	"strings"

	waTypes "go.mau.fi/whatsmeow/types"
	"wa-bot/internal/domain/repository"
)

func (h *Handler) ownMentionJIDs() []string {
	if h.client == nil || h.client.CoreClient() == nil || h.client.CoreClient().Store == nil {
		return nil
	}
	device := h.client.CoreClient().Store
	var ids []string
	if device.ID != nil {
		ids = append(ids, device.ID.ToNonAD().String())
	}
	if !device.LID.IsEmpty() {
		ids = append(ids, device.LID.ToNonAD().String())
	}
	return ids
}

func (h *Handler) resolveMentionJID(jid string) string {
	if h.msgRepo != nil {
		jid = h.msgRepo.ResolveChatID(jid)
	}
	if h.client == nil || h.client.CoreClient() == nil || h.client.CoreClient().Store == nil {
		return jid
	}
	client := h.client.CoreClient()
	parsed, err := waTypes.ParseJID(jid)
	if err == nil && parsed.Server == "lid" && client.Store.LIDs != nil {
		if phone, err := client.Store.LIDs.GetPNForLID(context.Background(), parsed); err == nil && !phone.IsEmpty() {
			if h.msgRepo != nil {
				_ = h.msgRepo.SaveLIDMapping(parsed.ToNonAD().String(), phone.ToNonAD().String())
			}
			return phone.ToNonAD().String()
		}
	}
	return jid
}

func (h *Handler) enrichMessageMentions(msg *repository.Message, ownJIDs []string) {
	if msg.Extra == nil || len(msg.Extra.Mentions) == 0 {
		return
	}
	extra := msg.Extra
	extra.MentionsMe = false
	if extra.MentionNames == nil {
		extra.MentionNames = make(map[string]string)
	}
	extra.MentionChats = make(map[string]string)
	resolver := h.newNameResolver()
	own := make(map[string]bool)
	for _, jid := range ownJIDs {
		own[h.resolveMentionJID(jid)] = true
	}
	for _, mentioned := range extra.Mentions {
		jid := h.resolveMentionJID(mentioned)
		extra.MentionChats[mentioned] = jid
		isOwn := own[jid]
		if isOwn && msg.Status == "received" && !msg.IsAutomatic {
			extra.MentionsMe = true
		}
		name := ""
		if h.msgRepo != nil {
			name, _ = h.msgRepo.GetContactName(jid)
		}
		if strings.TrimSpace(name) == "" && h.client != nil && h.client.CoreClient() != nil {
			client := h.client.CoreClient()
			parsed, err := waTypes.ParseJID(jid)
			if err == nil && client.Store != nil && client.Store.Contacts != nil {
				if contact, err := client.Store.Contacts.GetContact(context.Background(), parsed); err == nil && contact.Found {
					for _, candidate := range []string{contact.FullName, contact.PushName, contact.FirstName, contact.BusinessName} {
						if strings.TrimSpace(candidate) != "" {
							name = candidate
							break
						}
					}
				}
			}
		}
		if strings.TrimSpace(name) == "" && h.msgRepo != nil {
			if group, err := h.msgRepo.GetGroupCache(msg.ChatID); err == nil {
				for _, member := range group.Participants {
					if h.resolveMentionJID(member.JID) == jid && strings.TrimSpace(member.Name) != "" {
						name = member.Name
						break
					}
				}
			}
		}
		if strings.TrimSpace(name) == "" && isOwn {
			name = "You"
		}
		// Unknown contacts should show their real phone number, never a LID.
		if strings.TrimSpace(name) == "" && strings.HasSuffix(jid, "@s.whatsapp.net") {
			phone := strings.SplitN(jid, "@", 2)[0]
			if phone != "" && strings.Trim(phone, "0123456789") == "" {
				name = "+" + phone
			}
		}
		if !isOwn {
			name = resolver.resolve(jid, name)
		}
		if name = strings.TrimSpace(name); name != "" {
			extra.MentionNames[mentioned] = name
		}
	}
}

func (h *Handler) enrichMessagesMentions(messages []repository.Message) {
	h.applySenderNames(messages)
	own := h.ownMentionJIDs()
	for i := range messages {
		h.enrichMessageMentions(&messages[i], own)
	}
}

var mentionToken = regexp.MustCompile(`@(\d+)`)

// applyMentionNames replaces "@<number>" in content by "@<name>" for the
// mentions of extra that have a resolved name. Mentions without one are kept,
// so clients can decide how to show them.
func applyMentionNames(content string, extra *repository.MessageExtra) string {
	if extra == nil || len(extra.MentionNames) == 0 {
		return content
	}
	names := make(map[string]string, len(extra.MentionNames))
	for jid, name := range extra.MentionNames {
		if name = strings.TrimSpace(name); name != "" {
			names[strings.SplitN(jid, "@", 2)[0]] = name
		}
	}
	return mentionToken.ReplaceAllStringFunc(content, func(token string) string {
		if name, ok := names[token[1:]]; ok {
			return "@" + name
		}
		return token
	})
}

// applyChatPreviewMentions shows people's names instead of numbers in the
// last-message preview of the chat list.
func (h *Handler) applyChatPreviewMentions(chats []repository.Chat) {
	if h.msgRepo == nil {
		return
	}
	own := h.ownMentionJIDs()
	for i := range chats {
		if !mentionToken.MatchString(chats[i].LastMsg) {
			continue
		}
		msg, err := h.msgRepo.LastMessage(chats[i].ID)
		if err != nil || msg.Content != chats[i].LastMsg || msg.Extra == nil {
			continue
		}
		h.enrichMessageMentions(msg, own)
		chats[i].LastMsg = applyMentionNames(chats[i].LastMsg, msg.Extra)
	}
}

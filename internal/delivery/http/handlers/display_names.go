package handlers

import (
	"context"
	"strings"

	waTypes "go.mau.fi/whatsmeow/types"
	"wa-bot/internal/domain/repository"
)

// How other people's names are shown. WhatsApp itself shows the name saved in
// the phone's address book and falls back to the phone number; "profile"
// shows the name each person chose for themselves instead.
const (
	settingContactNameMode = "contact_name_mode"
	nameModeProfile        = "profile"
	nameModeSaved          = "saved"
)

// nameResolver turns a JID into the name to display under the current mode.
// Create one per request: it caches lookups for its lifetime.
type nameResolver struct {
	h     *Handler
	mode  string
	own   map[string]bool
	cache map[string]string
}

func (h *Handler) newNameResolver() *nameResolver {
	mode := nameModeProfile
	if h.settingsRepo != nil {
		if value, err := h.settingsRepo.Get(context.Background(), settingContactNameMode); err == nil && value == nameModeSaved {
			mode = nameModeSaved
		}
	}
	own := make(map[string]bool)
	for _, jid := range h.ownMentionJIDs() {
		own[jid] = true
	}
	return &nameResolver{h: h, mode: mode, own: own, cache: make(map[string]string)}
}

// resolve returns the display name for jid. stored is the name saved with the
// chat or message, used when nothing better is known.
func (r *nameResolver) resolve(jid, stored string) string {
	stored = strings.TrimSpace(stored)
	if !strings.HasSuffix(jid, "@s.whatsapp.net") && !strings.HasSuffix(jid, "@lid") {
		return stored
	}
	if r.own[jid] {
		return stored
	}
	key := jid + "\x00" + stored
	if name, ok := r.cache[key]; ok {
		return name
	}
	name := r.compute(jid, stored)
	r.cache[key] = name
	return name
}

func (r *nameResolver) compute(jid, stored string) string {
	phoneJID := r.h.resolveMentionJID(jid)
	if r.own[phoneJID] {
		return stored
	}
	var saved, push, business string
	if r.h.client != nil && r.h.client.CoreClient() != nil && r.h.client.CoreClient().Store != nil {
		contacts := r.h.client.CoreClient().Store.Contacts
		if contacts != nil {
			for _, candidate := range []string{phoneJID, jid} {
				parsed, err := waTypes.ParseJID(candidate)
				if err != nil {
					continue
				}
				info, err := contacts.GetContact(context.Background(), parsed)
				if err != nil || !info.Found {
					continue
				}
				if saved == "" {
					saved = firstNonEmpty(info.FullName, info.FirstName)
				}
				if push == "" {
					push = strings.TrimSpace(info.PushName)
				}
				if business == "" {
					business = strings.TrimSpace(info.BusinessName)
				}
			}
		}
	}
	number := ""
	if strings.HasSuffix(phoneJID, "@s.whatsapp.net") {
		if user := strings.SplitN(phoneJID, "@", 2)[0]; user != "" && strings.Trim(user, "0123456789") == "" {
			number = "+" + user
		}
	}
	// A "name" that is just the raw number or JID is not a real name.
	if stored == "" || strings.Contains(stored, "@") || strings.TrimLeft(stored, "+0123456789") == "" {
		stored = ""
	}

	if r.mode == nameModeSaved {
		// WhatsApp's own behaviour: saved name, else the number. Only a
		// number-less (LID) contact falls back to the profile name.
		return firstNonEmpty(saved, business, number, push, stored)
	}
	return firstNonEmpty(push, stored, saved, business, number)
}

func firstNonEmpty(values ...string) string {
	for _, value := range values {
		if value = strings.TrimSpace(value); value != "" {
			return value
		}
	}
	return ""
}

// applyChatNames renames one-to-one chats; group names are not affected.
func (h *Handler) applyChatNames(chats []repository.Chat) {
	resolver := h.newNameResolver()
	for i := range chats {
		if !chats[i].IsGroup {
			if name := resolver.resolve(chats[i].ID, chats[i].Name); name != "" {
				chats[i].Name = name
			}
		}
	}
}

func (h *Handler) applyContactNames(contacts []repository.Contact) {
	resolver := h.newNameResolver()
	for i := range contacts {
		if name := resolver.resolve(contacts[i].JID, contacts[i].Name); name != "" {
			contacts[i].Name = name
		}
	}
}

// applySenderNames rewrites the sender name of other people's messages.
func (h *Handler) applySenderNames(messages []repository.Message) {
	resolver := h.newNameResolver()
	for i := range messages {
		if messages[i].From == "me" || messages[i].From == "" {
			continue
		}
		if name := resolver.resolve(messages[i].From, messages[i].SenderName); name != "" {
			messages[i].SenderName = name
		}
	}
}

// broadcast sends a WebSocket event if a hub is attached.
func (h *Handler) broadcast(msgType string, payload interface{}) {
	if hub, ok := h.hub.(wsHub); ok && hub != nil {
		hub.BroadcastMessage(msgType, payload)
	}
}

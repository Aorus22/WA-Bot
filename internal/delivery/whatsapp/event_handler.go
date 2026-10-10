package whatsapp

import (
	"bytes"
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"go.mau.fi/whatsmeow"
	waProto "go.mau.fi/whatsmeow/proto/waE2E"
	waTypes "go.mau.fi/whatsmeow/types"
	"go.mau.fi/whatsmeow/types/events"
	"google.golang.org/protobuf/proto"

	"wa-bot/internal/domain/entity"
	"wa-bot/internal/domain/repository"
	"wa-bot/internal/infrastructure/ai"
	whatsappInfra "wa-bot/internal/infrastructure/whatsapp"
)

type HTTPServer interface {
	BroadcastMessage(msgType string, payload interface{})
	SaveAndBroadcastMessage(msg *repository.Message)
	UpdateMessageStatus(msgID, status string)
}

// AccountEvents is notified about account link changes so per-account data
// can follow the linked phone number.
type AccountEvents interface {
	HandlePaired(phone string)
	HandleLoggedOut()
}

type WhatsAppEventHandler struct {
	accounts     AccountEvents
	handlerUC    HandlerUseCaseInterface
	waService    *WhatsAppService
	stateRepo    repository.UserStateRepository
	waClient     *whatsappInfra.WhatsAppClient
	msgStore     *repository.MessageStore
	httpServer   HTTPServer
	storage      repository.StorageRepository
	luaService   LuaService
	aiClient     *ai.AIClient
	historySync  *HistorySyncService
	settingsRepo repository.SettingsRepository
}

type LuaService interface {
	ExecuteTriggers(ctx context.Context, msg *entity.Message) (bool, error)
	TestTrigger(ctx context.Context, pattern, script, message string) (map[string]interface{}, error)
}

func NewWhatsAppEventHandler(
	handlerUC HandlerUseCaseInterface,
	waService *WhatsAppService,
	stateRepo repository.UserStateRepository,
	waClient *whatsappInfra.WhatsAppClient,
	storage repository.StorageRepository,
) *WhatsAppEventHandler {
	return &WhatsAppEventHandler{
		handlerUC: handlerUC,
		waService: waService,
		stateRepo: stateRepo,
		waClient:  waClient,
		storage:   storage,
	}
}

func (h *WhatsAppEventHandler) SetMessageStore(msgStore *repository.MessageStore) {
	h.msgStore = msgStore
}

func (h *WhatsAppEventHandler) SetHTTPServer(server HTTPServer) {
	h.httpServer = server
}

func (h *WhatsAppEventHandler) SetLuaService(luaService LuaService) {
	h.luaService = luaService
}

func (h *WhatsAppEventHandler) SetAIClient(client *ai.AIClient) {
	h.aiClient = client
}

func (h *WhatsAppEventHandler) SetSettingsRepo(repo repository.SettingsRepository) {
	h.settingsRepo = repo
}

func (h *WhatsAppEventHandler) SetAccountEvents(a AccountEvents) {
	h.accounts = a
}

func (h *WhatsAppEventHandler) SetHistorySyncService(service *HistorySyncService) {
	h.historySync = service
}

func (h *WhatsAppEventHandler) HandleEvent(evt interface{}) {
	switch v := evt.(type) {
	case *events.PairSuccess:
		// Must run synchronously: the account data dir has to be bound before
		// the first history sync / messages of the new account are stored.
		if h.accounts != nil {
			h.accounts.HandlePaired(v.ID.User)
		}
	case *events.LoggedOut:
		// Unlinked from the phone. Reset off the receive loop: it disconnects.
		if h.accounts != nil {
			go h.accounts.HandleLoggedOut()
		}
	case *events.Message:
		if h.historySync != nil {
			if notif := v.Message.GetProtocolMessage().GetHistorySyncNotification(); notif != nil {
				h.historySync.StageNotification(v.Info.ID, notif)
			}
		}

		// Reactions and poll votes are metadata updates on existing messages,
		// not new messages: process them even when IsFromMe (echo from the
		// phone or another linked device) and never render them as bubbles.
		if rm := v.Message.GetReactionMessage(); rm != nil {
			h.handleReaction(v, rm)
			return
		}
		if v.Message.GetPollUpdateMessage() != nil {
			h.handlePollUpdate(v)
			return
		}

		// Status (story) updates arrive on the status broadcast chat; own
		// statuses posted from the phone are IsFromMe and must be kept.
		if v.Info.Chat.String() == waTypes.StatusBroadcastJID.String() {
			h.handleStatusMessage(v)
			return
		}

		// Channel (newsletter) posts are one-way broadcasts: surface them as
		// channel_message events, never as chat messages.
		if v.Info.Chat.Server == waTypes.NewsletterServer {
			h.handleChannelMessage(v)
			return
		}

		fmt.Printf("\n[MSG_IN] ID: %s | From: %s | Alt: %s | Group: %v\n", v.Info.ID, v.Info.Sender.String(), v.Info.SenderAlt.String(), v.Info.IsGroup)

		// Save LID mapping for all messages (both private and group)
		if h.msgStore != nil {
			sender := v.Info.Sender.ToNonAD()
			alt := v.Info.SenderAlt.ToNonAD()

			if !alt.IsEmpty() {
				fmt.Printf("[MAP_DEBUG] Checking: %s | Alt: %s\n", sender.String(), alt.String())

				// Server-agnostic mapping: if one is PN (s.whatsapp.net) and other is not, it's a mapping
				if sender.Server == "s.whatsapp.net" && alt.Server != "s.whatsapp.net" {
					h.msgStore.SaveLIDMapping(alt.String(), sender.String())
					fmt.Printf("[MAP_SAVE] Linked LID %s to PN %s\n", alt.String(), sender.String())
				} else if alt.Server == "s.whatsapp.net" && sender.Server != "s.whatsapp.net" {
					h.msgStore.SaveLIDMapping(sender.String(), alt.String())
					fmt.Printf("[MAP_SAVE] Linked LID %s to PN %s\n", sender.String(), alt.String())
				}
			}
		}

		h.handleMessage(v)
	case *events.Receipt:
		h.handleReceipt(v)
	case *events.Contact:
		h.handleContact(v)
	case *events.HistorySync:
		if h.historySync != nil {
			if err := h.historySync.StageData(v.Data); err != nil {
				fmt.Printf("[HISTORY] Failed to stage event: %v\n", err)
			}
		}
	case *events.Pin:
		h.handlePin(v)
	case *events.Mute:
		h.handleMute(v)
	case *events.Archive:
		h.handleArchive(v)
	case *events.MarkChatAsRead:
		h.handleMarkChatAsRead(v)
	case *events.DeleteChat:
		h.handleDeleteChat(v)
	case *events.ClearChat:
		h.handleClearChat(v)
	case *events.DeleteForMe:
		h.handleDeleteForMe(v)
	case *events.PushName:
		h.handlePushName(v)
	case *events.UserAbout:
		h.handleUserAbout(v)
	case *events.UnarchiveChatsSetting:
		h.handleUnarchiveChatsSetting(v)
	case *events.GroupInfo:
		h.handleGroupInfo(v)
	case *events.JoinedGroup:
		h.handleJoinedGroup(v)
	case *events.Picture:
		h.handlePictureChange(v)
	case *events.NewsletterJoin:
		if h.httpServer != nil {
			h.httpServer.BroadcastMessage("channels_changed", map[string]string{"reason": "joined"})
		}
	case *events.NewsletterLeave:
		if h.httpServer != nil {
			h.httpServer.BroadcastMessage("channels_changed", map[string]string{"reason": "left"})
		}
	case *events.NewsletterMuteChange:
		if h.httpServer != nil {
			h.httpServer.BroadcastMessage("channels_changed", map[string]string{"reason": "mute"})
		}
	case *events.NewsletterLiveUpdate:
		h.handleChannelLiveUpdate(v)
	case *events.ChatPresence:
		h.handleChatPresence(v)
	case *events.Presence:
		h.handlePresence(v)
	}
}

// refreshGroupCache re-fetches group metadata from the server, caches it, and
// returns the snapshot (nil on failure).
func (h *WhatsAppEventHandler) refreshGroupCache(ctx context.Context, chatID string) *repository.GroupCache {
	info, err := h.waClient.FetchGroupInfo(ctx, chatID)
	if err != nil || info == nil {
		fmt.Printf("[GROUP] Failed to refresh %s: %v\n", chatID, err)
		return nil
	}
	var ownJID waTypes.JID
	if id := h.waClient.CoreClient().Store.ID; id != nil {
		ownJID = id.ToNonAD()
	}
	cache := whatsappInfra.BuildGroupCache(info, ownJID)
	if err := h.msgStore.SaveGroupCache(cache); err != nil {
		fmt.Printf("[GROUP] Failed to cache %s: %v\n", chatID, err)
	}
	return cache
}

func (h *WhatsAppEventHandler) handleGroupInfo(evt *events.GroupInfo) {
	if h.msgStore == nil {
		return
	}
	chatID := evt.JID.String()
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
		defer cancel()
		cache := h.refreshGroupCache(ctx, chatID)
		if cache == nil {
			return
		}
		if cache.Name != "" {
			h.msgStore.UpdateChatName(chatID, cache.Name)
		}
		if h.httpServer != nil {
			h.httpServer.BroadcastMessage("group_updated", map[string]interface{}{
				"chatId": h.msgStore.ResolveChatID(chatID),
				"group":  cache,
			})
			if cache.Name != "" {
				h.httpServer.BroadcastMessage("chat_name_update", map[string]interface{}{
					"chatId": chatID,
					"name":   cache.Name,
					"avatar": "",
				})
			}
		}
	}()
}

func (h *WhatsAppEventHandler) handleJoinedGroup(evt *events.JoinedGroup) {
	if h.httpServer == nil || h.msgStore == nil {
		return
	}
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
		defer cancel()
		if cache := h.refreshGroupCache(ctx, evt.JID.String()); cache != nil && cache.Name != "" {
			h.msgStore.UpdateChatName(evt.JID.String(), cache.Name)
		}
		h.httpServer.BroadcastMessage("chats_changed", map[string]interface{}{"reason": "joined_group"})
	}()
}

func (h *WhatsAppEventHandler) handlePictureChange(evt *events.Picture) {
	if h.msgStore == nil {
		return
	}
	jid := evt.JID.String()
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
		defer cancel()
		avatar := ""
		if !evt.Remove {
			if url, err := h.waClient.GetProfilePictureInfo(ctx, jid); err == nil {
				avatar = url
			}
		}
		if err := h.msgStore.UpdateChatAvatar(jid, avatar); err == nil && h.httpServer != nil {
			h.httpServer.BroadcastMessage("chat_name_update", map[string]interface{}{
				"chatId": jid,
				"name":   "",
				"avatar": avatar,
			})
		}
	}()
}

func (h *WhatsAppEventHandler) handlePin(evt *events.Pin) {
	if h.msgStore == nil || evt.Action == nil {
		return
	}
	state, err := h.msgStore.GetChatState(evt.JID.String())
	if err != nil {
		return
	}
	if evt.Action.GetPinned() {
		pinnedAt := evt.Timestamp.UnixMilli()
		state.PinnedAt = &pinnedAt
	} else {
		state.PinnedAt = nil
	}
	h.persistAndBroadcastChatState(state)
}

func (h *WhatsAppEventHandler) handleMute(evt *events.Mute) {
	if h.msgStore == nil || evt.Action == nil {
		return
	}
	state, err := h.msgStore.GetChatState(evt.JID.String())
	if err != nil {
		return
	}
	state.MuteMode = "off"
	state.MutedUntil = nil
	if evt.Action.GetMuted() {
		end := evt.Action.GetMuteEndTimestamp()
		if end < 0 {
			state.MuteMode = "forever"
		} else if end > time.Now().UnixMilli() {
			state.MuteMode = "until"
			state.MutedUntil = &end
		}
	}
	h.persistAndBroadcastChatState(state)
}

func (h *WhatsAppEventHandler) handleArchive(evt *events.Archive) {
	if h.msgStore == nil || evt.Action == nil {
		return
	}
	state, err := h.msgStore.GetChatState(evt.JID.String())
	if err != nil {
		return
	}
	state.Archived = evt.Action.GetArchived()
	if state.Archived {
		state.PinnedAt = nil
	}
	h.persistAndBroadcastChatState(state)
}

// handleMarkChatAsRead clears the unread badge when the chat was read on
// another device (app-state sync).
func (h *WhatsAppEventHandler) handleMarkChatAsRead(evt *events.MarkChatAsRead) {
	if evt.Action == nil {
		return
	}
	chatID := evt.JID.ToNonAD().String()
	if evt.Action.GetRead() {
		h.clearChatUnread(chatID, !evt.FromFullSync)
		return
	}
	// Marked unread on another device: our own unread flag.
	if h.msgStore == nil {
		return
	}
	changed, err := h.msgStore.MarkChatUnread(chatID)
	if err != nil {
		fmt.Printf("[READ_SYNC] Failed to mark %s unread: %v\n", chatID, err)
		return
	}
	if changed && !evt.FromFullSync {
		h.broadcastChatEvent("chat_update", chatID, map[string]interface{}{"unread": 1})
	}
}

// handleDeleteChat hides a chat deleted on another device. The chat and its
// messages stay in the DB, flagged deleted_at; a newer message revives it.
func (h *WhatsAppEventHandler) handleDeleteChat(evt *events.DeleteChat) {
	if h.msgStore == nil {
		return
	}
	changed, err := h.msgStore.MarkChatDeleted(evt.JID.ToNonAD().String(), evt.Timestamp.UnixMilli())
	if err != nil {
		fmt.Printf("[CHAT_SYNC] Failed to mark %s deleted: %v\n", evt.JID, err)
		return
	}
	if changed && !evt.FromFullSync {
		h.broadcastChatEvent("chat_deleted", evt.JID.ToNonAD().String(), nil)
	}
}

// handleClearChat empties a chat cleared on another device; messages are
// flagged deleted_at, not removed.
func (h *WhatsAppEventHandler) handleClearChat(evt *events.ClearChat) {
	if h.msgStore == nil {
		return
	}
	if err := h.msgStore.MarkChatCleared(evt.JID.ToNonAD().String(), evt.Timestamp.UnixMilli()); err != nil {
		fmt.Printf("[CHAT_SYNC] Failed to mark %s cleared: %v\n", evt.JID, err)
		return
	}
	if !evt.FromFullSync {
		h.broadcastChatEvent("chat_cleared", evt.JID.ToNonAD().String(), nil)
	}
}

// handleDeleteForMe hides a message deleted for the user on another device;
// the row is kept, flagged deleted_at.
func (h *WhatsAppEventHandler) handleDeleteForMe(evt *events.DeleteForMe) {
	if h.msgStore == nil {
		return
	}
	chatID, lastMsg, err := h.msgStore.MarkMessageDeleted(evt.MessageID, evt.Timestamp.UnixMilli())
	if err != nil {
		fmt.Printf("[CHAT_SYNC] Failed to mark message %s deleted: %v\n", evt.MessageID, err)
		return
	}
	if chatID == "" || evt.FromFullSync || h.httpServer == nil {
		return
	}
	h.httpServer.BroadcastMessage("message_deleted", map[string]string{
		"chatId": h.msgStore.ResolveChatID(chatID),
		"id":     evt.MessageID,
	})
	h.broadcastChatEvent("chat_update", chatID, map[string]interface{}{"lastMsg": lastMsg})
}

// handlePushName follows a contact renaming themselves.
func (h *WhatsAppEventHandler) handlePushName(evt *events.PushName) {
	if h.msgStore == nil || strings.TrimSpace(evt.NewPushName) == "" {
		return
	}
	seen := map[string]struct{}{}
	for _, jid := range []waTypes.JID{evt.JID.ToNonAD(), evt.JIDAlt.ToNonAD()} {
		if jid.IsEmpty() {
			continue
		}
		id := jid.String()
		if _, ok := seen[id]; ok {
			continue
		}
		seen[id] = struct{}{}
		renamed, err := h.msgStore.ApplyPushName(id, evt.OldPushName, evt.NewPushName)
		if err != nil {
			fmt.Printf("[CONTACT] Failed to apply push name for %s: %v\n", id, err)
			continue
		}
		if renamed && h.httpServer != nil {
			h.httpServer.BroadcastMessage("chat_name_update", map[string]interface{}{
				"chatId": id,
				"name":   strings.TrimSpace(evt.NewPushName),
				"avatar": "",
			})
		}
	}
}

// handleUserAbout stores a contact's updated about text.
func (h *WhatsAppEventHandler) handleUserAbout(evt *events.UserAbout) {
	if h.msgStore == nil || evt.JID.IsEmpty() {
		return
	}
	if err := h.msgStore.SaveContactAbout(evt.JID.ToNonAD().String(), evt.Status); err != nil {
		fmt.Printf("[CONTACT] Failed to save about for %s: %v\n", evt.JID, err)
	}
}

// settingUnarchiveOnMessage is the app setting mirroring WhatsApp's "Keep
// chats archived" (inverted). Unset means WhatsApp's default: unarchive.
const settingUnarchiveOnMessage = "unarchive_on_new_message"

func (h *WhatsAppEventHandler) handleUnarchiveChatsSetting(evt *events.UnarchiveChatsSetting) {
	if h.settingsRepo == nil || evt.Action == nil {
		return
	}
	val := "false"
	if evt.Action.GetUnarchiveChats() {
		val = "true"
	}
	if err := h.settingsRepo.Set(context.Background(), settingUnarchiveOnMessage, val); err != nil {
		fmt.Printf("[SETTINGS] Failed to save %s: %v\n", settingUnarchiveOnMessage, err)
	}
}

// unarchiveOnIncoming brings an archived chat back to the list when a new
// message arrives, unless the user keeps chats archived or the chat is muted
// (WhatsApp leaves muted chats archived).
func (h *WhatsAppEventHandler) unarchiveOnIncoming(chatID string) {
	if h.msgStore == nil {
		return
	}
	if h.settingsRepo != nil {
		if val, err := h.settingsRepo.Get(context.Background(), settingUnarchiveOnMessage); err == nil && val == "false" {
			return
		}
	}
	state, err := h.msgStore.GetChatState(chatID)
	if err != nil || !state.Archived || state.MuteMode != "off" {
		return
	}
	state.Archived = false
	h.persistAndBroadcastChatState(state)
}

// clearChatUnread zeroes the stored unread count and tells clients to refresh.
func (h *WhatsAppEventHandler) clearChatUnread(chatID string, broadcast bool) {
	if h.msgStore == nil || chatID == "" {
		return
	}
	if err := h.msgStore.MarkAsRead(chatID); err != nil {
		fmt.Printf("[READ_SYNC] Failed to clear unread for %s: %v\n", chatID, err)
		return
	}
	if broadcast {
		h.broadcastChatEvent("chat_update", chatID, map[string]interface{}{"unread": 0})
	}
}

// broadcastChatEvent sends a targeted chat event keyed by the chat's resolved
// ID so clients patch their list in place instead of refetching it.
func (h *WhatsAppEventHandler) broadcastChatEvent(eventType, chatID string, fields map[string]interface{}) {
	if h.httpServer == nil {
		return
	}
	payload := map[string]interface{}{"chatId": h.msgStore.ResolveChatID(chatID)}
	for k, v := range fields {
		payload[k] = v
	}
	h.httpServer.BroadcastMessage(eventType, payload)
}

func (h *WhatsAppEventHandler) persistAndBroadcastChatState(state repository.ChatState) {
	if err := h.msgStore.UpdateChatState(state); err != nil {
		fmt.Printf("[CHAT_STATE] Failed to persist %s: %v\n", state.ChatID, err)
		return
	}
	if h.httpServer != nil {
		h.httpServer.BroadcastMessage("chat_state", state)
	}
}

func (h *WhatsAppEventHandler) handleReceipt(evt *events.Receipt) {
	// A read receipt from our own account means we read the chat on another
	// device: "read-self" when read receipts are off in privacy settings,
	// otherwise a plain "read" sent by our own JID. Clear the chat's unread
	// badge; the messages are incoming, so there is no tick to update.
	if evt.IsFromMe && (evt.Type == events.ReceiptTypeRead || evt.Type == events.ReceiptTypeReadSelf) {
		fmt.Printf("[READ_SYNC] %s read on another device (%d msgs)\n", evt.Chat, len(evt.MessageIDs))
		h.clearChatUnread(evt.Chat.ToNonAD().String(), true)
		return
	}

	status := "sent"
	if evt.Type == events.ReceiptTypeDelivered {
		status = "delivered"
	} else if evt.Type == events.ReceiptTypeRead {
		status = "read"
	} else {
		return
	}

	if evt.Type == events.ReceiptTypeRead && h.msgStore != nil && evt.IsGroup {
		reader := evt.Sender.ToNonAD()
		if reader.Server == "lid" {
			alt := evt.SenderAlt.ToNonAD()
			if alt.Server == "s.whatsapp.net" {
				reader = alt
			} else if resolved := h.msgStore.ResolveChatID(reader.String()); resolved != reader.String() {
				if parsed, err := waTypes.ParseJID(resolved); err == nil {
					reader = parsed
				}
			}
		}
		for _, msgID := range evt.MessageIDs {
			_, err := h.msgStore.SaveReadReceipt(evt.Chat.String(), string(msgID), reader.String(), evt.Timestamp.UnixMilli())
			if err != nil {
				fmt.Printf("[RECEIPT] Failed to save read receipt for %s: %v\n", msgID, err)
			}
		}
	}
	if h.httpServer == nil {
		return
	}

	for _, msgID := range evt.MessageIDs {
		h.httpServer.UpdateMessageStatus(msgID, status)
		fmt.Printf("✅ Updated status for %s to %s\n", msgID, status)
	}
}

func (h *WhatsAppEventHandler) handleContact(evt *events.Contact) {
	if h.msgStore == nil || evt == nil || evt.Action == nil {
		return
	}
	name := strings.TrimSpace(evt.Action.GetFullName())
	if name == "" {
		name = strings.TrimSpace(evt.Action.GetFirstName())
	}
	if name == "" {
		name = strings.TrimSpace(evt.Action.GetUsername())
	}
	if name == "" {
		return
	}
	jids := []waTypes.JID{evt.JID.ToNonAD()}
	for _, raw := range []string{evt.Action.GetPnJID(), evt.Action.GetLidJID()} {
		if jid, err := waTypes.ParseJID(raw); err == nil && !jid.IsEmpty() {
			jids = append(jids, jid.ToNonAD())
		}
	}
	seen := make(map[string]struct{}, len(jids))
	for _, jid := range jids {
		if jid.IsEmpty() {
			continue
		}
		value := jid.String()
		if _, ok := seen[value]; ok {
			continue
		}
		seen[value] = struct{}{}
		if err := h.msgStore.UpdateContactName(value, name); err != nil {
			fmt.Printf("[CONTACT] Failed to save contact name for %s: %v\n", value, err)
		}
	}
}

func (h *WhatsAppEventHandler) handleMessage(evt *events.Message) {
	// Linked-device echoes may follow messages already persisted by the send
	// handler. Ignore an existing ID, while allowing messages sent directly
	// from the paired phone to be stored through the normal chat path.
	if h.msgStore != nil && h.msgStore.HasMessage(evt.Info.ID) {
		fmt.Printf("[DEBUG] Skipping duplicate message: %s (chat=%s)\n", evt.Info.ID, evt.Info.Chat.String())
		return
	}

	// Extract JIDs
	var senderJID waTypes.JID
	if evt.Info.IsFromMe {
		senderJID = evt.Info.Sender.ToNonAD()
		if senderJID.IsEmpty() && h.waClient != nil && h.waClient.CoreClient().Store.ID != nil {
			senderJID = h.waClient.CoreClient().Store.ID.ToNonAD()
		}
	} else if evt.Info.IsGroup {
		senderJID = evt.Info.Sender.ToNonAD()
		// If sender is LID, try to resolve to PN JID
		if senderJID.Server == "lid" {
			alt := evt.Info.SenderAlt.ToNonAD()
			if !alt.IsEmpty() && alt.Server == "s.whatsapp.net" {
				senderJID = alt
			} else if h.msgStore != nil {
				resolved := h.msgStore.ResolveChatID(senderJID.String())
				if resolved != senderJID.String() {
					senderJID, _ = waTypes.ParseJID(resolved)
				}
			}
		}
	} else {
		senderJID = evt.Info.Chat.ToNonAD()
	}

	chatID := evt.Info.Chat.String()
	// Ignore WhatsApp Status updates
	if chatID == "status@broadcast" {
		return
	}

	// Handle Protocol Messages (Edit/Revoke)
	if evt.Message.GetProtocolMessage() != nil {
		pm := evt.Message.GetProtocolMessage()
		if pm.GetType() == 14 { // REVOKE is 14
			targetID := pm.GetKey().GetID()
			if h.msgStore != nil {
				h.msgStore.DeleteMessage(targetID)
			}
			if h.httpServer != nil {
				h.httpServer.BroadcastMessage("message_deleted", map[string]string{
					"chatId": chatID,
					"id":     targetID,
				})
			}
			return
		} else if pm.GetType() == 16 { // MESSAGE_EDIT is 16
			targetID := pm.GetKey().GetID()
			newContent := ""
			if pm.GetEditedMessage().GetConversation() != "" {
				newContent = pm.GetEditedMessage().GetConversation()
			} else if pm.GetEditedMessage().GetExtendedTextMessage() != nil {
				newContent = pm.GetEditedMessage().GetExtendedTextMessage().GetText()
			}

			if h.msgStore != nil {
				h.msgStore.UpdateMessageContent(targetID, newContent)
			}
			if h.httpServer != nil {
				h.httpServer.BroadcastMessage("message_edited", map[string]string{
					"chatId":  chatID,
					"id":      targetID,
					"content": newContent,
				})
			}
			return
		}
	}

	senderName := evt.Info.PushName

	// If push name is missing, try to get from our contact store
	if senderName == "" && !evt.Info.IsFromMe && h.msgStore != nil {
		if storedName, err := h.msgStore.GetContactName(senderJID.String()); err == nil && storedName != "" {
			senderName = storedName
		}
	}

	// Update sender info in background
	if h.msgStore != nil && !evt.Info.IsFromMe {
		go func() {
			avatar, _ := h.waClient.GetProfilePictureInfo(context.Background(), senderJID.String())
			h.msgStore.SaveContact(&repository.Contact{
				ID:     senderJID.String(),
				Name:   senderName,
				JID:    senderJID.String(),
				Avatar: avatar,
			})
		}()
	}

	// Extract message text
	var messageText string
	isMedia := false
	if evt.Message.ExtendedTextMessage != nil && evt.Message.ExtendedTextMessage.Text != nil {
		messageText = *evt.Message.ExtendedTextMessage.Text
	} else if evt.Message.ImageMessage != nil && evt.Message.ImageMessage.Caption != nil {
		messageText = *evt.Message.ImageMessage.Caption
		isMedia = true
	} else if evt.Message.VideoMessage != nil && evt.Message.VideoMessage.Caption != nil {
		messageText = *evt.Message.VideoMessage.Caption
		isMedia = true
	} else if evt.Message.DocumentMessage != nil && evt.Message.DocumentMessage.Caption != nil {
		messageText = *evt.Message.DocumentMessage.Caption
		isMedia = true
	} else if evt.Message.ImageMessage != nil || evt.Message.VideoMessage != nil || evt.Message.DocumentMessage != nil || evt.Message.StickerMessage != nil || evt.Message.AudioMessage != nil {
		isMedia = true
		messageText = evt.Message.GetConversation()
	} else {
		messageText = evt.Message.GetConversation()
	}

	// 0. Filter out empty messages that are not media and carry no other
	// renderable payload (polls, locations, contacts, view-once wraps).
	hasPayload := isMedia ||
		evt.Message.GetPollCreationMessage() != nil ||
		evt.Message.GetLocationMessage() != nil ||
		evt.Message.GetLiveLocationMessage() != nil ||
		evt.Message.GetContactMessage() != nil ||
		evt.Message.GetContactsArrayMessage() != nil ||
		evt.Message.GetViewOnceMessage().GetMessage() != nil ||
		evt.Message.GetViewOnceMessageV2().GetMessage() != nil
	if messageText == "" && !hasPayload {
		fmt.Printf("[DEBUG] Skipping empty message: %s (chat=%s)\n", evt.Info.ID, evt.Info.Chat.String())
		return
	}

	// Determine what name to show in the Sidebar
	displayChatName := ""
	if !evt.Info.IsGroup {
		if evt.Info.IsFromMe {
			peerJID := evt.Info.Chat.ToNonAD()
			if h.msgStore != nil {
				displayChatName, _ = h.msgStore.GetContactName(peerJID.String())
			}
			if displayChatName == "" {
				displayChatName = peerJID.User
			}
		} else {
			displayChatName = senderName
		}
		if displayChatName == "" {
			displayChatName = senderJID.User
		}
	}

	// 1. Show message immediately
	h.showMessage(evt, senderJID, messageText, chatID, displayChatName, senderName)
	fmt.Printf("[MSG] Logged message: [%s] from=%s name=%s\n", chatID, senderJID.String(), senderName)

	// 2. Update group name/avatar in background if it's a group
	if evt.Info.IsGroup {
		go func() {
			groupInfo, err := h.waClient.GetGroupInfo(context.Background(), chatID)
			if err == nil && groupInfo != nil {
				avatarURL, _ := h.waClient.GetProfilePictureInfo(context.Background(), chatID)
				if h.msgStore != nil {
					if groupInfo.Name != "" {
						h.msgStore.UpdateChatName(chatID, groupInfo.Name)
					}
					if avatarURL != "" {
						h.msgStore.UpdateChatAvatar(chatID, avatarURL)
					}

					if h.httpServer != nil {
						h.httpServer.BroadcastMessage("chat_name_update", map[string]interface{}{
							"chatId": chatID,
							"name":   groupInfo.Name,
							"avatar": avatarURL,
						})
					}
				}
			}
		}()
	} else {
		go func() {
			avatarURL, err := h.waClient.GetProfilePictureInfo(context.Background(), chatID)
			if err == nil && avatarURL != "" && h.msgStore != nil {
				h.msgStore.UpdateChatAvatar(chatID, avatarURL)
			}
		}()
	}

	// Dispatch to AI companion (fire-and-forget)
	if !evt.Info.IsFromMe && h.aiClient != nil && messageText != "" {
		go func() {
			ctx, cancel := context.WithTimeout(context.Background(), 12*time.Second)
			defer cancel()
			h.aiClient.Dispatch(ctx, chatID, senderJID.String(), senderName, messageText, evt.Info.ID, evt.Info.IsGroup)
		}()
	}

	// --- LOGIC BOT (RUN IN BACKGROUND WITH FILTERS) ---
	if evt.Info.IsFromMe {
		return
	}
	go func() {
		defer func() {
			if r := recover(); r != nil {
				fmt.Printf("[PANIC] Panic in bot logic: %v\n", r)
			}
		}()

		if senderJID.UserInt() == 13135550002 {
			return
		}

		msgTime := evt.Info.Timestamp
		now := time.Now()
		if now.Sub(msgTime) > 1*time.Minute {
			return
		}

		ctx := context.Background()
		h.processCommand(ctx, evt, senderJID, messageText, chatID)
	}()
}

func (h *WhatsAppEventHandler) showMessage(evt *events.Message, senderJID waTypes.JID, messageText, chatID, chatName, senderName string) {
	fmt.Printf("[DEBUG] showMessage called: msgID=%s, IsFromMe=%v, chat=%s\n", evt.Info.ID, evt.Info.IsFromMe, chatID)
	ctx := context.Background()

	if h.msgStore != nil {
		msgType := "text"
		var mediaURL string
		var content string
		var extra repository.MessageExtra

		// View-once media is wrapped in a FutureProofMessage; unwrap it so the
		// regular media branches below handle the inner image/video.
		payload := evt.Message
		if inner := payload.GetViewOnceMessageV2().GetMessage(); inner != nil {
			payload = inner
			extra.ViewOnce = &repository.ViewOnceMeta{MediaType: "image", Viewed: false}
		} else if inner := payload.GetViewOnceMessage().GetMessage(); inner != nil {
			payload = inner
			extra.ViewOnce = &repository.ViewOnceMeta{MediaType: "image", Viewed: false}
		}

		forwarded := false
		if ci := getContextInfo(payload); ci != nil {
			forwarded = ci.GetIsForwarded()
			if len(ci.GetMentionedJID()) > 0 {
				extra.Mentions = append([]string(nil), ci.GetMentionedJID()...)
			}
		}

		if payload.GetImageMessage() != nil {
			msgType = "image"
			img := payload.GetImageMessage()
			if extra.ViewOnce != nil {
				extra.ViewOnce.MediaType = "image"
			}
			msg := &entity.Message{
				VMessage:  payload,
				Timestamp: evt.Info.Timestamp,
				IsGroup:   evt.Info.IsGroup,
				SenderJID: senderJID.String(),
			}

			data, _, err := h.waClient.DownloadMedia(ctx, msg)
			if err == nil && len(data) > 0 {
				ext := ".jpg"
				if img.GetMimetype() == "image/png" {
					ext = ".png"
				} else if img.GetMimetype() == "image/webp" {
					ext = ".webp"
				}
				safeJID := strings.ReplaceAll(senderJID.String(), "@", "_")
				safeJID = strings.ReplaceAll(safeJID, ".", "_")
				filename := fmt.Sprintf("img_%d_%s%s", time.Now().UnixMilli(), safeJID, ext)

				if _, err := h.storage.Save(ctx, filename, bytes.NewReader(data)); err == nil {
					mediaURL = fmt.Sprintf("/media/%s", filename)
				}
				content = img.GetCaption()
			} else {
				content = img.GetCaption()
				if content == "" {
					content = "[Image]"
				}
			}
		} else if payload.GetStickerMessage() != nil {
			msgType = "sticker"
			msg := &entity.Message{
				VMessage:  payload,
				Timestamp: evt.Info.Timestamp,
				IsGroup:   evt.Info.IsGroup,
				SenderJID: senderJID.String(),
			}

			data, _, err := h.waClient.DownloadMedia(ctx, msg)
			if err == nil && len(data) > 0 {
				filename := fmt.Sprintf("sticker_%d_%s.webp", time.Now().UnixMilli(), strings.ReplaceAll(senderJID.String(), "@", "_"))

				if _, err := h.storage.Save(ctx, filename, bytes.NewReader(data)); err == nil {
					mediaURL = fmt.Sprintf("/media/%s", filename)
				}
			}
			content = "[Sticker]"
		} else if payload.GetVideoMessage() != nil {
			vid := payload.GetVideoMessage()
			if extra.ViewOnce != nil {
				extra.ViewOnce.MediaType = "video"
			}
			if vid.GetGifPlayback() {
				// WhatsApp represents GIFs as silent MP4 videos with the
				// gifPlayback flag set.
				msgType = "gif"
				extra.GIF = true
				content = vid.GetCaption()
				if content == "" {
					content = "[GIF]"
				}
			} else {
				msgType = "video"
				content = vid.GetCaption()
				if content == "" {
					content = "[Video]"
				}
			}
		} else if payload.GetDocumentMessage() != nil {
			msgType = "document"
			doc := payload.GetDocumentMessage()
			msg := &entity.Message{
				VMessage:  payload,
				Timestamp: evt.Info.Timestamp,
				IsGroup:   evt.Info.IsGroup,
				SenderJID: senderJID.String(),
			}

			data, _, err := h.waClient.DownloadMedia(ctx, msg)
			if err == nil && len(data) > 0 {
				ext := filepath.Ext(doc.GetFileName())
				if ext == "" {
					ext = ".bin"
				}
				safeJID := strings.ReplaceAll(senderJID.String(), "@", "_")
				safeJID = strings.ReplaceAll(safeJID, ".", "_")
				filename := fmt.Sprintf("doc_%d_%s%s", time.Now().UnixMilli(), safeJID, ext)

				if _, err := h.storage.Save(ctx, filename, bytes.NewReader(data)); err == nil {
					mediaURL = fmt.Sprintf("/media/%s", filename)
				}
				content = doc.GetFileName()
				if content == "" {
					content = doc.GetTitle()
				}
			} else {
				content = doc.GetTitle()
				if content == "" {
					content = "[Document]"
				}
			}
		} else if payload.GetAudioMessage() != nil {
			audio := payload.GetAudioMessage()
			if audio.GetPTT() {
				msgType = "ptt"
			} else {
				msgType = "audio"
			}
			msg := &entity.Message{
				VMessage:  payload,
				Timestamp: evt.Info.Timestamp,
				IsGroup:   evt.Info.IsGroup,
				SenderJID: senderJID.String(),
			}

			data, _, err := h.waClient.DownloadMedia(ctx, msg)
			if err == nil && len(data) > 0 {
				ext := ".ogg"
				switch audio.GetMimetype() {
				case "audio/mpeg":
					ext = ".mp3"
				case "audio/mp4":
					ext = ".m4a"
				case "audio/opus":
					ext = ".opus"
				case "audio/wav":
					ext = ".wav"
				case "audio/webm":
					ext = ".webm"
				}
				safeJID := strings.ReplaceAll(senderJID.String(), "@", "_")
				safeJID = strings.ReplaceAll(safeJID, ".", "_")
				filename := fmt.Sprintf("audio_%d_%s%s", time.Now().UnixMilli(), safeJID, ext)

				if _, err := h.storage.Save(ctx, filename, bytes.NewReader(data)); err == nil {
					mediaURL = fmt.Sprintf("/media/%s", filename)
				}
			}
			content = "[Audio]"
			if audio.GetPTT() {
				content = "[Voice Message]"
			}
		} else if loc := payload.GetLocationMessage(); loc != nil {
			msgType = "location"
			extra.Location = &repository.LocationMeta{
				Latitude:  loc.GetDegreesLatitude(),
				Longitude: loc.GetDegreesLongitude(),
				Name:      loc.GetName(),
				Address:   loc.GetAddress(),
			}
			content = strings.TrimSpace(loc.GetName() + " " + loc.GetAddress())
			if content == "" {
				content = "[Lokasi]"
			}
			if thumb := loc.GetJPEGThumbnail(); len(thumb) > 0 {
				if url := h.savePreviewBytes(ctx, thumb); url != "" {
					extra.Location.ThumbnailURL = url
				}
			}
		} else if ll := payload.GetLiveLocationMessage(); ll != nil {
			msgType = "location"
			extra.Location = &repository.LocationMeta{
				Latitude:  ll.GetDegreesLatitude(),
				Longitude: ll.GetDegreesLongitude(),
				Live:      true,
			}
			content = "[Lokasi Langsung]"
			if caption := ll.GetCaption(); caption != "" {
				content = caption
			}
			if thumb := ll.GetJPEGThumbnail(); len(thumb) > 0 {
				if url := h.savePreviewBytes(ctx, thumb); url != "" {
					extra.Location.ThumbnailURL = url
				}
			}
		} else if cm := payload.GetContactMessage(); cm != nil {
			msgType = "contact"
			extra.Contact = &repository.ContactMeta{
				DisplayName: cm.GetDisplayName(),
				Contacts: []repository.ContactEntry{{
					DisplayName: cm.GetDisplayName(),
					VCard:       cm.GetVcard(),
				}},
			}
			content = cm.GetDisplayName()
			if content == "" {
				content = "[Kontak]"
			}
		} else if ca := payload.GetContactsArrayMessage(); ca != nil {
			msgType = "contact"
			entries := make([]repository.ContactEntry, 0, len(ca.GetContacts()))
			for _, c := range ca.GetContacts() {
				entries = append(entries, repository.ContactEntry{
					DisplayName: c.GetDisplayName(),
					VCard:       c.GetVcard(),
				})
			}
			extra.Contact = &repository.ContactMeta{
				DisplayName: ca.GetDisplayName(),
				Contacts:    entries,
			}
			content = fmt.Sprintf("%s (%d kontak)", ca.GetDisplayName(), len(entries))
		} else if poll := payload.GetPollCreationMessage(); poll != nil {
			msgType = "poll"
			options := make([]repository.PollOption, 0, len(poll.GetOptions()))
			for _, o := range poll.GetOptions() {
				options = append(options, repository.PollOption{Name: o.GetOptionName()})
			}
			extra.Poll = &repository.PollMeta{
				Question:    poll.GetName(),
				Options:     options,
				MultiSelect: poll.GetSelectableOptionsCount() > 1,
				Votes:       map[string][]string{},
			}
			content = "📊 " + poll.GetName()
		} else {
			content = messageText
			if et := payload.GetExtendedTextMessage(); et != nil {
				if preview := h.extractLinkPreview(ctx, et); preview != nil {
					extra.LinkPreview = preview
				}
			}
		}

		from := senderJID.String()
		to := "me"
		status := "received"
		if evt.Info.IsFromMe {
			from = "me"
			to = chatID
			status = "sent"
		}
		msg := &repository.Message{
			ID:          evt.Info.ID,
			ChatID:      chatID,
			From:        from,
			To:          to,
			Content:     content,
			Timestamp:   evt.Info.Timestamp.UnixMilli(),
			Status:      status,
			Type:        msgType,
			MediaURL:    mediaURL,
			IsAutomatic: false,
			SenderName:  senderName,
			ChatName:    chatName,
			Forwarded:   forwarded,
			ReplyToID: func() string {
				ctxInfo := getContextInfo(payload)
				if ctxInfo != nil {
					return ctxInfo.GetStanzaID()
				}
				return ""
			}(),
		}
		if extra.Poll != nil || extra.Location != nil || extra.Contact != nil || extra.ViewOnce != nil || extra.GIF || extra.LinkPreview != nil || len(extra.Mentions) > 0 {
			msg.Extra = &extra
		}
		// Keep the serialized original proto for lossless forwarding.
		if raw, err := proto.Marshal(evt.Message); err == nil {
			msg.RawProto = raw
		}

		if h.httpServer != nil {
			h.httpServer.SaveAndBroadcastMessage(msg)
		} else {
			h.msgStore.SaveMessage(msg)
		}
		if !evt.Info.IsFromMe {
			h.unarchiveOnIncoming(chatID)
		}
	}
}

// savePreviewBytes persists a sender-supplied thumbnail (JPEG bytes) to local
// storage and returns its /media URL, or "" on failure.
func (h *WhatsAppEventHandler) savePreviewBytes(ctx context.Context, thumb []byte) string {
	filename := fmt.Sprintf("preview_%d.jpg", time.Now().UnixNano())
	if _, err := h.storage.Save(ctx, filename, bytes.NewReader(thumb)); err != nil {
		return ""
	}
	return fmt.Sprintf("/media/%s", filename)
}

// extractLinkPreview reads the link preview metadata a sender attached to an
// ExtendedTextMessage (title/description/thumbnail) and materializes the
// thumbnail into local storage.
func (h *WhatsAppEventHandler) extractLinkPreview(ctx context.Context, et *waProto.ExtendedTextMessage) *repository.LinkPreviewMeta {
	url := et.GetMatchedText()
	if url == "" {
		url = whatsappInfra.FirstURLInText(et.GetText())
	}
	if url == "" {
		return nil
	}
	meta := &repository.LinkPreviewMeta{
		URL:         url,
		Title:       et.GetTitle(),
		Description: et.GetDescription(),
	}
	if thumb := et.GetJPEGThumbnail(); len(thumb) > 0 {
		meta.ThumbnailURL = h.savePreviewBytes(ctx, thumb)
	}
	return meta
}

// handleReaction applies an incoming ReactionMessage (from anyone, including
// echoes of our own reactions sent from the phone) onto the stored message
// and broadcasts the updated reaction list.
func (h *WhatsAppEventHandler) handleReaction(evt *events.Message, rm *waProto.ReactionMessage) {
	if h.msgStore == nil {
		return
	}
	targetID := rm.GetKey().GetID()
	if targetID == "" {
		return
	}

	reactor := evt.Info.Sender.ToNonAD()
	if reactor.Server != waTypes.DefaultUserServer {
		if alt := evt.Info.SenderAlt.ToNonAD(); !alt.IsEmpty() && alt.Server == waTypes.DefaultUserServer {
			reactor = alt
		} else if resolved := h.msgStore.ResolveChatID(reactor.String()); resolved != reactor.String() {
			reactor, _ = waTypes.ParseJID(resolved)
		}
	}

	senderID := reactor.String()
	if evt.Info.IsFromMe {
		senderID = "me"
	}

	stored, err := h.msgStore.GetMessageByID(targetID)
	if err != nil {
		fmt.Printf("[REACTION] Target message %s not found: %v\n", targetID, err)
		return
	}

	reactions := repository.ApplyReaction(stored.Reactions, senderID, rm.GetText())
	if err := h.msgStore.UpdateMessageReactions(targetID, reactions); err != nil {
		fmt.Printf("[REACTION] Failed to persist: %v\n", err)
		return
	}

	if h.httpServer != nil {
		h.httpServer.BroadcastMessage("message_reaction", map[string]interface{}{
			"chatId":    h.msgStore.ResolveChatID(stored.ChatID),
			"id":        targetID,
			"reactions": reactions,
		})
	}
	fmt.Printf("[REACTION] %s -> %q on %s\n", senderID, rm.GetText(), targetID)
}

// handlePollUpdate decrypts an incoming poll vote, maps the selected option
// hashes back to names, and stores/broadcasts the updated tally.
func (h *WhatsAppEventHandler) handlePollUpdate(evt *events.Message) {
	if h.msgStore == nil {
		return
	}
	pu := evt.Message.GetPollUpdateMessage()
	pollID := pu.GetPollCreationMessageKey().GetID()
	if pollID == "" {
		return
	}

	stored, err := h.msgStore.GetMessageByID(pollID)
	if err != nil || stored.Extra == nil || stored.Extra.Poll == nil {
		fmt.Printf("[POLL] Vote for unknown poll %s: %v\n", pollID, err)
		return
	}

	vote, err := h.waClient.CoreClient().DecryptPollVote(context.Background(), evt)
	if err != nil {
		fmt.Printf("[POLL] Failed to decrypt vote: %v\n", err)
		return
	}

	optionNames := make([]string, len(stored.Extra.Poll.Options))
	for i, o := range stored.Extra.Poll.Options {
		optionNames[i] = o.Name
	}
	hashes := whatsmeow.HashPollOptions(optionNames)
	selected := make([]string, 0, len(vote.GetSelectedOptions()))
	for _, selHash := range vote.GetSelectedOptions() {
		for i, h2 := range hashes {
			if bytes.Equal(h2, selHash) {
				selected = append(selected, optionNames[i])
			}
		}
	}

	voter := evt.Info.Sender.ToNonAD().String()
	if evt.Info.IsFromMe {
		voter = "me"
	}
	if stored.Extra.Poll.Votes == nil {
		stored.Extra.Poll.Votes = map[string][]string{}
	}
	if len(selected) == 0 {
		delete(stored.Extra.Poll.Votes, voter)
	} else {
		stored.Extra.Poll.Votes[voter] = selected
	}

	if err := h.msgStore.UpdateMessageExtra(pollID, stored.Extra); err != nil {
		fmt.Printf("[POLL] Failed to persist vote: %v\n", err)
		return
	}
	if h.httpServer != nil {
		h.httpServer.BroadcastMessage("poll_update", map[string]interface{}{
			"chatId": h.msgStore.ResolveChatID(stored.ChatID),
			"id":     pollID,
			"extra":  stored.Extra,
		})
	}
	fmt.Printf("[POLL] %s voted %v on %s\n", voter, selected, pollID)
}

func (h *WhatsAppEventHandler) processCommand(ctx context.Context, evt *events.Message, senderJID waTypes.JID, messageText, chatID string) {
	msgType := "text"
	mediaURL := ""

	if evt.Message.GetStickerMessage() != nil {
		msgType = "sticker"
		mediaURL = "/media/stickers/" + evt.Info.ID + ".webp"
	} else if evt.Message.GetImageMessage() != nil {
		msgType = "image"
		mediaURL = "/media/images/" + evt.Info.ID + ".jpg"
	} else if evt.Message.GetVideoMessage() != nil {
		msgType = "video"
		mediaURL = "/media/videos/" + evt.Info.ID + ".mp4"
	} else if evt.Message.GetDocumentMessage() != nil {
		msgType = "document"
		mediaURL = "/media/documents/" + evt.Info.ID + "_" + evt.Message.GetDocumentMessage().GetFileName()
	} else if evt.Message.GetAudioMessage() != nil {
		if evt.Message.GetAudioMessage().GetPTT() {
			msgType = "ptt"
		} else {
			msgType = "audio"
		}
		mediaURL = "/media/audio/" + evt.Info.ID + ".ogg"
	}

	msg := &entity.Message{
		ID:        evt.Info.ID,
		ChatID:    chatID,
		Text:      messageText,
		Type:      msgType,
		MediaURL:  mediaURL,
		VMessage:  evt.Message,
		Timestamp: evt.Info.Timestamp,
		IsGroup:   evt.Info.IsGroup,
		SenderJID: senderJID.String(),
		SenderLID: evt.Info.Sender.ToNonAD().String(),
	}

	// Try to get actual LID if available
	if evt.Info.Sender.Server == waTypes.MessengerServer {
		msg.SenderLID = evt.Info.Sender.String()
	}

	// Check Lua Triggers
	if h.luaService != nil {
		if matched, err := h.luaService.ExecuteTriggers(ctx, msg); err == nil && matched {
			fmt.Printf("[LUA] Lua Trigger Matched for: %s\n", messageText)
			return
		}
	}

	// Unknown Command Fallback (Optional)
	if strings.HasPrefix(messageText, "!") && !evt.Info.IsGroup {
		// h.waClient.SendMessageToJID(ctx, senderJID, "Unknown Command. Please use triggers configured in Dashboard.", true)
	}
}

type HandlerUseCaseInterface interface {
	HandleCancel(senderJID string)
}

type WhatsAppService struct {
	waClient *whatsappInfra.WhatsAppClient
	config   repository.ConfigRepository
}

func NewWhatsAppService(waClient *whatsappInfra.WhatsAppClient, config repository.ConfigRepository) *WhatsAppService {
	return &WhatsAppService{
		waClient: waClient,
		config:   config,
	}
}

func (h *WhatsAppEventHandler) isFromAllowedGroups(vInfo *waTypes.MessageInfo) bool {
	adminGroups := strings.Split(os.Getenv("ADMIN_GROUPS_JID"), ",")
	groupJID := vInfo.Chat.String()
	for _, allowedGroup := range adminGroups {
		if strings.EqualFold(strings.TrimSpace(allowedGroup), groupJID) {
			return true
		}
	}
	return false
}

func getContextInfo(msg *waProto.Message) *waProto.ContextInfo {
	if msg == nil {
		return nil
	}
	if msg.ExtendedTextMessage != nil {
		return msg.ExtendedTextMessage.ContextInfo
	}
	if msg.ImageMessage != nil {
		return msg.ImageMessage.ContextInfo
	}
	if msg.VideoMessage != nil {
		return msg.VideoMessage.ContextInfo
	}
	if msg.AudioMessage != nil {
		return msg.AudioMessage.ContextInfo
	}
	if msg.DocumentMessage != nil {
		return msg.DocumentMessage.ContextInfo
	}
	if msg.StickerMessage != nil {
		return msg.StickerMessage.ContextInfo
	}
	if msg.ContactMessage != nil {
		return msg.ContactMessage.ContextInfo
	}
	if msg.ContactsArrayMessage != nil {
		return msg.ContactsArrayMessage.ContextInfo
	}
	if msg.ListMessage != nil {
		return msg.ListMessage.ContextInfo
	}
	if msg.ButtonsMessage != nil {
		return msg.ButtonsMessage.ContextInfo
	}
	if msg.TemplateMessage != nil {
		return msg.TemplateMessage.ContextInfo
	}
	return nil
}

// handleStatusMessage stores an incoming status (story) update: media is
// downloaded to local storage where possible, then a status_new event is
// broadcast.
func (h *WhatsAppEventHandler) handleStatusMessage(evt *events.Message) {
	if h.msgStore == nil {
		return
	}
	sender := evt.Info.Sender.ToNonAD()
	if sender.Server != waTypes.DefaultUserServer {
		if alt := evt.Info.SenderAlt.ToNonAD(); !alt.IsEmpty() && alt.Server == waTypes.DefaultUserServer {
			sender = alt
		}
	}

	entry := &repository.StatusEntry{
		ID:        evt.Info.ID,
		Sender:    sender.String(),
		Timestamp: evt.Info.Timestamp.UnixMilli(),
		ExpiresAt: evt.Info.Timestamp.Add(24 * time.Hour).UnixMilli(),
		Type:      "text",
	}

	msg := evt.Message
	// Statuses may be wrapped (view-once style); unwrap for rendering.
	if inner := msg.GetViewOnceMessageV2().GetMessage(); inner != nil {
		msg = inner
	}

	ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
	defer cancel()

	switch {
	case msg.GetExtendedTextMessage() != nil:
		entry.Content = msg.GetExtendedTextMessage().GetText()
	case msg.GetConversation() != "":
		entry.Content = msg.GetConversation()
	case msg.GetImageMessage() != nil:
		entry.Type = "image"
		entry.Content = msg.GetImageMessage().GetCaption()
		data, err := h.waClient.CoreClient().Download(ctx, msg.GetImageMessage())
		if err == nil && len(data) > 0 {
			filename := fmt.Sprintf("status_%d_%s.jpg", time.Now().UnixMilli(), strings.ReplaceAll(sender.String(), "@", "_"))
			if _, serr := h.storage.Save(ctx, filename, bytes.NewReader(data)); serr == nil {
				entry.MediaURL = "/media/" + filename
			}
		}
	case msg.GetVideoMessage() != nil:
		entry.Type = "video"
		entry.Content = msg.GetVideoMessage().GetCaption()
	case msg.GetAudioMessage() != nil:
		entry.Type = "audio"
	}
	if entry.Type == "text" && strings.TrimSpace(entry.Content) == "" {
		return // unsupported/empty status
	}

	if raw, err := proto.Marshal(evt.Message); err == nil {
		h.msgStore.SaveStatus(entry, raw)
	} else {
		h.msgStore.SaveStatus(entry, nil)
	}

	if h.httpServer != nil {
		h.httpServer.BroadcastMessage("status_new", map[string]interface{}{
			"sender": h.msgStore.ResolveChatID(entry.Sender),
			"id":     entry.ID,
		})
	}
	fmt.Printf("[STATUS] Stored %s from %s (type=%s)", entry.ID, entry.Sender, entry.Type)
}

// handleChannelMessage broadcasts an incoming channel post to clients.
func (h *WhatsAppEventHandler) handleChannelMessage(evt *events.Message) {
	if h.httpServer == nil {
		return
	}
	msg := whatsappInfra.ChannelMessageFromEvent(evt.Info.ID, evt.Info.Timestamp, evt.Message)
	h.httpServer.BroadcastMessage("channel_message", map[string]interface{}{
		"channelId": evt.Info.Chat.String(),
		"message":   msg,
	})
}

// handleChannelLiveUpdate forwards reaction/view count updates.
func (h *WhatsAppEventHandler) handleChannelLiveUpdate(evt *events.NewsletterLiveUpdate) {
	if h.httpServer == nil {
		return
	}
	msgs := make([]*whatsappInfra.ChannelMessage, 0, len(evt.Messages))
	for _, m := range evt.Messages {
		msgs = append(msgs, whatsappInfra.ChannelMessageFromRaw(m))
	}
	h.httpServer.BroadcastMessage("channel_update", map[string]interface{}{
		"channelId": evt.JID.String(),
		"messages":  msgs,
	})
}

// handleChatPresence broadcasts incoming typing/recording indicators.
func (h *WhatsAppEventHandler) handleChatPresence(evt *events.ChatPresence) {
	if h.httpServer == nil {
		return
	}
	chatID := evt.Chat.String()
	sender := ""
	if !evt.Sender.IsEmpty() {
		sender = evt.Sender.ToNonAD().String()
	}
	state := "paused"
	if evt.State == waTypes.ChatPresenceComposing {
		state = "composing"
	}
	media := "text"
	if evt.Media == waTypes.ChatPresenceMediaAudio {
		media = "audio"
	}
	if h.msgStore != nil {
		chatID = h.msgStore.ResolveChatID(chatID)
	}
	h.httpServer.BroadcastMessage("chat_presence", map[string]interface{}{
		"chatId": chatID,
		"sender": sender,
		"state":  state,
		"media":  media,
	})
}

// handlePresence broadcasts availability updates for subscribed contacts.
func (h *WhatsAppEventHandler) handlePresence(evt *events.Presence) {
	if h.httpServer == nil || h.msgStore == nil {
		return
	}
	jid := h.msgStore.ResolveChatID(evt.From.ToNonAD().String())
	var lastSeen int64
	if !evt.LastSeen.IsZero() {
		lastSeen = evt.LastSeen.UnixMilli()
	}
	h.httpServer.BroadcastMessage("presence", map[string]interface{}{
		"jid":       jid,
		"available": !evt.Unavailable,
		"lastSeen":  lastSeen,
	})
}

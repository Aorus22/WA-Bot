package whatsapp

import (
	"bytes"
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	waTypes "go.mau.fi/whatsmeow/types"
	"go.mau.fi/whatsmeow/types/events"
	waProto "go.mau.fi/whatsmeow/proto/waE2E"

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

type WhatsAppEventHandler struct {
	handlerUC  HandlerUseCaseInterface
	waService  *WhatsAppService
	stateRepo  repository.UserStateRepository
	waClient   *whatsappInfra.WhatsAppClient
	msgStore   *repository.MessageStore
	httpServer HTTPServer
	storage    repository.StorageRepository
	luaService LuaService
	aiClient   *ai.AIClient
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

func (h *WhatsAppEventHandler) HandleEvent(evt interface{}) {
	switch v := evt.(type) {
	case *events.Message:
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
	}
}

func (h *WhatsAppEventHandler) handleReceipt(evt *events.Receipt) {
	if h.httpServer == nil {
		return
	}

	status := "sent"
	if evt.Type == events.ReceiptTypeDelivered {
		status = "delivered"
	} else if evt.Type == events.ReceiptTypeRead || evt.Type == events.ReceiptTypeReadSelf {
		status = "read"
	} else {
		return
	}

	for _, msgID := range evt.MessageIDs {
		h.httpServer.UpdateMessageStatus(msgID, status)
		fmt.Printf("✅ Updated status for %s to %s\n", msgID, status)
	}
}

func (h *WhatsAppEventHandler) handleMessage(evt *events.Message) {
	// Ignore messages from self to prevent duplicate broadcasts (already handled by LogSentMessage)
	if evt.Info.IsFromMe {
		fmt.Printf("[DEBUG] Skipping self-message: %s (chat=%s)\n", evt.Info.ID, evt.Info.Chat.String())
		return
	}

	// Extract JIDs
	var senderJID waTypes.JID
	if evt.Info.IsGroup {
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
	if senderName == "" && h.msgStore != nil {
		if storedName, err := h.msgStore.GetContactName(senderJID.String()); err == nil && storedName != "" {
			senderName = storedName
		}
	}

	// Update sender info in background
	if h.msgStore != nil {
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

	// 0. Filter out empty messages that are not media
	if messageText == "" && !isMedia {
		fmt.Printf("[DEBUG] Skipping empty message: %s (chat=%s)\n", evt.Info.ID, evt.Info.Chat.String())
		return
	}

	// Determine what name to show in the Sidebar
	displayChatName := ""
	if !evt.Info.IsGroup {
		displayChatName = senderName
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
	if h.aiClient != nil && messageText != "" {
		go func() {
			ctx, cancel := context.WithTimeout(context.Background(), 12*time.Second)
			defer cancel()
			h.aiClient.Dispatch(ctx, chatID, senderJID.String(), senderName, messageText, evt.Info.ID, evt.Info.IsGroup)
		}()
	}

	// --- LOGIC BOT (RUN IN BACKGROUND WITH FILTERS) ---
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

		if evt.Message.GetImageMessage() != nil {
			msgType = "image"
			img := evt.Message.GetImageMessage()
			msg := &entity.Message{
				VMessage:  evt.Message,
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
		} else if evt.Message.GetStickerMessage() != nil {
			msgType = "sticker"
			msg := &entity.Message{
				VMessage:  evt.Message,
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
		} else if evt.Message.GetVideoMessage() != nil {
			msgType = "video"
			vid := evt.Message.GetVideoMessage()
			content = vid.GetCaption()
			if content == "" {
				content = "[Video]"
			}
		} else if evt.Message.GetDocumentMessage() != nil {
			msgType = "document"
			doc := evt.Message.GetDocumentMessage()
			msg := &entity.Message{
				VMessage:  evt.Message,
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
		} else if evt.Message.GetAudioMessage() != nil {
			audio := evt.Message.GetAudioMessage()
			if audio.GetPTT() {
				msgType = "ptt"
			} else {
				msgType = "audio"
			}
			msg := &entity.Message{
				VMessage:  evt.Message,
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
		} else {
			content = messageText
		}

		msg := &repository.Message{
			ID:          evt.Info.ID,
			ChatID:      chatID,
			From:        senderJID.String(),
			To:          "me",
			Content:     content,
			Timestamp:   evt.Info.Timestamp.UnixMilli(),
			Status:      "received",
			Type:        msgType,
			MediaURL:    mediaURL,
			IsAutomatic: false,
			SenderName:  senderName,
			ChatName:    chatName,
			ReplyToID: func() string {
				ctxInfo := getContextInfo(evt.Message)
				if ctxInfo != nil {
					return ctxInfo.GetStanzaID()
				}
				return ""
			}(),
		}

		if h.httpServer != nil {
			h.httpServer.SaveAndBroadcastMessage(msg)
		} else {
			h.msgStore.SaveMessage(msg)
		}
	}
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

package whatsapp

import (
	"bytes"
	"context"
	"fmt"
	"os"
	"regexp"
	"strings"
	"time"

	waTypes "go.mau.fi/whatsmeow/types"
	"go.mau.fi/whatsmeow/types/events"

	"wa-bot/internal/domain/entity"
	"wa-bot/internal/domain/repository"
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

func (h *WhatsAppEventHandler) HandleEvent(evt interface{}) {
	switch v := evt.(type) {
	case *events.Message:
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
	// Extract JIDs
	var senderJID waTypes.JID
	if evt.Info.IsGroup {
		senderJID = evt.Info.Sender.ToNonAD()
	} else {
		senderJID = evt.Info.Chat.ToNonAD()
	}

	chatID := evt.Info.Chat.String()
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
	if evt.Message.ExtendedTextMessage != nil && evt.Message.ExtendedTextMessage.Text != nil {
		messageText = *evt.Message.ExtendedTextMessage.Text
	} else if evt.Message.ImageMessage != nil && evt.Message.ImageMessage.Caption != nil {
		messageText = *evt.Message.ImageMessage.Caption
	} else if evt.Message.VideoMessage != nil && evt.Message.VideoMessage.Caption != nil {
		messageText = *evt.Message.VideoMessage.Caption
	} else if evt.Message.DocumentMessage != nil && evt.Message.DocumentMessage.Caption != nil {
		messageText = *evt.Message.DocumentMessage.Caption
	} else {
		messageText = evt.Message.GetConversation()
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
	// If it's a group, displayChatName is "" so SaveMessage won't overwrite existing group name
	h.showMessage(evt, senderJID, messageText, chatID, displayChatName, senderName)
	fmt.Printf("[MSG] Logged message: [%s] from=%s name=%s\n", chatID, senderJID.String(), senderName)

	// 2. Update group name/avatar in background if it's a group
	if evt.Info.IsGroup {
		go func() {
			groupInfo, err := h.waClient.GetGroupInfo(context.Background(), chatID)
			if err == nil && groupInfo != nil {
				// Update Group Name and Avatar
				avatarURL, _ := h.waClient.GetProfilePictureInfo(context.Background(), chatID)
				if h.msgStore != nil {
					if groupInfo.Name != "" {
						h.msgStore.UpdateChatName(chatID, groupInfo.Name)
					}
					if avatarURL != "" {
						h.msgStore.UpdateChatAvatar(chatID, avatarURL)
					}

					// Broadcast updates to FE
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
		// Update private chat avatar
		go func() {
			avatarURL, err := h.waClient.GetProfilePictureInfo(context.Background(), chatID)
			if err == nil && avatarURL != "" && h.msgStore != nil {
				h.msgStore.UpdateChatAvatar(chatID, avatarURL)
			}
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
			fmt.Printf("⚠️ Message too old (%v), skipping bot response\n", now.Sub(msgTime))
			return
		}

		ctx := context.Background()
		quickRole := h.getQuickRole(senderJID.String())
		h.processCommand(ctx, evt, senderJID, messageText, chatID, quickRole)

		accurateRole := h.getUserRole(senderJID.String())
		if accurateRole != quickRole {
			fmt.Printf("[ROLE] Role updated: %s -> %s\n", quickRole, accurateRole)
		}
	}()
}

func (h *WhatsAppEventHandler) showMessage(evt *events.Message, senderJID waTypes.JID, messageText, chatID, chatName, senderName string) {
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
			content = doc.GetTitle()
			if content == "" {
				content = "[Document]"
			}
		} else {
			content = messageText
		}

		msg := &repository.Message{
			ID:          fmt.Sprintf("msg_%d_%s", time.Now().UnixMilli(), senderJID.String()),
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
		}

		if h.httpServer != nil {
			h.httpServer.SaveAndBroadcastMessage(msg)
		} else {
			h.msgStore.SaveMessage(msg)
		}
	}
}

func (h *WhatsAppEventHandler) getQuickRole(senderJID string) string {
	owner := os.Getenv("OWNER_JID")
	if owner != "" && strings.EqualFold(senderJID, owner) {
		return "OWNER"
	}
	return "COMMON"
}

func (h *WhatsAppEventHandler) processCommand(ctx context.Context, evt *events.Message, senderJID waTypes.JID, messageText, chatID, role string) {
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
	}

	// Check Lua Triggers
	if h.luaService != nil {
		if matched, err := h.luaService.ExecuteTriggers(ctx, msg); err == nil && matched {
			fmt.Printf("[LUA] Lua Trigger Matched for: %s\n", messageText)
			return
		}
	}

	// Check user state
	userState, err := h.stateRepo.GetUserState(senderJID.String())
	if err == nil && userState != "" {
		if strings.HasPrefix(messageText, "!cancel") {
			h.handlerUC.HandleCancel(senderJID.String())
			return
		} else if strings.HasPrefix(messageText, "!") {
			h.waClient.SendMessageToJID(ctx, senderJID, "There is another process, !cancel to cancel it", true)
			return
		}
	}

	// Command patterns
	pdfRegex := regexp.MustCompile(`^!pdf\s+\S+$`)
	answerPdfRegex := regexp.MustCompile(`^!answer(\s+\S+)*$`)
	geminiRegex := regexp.MustCompile(`^!gemini(\s+\S+)*$`)
	args := map[string]interface{}{
		"senderJID":    senderJID,
		"groupName":    "",
		"isFromGroup":  evt.Info.IsGroup,
		"userRole":     role,
		"userState":    userState,
		"rawSenderJID": senderJID,
	}

	// Command routing
	switch {
	case messageText == "!check":
		h.handlerUC.HandleCheck(ctx, senderJID, args)
	case messageText == "!listgroups":
		h.handlerUC.HandleListGroups(ctx, senderJID, role)
	case messageText == "!token":
		h.handlerUC.HandleToken(ctx, senderJID, role, evt.Info.IsGroup)
	case messageText == "!listmapel":
		h.handlerUC.HandleListMapel(ctx, senderJID, role)
	case messageText == "!listmember":
		h.handlerUC.HandleListMember(ctx, senderJID, role)
	case pdfRegex.MatchString(messageText):
		h.handlerUC.HandlePDF(ctx, senderJID, messageText, role, msg)
	case answerPdfRegex.MatchString(messageText):
		h.handlerUC.HandlePDF(ctx, senderJID, messageText, role, msg)
	case geminiRegex.MatchString(messageText):
		h.handlerUC.HandlePDF(ctx, senderJID, messageText, role, msg)
	case messageText == "!help":
		h.handlerUC.HandleHelp(ctx, senderJID, role, args)
	default:
		if userState == "PendingToken" {
			h.handlerUC.HandlePendingToken(ctx, senderJID, messageText)
			return
		}

		if strings.HasPrefix(messageText, "!") {
			h.waClient.SendMessageToJID(ctx, senderJID, "Invalid Command", true)
			return
		}

		if !evt.Info.IsGroup {
			if role == "COMMON" || role == "UNKNOWN" {
				h.waClient.SendMessageToJID(ctx, senderJID, "!help to see the command list", true)
			} else if role == "USER" {
				h.waClient.SendMessageToJID(ctx, senderJID, "!help untuk melihat list command", true)
			}
		}
	}
}

func (h *WhatsAppEventHandler) getUserRole(senderJID string) string {
	owner := os.Getenv("OWNER_JID")
	adminGroups := strings.Split(os.Getenv("ADMIN_GROUPS_JID"), ",")
	userGroups := strings.Split(os.Getenv("USER_GROUPS_JID"), ",")

	if owner != "" && strings.EqualFold(senderJID, owner) {
		return "OWNER"
	}

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	var senderLID string
	userInfo, err := h.waClient.GetUserInfo(ctx, senderJID)
	if err == nil && userInfo != nil {
		senderLID = userInfo.LID
	}

	for _, adminGroup := range adminGroups {
		adminGroup = strings.TrimSpace(adminGroup)
		if adminGroup == "" {
			continue
		}
		gCtx, gCancel := context.WithTimeout(context.Background(), 3*time.Second)
		groupInfo, err := h.waClient.GetGroupInfo(gCtx, adminGroup)
		gCancel()
		if err != nil {
			continue
		}
		for _, participant := range groupInfo.Participants {
			if strings.Contains(participant.LID, "@lid") && senderLID != "" {
				pLID, _ := waTypes.ParseJID(participant.LID)
				sLID, _ := waTypes.ParseJID(senderLID)
				if pLID.User == sLID.User {
					return "ADMIN"
				}
			}
			if strings.EqualFold(participant.JID, senderJID) {
				return "ADMIN"
			}
		}
	}

	for _, userGroup := range userGroups {
		userGroup = strings.TrimSpace(userGroup)
		if userGroup == "" {
			continue
		}
		gCtx, gCancel := context.WithTimeout(context.Background(), 3*time.Second)
		groupInfo, err := h.waClient.GetGroupInfo(gCtx, userGroup)
		gCancel()
		if err != nil {
			continue
		}
		for _, participant := range groupInfo.Participants {
			if strings.Contains(participant.LID, "@lid") && senderLID != "" {
				pLID, _ := waTypes.ParseJID(participant.LID)
				sLID, _ := waTypes.ParseJID(senderLID)
				if pLID.User == sLID.User {
					return "USER"
				}
			}
			if strings.EqualFold(participant.JID, senderJID) {
				return "USER"
			}
		}
	}

	return "COMMON"
}

type CommandRouter struct {
	handlerUC HandlerUseCaseInterface
}

func NewCommandRouter(handlerUC HandlerUseCaseInterface) *CommandRouter {
	return &CommandRouter{
		handlerUC: handlerUC,
	}
}

type HandlerUseCaseInterface interface {
	HandleCheck(ctx interface{}, senderJID waTypes.JID, args map[string]interface{})
	HandleListGroups(ctx interface{}, senderJID waTypes.JID, role string)
	HandleToken(ctx interface{}, senderJID waTypes.JID, role string, isFromGroup bool)
	HandleListMapel(ctx interface{}, senderJID waTypes.JID, role string)
	HandleListMember(ctx interface{}, senderJID waTypes.JID, role string)
	HandlePDF(ctx interface{}, senderJID waTypes.JID, messageText string, role string, msg *entity.Message)
	HandleHelp(ctx interface{}, senderJID waTypes.JID, role string, args map[string]interface{})
	HandleCancel(senderJID string)
	HandlePendingToken(ctx interface{}, senderJID waTypes.JID, messageText string)
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

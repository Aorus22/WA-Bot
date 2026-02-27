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
	ExecuteTriggers(ctx context.Context, senderJID string, messageText string) (bool, error)
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
		// We only care about delivered and read status
		return
	}

	for _, msgID := range evt.MessageIDs {
		h.httpServer.UpdateMessageStatus(msgID, status)
		fmt.Printf("\u2705 Updated status for %s to %s\n", msgID, status)
	}
}

func (h *WhatsAppEventHandler) handleMessage(evt *events.Message) {
	// --- LOGIC PENYIMPANAN (ALWAYS RUN FIRST) ---

	// Extract sender JID
	var senderJID waTypes.JID
	if evt.Info.IsGroup {
		senderJID = evt.Info.Chat.ToNonAD()
	} else {
		senderJID = evt.Info.Sender.ToNonAD()
	}

	// Extract message text
	var messageText string
	if evt.Message.ExtendedTextMessage != nil && evt.Message.ExtendedTextMessage.Text != nil {
		messageText = *evt.Message.ExtendedTextMessage.Text
	} else if evt.Message.ImageMessage != nil && evt.Message.ImageMessage.Caption != nil {
		messageText = *evt.Message.ImageMessage.Caption
	} else if evt.Message.VideoMessage != nil && evt.Message.VideoMessage.Caption != nil {
		messageText = *evt.Message.VideoMessage.Caption
	} else {
		messageText = evt.Message.GetConversation()
	}

	chatID := evt.Info.Chat.String()
	senderName := evt.Info.PushName

	// Save to DB and broadcast to FE immediately (NO FILTERS)
	h.showMessage(evt, senderJID, messageText, chatID, senderName)
	fmt.Printf("📩 Logged message: [%s] from=%s text=%s\n", chatID, senderJID.String(), messageText)

	// --- LOGIC BOT (RUN IN BACKGROUND WITH FILTERS) ---
	go func() {
		defer func() {
			if r := recover(); r != nil {
				fmt.Printf("🔥 Panic in bot logic: %v\n", r)
			}
		}()

		// Apply filters ONLY for bot responses
		// 1. Skip blocked sender
		if senderJID.UserInt() == 13135550002 {
			return
		}

		// 2. Check message age
		msgTime := evt.Info.Timestamp
		now := time.Now()
		if now.Sub(msgTime) > 1*time.Minute {
			fmt.Printf("\u26a0 Message too old (%v), skipping bot response\n", now.Sub(msgTime))
			return
		}

		// 3. Process the command
		ctx := context.Background()
		quickRole := h.getQuickRole(senderJID.String())
		h.processCommand(ctx, evt, senderJID, messageText, chatID, quickRole)

		// 4. Update role in background (accurate but slow)
		accurateRole := h.getUserRole(senderJID.String())
		if accurateRole != quickRole {
			fmt.Printf("📄 Role updated: %s -> %s\n", quickRole, accurateRole)
		}
	}()
}

func (h *WhatsAppEventHandler) showMessage(evt *events.Message, senderJID waTypes.JID, messageText, chatID, senderName string) {
	ctx := context.Background()

	// Save/update contact info with avatar
	go func() {
		if avatarURL, err := h.waClient.GetProfilePictureInfo(ctx, senderJID.String()); err == nil {
			if h.msgStore != nil {
				h.msgStore.UpdateChatAvatar(chatID, avatarURL)
			}
		}
	}()

	// Save to database
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
	msg := &entity.Message{
		VMessage:  evt.Message,
		Timestamp: evt.Info.Timestamp,
		IsGroup:   evt.Info.IsGroup,
		SenderJID: senderJID.String(),
	}

	// Check Lua Triggers
	if h.luaService != nil {
		if matched, err := h.luaService.ExecuteTriggers(ctx, senderJID.String(), messageText); err == nil && matched {
			fmt.Printf("🔮 Lua Trigger Matched for: %s\n", messageText)
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
	stickerRegex := regexp.MustCompile(`^!sticker(\s+\S+)*$`)
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
	case stickerRegex.MatchString(messageText):
		h.handlerUC.HandleSticker(ctx, senderJID, messageText, role, msg)
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

		// Show help for non-commands if they are in DMs (common WA bot behavior)
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
	HandleSticker(ctx interface{}, senderJID waTypes.JID, messageText string, role string, msg *entity.Message)
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

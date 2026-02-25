package whatsapp

import (
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
	"wa-bot/internal/infrastructure/whatsapp"
)

type WhatsAppEventHandler struct {
	handlerUC HandlerUseCaseInterface
	waService *WhatsAppService
	stateRepo repository.UserStateRepository
	waClient  *whatsapp.WhatsAppClient
}

func NewWhatsAppEventHandler(handlerUC HandlerUseCaseInterface, waService *WhatsAppService, stateRepo repository.UserStateRepository, waClient *whatsapp.WhatsAppClient) *WhatsAppEventHandler {
	return &WhatsAppEventHandler{
		handlerUC: handlerUC,
		waService: waService,
		stateRepo: stateRepo,
		waClient:  waClient,
	}
}

func (h *WhatsAppEventHandler) HandleEvent(evt interface{}) {
	switch v := evt.(type) {
	case *events.Message:
		h.handleMessage(v)
	}
}

func (h *WhatsAppEventHandler) handleMessage(evt *events.Message) {
	ctx := context.Background()

	if evt.Info.IsGroup {
		allowed := h.isFromAllowedGroups(&evt.Info)
		if !allowed {
			return
		}
	}

	msgTime := evt.Info.Timestamp
	now := time.Now()
	if now.Sub(msgTime).Seconds() > 10 {
		return
	}

	var senderJID waTypes.JID
	if evt.Info.IsGroup {
		senderJID = evt.Info.Chat.ToNonAD()
	} else {
		senderJID = evt.Info.Sender.ToNonAD()
	}

	if senderJID.UserInt() == 13135550002 {
		return
	}

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

	msg := &entity.Message{
		VMessage:  evt.Message,
		Timestamp: evt.Info.Timestamp,
		IsGroup:   evt.Info.IsGroup,
		SenderJID: senderJID.String(),
	}

	role := h.getUserRole(senderJID.String())
	groupName := ""
	if evt.Info.IsGroup {
		groupInfo, err := h.waClient.GetGroupInfo(ctx, evt.Info.Chat.String())
		if err == nil {
			groupName = groupInfo.Name
		}
	}

	fmt.Printf("%s [%s] %d => %s\n",
		func() string {
			if msg.IsGroup {
				return "[Group]"
			}
			return ""
		}(),
		role,
		senderJID.UserInt(),
		messageText,
	)

	userState, err := h.stateRepo.GetUserState(senderJID.String())
	if err == nil && userState != "" {
		if strings.HasPrefix(messageText, "!cancel") {
			h.handlerUC.HandleCancel(senderJID.String())
			return
		} else if strings.HasPrefix(messageText, "!") {
			h.waClient.SendMessageToJID(ctx, senderJID, "There is another process, !cancel to cancel it")
			return
		}
	}

	stickerRegex := regexp.MustCompile(`^!sticker(\s+\S+)*$`)
	pdfRegex := regexp.MustCompile(`^!pdf\s+\S+$`)
	answerPdfRegex := regexp.MustCompile(`^!answer(\s+\S+)*$`)
	geminiRegex := regexp.MustCompile(`^!gemini(\s+\S+)*$`)

	args := map[string]interface{}{
		"senderJID":    senderJID,
		"groupName":    groupName,
		"isFromGroup":  evt.Info.IsGroup,
		"userRole":     role,
		"userState":    userState,
		"rawSenderJID": senderJID,
	}

	switch {
	case messageText == "!check":
		fmt.Printf("DEBUG: Handling !check\n")
		h.handlerUC.HandleCheck(ctx, senderJID, args)

	case messageText == "!listgroups":
		fmt.Printf("DEBUG: Handling !listgroups\n")
		h.handlerUC.HandleListGroups(ctx, senderJID, role)

	case messageText == "!token":
		fmt.Printf("DEBUG: Handling !token\n")
		h.handlerUC.HandleToken(ctx, senderJID, role, evt.Info.IsGroup)

	case messageText == "!listmapel":
		fmt.Printf("DEBUG: Handling !listmapel\n")
		h.handlerUC.HandleListMapel(ctx, senderJID, role)

	case messageText == "!listmember":
		fmt.Printf("DEBUG: Handling !listmember\n")
		h.handlerUC.HandleListMember(ctx, senderJID, role)

	case pdfRegex.MatchString(messageText):
		fmt.Printf("DEBUG: Handling !pdf command\n")
		h.handlerUC.HandlePDF(ctx, senderJID, messageText, role, msg)

	case answerPdfRegex.MatchString(messageText):
		fmt.Printf("DEBUG: Handling !answer command\n")
		h.handlerUC.HandlePDF(ctx, senderJID, messageText, role, msg)

	case geminiRegex.MatchString(messageText):
		fmt.Printf("DEBUG: Handling !gemini command\n")
		h.handlerUC.HandlePDF(ctx, senderJID, messageText, role, msg)

	case stickerRegex.MatchString(messageText):
		fmt.Printf("DEBUG: Handling sticker command\n")
		h.handlerUC.HandleSticker(ctx, senderJID, messageText, role, msg)

	case messageText == "!help":
		fmt.Printf("DEBUG: Handling !help\n")
		h.handlerUC.HandleHelp(ctx, senderJID, role, args)

	default:
		fmt.Printf("DEBUG: No matching command, userState: %s\n", userState)
		if userState == "PendingToken" {
			h.handlerUC.HandlePendingToken(ctx, senderJID, messageText)
			return
		}

		if strings.HasPrefix(messageText, "!") {
			fmt.Printf("DEBUG: Invalid command: %s\n", messageText)
			h.waClient.SendMessageToJID(ctx, senderJID, "Invalid Command")
			return
		}

		if role == "COMMON" {
			h.waClient.SendMessageToJID(ctx, senderJID, "!help to see the command list")
		} else if role == "USER" {
			h.waClient.SendMessageToJID(ctx, senderJID, "!help untuk melihat list command")
		}
	}
}

func (h *WhatsAppEventHandler) getUserRole(senderJID string) string {
	owner := os.Getenv("OWNER_JID")
	adminGroups := strings.Split(os.Getenv("ADMIN_GROUPS_JID"), ",")
	userGroups := strings.Split(os.Getenv("USER_GROUPS_JID"), ",")

	// Check if sender is owner
	if strings.EqualFold(senderJID, owner) {
		return "OWNER"
	}

	// Get sender's LID for comparison with group participants
	var senderLID string
	userInfo, err := h.waClient.GetUserInfo(context.Background(), senderJID)
	if err == nil && userInfo != nil {
		senderLID = userInfo.LID
	}

	// Check if sender is participant in any admin group
	for _, adminGroup := range adminGroups {
		adminGroup = strings.TrimSpace(adminGroup)
		if adminGroup == "" {
			continue
		}

		groupInfo, err := h.waClient.GetGroupInfo(context.Background(), adminGroup)
		if err != nil {
			continue
		}

		for _, participant := range groupInfo.Participants {
			// Handle LID format - compare with sender's LID
			if strings.Contains(participant.LID, "@lid") && senderLID != "" {
				participantLIDObj, err := waTypes.ParseJID(participant.LID)
				if err == nil && participantLIDObj.Server == "lid" {
					senderLIDObj, err := waTypes.ParseJID(senderLID)
					if err == nil && senderLIDObj.User == participantLIDObj.User {
						return "ADMIN"
					}
				}
				continue
			}

			// Normal phone number comparison
			if strings.EqualFold(participant.JID, senderJID) {
				return "ADMIN"
			}
		}
	}

	// Check if sender is participant in any user group
	for _, userGroup := range userGroups {
		userGroup = strings.TrimSpace(userGroup)
		if userGroup == "" {
			continue
		}

		groupInfo, err := h.waClient.GetGroupInfo(context.Background(), userGroup)
		if err != nil {
			continue
		}

		for _, participant := range groupInfo.Participants {
			// Handle LID format - compare with sender's LID
			if strings.Contains(participant.LID, "@lid") && senderLID != "" {
				participantLIDObj, err := waTypes.ParseJID(participant.LID)
				if err == nil && participantLIDObj.Server == "lid" {
					senderLIDObj, err := waTypes.ParseJID(senderLID)
					if err == nil && senderLIDObj.User == participantLIDObj.User {
						return "USER"
					}
				}
				continue
			}

			// Normal phone number comparison
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
	waClient *whatsapp.WhatsAppClient
	config   repository.ConfigRepository
}

func NewWhatsAppService(waClient *whatsapp.WhatsAppClient, config repository.ConfigRepository) *WhatsAppService {
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

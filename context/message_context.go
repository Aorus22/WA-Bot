package context

import (
	goctx "context"
	"fmt"
	"os"
	"strings"
	"time"
	"wa-bot/utils"

	"go.mau.fi/whatsmeow"
	waProto "go.mau.fi/whatsmeow/binary/proto"
	"go.mau.fi/whatsmeow/types"
	waTypes "go.mau.fi/whatsmeow/types"
	"google.golang.org/protobuf/proto"
)

type MessageContext struct {
	Client      *whatsmeow.Client
	VMessage    *waProto.Message
	SenderJID   waTypes.JID
	MessageText string
	IsFromGroup bool
	UserRole 	string
}

func NewMessageContext(client *whatsmeow.Client, vMessage *waProto.Message, senderJID waTypes.JID, messageText string, isFromGroup bool) *MessageContext {
	return &MessageContext{
		Client:      	client,
		VMessage:    	vMessage,
		SenderJID:   	senderJID,
		MessageText:	messageText,
		IsFromGroup: 	isFromGroup,
		UserRole:		getUserRole(client, isFromGroup, senderJID),
	}
}

func getUserRole(client *whatsmeow.Client, isFromGroup bool, senderJID waTypes.JID) string {
	owner := os.Getenv("OWNER_JID")
	if senderJID.String() == owner {
		return "OWNER"
	}

	adminGroups := strings.Split(os.Getenv("ADMIN_GROUPS_JID"), ",")
	userGroups := strings.Split(os.Getenv("USER_GROUPS_JID"), ",")

	if isFromGroup {
		if utils.Contains(adminGroups, senderJID.String()) {
			return "ADMIN"
		} else if utils.Contains(adminGroups, senderJID.String()) {
			return "USER"
		}
	}

	isAdmin := false
	for _, adminGroup := range adminGroups {
		targetGroupJID, err := waTypes.ParseJID(adminGroup)
		if err != nil {
			fmt.Println("Invalid group JID:", err)
			continue
		}

		groupInfo, err := client.GetGroupInfo(targetGroupJID)
		if err != nil {
			fmt.Println("Failed to get group info for", adminGroup, ":", err)
			continue
		}

		for _, participant := range groupInfo.Participants {
			if participant.JID.String() == senderJID.String() {
				isAdmin = true
				break
			}
		}

		if isAdmin {
			return "ADMIN"
		}
	}

	isUser := false
	for _, userGroup := range userGroups {
		targetGroupJID, err := waTypes.ParseJID(userGroup)
		if err != nil {
			fmt.Println("Invalid group JID:", err)
			continue
		}

		groupInfo, err :=  client.GetGroupInfo(targetGroupJID)
		if err != nil {
			fmt.Println("Failed to get group info for", userGroup, ":", err)
			continue
		}

		for _, participant := range groupInfo.Participants {
			if participant.JID.String() == senderJID.String() {
				isUser = true
				break
			}
		}

		if isUser {
			return "USER"
		}
	}

	return "COMMON"
}

func FromAllowedGroups(vInfo *types.MessageInfo) bool {
	adminGroups := strings.Split(os.Getenv("ADMIN_GROUPS_JID"), ",")
	groupJID := vInfo.Chat.String()

	return utils.Contains(adminGroups, groupJID)
}

func (ctx *MessageContext) Reply(text string) {
	ctx.Client.SendMessage(goctx.Background(), ctx.SenderJID, &waProto.Message{
		Conversation: proto.String(text),
	})
}

func (ctx *MessageContext) UploadToWhatsapp(filedata []byte, dataType string) (*whatsmeow.UploadResponse, error) {
	var mediaType whatsmeow.MediaType
	switch dataType {
	case "image":
		mediaType = whatsmeow.MediaImage
	default:
		mediaType = whatsmeow.MediaDocument
	}

	uploaded, err := ctx.Client.Upload(goctx.Background(), filedata, mediaType)
	return &uploaded, err
}

func (ctx *MessageContext) SendDocumentMessage(uploadedData *whatsmeow.UploadResponse, documentTitle string) (error) {
	_, err := ctx.Client.SendMessage(goctx.Background(), ctx.SenderJID, &waProto.Message{
		DocumentMessage: &waProto.DocumentMessage{
			Title:        proto.String(documentTitle),
			Mimetype:     proto.String("application/pdf"),
			URL:          proto.String(uploadedData.URL),
			DirectPath:   proto.String(uploadedData.DirectPath),
			MediaKey:     uploadedData.MediaKey,
			FileEncSHA256: uploadedData.FileEncSHA256,
			FileSHA256:   uploadedData.FileSHA256,
			FileLength:   proto.Uint64(uploadedData.FileLength),
		},
	})
	return err
}

func (ctx *MessageContext) GetDownloadableMedia(isVideo bool) ([]byte, error) {
	var downloadableMedia whatsmeow.DownloadableMessage
	if isVideo {
		downloadableMedia = ctx.VMessage.GetVideoMessage()
	} else {
		downloadableMedia = ctx.VMessage.GetImageMessage()
	}

	if downloadableMedia == nil {
		return nil, fmt.Errorf("no downloadable media found")
	}

	data, err := ctx.Client.Download(downloadableMedia)
	if err != nil {
		return nil, fmt.Errorf("download failed: %w", err)
	}

	return data, nil
}

func (ctx *MessageContext) SendStickerMessage(uploadedData *whatsmeow.UploadResponse, isAnimated bool) (error) {
	_, err := ctx.Client.SendMessage(goctx.Background(), ctx.SenderJID, &waProto.Message{
		StickerMessage: &waProto.StickerMessage{
			Mimetype:      proto.String("image/webp"),
			URL:           proto.String(uploadedData.URL),
			DirectPath:    proto.String(uploadedData.DirectPath),
			MediaKey:      uploadedData.MediaKey,
			FileEncSHA256: uploadedData.FileEncSHA256,
			FileSHA256:    uploadedData.FileSHA256,
			FileLength:    proto.Uint64(uploadedData.FileLength),
			IsAnimated:    proto.Bool(isAnimated),
		},
	})

	return err
}

func (ctx *MessageContext) AddUserToState(status string, cancel func()) {
	UserState.AddUser(ctx.SenderJID.String(), status, cancel)
}

func (ctx *MessageContext) ClearUserState() {
	UserState.ClearUser(ctx.SenderJID.String())
}

func (ctx *MessageContext) CheckUserState() string {
	data, exists := UserState.GetUserStatus(ctx.SenderJID.String())
	if !exists{
		return ""
	}
	return data.Status
}

func (ctx *MessageContext) GetUserPendingStartTime() (time.Time, error) {
	data, exists := UserState.GetUserStatus(ctx.SenderJID.String())
	if !exists {
		return data.StartTime, fmt.Errorf("user not found in state")
	}

	return data.StartTime, nil
}

func (ctx *MessageContext) UpdateUserProcess(cancel func()) {
	UserState.UpdateProcessContext(ctx.SenderJID.String(), cancel)
}

func (ctx *MessageContext) CancelCurrentProcess() error {
	return UserState.CancelUser(ctx.SenderJID.String())
}
package state

import (
	context "context"
	"errors"
	"fmt"
	"time"
	"wa-bot/utils"

	"go.mau.fi/whatsmeow"
	waProto "go.mau.fi/whatsmeow/proto/waE2E"
	waTypes "go.mau.fi/whatsmeow/types"
	"google.golang.org/protobuf/proto"
)

type MessageState struct {
	Client      *whatsmeow.Client
	VMessage    *waProto.Message
	SenderJID   waTypes.JID
	MessageText string
	IsFromGroup bool
	UserRole 	string
}

func NewMessageContext(client *whatsmeow.Client, vMessage *waProto.Message, senderJID waTypes.JID, messageText string, isFromGroup bool) *MessageState {
	return &MessageState{
		Client:      	client,
		VMessage:    	vMessage,
		SenderJID:   	senderJID,
		MessageText:	messageText,
		IsFromGroup: 	isFromGroup,
		UserRole:		utils.AssignRole(client, isFromGroup, senderJID),
	}
}

func (s *MessageState) Reply(text string) {
	s.Client.SendMessage(context.Background(), s.SenderJID, &waProto.Message{
		Conversation: proto.String(text),
	})
}

func (s *MessageState) UploadToWhatsapp(ctx context.Context, filedata []byte, dataType string) (*whatsmeow.UploadResponse, error) {
	var mediaType whatsmeow.MediaType
	switch dataType {
	case "image":
		mediaType = whatsmeow.MediaImage
	default:
		mediaType = whatsmeow.MediaDocument
	}

	uploaded, err := s.Client.Upload(ctx, filedata, mediaType)
	return &uploaded, err
}

func (s *MessageState) SendDocumentMessage(ctx context.Context, uploadedData *whatsmeow.UploadResponse, documentTitle string) (error) {
	_, err := s.Client.SendMessage(ctx, s.SenderJID, &waProto.Message{
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

func (s *MessageState) GetDownloadableMedia(isVideo bool) ([]byte, error) {
	var downloadableMedia whatsmeow.DownloadableMessage
	if isVideo {
		downloadableMedia = s.VMessage.GetVideoMessage()
	} else {
		downloadableMedia = s.VMessage.GetImageMessage()
	}

	if downloadableMedia == nil {
		return nil, fmt.Errorf("no downloadable media found")
	}

	data, err := s.Client.Download(downloadableMedia)
	if err != nil {
		return nil, fmt.Errorf("download failed: %w", err)
	}

	return data, nil
}

func (s *MessageState) SendStickerMessage(ctx context.Context, uploadedData *whatsmeow.UploadResponse, isAnimated bool) (error) {
	_, err := s.Client.SendMessage(ctx, s.SenderJID, &waProto.Message{
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

func (s *MessageState) ReplyNoCancelError(ctx context.Context, err error, msg string) bool {
    if err != nil {
        if !errors.Is(err, context.Canceled) && !errors.Is(err, context.DeadlineExceeded) {
            s.Reply(msg)
        }
        return true
    }
    return false
}

func (s *MessageState) AddUserToState(status string, cancel func()) {
	UserState.AddUser(s.SenderJID.String(), status, cancel)
}

func (s *MessageState) ClearUserState() {
	UserState.ClearUser(s.SenderJID.String())
}

func (s *MessageState) CheckUserState() string {
	data, exists := UserState.GetUserStatus(s.SenderJID.String())
	if !exists{
		return ""
	}
	return data.Status
}

func (s *MessageState) GetUserPendingStartTime() (time.Time, error) {
	data, exists := UserState.GetUserStatus(s.SenderJID.String())
	if !exists {
		return data.StartTime, fmt.Errorf("user not found in state")
	}

	return data.StartTime, nil
}

func (s *MessageState) UpdateUserProcess(cancel func()) {
	UserState.UpdateProcessContext(s.SenderJID.String(), cancel)
}

func (s *MessageState) CancelCurrentProcess() error {
	return UserState.CancelUser(s.SenderJID.String())
}
package whatsapp

import (
	"context"
	"fmt"

	"go.mau.fi/whatsmeow"
	waProto "go.mau.fi/whatsmeow/proto/waE2E"
	"go.mau.fi/whatsmeow/store/sqlstore"
	waTypes "go.mau.fi/whatsmeow/types"
	waLog "go.mau.fi/whatsmeow/util/log"
	"google.golang.org/protobuf/proto"

	"wa-bot/internal/domain/entity"
)

type WhatsAppClient struct {
	client    *whatsmeow.Client
	logLevel  string
	dbURL     string
	dbLog     waLog.Logger
	container *sqlstore.Container
}

type QREvent struct {
	Event string
	Code  string
}

func NewWhatsAppClient(dbURL, logLevel string, dbLog waLog.Logger) (*WhatsAppClient, error) {
	container, err := sqlstore.New(context.Background(), "sqlite3", dbURL, dbLog)
	if err != nil {
		return nil, err
	}

	deviceStore, err := container.GetFirstDevice(context.Background())
	if err != nil {
		return nil, err
	}

	client := whatsmeow.NewClient(deviceStore, dbLog)

	return &WhatsAppClient{
		client:    client,
		logLevel:  logLevel,
		dbURL:     dbURL,
		dbLog:     dbLog,
		container: container,
	}, nil
}

func (w *WhatsAppClient) SendMessage(ctx context.Context, to string, text string) error {
	targetJID := waTypes.NewJID(to, "")
	_, err := w.client.SendMessage(ctx, targetJID, &waProto.Message{
		Conversation: proto.String(text),
	})
	return err
}

func (w *WhatsAppClient) SendMessageToJID(ctx context.Context, to waTypes.JID, text string) error {
	_, err := w.client.SendMessage(ctx, to, &waProto.Message{
		Conversation: proto.String(text),
	})
	return err
}

func (w *WhatsAppClient) SendDocument(ctx context.Context, to string, data []byte, title string) error {
	var mediaType whatsmeow.MediaType
	mediaType = whatsmeow.MediaDocument

	uploaded, err := w.client.Upload(ctx, data, mediaType)
	if err != nil {
		return err
	}

	targetJID := waTypes.NewJID(to, waTypes.DefaultUserServer)
	_, err = w.client.SendMessage(ctx, targetJID, &waProto.Message{
		DocumentMessage: &waProto.DocumentMessage{
			Title:         proto.String(title),
			Mimetype:      proto.String("application/pdf"),
			URL:           proto.String(uploaded.URL),
			DirectPath:    proto.String(uploaded.DirectPath),
			MediaKey:      uploaded.MediaKey,
			FileEncSHA256: uploaded.FileEncSHA256,
			FileSHA256:    uploaded.FileSHA256,
			FileLength:    proto.Uint64(uploaded.FileLength),
			FileName:      proto.String(title),
		},
	})
	return err
}

func (w *WhatsAppClient) SendSticker(ctx context.Context, to string, data []byte, isAnimated bool) error {
	var mediaType whatsmeow.MediaType
	mediaType = whatsmeow.MediaImage

	uploaded, err := w.client.Upload(ctx, data, mediaType)
	if err != nil {
		return err
	}

	targetJID := waTypes.NewJID(to, waTypes.DefaultUserServer)
	_, err = w.client.SendMessage(ctx, targetJID, &waProto.Message{
		StickerMessage: &waProto.StickerMessage{
			Mimetype:      proto.String("image/webp"),
			URL:           proto.String(uploaded.URL),
			DirectPath:    proto.String(uploaded.DirectPath),
			MediaKey:      uploaded.MediaKey,
			FileEncSHA256: uploaded.FileEncSHA256,
			FileSHA256:    uploaded.FileSHA256,
			FileLength:    proto.Uint64(uploaded.FileLength),
			IsAnimated:    proto.Bool(isAnimated),
		},
	})
	return err
}

func (w *WhatsAppClient) SendDocumentToJID(ctx context.Context, to waTypes.JID, data []byte, title string) error {
	var mediaType whatsmeow.MediaType
	mediaType = whatsmeow.MediaDocument

	uploaded, err := w.client.Upload(ctx, data, mediaType)
	if err != nil {
		return err
	}

	_, err = w.client.SendMessage(ctx, to, &waProto.Message{
		DocumentMessage: &waProto.DocumentMessage{
			Title:         proto.String(title),
			Mimetype:      proto.String("application/pdf"),
			URL:           proto.String(uploaded.URL),
			DirectPath:    proto.String(uploaded.DirectPath),
			MediaKey:      uploaded.MediaKey,
			FileEncSHA256: uploaded.FileEncSHA256,
			FileSHA256:    uploaded.FileSHA256,
			FileLength:    proto.Uint64(uploaded.FileLength),
			FileName:      proto.String(title),
		},
	})
	return err
}

func (w *WhatsAppClient) SendStickerToJID(ctx context.Context, to waTypes.JID, data []byte, isAnimated bool) error {
	var mediaType whatsmeow.MediaType
	mediaType = whatsmeow.MediaImage

	uploaded, err := w.client.Upload(ctx, data, mediaType)
	if err != nil {
		return err
	}

	_, err = w.client.SendMessage(ctx, to, &waProto.Message{
		StickerMessage: &waProto.StickerMessage{
			Mimetype:      proto.String("image/webp"),
			URL:           proto.String(uploaded.URL),
			DirectPath:    proto.String(uploaded.DirectPath),
			MediaKey:      uploaded.MediaKey,
			FileEncSHA256: uploaded.FileEncSHA256,
			FileSHA256:    uploaded.FileSHA256,
			FileLength:    proto.Uint64(uploaded.FileLength),
			IsAnimated:    proto.Bool(isAnimated),
		},
	})
	return err
}

func (w *WhatsAppClient) DownloadMedia(ctx context.Context, msg *entity.Message) ([]byte, bool, error) {
	var downloadableMedia whatsmeow.DownloadableMessage
	var isAnimated bool

	if msg.VMessage.GetVideoMessage() != nil {
		downloadableMedia = msg.VMessage.GetVideoMessage()
		isAnimated = true
	} else if msg.VMessage.GetImageMessage() != nil {
		downloadableMedia = msg.VMessage.GetImageMessage()
		isAnimated = false
	}

	if downloadableMedia == nil {
		return nil, isAnimated, fmt.Errorf("no downloadable media found")
	}

	data, err := w.client.Download(ctx, downloadableMedia)
	if err != nil {
		return nil, isAnimated, fmt.Errorf("download failed: %w", err)
	}

	return data, isAnimated, nil
}

func (w *WhatsAppClient) UploadMedia(ctx context.Context, data []byte, mediaType string) (*entity.UploadResult, error) {
	var mt whatsmeow.MediaType
	switch mediaType {
	case "image":
		mt = whatsmeow.MediaImage
	default:
		mt = whatsmeow.MediaDocument
	}

	uploaded, err := w.client.Upload(ctx, data, mt)
	if err != nil {
		return nil, err
	}

	return &entity.UploadResult{
		URL:           uploaded.URL,
		DirectPath:    uploaded.DirectPath,
		MediaKey:      uploaded.MediaKey,
		FileEncSHA256: uploaded.FileEncSHA256,
		FileSHA256:    uploaded.FileSHA256,
		FileLength:    uploaded.FileLength,
	}, nil
}

func (w *WhatsAppClient) GetUserInfo(ctx context.Context, jid string) (*entity.UserInfo, error) {
	targetJID := waTypes.NewJID(jid, waTypes.DefaultUserServer)
	userInfo, err := w.client.GetUserInfo(ctx, []waTypes.JID{targetJID})
	if err != nil {
		return nil, err
	}

	if len(userInfo) == 0 {
		return &entity.UserInfo{JID: targetJID.String()}, nil
	}

	for _, info := range userInfo {
		if !info.LID.IsEmpty() {
			return &entity.UserInfo{
				JID: targetJID.String(),
				LID: info.LID.String(),
			}, nil
		}
	}

	return &entity.UserInfo{JID: targetJID.String()}, nil
}

func (w *WhatsAppClient) GetGroupInfo(ctx context.Context, jid string) (*entity.Group, error) {
	targetJID := waTypes.NewJID(jid, waTypes.DefaultUserServer)
	groupInfo, err := w.client.GetGroupInfo(ctx, targetJID)
	if err != nil {
		return nil, err
	}

	participants := make([]*entity.Participant, len(groupInfo.Participants))
	for i, p := range groupInfo.Participants {
		participants[i] = &entity.Participant{
			JID: p.JID.String(),
			LID: p.JID.String(),
		}
	}

	return &entity.Group{
		JID:          groupInfo.JID.String(),
		Name:         groupInfo.Name,
		Participants: participants,
	}, nil
}

func (w *WhatsAppClient) GetJoinedGroups(ctx context.Context) ([]*entity.Group, error) {
	groups, err := w.client.GetJoinedGroups(ctx)
	if err != nil {
		return nil, err
	}

	result := make([]*entity.Group, len(groups))
	for i, g := range groups {
		participants := make([]*entity.Participant, len(g.Participants))
		for j, p := range g.Participants {
			participants[j] = &entity.Participant{
				JID: p.JID.String(),
				LID: p.JID.String(),
			}
		}
		result[i] = &entity.Group{
			JID:          g.JID.String(),
			Name:         g.Name,
			Participants: participants,
		}
	}
	return result, nil
}

func (w *WhatsAppClient) Connect() error {
	return w.client.Connect()
}

func (w *WhatsAppClient) Disconnect() {
	w.client.Disconnect()
}

func (w *WhatsAppClient) GetClient() interface{} {
	return w.client
}

func (w *WhatsAppClient) AddEventHandler(handler func(event interface{})) {
	w.client.AddEventHandler(handler)
}

func (w *WhatsAppClient) GetQRChannel(ctx context.Context) (<-chan QREvent, error) {
	qrChan, err := w.client.GetQRChannel(ctx)
	if err != nil {
		return nil, err
	}

	resultChan := make(chan QREvent)
	go func() {
		for evt := range qrChan {
			resultChan <- QREvent{
				Event: evt.Event,
				Code:  evt.Code,
			}
		}
		close(resultChan)
	}()

	return resultChan, nil
}

func (w *WhatsAppClient) IsLoggedIn() bool {
	return w.client.Store.ID != nil
}

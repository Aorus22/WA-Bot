package whatsapp

import (
	"context"
	"fmt"
	"net/http"
	"time"

	_ "github.com/mattn/go-sqlite3"
	"github.com/purpshell/meowcaller"
	"go.mau.fi/whatsmeow"
	waCommon "go.mau.fi/whatsmeow/proto/waCommon"
	waProto "go.mau.fi/whatsmeow/proto/waE2E"
	"go.mau.fi/whatsmeow/store/sqlstore"
	waTypes "go.mau.fi/whatsmeow/types"
	waLog "go.mau.fi/whatsmeow/util/log"
	"google.golang.org/protobuf/proto"

	"wa-bot/internal/domain/entity"
)

type MessageLogger interface {
	LogSentMessage(msgID, chatID, from, to, content, msgType, mediaURL string, isAutomatic bool, replyToID string)
}

type QREvent struct {
	Event string
	Code  string
}

type WhatsAppClient struct {
	client     *whatsmeow.Client
	callClient *meowcaller.Client
	logLevel   string
	dbURL      string
	dbLog      waLog.Logger
	container  *sqlstore.Container
	logger     MessageLogger
}

func NewWhatsAppClient(dbURL string, logLevel string, dbLog waLog.Logger) (*WhatsAppClient, error) {
	container, err := sqlstore.New(context.Background(), "sqlite3", dbURL, dbLog)
	if err != nil {
		return nil, err
	}
	deviceStore, err := container.GetFirstDevice(context.Background())
	if err != nil {
		return nil, err
	}
	client := whatsmeow.NewClient(deviceStore, waLog.Stdout("Client", logLevel, true))

	w := &WhatsAppClient{
		client:    client,
		logLevel:  logLevel,
		dbURL:     dbURL,
		dbLog:     dbLog,
		container: container,
	}
	// meowcaller wraps the same whatsmeow.Client and MUST be created before
	// whatsmeow.Connect() so call hooks are attached before the receive loop starts.
	w.callClient = meowcaller.NewClient(client)

	return w, nil
}

func (w *WhatsAppClient) SetLogger(logger MessageLogger) {
	w.logger = logger
}

func (w *WhatsAppClient) log(msgID string, to string, content string, msgType string, mediaURL string, isAutomatic bool, replyToID string) {
	if w.logger != nil {
		w.logger.LogSentMessage(msgID, to, "me", to, content, msgType, mediaURL, isAutomatic, replyToID)
	}
}

func (w *WhatsAppClient) SendMessage(ctx context.Context, to string, text string, isAutomatic bool) (string, error) {
	targetJID, err := waTypes.ParseJID(to)
	if err != nil {
		targetJID = waTypes.NewJID(to, waTypes.DefaultUserServer)
	}

	resp, err := w.client.SendMessage(ctx, targetJID, &waProto.Message{
		Conversation: proto.String(text),
	})
	if err == nil {
		w.log(resp.ID, targetJID.String(), text, "text", "", isAutomatic, "")
		return resp.ID, nil
	}
	return "", err
}

func (w *WhatsAppClient) SendMessageToJID(ctx context.Context, to waTypes.JID, text string, isAutomatic bool) (string, error) {
	resp, sendErr := w.client.SendMessage(ctx, to, &waProto.Message{
		Conversation: proto.String(text),
	})
	if sendErr == nil {
		w.log(resp.ID, to.String(), text, "text", "", isAutomatic, "")
		return resp.ID, nil
	}
	return "", sendErr
}

func (w *WhatsAppClient) SendImage(ctx context.Context, to string, data []byte, caption string, mediaURL string, isAutomatic bool) (string, error) {
	uploaded, err := w.client.Upload(ctx, data, whatsmeow.MediaImage)
	if err != nil {
		return "", err
	}

	targetJID, _ := waTypes.ParseJID(to)
	if targetJID.IsEmpty() {
		targetJID = waTypes.NewJID(to, waTypes.DefaultUserServer)
	}

	resp, err := w.client.SendMessage(ctx, targetJID, &waProto.Message{
		ImageMessage: &waProto.ImageMessage{
			Caption:       proto.String(caption),
			Mimetype:      proto.String("image/jpeg"),
			URL:           proto.String(uploaded.URL),
			DirectPath:    proto.String(uploaded.DirectPath),
			MediaKey:      uploaded.MediaKey,
			FileEncSHA256: uploaded.FileEncSHA256,
			FileSHA256:    uploaded.FileSHA256,
			FileLength:    proto.Uint64(uploaded.FileLength),
		},
	})
	if err == nil {
		w.log(resp.ID, targetJID.String(), caption, "image", mediaURL, isAutomatic, "")
		return resp.ID, nil
	}
	return "", err
}

func (w *WhatsAppClient) SendVideo(ctx context.Context, to string, data []byte, caption string, mediaURL string, isAutomatic bool) (string, error) {
	uploaded, err := w.client.Upload(ctx, data, whatsmeow.MediaVideo)
	if err != nil {
		return "", err
	}

	targetJID, _ := waTypes.ParseJID(to)
	if targetJID.IsEmpty() {
		targetJID = waTypes.NewJID(to, waTypes.DefaultUserServer)
	}

	resp, err := w.client.SendMessage(ctx, targetJID, &waProto.Message{
		VideoMessage: &waProto.VideoMessage{
			Caption:       proto.String(caption),
			Mimetype:      proto.String("video/mp4"),
			URL:           proto.String(uploaded.URL),
			DirectPath:    proto.String(uploaded.DirectPath),
			MediaKey:      uploaded.MediaKey,
			FileEncSHA256: uploaded.FileEncSHA256,
			FileSHA256:    uploaded.FileSHA256,
			FileLength:    proto.Uint64(uploaded.FileLength),
		},
	})
	if err == nil {
		w.log(resp.ID, targetJID.String(), caption, "video", mediaURL, isAutomatic, "")
		return resp.ID, nil
	}
	return "", err
}

func (w *WhatsAppClient) SendDocument(ctx context.Context, to string, data []byte, title string, mediaURL string, isAutomatic bool) (string, error) {
	uploaded, err := w.client.Upload(ctx, data, whatsmeow.MediaDocument)
	if err != nil {
		return "", err
	}

	targetJID, _ := waTypes.ParseJID(to)
	if targetJID.IsEmpty() {
		targetJID = waTypes.NewJID(to, waTypes.DefaultUserServer)
	}

	mimetype := http.DetectContentType(data)

	resp, err := w.client.SendMessage(ctx, targetJID, &waProto.Message{
		DocumentMessage: &waProto.DocumentMessage{
			Title:         proto.String(title),
			Mimetype:      proto.String(mimetype),
			URL:           proto.String(uploaded.URL),
			DirectPath:    proto.String(uploaded.DirectPath),
			MediaKey:      uploaded.MediaKey,
			FileEncSHA256: uploaded.FileEncSHA256,
			FileSHA256:    uploaded.FileSHA256,
			FileLength:    proto.Uint64(uploaded.FileLength),
			FileName:      proto.String(title),
		},
	})
	if err == nil {
		w.log(resp.ID, targetJID.String(), title, "document", mediaURL, isAutomatic, "")
		return resp.ID, nil
	}
	return "", err
}

func (w *WhatsAppClient) SendSticker(ctx context.Context, to string, data []byte, isAnimated bool, mediaURL string, isAutomatic bool) (string, error) {
	uploaded, err := w.client.Upload(ctx, data, whatsmeow.MediaImage)
	if err != nil {
		return "", err
	}

	targetJID, _ := waTypes.ParseJID(to)
	if targetJID.IsEmpty() {
		targetJID = waTypes.NewJID(to, waTypes.DefaultUserServer)
	}

	resp, err := w.client.SendMessage(ctx, targetJID, &waProto.Message{
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
	if err == nil {
		w.log(resp.ID, targetJID.String(), "[Sticker]", "sticker", mediaURL, isAutomatic, "")
		return resp.ID, nil
	}
	return "", err
}

func (w *WhatsAppClient) SendDocumentToJID(ctx context.Context, to waTypes.JID, data []byte, title string, mediaURL string, isAutomatic bool) (string, error) {
	uploaded, err := w.client.Upload(ctx, data, whatsmeow.MediaDocument)
	if err != nil {
		return "", err
	}

	mimetype := http.DetectContentType(data)

	resp, err := w.client.SendMessage(ctx, to, &waProto.Message{
		DocumentMessage: &waProto.DocumentMessage{
			Title:         proto.String(title),
			Mimetype:      proto.String(mimetype),
			URL:           proto.String(uploaded.URL),
			DirectPath:    proto.String(uploaded.DirectPath),
			MediaKey:      uploaded.MediaKey,
			FileEncSHA256: uploaded.FileEncSHA256,
			FileSHA256:    uploaded.FileSHA256,
			FileLength:    proto.Uint64(uploaded.FileLength),
			FileName:      proto.String(title),
		},
	})
	if err == nil {
		w.log(resp.ID, to.String(), title, "document", mediaURL, isAutomatic, "")
		return resp.ID, nil
	}
	return "", err
}

func (w *WhatsAppClient) SendAudio(ctx context.Context, to string, data []byte, mimetype string, ptt bool, seconds uint32, waveform []byte, mediaURL string) (string, error) {
	uploaded, err := w.client.Upload(ctx, data, whatsmeow.MediaAudio)
	if err != nil {
		return "", err
	}

	targetJID, _ := waTypes.ParseJID(to)
	if targetJID.IsEmpty() {
		targetJID = waTypes.NewJID(to, waTypes.DefaultUserServer)
	}

	return w.SendAudioToJID(ctx, targetJID, data, uploaded, mimetype, ptt, seconds, waveform, mediaURL)
}

func (w *WhatsAppClient) SendAudioToJID(ctx context.Context, to waTypes.JID, data []byte, uploaded whatsmeow.UploadResponse, mimetype string, ptt bool, seconds uint32, waveform []byte, mediaURL string) (string, error) {
	resp, err := w.client.SendMessage(ctx, to, &waProto.Message{
		AudioMessage: &waProto.AudioMessage{
			Mimetype:      proto.String(mimetype),
			URL:           proto.String(uploaded.URL),
			DirectPath:    proto.String(uploaded.DirectPath),
			MediaKey:      uploaded.MediaKey,
			FileEncSHA256: uploaded.FileEncSHA256,
			FileSHA256:    uploaded.FileSHA256,
			FileLength:    proto.Uint64(uint64(len(data))),
			Seconds:       proto.Uint32(seconds),
			PTT:           proto.Bool(ptt),
			Waveform:      waveform,
		},
	})
	if err == nil {
		msgType := "audio"
		if ptt {
			msgType = "ptt"
		}
		content := "[Audio]"
		if ptt {
			content = "[Voice Message]"
		}
		w.log(resp.ID, to.String(), content, msgType, mediaURL, false, "")
		return resp.ID, nil
	}
	return "", err
}

func (w *WhatsAppClient) SendStickerToJID(ctx context.Context, to waTypes.JID, data []byte, isAnimated bool, mediaURL string, isAutomatic bool) (string, error) {
	uploaded, err := w.client.Upload(ctx, data, whatsmeow.MediaImage)
	if err != nil {
		return "", err
	}

	resp, err := w.client.SendMessage(ctx, to, &waProto.Message{
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
	if err == nil {
		w.log(resp.ID, to.String(), "[Sticker]", "sticker", mediaURL, isAutomatic, "")
		return resp.ID, nil
	}
	return "", err
}

func (w *WhatsAppClient) DeleteMessage(ctx context.Context, to string, msgID string) error {
	targetJID, err := waTypes.ParseJID(to)
	if err != nil {
		return err
	}
	senderJID := w.client.Store.ID.ToNonAD()
	_, err = w.client.SendMessage(ctx, targetJID, w.client.BuildRevoke(targetJID, senderJID, msgID))
	return err
}

func (w *WhatsAppClient) EditMessage(ctx context.Context, to string, msgID string, newText string) error {
	targetJID, err := waTypes.ParseJID(to)
	if err != nil {
		return err
	}
	_, err = w.client.SendMessage(ctx, targetJID, w.client.BuildEdit(targetJID, msgID, &waProto.Message{
		Conversation: proto.String(newText),
	}))
	return err
}

func (w *WhatsAppClient) ReplyMessage(ctx context.Context, to string, msgID string, text string) error {
	targetJID, err := waTypes.ParseJID(to)
	if err != nil {
		return err
	}

	resp, err := w.client.SendMessage(ctx, targetJID, &waProto.Message{
		ExtendedTextMessage: &waProto.ExtendedTextMessage{
			Text: proto.String(text),
			ContextInfo: &waProto.ContextInfo{
				StanzaID: proto.String(msgID),
			},
		},
	})
	if err == nil {
		w.log(resp.ID, targetJID.String(), text, "text", "", false, msgID)
	}
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
	} else if msg.VMessage.GetStickerMessage() != nil {
		downloadableMedia = msg.VMessage.GetStickerMessage()
		isAnimated = msg.VMessage.GetStickerMessage().GetIsAnimated()
	} else if msg.VMessage.GetDocumentMessage() != nil {
		downloadableMedia = msg.VMessage.GetDocumentMessage()
		isAnimated = false
	} else if msg.VMessage.GetAudioMessage() != nil {
		downloadableMedia = msg.VMessage.GetAudioMessage()
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

func (w *WhatsAppClient) GetUserInfo(ctx context.Context, jid string) (*entity.UserInfo, error) {
	targetJID, err := waTypes.ParseJID(jid)
	if err != nil {
		targetJID = waTypes.NewJID(jid, waTypes.DefaultUserServer)
	}
	userInfo, err := w.client.GetUserInfo(ctx, []waTypes.JID{targetJID})
	if err != nil {
		return nil, err
	}

	if len(userInfo) == 0 {
		return &entity.UserInfo{JID: targetJID.String()}, nil
	}

	for _, info := range userInfo {
		return &entity.UserInfo{
			JID: targetJID.String(),
			LID: info.LID.String(),
		}, nil
	}

	return &entity.UserInfo{JID: targetJID.String()}, nil
}

func (w *WhatsAppClient) GetGroupInfo(ctx context.Context, jid string) (*entity.Group, error) {
	targetJID, _ := waTypes.ParseJID(jid)
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

func (w *WhatsAppClient) GetJoinedGroups() ([]*waTypes.GroupInfo, error) {
	return w.client.GetJoinedGroups(context.Background())
}

func (w *WhatsAppClient) GetGroupParticipants(groupJID waTypes.JID) ([]waTypes.GroupParticipant, error) {
	info, err := w.client.GetGroupInfo(context.Background(), groupJID)
	if err != nil {
		return nil, err
	}
	return info.Participants, nil
}

func (w *WhatsAppClient) Connect() error {
	return w.client.Connect()
}

func (w *WhatsAppClient) Disconnect() {
	w.client.Disconnect()
}

func (w *WhatsAppClient) Logout() error {
	return w.client.Logout(context.Background())
}

func (w *WhatsAppClient) GetClient() interface{} {
	return w.client
}

// GetCallClient returns the meowcaller.Client wrapping the same whatsmeow.Client.
// It is non-nil as soon as the WhatsAppClient is constructed (before Connect).
func (w *WhatsAppClient) GetCallClient() *meowcaller.Client {
	return w.callClient
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

// IsConnected reports whether the underlying whatsmeow websocket is actively
// connected. Unlike IsLoggedIn (which only checks a stored device), this is true
// only while the connection is live.
func (w *WhatsAppClient) IsConnected() bool {
	return w.client != nil && w.client.IsConnected()
}

func (w *WhatsAppClient) GetProfilePictureInfo(ctx context.Context, jid string) (string, error) {
	targetJID, err := waTypes.ParseJID(jid)
	if err != nil {
		return "", err
	}

	info, err := w.client.GetProfilePictureInfo(ctx, targetJID, nil)
	if err != nil {
		return "", err
	}

	if info == nil {
		return "", fmt.Errorf("no profile picture found")
	}

	if info.URL != "" {
		return info.URL, nil
	}

	return "", fmt.Errorf("no profile picture URL found")
}

func (w *WhatsAppClient) SendPresence(to string, isTyping bool) error {
	targetJID, err := waTypes.ParseJID(to)
	if err != nil {
		return err
	}

	presence := waTypes.ChatPresencePaused
	if isTyping {
		presence = waTypes.ChatPresenceComposing
	}

	return w.client.SendChatPresence(context.Background(), targetJID, presence, waTypes.ChatPresenceMediaText)
}

// SendReaction sends an emoji reaction to a specific message.
func (w *WhatsAppClient) SendReaction(to, msgID, emoji string) error {
	targetJID, err := waTypes.ParseJID(to)
	if err != nil {
		return err
	}

	_, err = w.client.SendMessage(context.Background(), targetJID, &waProto.Message{
		ReactionMessage: &waProto.ReactionMessage{
			Key: &waCommon.MessageKey{
				RemoteJID: proto.String(to),
				FromMe:    proto.Bool(false),
				ID:        proto.String(msgID),
			},
			Text:              proto.String(emoji),
			SenderTimestampMS: proto.Int64(time.Now().UnixMilli()),
		},
	})
	if err == nil {
		w.log("reaction", targetJID.String(), emoji, "reaction", "", false, msgID)
	}
	return err
}

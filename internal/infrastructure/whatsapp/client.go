package whatsapp

import (
	"context"
	"fmt"
	"net/http"
	"strings"
	"time"

	_ "github.com/mattn/go-sqlite3"
	"github.com/purpshell/meowcaller"
	"go.mau.fi/whatsmeow"
	"go.mau.fi/whatsmeow/appstate"
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
	// Pairing history is downloaded by our staging service. Keeping this true
	// prevents whatsmeow from dispatching it straight into the visible message
	// pipeline before the user clicks Sync in Settings.
	client.ManualHistorySyncDownload = true

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

// SendMessageWithPreview sends a text message; when the text contains an
// http(s) URL it attaches a scraped link preview, mirroring the official
// clients. Falls back to a plain message when scraping fails.
func (w *WhatsAppClient) SendMessageWithPreview(ctx context.Context, to string, text string, isAutomatic bool) (string, error) {
	rawURL := FirstURLInText(text)
	if rawURL == "" {
		return w.SendMessage(ctx, to, text, isAutomatic)
	}
	preview := ScrapeLinkPreview(ctx, rawURL)
	if preview == nil {
		return w.SendMessage(ctx, to, text, isAutomatic)
	}

	targetJID, err := waTypes.ParseJID(to)
	if err != nil {
		targetJID = waTypes.NewJID(to, waTypes.DefaultUserServer)
	}
	et := &waProto.ExtendedTextMessage{
		Text:        proto.String(text),
		MatchedText: proto.String(preview.URL),
		Title:       proto.String(preview.Title),
		Description: proto.String(preview.Description),
	}
	if len(preview.JPEGThumbnail) > 0 {
		et.JPEGThumbnail = preview.JPEGThumbnail
	}
	resp, err := w.client.SendMessage(ctx, targetJID, &waProto.Message{ExtendedTextMessage: et})
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

// CoreClient exposes the typed client for history parsing and peer requests.
func (w *WhatsAppClient) CoreClient() *whatsmeow.Client { return w.client }

func (w *WhatsAppClient) SetChatPinned(ctx context.Context, jid waTypes.JID, pinned bool) error {
	return w.client.SendAppState(ctx, appstate.BuildPin(jid, pinned))
}

func (w *WhatsAppClient) SetChatArchived(ctx context.Context, jid waTypes.JID, archived bool, lastMessageTimestamp time.Time, lastMessageKey *waCommon.MessageKey) error {
	return w.client.SendAppState(ctx, appstate.BuildArchive(jid, archived, lastMessageTimestamp, lastMessageKey))
}

func (w *WhatsAppClient) SetChatMuted(ctx context.Context, jid waTypes.JID, muted bool, duration time.Duration) error {
	return w.client.SendAppState(ctx, appstate.BuildMute(jid, muted, duration))
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

// ReadTarget pairs a message ID with its sender (used to group group-chat
// read receipts per participant).
type ReadTarget struct {
	ID     string
	Sender string
}

// SendReadReceipts sends WhatsApp read receipts for the given messages. In
// groups one receipt is sent per distinct sender (as WhatsApp requires).
func (w *WhatsAppClient) SendReadReceipts(ctx context.Context, chatID string, targets []ReadTarget) error {
	if len(targets) == 0 {
		return nil
	}
	chatJID, err := waTypes.ParseJID(chatID)
	if err != nil {
		return err
	}
	now := time.Now()

	if chatJID.Server != waTypes.GroupServer {
		ids := make([]waTypes.MessageID, 0, len(targets))
		for _, t := range targets {
			ids = append(ids, t.ID)
		}
		return w.client.MarkRead(ctx, ids, now, chatJID, chatJID)
	}

	bySender := map[waTypes.JID][]waTypes.MessageID{}
	for _, t := range targets {
		sender := waTypes.EmptyJID
		if parsed, perr := waTypes.ParseJID(t.Sender); perr == nil {
			sender = parsed.ToNonAD()
		}
		bySender[sender] = append(bySender[sender], t.ID)
	}
	for sender, ids := range bySender {
		if sender.IsEmpty() {
			continue
		}
		if err := w.client.MarkRead(ctx, ids, now, chatJID, sender); err != nil {
			return err
		}
	}
	return nil
}

// SubscribeUserPresence subscribes to a user's availability updates. Presence
// events only flow after this; WhatsApp requires re-subscribing per chat.
func (w *WhatsAppClient) SubscribeUserPresence(ctx context.Context, jid string) error {
	targetJID, err := waTypes.ParseJID(jid)
	if err != nil {
		return err
	}
	return w.client.SubscribePresence(ctx, targetJID)
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

// SendReaction sends an emoji reaction to a specific message. An empty emoji
// removes the reaction. authorJID is the sender of the reacted-to message
// (empty for own messages).
func (w *WhatsAppClient) SendReaction(ctx context.Context, to, msgID, emoji, authorJID string) error {
	targetJID, err := waTypes.ParseJID(to)
	if err != nil {
		return err
	}
	author := waTypes.EmptyJID
	if authorJID != "" && authorJID != "me" {
		if parsed, perr := waTypes.ParseJID(authorJID); perr == nil {
			author = parsed.ToNonAD()
		}
	}
	_, err = w.client.SendMessage(ctx, targetJID, w.client.BuildReaction(targetJID, author, msgID, emoji))
	return err
}

// SendPoll sends a poll. selectableCount <= 0 means single choice; pass
// len(options) for a multi-select poll.
func (w *WhatsAppClient) SendPoll(ctx context.Context, to, question string, options []string, selectableCount int) (string, error) {
	if selectableCount <= 0 || selectableCount > len(options) {
		selectableCount = 1
	}
	targetJID := parseTargetJID(to)
	resp, err := w.client.SendMessage(ctx, targetJID, w.client.BuildPollCreation(question, options, selectableCount))
	if err != nil {
		return "", err
	}
	return resp.ID, nil
}

// SendPollVote votes on a poll. pollMsgInfo must describe the original poll
// creation message (ID, chat, sender, isFromMe).
func (w *WhatsAppClient) SendPollVote(ctx context.Context, pollMsgInfo waTypes.MessageInfo, optionNames []string) error {
	vote, err := w.client.BuildPollVote(ctx, &pollMsgInfo, optionNames)
	if err != nil {
		return err
	}
	_, err = w.client.SendMessage(ctx, pollMsgInfo.Chat, vote)
	return err
}

// SendLocation shares a static location pin.
func (w *WhatsAppClient) SendLocation(ctx context.Context, to string, latitude, longitude float64, name, address string) (string, error) {
	targetJID := parseTargetJID(to)
	resp, err := w.client.SendMessage(ctx, targetJID, &waProto.Message{
		LocationMessage: &waProto.LocationMessage{
			DegreesLatitude:  proto.Float64(latitude),
			DegreesLongitude: proto.Float64(longitude),
			Name:             proto.String(name),
			Address:          proto.String(address),
		},
	})
	if err != nil {
		return "", err
	}
	return resp.ID, nil
}

// SendLiveLocation starts sharing a live location. Updates are sent as new
// LiveLocationMessages with increasing sequence numbers; StopLiveLocation
// (sequence -1) ends the share.
func (w *WhatsAppClient) SendLiveLocation(ctx context.Context, to string, latitude, longitude float64, caption string, sequence int64) (string, error) {
	targetJID := parseTargetJID(to)
	resp, err := w.client.SendMessage(ctx, targetJID, &waProto.Message{
		LiveLocationMessage: &waProto.LiveLocationMessage{
			DegreesLatitude:  proto.Float64(latitude),
			DegreesLongitude: proto.Float64(longitude),
			Caption:          proto.String(caption),
			SequenceNumber:   proto.Int64(sequence),
		},
	})
	if err != nil {
		return "", err
	}
	return resp.ID, nil
}

// StopLiveLocation terminates an ongoing live location share.
func (w *WhatsAppClient) StopLiveLocation(ctx context.Context, to string, latitude, longitude float64, caption string) error {
	_, err := w.SendLiveLocation(ctx, to, latitude, longitude, caption, -1)
	return err
}

type ContactPair struct {
	DisplayName string
	VCard       string
	Phone       string
}

// BuildVCard creates a minimal vCard 3.0 payload from a name and phone number.
func BuildVCard(name, phone string) string {
	digits := strings.Map(func(r rune) rune {
		if r >= '0' && r <= '9' {
			return r
		}
		return -1
	}, phone)
	card := "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:" + name + "\r\n"
	if digits != "" {
		card += "TEL;type=CELL;waid=" + digits + ":" + phone + "\r\n"
	}
	return card + "END:VCARD\r\n"
}

// SendContact shares a single contact card. If vcard is empty it is generated
// from displayName + phone.
func (w *WhatsAppClient) SendContact(ctx context.Context, to, displayName, phone, vcard string) (string, error) {
	if vcard == "" {
		vcard = BuildVCard(displayName, phone)
	}
	targetJID := parseTargetJID(to)
	resp, err := w.client.SendMessage(ctx, targetJID, &waProto.Message{
		ContactMessage: &waProto.ContactMessage{
			DisplayName: proto.String(displayName),
			Vcard:       proto.String(vcard),
		},
	})
	if err != nil {
		return "", err
	}
	return resp.ID, nil
}

// SendContacts shares multiple contact cards at once.
func (w *WhatsAppClient) SendContacts(ctx context.Context, to, displayName string, contacts []ContactPair) (string, error) {
	cards := make([]*waProto.ContactMessage, 0, len(contacts))
	for _, c := range contacts {
		vcard := c.VCard
		if vcard == "" {
			vcard = BuildVCard(c.DisplayName, c.Phone)
		}
		cards = append(cards, &waProto.ContactMessage{
			DisplayName: proto.String(c.DisplayName),
			Vcard:       proto.String(vcard),
		})
	}
	targetJID := parseTargetJID(to)
	resp, err := w.client.SendMessage(ctx, targetJID, &waProto.Message{
		ContactsArrayMessage: &waProto.ContactsArrayMessage{
			DisplayName: proto.String(displayName),
			Contacts:    cards,
		},
	})
	if err != nil {
		return "", err
	}
	return resp.ID, nil
}

// SendViewOnceMedia sends an image or video as view-once (kind: "image" or
// "video"). View-once bubbles cannot carry captions per WhatsApp policy.
func (w *WhatsAppClient) SendViewOnceMedia(ctx context.Context, to, kind string, data []byte, mediaURL string) (string, error) {
	mediaType := whatsmeow.MediaImage
	if kind == "video" {
		mediaType = whatsmeow.MediaVideo
	}
	uploaded, err := w.client.Upload(ctx, data, mediaType)
	if err != nil {
		return "", err
	}

	var inner *waProto.Message
	if kind == "video" {
		inner = &waProto.Message{VideoMessage: &waProto.VideoMessage{
			Mimetype:      proto.String("video/mp4"),
			URL:           proto.String(uploaded.URL),
			DirectPath:    proto.String(uploaded.DirectPath),
			MediaKey:      uploaded.MediaKey,
			FileEncSHA256: uploaded.FileEncSHA256,
			FileSHA256:    uploaded.FileSHA256,
			FileLength:    proto.Uint64(uploaded.FileLength),
		}}
	} else {
		inner = &waProto.Message{ImageMessage: &waProto.ImageMessage{
			Mimetype:      proto.String("image/jpeg"),
			URL:           proto.String(uploaded.URL),
			DirectPath:    proto.String(uploaded.DirectPath),
			MediaKey:      uploaded.MediaKey,
			FileEncSHA256: uploaded.FileEncSHA256,
			FileSHA256:    uploaded.FileSHA256,
			FileLength:    proto.Uint64(uploaded.FileLength),
		}}
	}

	targetJID := parseTargetJID(to)
	resp, err := w.client.SendMessage(ctx, targetJID, &waProto.Message{
		ViewOnceMessageV2: &waProto.FutureProofMessage{Message: inner},
	})
	if err != nil {
		return "", err
	}
	return resp.ID, nil
}

// SendGIF sends a silent MP4 as an animated GIF (WhatsApp represents GIFs as
// MP4 videos with gifPlayback + attribution). Caller must transcode source
// GIF bytes to MP4 first.
func (w *WhatsAppClient) SendGIF(ctx context.Context, to string, data []byte, caption string, mediaURL string) (string, error) {
	uploaded, err := w.client.Upload(ctx, data, whatsmeow.MediaVideo)
	if err != nil {
		return "", err
	}
	targetJID := parseTargetJID(to)
	resp, err := w.client.SendMessage(ctx, targetJID, &waProto.Message{
		VideoMessage: &waProto.VideoMessage{
			Mimetype:       proto.String("video/mp4"),
			URL:            proto.String(uploaded.URL),
			DirectPath:     proto.String(uploaded.DirectPath),
			MediaKey:       uploaded.MediaKey,
			FileEncSHA256:  uploaded.FileEncSHA256,
			FileSHA256:     uploaded.FileSHA256,
			FileLength:     proto.Uint64(uploaded.FileLength),
			GifPlayback:    proto.Bool(true),
			GifAttribution: waProto.VideoMessage_GIPHY.Enum(),
		},
	})
	if err != nil {
		return "", err
	}
	w.log(resp.ID, targetJID.String(), caption, "gif", mediaURL, false, "")
	return resp.ID, nil
}

// ForwardMessage re-sends a stored raw message proto to another chat with the
// forwarded flag set, mirroring WhatsApp's native forward.
func (w *WhatsAppClient) ForwardMessage(ctx context.Context, to string, rawProto []byte) (string, error) {
	var orig waProto.Message
	if err := proto.Unmarshal(rawProto, &orig); err != nil {
		return "", fmt.Errorf("failed to decode stored message: %w", err)
	}
	clone := proto.Clone(&orig).(*waProto.Message)
	markForwarded(clone)
	targetJID := parseTargetJID(to)
	resp, err := w.client.SendMessage(ctx, targetJID, clone)
	if err != nil {
		return "", err
	}
	return resp.ID, nil
}

// markForwarded sets the forwarded flag on whichever submessage carries a
// ContextInfo, converting plain conversations to ExtendedTextMessage so the
// flag survives.
func markForwarded(msg *waProto.Message) {
	if msg.GetConversation() != "" {
		msg.ExtendedTextMessage = &waProto.ExtendedTextMessage{
			Text: proto.String(msg.GetConversation()),
			ContextInfo: &waProto.ContextInfo{
				IsForwarded:     proto.Bool(true),
				ForwardingScore: proto.Uint32(1),
			},
		}
		msg.Conversation = nil
		return
	}
	if ci := getContextInfoForForward(msg); ci != nil {
		ci.IsForwarded = proto.Bool(true)
		if ci.GetForwardingScore() == 0 {
			ci.ForwardingScore = proto.Uint32(1)
		} else {
			ci.ForwardingScore = proto.Uint32(ci.GetForwardingScore() + 1)
		}
	}
}

func getContextInfoForForward(msg *waProto.Message) *waProto.ContextInfo {
	switch {
	case msg.GetExtendedTextMessage() != nil:
		if msg.GetExtendedTextMessage().ContextInfo == nil {
			msg.GetExtendedTextMessage().ContextInfo = &waProto.ContextInfo{}
		}
		return msg.GetExtendedTextMessage().ContextInfo
	case msg.GetImageMessage() != nil:
		if msg.GetImageMessage().ContextInfo == nil {
			msg.GetImageMessage().ContextInfo = &waProto.ContextInfo{}
		}
		return msg.GetImageMessage().ContextInfo
	case msg.GetVideoMessage() != nil:
		if msg.GetVideoMessage().ContextInfo == nil {
			msg.GetVideoMessage().ContextInfo = &waProto.ContextInfo{}
		}
		return msg.GetVideoMessage().ContextInfo
	case msg.GetAudioMessage() != nil:
		if msg.GetAudioMessage().ContextInfo == nil {
			msg.GetAudioMessage().ContextInfo = &waProto.ContextInfo{}
		}
		return msg.GetAudioMessage().ContextInfo
	case msg.GetDocumentMessage() != nil:
		if msg.GetDocumentMessage().ContextInfo == nil {
			msg.GetDocumentMessage().ContextInfo = &waProto.ContextInfo{}
		}
		return msg.GetDocumentMessage().ContextInfo
	case msg.GetStickerMessage() != nil:
		if msg.GetStickerMessage().ContextInfo == nil {
			msg.GetStickerMessage().ContextInfo = &waProto.ContextInfo{}
		}
		return msg.GetStickerMessage().ContextInfo
	case msg.GetLocationMessage() != nil:
		if msg.GetLocationMessage().ContextInfo == nil {
			msg.GetLocationMessage().ContextInfo = &waProto.ContextInfo{}
		}
		return msg.GetLocationMessage().ContextInfo
	case msg.GetContactMessage() != nil:
		if msg.GetContactMessage().ContextInfo == nil {
			msg.GetContactMessage().ContextInfo = &waProto.ContextInfo{}
		}
		return msg.GetContactMessage().ContextInfo
	}
	return nil
}

func parseTargetJID(to string) waTypes.JID {
	targetJID, err := waTypes.ParseJID(to)
	if err != nil || targetJID.IsEmpty() {
		targetJID = waTypes.NewJID(to, waTypes.DefaultUserServer)
	}
	return targetJID
}

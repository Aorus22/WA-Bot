package whatsapp

import (
	"context"
	"time"

	"go.mau.fi/whatsmeow"
	waProto "go.mau.fi/whatsmeow/proto/waE2E"
	waTypes "go.mau.fi/whatsmeow/types"
	"google.golang.org/protobuf/proto"
)

// SendStatusText posts a text status (story) with a solid background color.
// backgroundARGB defaults to WhatsApp's dark teal when 0.
func (w *WhatsAppClient) SendStatusText(ctx context.Context, text string, backgroundARGB uint32) (string, error) {
	if backgroundARGB == 0 {
		backgroundARGB = 0xFF075E54
	}
	resp, err := w.client.SendMessage(ctx, waTypes.StatusBroadcastJID, &waProto.Message{
		ExtendedTextMessage: &waProto.ExtendedTextMessage{
			Text:           proto.String(text),
			BackgroundArgb: proto.Uint32(backgroundARGB),
			TextArgb:       proto.Uint32(0xFFFFFFFF),
		},
	})
	if err != nil {
		return "", err
	}
	return resp.ID, nil
}

// SendStatusMedia posts an image or video status (kind: "image" | "video").
func (w *WhatsAppClient) SendStatusMedia(ctx context.Context, kind string, data []byte, caption string, mediaURL string) (string, error) {
	mediaType := whatsmeow.MediaImage
	if kind == "video" {
		mediaType = whatsmeow.MediaVideo
	}
	uploaded, err := w.client.Upload(ctx, data, mediaType)
	if err != nil {
		return "", err
	}

	var msg *waProto.Message
	if kind == "video" {
		msg = &waProto.Message{VideoMessage: &waProto.VideoMessage{
			Caption:       proto.String(caption),
			Mimetype:      proto.String("video/mp4"),
			URL:           proto.String(uploaded.URL),
			DirectPath:    proto.String(uploaded.DirectPath),
			MediaKey:      uploaded.MediaKey,
			FileEncSHA256: uploaded.FileEncSHA256,
			FileSHA256:    uploaded.FileSHA256,
			FileLength:    proto.Uint64(uploaded.FileLength),
		}}
	} else {
		msg = &waProto.Message{ImageMessage: &waProto.ImageMessage{
			Caption:       proto.String(caption),
			Mimetype:      proto.String("image/jpeg"),
			URL:           proto.String(uploaded.URL),
			DirectPath:    proto.String(uploaded.DirectPath),
			MediaKey:      uploaded.MediaKey,
			FileEncSHA256: uploaded.FileEncSHA256,
			FileSHA256:    uploaded.FileSHA256,
			FileLength:    proto.Uint64(uploaded.FileLength),
		}}
	}

	resp, err := w.client.SendMessage(ctx, waTypes.StatusBroadcastJID, msg)
	if err != nil {
		return "", err
	}
	return resp.ID, nil
}

// MarkStatusViewed sends the read receipt for viewed statuses so the sender
// sees the viewer list.
func (w *WhatsAppClient) MarkStatusViewed(ctx context.Context, statusIDs []string, owner string) error {
	ownerJID, err := waTypes.ParseJID(owner)
	if err != nil {
		return err
	}
	return w.client.MarkRead(ctx, statusIDs, time.Now(), waTypes.StatusBroadcastJID, ownerJID)
}

// GetStatusPrivacy returns the current status privacy settings.
func (w *WhatsAppClient) GetStatusPrivacy(ctx context.Context) ([]waTypes.StatusPrivacy, error) {
	return w.client.GetStatusPrivacy(ctx)
}

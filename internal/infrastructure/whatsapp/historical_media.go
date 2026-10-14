package whatsapp

import (
	"mime"
	"path/filepath"
	"strings"

	"go.mau.fi/whatsmeow"
	waE2E "go.mau.fi/whatsmeow/proto/waE2E"
)

// HistoricalMedia is the downloadable part of a message imported from history,
// with what the file is called when the message does not say.
type HistoricalMedia struct {
	Message  whatsmeow.DownloadableMessage
	Mime     string
	FileName string
}

// ResolveHistoricalMedia finds the media of msg. ok is false for a message that
// carries none (or a kind that cannot be downloaded).
func ResolveHistoricalMedia(msg *waE2E.Message) (HistoricalMedia, bool) {
	if media := msg.GetImageMessage(); media != nil {
		return HistoricalMedia{media, fallbackMime(media.GetMimetype(), "image/jpeg"), "image.jpg"}, true
	}
	if media := msg.GetVideoMessage(); media != nil {
		return HistoricalMedia{media, fallbackMime(media.GetMimetype(), "video/mp4"), "video.mp4"}, true
	}
	if media := msg.GetDocumentMessage(); media != nil {
		return HistoricalMedia{media, fallbackMime(media.GetMimetype(), "application/octet-stream"), media.GetFileName()}, true
	}
	if media := msg.GetAudioMessage(); media != nil {
		return HistoricalMedia{media, fallbackMime(media.GetMimetype(), "audio/ogg"), "audio.ogg"}, true
	}
	if media := msg.GetStickerMessage(); media != nil {
		return HistoricalMedia{media, fallbackMime(media.GetMimetype(), "image/webp"), "sticker.webp"}, true
	}
	return HistoricalMedia{}, false
}

// HistoricalMediaKey is where the downloaded file of a history message is kept
// in the storage: "history/<chat>/<message><ext>".
func HistoricalMediaKey(chatID, messageID, fileName, mimeType string) string {
	ext := filepath.Ext(fileName)
	if ext == "" {
		if extensions, _ := mime.ExtensionsByType(mimeType); len(extensions) > 0 {
			ext = extensions[0]
		} else {
			ext = ".bin"
		}
	}
	return filepath.ToSlash(filepath.Join("history", safePathPart(chatID), safePathPart(messageID)+ext))
}

func fallbackMime(value, fallbackValue string) string {
	if value == "" {
		return fallbackValue
	}
	return value
}

func safePathPart(value string) string {
	value = strings.NewReplacer("@", "_", ".", "_", "/", "_", "\\", "_").Replace(value)
	if value == "" {
		return "unknown"
	}
	return value
}

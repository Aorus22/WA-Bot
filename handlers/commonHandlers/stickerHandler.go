package commonHandlers

import (
	"context"
	"errors"
	"fmt"
	"os"
	"strings"
	"time"

	"wa-bot/state"
	"wa-bot/utils"
)

func StickerHandler(s *state.MessageState) {
	isAllowed := s.UserRole == "OWNER" || s.UserRole == "COMMON"

	if !isAllowed {
		s.Reply("Invalid Command")
		return
	}

	s.Reply("⏳ Loading...")

	ctx, cancel := context.WithCancel(context.Background())
	s.AddUserToState("processing", cancel)

	go func() {
		defer s.ClearUserState()
		defer cancel()

		nocrop := strings.Contains(strings.ToLower(s.MessageText), "nocrop")

		var (
			mediaPath string
			isVideo   bool
			err       error
		)

		switch {
		case s.VMessage.GetImageMessage() != nil:
			mediaPath, isVideo, err = getWaMedia(s, false)
		case s.VMessage.GetVideoMessage() != nil:
			mediaPath, isVideo, err = getWaMedia(s, true)
		default:
			mediaPath, isVideo, err = getMediaFromUrl(ctx, s.MessageText)
		}
		defer os.Remove(mediaPath)
		if err != nil {
			utils.LogNoCancelErr(ctx, err, "Error getting media:")
			if errors.Is(err, ErrorNotSupportedLink){
				s.ReplyNoCancelError(ctx, err, "Link not supported")
			} else if errors.Is(err, ErrorNoLinkProvided) {
				s.ReplyNoCancelError(ctx, err, "No Link Provided")
			} else {
				s.ReplyNoCancelError(ctx, err, "Invalid Media / Link")
			}
			return
		}

		if utils.IsCanceledGoroutine(ctx) { return }

		err = sendMediaAsSticker(ctx, s, mediaPath, nocrop, isVideo)
		if err != nil {
			utils.LogNoCancelErr(ctx, err, "error:")
			s.ReplyNoCancelError(ctx, err, "Server error: failed to convert sticker")
		}
	}()
}

func getWaMedia(s *state.MessageState, isVideo bool) (string, bool, error) {
	data, err := s.GetDownloadableMedia(isVideo)
	if err != nil {
		return "", false, err
	}

	ext := ".jpg"
	if isVideo {
		ext = ".mp4"
	}
	mediaPath := fmt.Sprintf("media/%d%s", time.Now().UnixMilli(), ext)

	err = os.WriteFile(mediaPath, data, 0644)
	if err != nil {
		return "", false, fmt.Errorf("failed to save media: %w", err)
	}

	return mediaPath, isVideo, nil
}

var ErrorNotSupportedLink = errors.New("link not supported")
var ErrorNoLinkProvided = errors.New("no link provided")

func getMediaFromUrl(ctx context.Context, messageText string) (string, bool, error) {
	url, err := utils.GetLinkFromString(messageText)
	if err != nil {
		return "", false, ErrorNoLinkProvided
	}

	mediaPath, err := utils.DownloadMediaFromURL(ctx, url)
	if err != nil {
		return mediaPath, false, err
	}

	mimeType, err := utils.GetMimeType(mediaPath)
	if err != nil {
		return mediaPath, false, err
	}

	if !strings.HasPrefix(mimeType, "image/") && !strings.HasPrefix(mimeType, "video/") {
		return mediaPath, false, ErrorNotSupportedLink
	}

	isVideo := strings.HasPrefix(mimeType, "video/")
	return mediaPath, isVideo, nil
}

func sendMediaAsSticker(ctx context.Context, s *state.MessageState, mediaPath string, nocrop bool, isAnimated bool) error {
	var err error

	webpPath, err := utils.ConvertToWebp(ctx, mediaPath, nocrop)
	defer os.Remove(webpPath)
	if err != nil {
		return fmt.Errorf("convert to WebP: %w", err)
	}

	author := os.Getenv("APP_NAME")
	finalWebpPath, err := utils.WriteWebpExifFile(ctx, webpPath, "+62 812-3436-3620", author)
	if err != nil {
		return fmt.Errorf("write EXIF: %w", err)
	}

	defer os.Remove(finalWebpPath)

	webpData, err := os.ReadFile(finalWebpPath)
	if utils.IsCanceledGoroutine(ctx) { return nil }
	if err != nil {
		return fmt.Errorf("read WebP: %w", err)
	}

	uploaded, err := s.UploadToWhatsapp(ctx, webpData, "image")
	if err != nil {
		return fmt.Errorf("upload to WhatsApp: %w", err)
	}

	err = s.SendStickerMessage(ctx, uploaded, isAnimated)
	if err != nil {
		return fmt.Errorf("send sticker: %w", err)
	}

	return nil
}

package commonHandlers

import (
	"context"
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
		if err != nil {
			utils.LogNoCancelErr(ctx, err, "Error getting media:")
			s.ReplyNoCancelError(ctx, err, "Failed to convert sticker")
		}
		defer os.Remove(mediaPath)

		if utils.IsCanceledGoroutine(ctx) { return }

		sendMediaAsSticker(ctx, s, mediaPath, nocrop, isVideo)
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

func getMediaFromUrl(ctx context.Context, messageText string) (string, bool, error) {
	url, err := utils.GetLinkFromString(messageText)
	if err != nil {
		return "", false, err
	}

	mediaPath, err := utils.DownloadMediaFromURL(ctx, url)
	if err != nil {
		return "", false, err
	}

	mimeType, err := utils.GetMimeType(mediaPath)
	if err != nil {
		return "", false, err
	}

	isVideo := strings.HasPrefix(mimeType, "video/")
	return mediaPath, isVideo, nil
}

func sendMediaAsSticker(ctx context.Context, s *state.MessageState, mediaPath string, nocrop bool, isAnimated bool) {
	var err error

	webpPath, err := utils.ConvertToWebp(ctx, mediaPath, nocrop)
	defer os.Remove(webpPath)
	if err != nil {
		utils.LogNoCancelErr(ctx, err, "Error converting to WebP:")
		s.ReplyNoCancelError(ctx, err,"Failed to convert media to WebP")
		return
	}

	author := os.Getenv("APP_NAME")
	finalWebpPath, err := utils.WriteWebpExifFile(ctx, webpPath, "+62 812-3436-3620", author)
	if err != nil {
		utils.LogNoCancelErr(ctx, err, "Error writing EXIF data:")
		s.ReplyNoCancelError(ctx, err, "Failed to convert sticker")
		return
	}
	defer os.Remove(finalWebpPath)

	webpData, err := os.ReadFile(finalWebpPath)
	if utils.IsCanceledGoroutine(ctx) { return }
	if err != nil {
		utils.LogNoCancelErr(ctx, err, "Error reading WebP file:")
		s.ReplyNoCancelError(ctx, err, "Failed to convert sticker")
		return
	}

	uploaded, err := s.UploadToWhatsapp(ctx, webpData, "image")
	if err != nil {
		utils.LogNoCancelErr(ctx, err, "Error uploading WebP file:")
		s.ReplyNoCancelError(ctx, err, "Failed to convert sticker")
		return
	}

	err = s.SendStickerMessage(ctx, uploaded, isAnimated)
	if err != nil {
		utils.LogNoCancelErr(ctx, err, "Error sending sticker message:")
		s.ReplyNoCancelError(ctx, err, "Failed to convert sticker")
		return
	}
}

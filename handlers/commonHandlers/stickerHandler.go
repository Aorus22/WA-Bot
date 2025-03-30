package commonHandlers

import (
	goctx "context"
	"fmt"
	"os"
	"strings"
	"time"

	"wa-bot/context"
	"wa-bot/utils"
)

func StickerHandler(ctx *context.MessageContext) {
	isAllowed := ctx.UserRole == "OWNER" || ctx.UserRole == "COMMON"

	if !isAllowed {
		ctx.Reply("Invalid Command")
		return
	}

	ctx.Reply("⏳ Loading...")

	procCtx, cancel := goctx.WithCancel(goctx.Background())
	ctx.AddUserToState("processing", cancel)

	go func() {
		defer ctx.ClearUserState()

		nocrop := strings.Contains(strings.ToLower(ctx.MessageText), "nocrop")

		var (
			mediaPath string
			isVideo   bool
			err       error
		)

		switch {
		case ctx.VMessage.GetImageMessage() != nil:
			mediaPath, isVideo, err = getWaMedia(ctx, false)
		case ctx.VMessage.GetVideoMessage() != nil:
			mediaPath, isVideo, err = getWaMedia(ctx, true)
		default:
			mediaPath, isVideo, err = getMediaFromUrl(ctx.MessageText)
		}

		if err != nil {
			ctx.Reply("Failed to process media")
			return
		}
		defer os.Remove(mediaPath)

		if utils.IsCanceledGoroutine(procCtx) { return }

		sendMediaAsSticker(procCtx, ctx, mediaPath, nocrop, isVideo)
	}()
}

func getWaMedia(ctx *context.MessageContext, isVideo bool) (string, bool, error) {
	data, err := ctx.GetDownloadableMedia(isVideo)

	if err != nil {
		return "", false, fmt.Errorf("download failed: %w", err)
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

func getMediaFromUrl(messageText string) (string, bool, error) {
	url, err := utils.GetLinkFromString(messageText)
	if err != nil {
		return "", false, fmt.Errorf("invalid URL: %w", err)
	}

	mediaPath, err := utils.DownloadMediaFromURL(url)
	if err != nil {
		return "", false, fmt.Errorf("failed to download from URL: %w", err)
	}

	mimeType, err := utils.GetMimeType(mediaPath)
	if err != nil {
		return "", false, fmt.Errorf("failed to get MIME type: %w", err)
	}

	isVideo := strings.HasPrefix(mimeType, "video/")
	return mediaPath, isVideo, nil
}

func sendMediaAsSticker(procCtx goctx.Context, ctx *context.MessageContext, mediaPath string, nocrop bool, isAnimated bool) {
	var err error

	if utils.IsCanceledGoroutine(procCtx) { return }

	webpPath, err := utils.ConvertToWebp(mediaPath, nocrop)
	if err != nil {
		return
	}
	defer os.Remove(webpPath)

	if utils.IsCanceledGoroutine(procCtx) { return }

	author := os.Getenv("APP_NAME")
	finalWebpPath, err := utils.WriteWebpExifFile(webpPath, "+62 812-3436-3620", author)
	if err != nil {
		ctx.Reply("Failed to embed metadata")
		return
	}
	defer os.Remove(finalWebpPath)

	if utils.IsCanceledGoroutine(procCtx) { return }

	webpData, err := os.ReadFile(finalWebpPath)
	if err != nil {
		fmt.Println("Failed to read WebP file with metadata:", err)
		return
	}

	uploaded, err := ctx.UploadToWhatsapp(webpData, "image")
	if err != nil {
		fmt.Println("Failed to upload sticker:", err)
		return
	}

	err = ctx.SendStickerMessage(uploaded, isAnimated)
	if err != nil {
		ctx.Reply("Failed to send sticker")
	}
}

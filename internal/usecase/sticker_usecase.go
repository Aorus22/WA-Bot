package usecase

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
	"time"

	waTypes "go.mau.fi/whatsmeow/types"

	"wa-bot/internal/domain/entity"
	"wa-bot/internal/domain/repository"
	"wa-bot/internal/domain/valueobject"
	"wa-bot/internal/infrastructure/media"
	"wa-bot/internal/infrastructure/whatsapp"
)

type StickerUseCase struct {
	waClient  *whatsapp.WhatsAppClient
	mediaDown *media.MediaDownloader
	stateRepo repository.UserStateRepository
	config    repository.ConfigRepository
}

func NewStickerUseCase(waClient *whatsapp.WhatsAppClient, mediaDown *media.MediaDownloader, stateRepo repository.UserStateRepository, config repository.ConfigRepository) *StickerUseCase {
	return &StickerUseCase{
		waClient:  waClient,
		mediaDown: mediaDown,
		stateRepo: stateRepo,
		config:    config,
	}
}

func (uc *StickerUseCase) ConvertToSticker(ctx context.Context, senderJID waTypes.JID, messageText string, role string, msg *entity.Message) error {
	if role != "OWNER" && role != "COMMON" {
		return uc.waClient.SendMessageToJID(ctx, senderJID, "Invalid Command", true)
	}

	uc.waClient.SendMessageToJID(ctx, senderJID, "⏳ Loading...", true)

	cancelCtx, cancel := context.WithCancel(context.Background())
	uc.stateRepo.AddUser(senderJID.String(), "processing", cancel)

	go func() {
		defer uc.stateRepo.ClearUserState(senderJID.String())
		defer cancel()

		opt, err := uc.parseStickerOptions(messageText)
		if err != nil {
			uc.waClient.SendMessageToJID(cancelCtx, senderJID, err.Error(), true)
			return
		}

		if opt.StartTime != "" && opt.EndTime != "" {
			if err := uc.validateTimeRange(opt); err != nil {
				uc.waClient.SendMessageToJID(cancelCtx, senderJID, err.Error(), true)
				return
			}
		}

		mediaPath, isAnimated, err := uc.getMedia(cancelCtx, senderJID, messageText, msg)
		defer os.Remove(mediaPath)
		if err != nil {
			uc.handleMediaError(cancelCtx, senderJID, err)
			return
		}
		opt.IsAnimated = isAnimated

		if !uc.validateVideoDuration(cancelCtx, senderJID, mediaPath, opt) {
			return
		}

		webpPath, err := uc.mediaDown.ConvertToWebP(cancelCtx, mediaPath, opt)
		defer os.Remove(webpPath)
		if err != nil {
			if errors.Is(err, valueobject.ErrNotUnder1MB) {
				uc.waClient.SendMessageToJID(cancelCtx, senderJID,
					"Failed to convert media under 1MB. Consider trying one of the following:\n"+
						"- Lower the quality with: quality=<0-100>\n"+
						"- Reduce the video duration: start=MM:SS end=MM:SS\n"+
						"- Reduce the video FPS: fps=<1-60>",
					true,
				)
				return
			}
			uc.waClient.SendMessageToJID(cancelCtx, senderJID, "Server error: failed to convert sticker", true)
			return
		}

		author := os.Getenv("APP_NAME")
		finalWebpPath, err := uc.mediaDown.WriteWebpExif(cancelCtx, webpPath, "+62 812-3436-3620", author)
		defer os.Remove(finalWebpPath)
		if err != nil {
			uc.waClient.SendMessageToJID(cancelCtx, senderJID, "Server error: failed to write EXIF", true)
			return
		}

		webpData, err := os.ReadFile(finalWebpPath)
		if err != nil {
			uc.waClient.SendMessageToJID(cancelCtx, senderJID, "Server error: failed to read sticker data", true)
			return
		}

		// Save to persistent media folder for frontend display
		os.MkdirAll("media", 0755)
		filename := fmt.Sprintf("sent_sticker_%d.webp", time.Now().UnixMilli())
		persistedPath := filepath.Join("media", filename)
		mediaURL := fmt.Sprintf("/media/%s", filename)
		
		if err := os.WriteFile(persistedPath, webpData, 0644); err != nil {
			fmt.Printf("Failed to persist sent sticker: %v\n", err)
			mediaURL = "" // Fallback to empty if save fails
		}

		err = uc.waClient.SendStickerToJID(cancelCtx, senderJID, webpData, opt.IsAnimated, mediaURL, true)
		if err != nil {
			uc.waClient.SendMessageToJID(cancelCtx, senderJID, "Server error: failed to send sticker", true)
			return
		}
	}()

	return nil
}

func (uc *StickerUseCase) parseStickerOptions(messageText string) (*valueobject.StickerOptions, error) {
	opt := &valueobject.StickerOptions{}
	var err error
	opt.NoCrop = strings.Contains(strings.ToLower(messageText), " nocrop")

	parts := strings.Fields(messageText)
	for _, part := range parts {
		switch {
		case strings.HasPrefix(part, "start="):
			opt.StartTime = strings.TrimPrefix(part, "start=")
		case strings.HasPrefix(part, "end="):
			opt.EndTime = strings.TrimPrefix(part, "end=")
		case strings.HasPrefix(part, "fps="):
			fpsStr := strings.TrimPrefix(part, "fps=")
			opt.FPS, err = strconv.Atoi(fpsStr)
			if err != nil || opt.FPS < 1 || opt.FPS > 60 {
				return nil, errors.New("FPS must be between 1 and 60")
			}
		case strings.HasPrefix(part, "quality="):
			qualityStr := strings.TrimPrefix(part, "quality=")
			opt.Quality, err = strconv.Atoi(qualityStr)
			if err != nil || opt.Quality < 1 || opt.Quality > 100 {
				return nil, errors.New("Quality must be between 1 and 100")
			}
		case strings.HasPrefix(part, "direction="):
			rawDirection := strings.TrimPrefix(part, "direction=")
			dParts := strings.Split(rawDirection, "-")
			side := dParts[0]
			if side != "up" && side != "down" && side != "left" && side != "right" {
				return nil, errors.New("Direction invalid. Use up, down, left, or right (with optional -0 to -50)")
			}
			if len(dParts) == 2 {
				percentStr := dParts[1]
				percent, convErr := strconv.Atoi(percentStr)
				if convErr != nil || percent < 0 || percent > 50 {
					return nil, errors.New("Direction offset must be between 0 and 50")
				}
			}
			opt.Direction = rawDirection
		}
	}

	return opt, nil
}

func (uc *StickerUseCase) validateTimeRange(opt *valueobject.StickerOptions) error {
	if opt.StartTime == "" && opt.EndTime != "" {
		return errors.New("End Time given, but Start Time not")
	}
	if (opt.StartTime != "" && !valueobject.IsValidTimeFormat(opt.StartTime)) ||
		(opt.EndTime != "" && !valueobject.IsValidTimeFormat(opt.EndTime)) {
		return errors.New("Invalid time format. Use MM:SS, e.g., start=00:10 end=00:20")
	}
	if valueobject.ParseTimeFromString(opt.StartTime) >= valueobject.ParseTimeFromString(opt.EndTime) {
		return errors.New("Start time must be earlier than end time")
	}
	return nil
}

func (uc *StickerUseCase) getMedia(ctx context.Context, senderJID waTypes.JID, messageText string, msg *entity.Message) (string, bool, error) {
	if msg.VMessage.GetImageMessage() != nil || msg.VMessage.GetVideoMessage() != nil {
		return uc.getWaMedia(ctx, msg)
	}
	return uc.getMediaFromUrl(ctx, messageText)
}

func (uc *StickerUseCase) getWaMedia(ctx context.Context, msg *entity.Message) (string, bool, error) {
	data, isAnimated, err := uc.waClient.DownloadMedia(ctx, msg)
	if err != nil {
		return "", false, err
	}

	mediaPath := fmt.Sprintf("media/%d", time.Now().UnixMilli())
	if err := os.WriteFile(mediaPath, data, 0644); err != nil {
		return "", false, err
	}

	return mediaPath, isAnimated, nil
}

func (uc *StickerUseCase) getMediaFromUrl(ctx context.Context, messageText string) (string, bool, error) {
	url, err := valueobject.GetLinkFromString(messageText)
	if err != nil {
		return "", false, errors.New("no link provided")
	}

	page := func() int {
		re := regexp.MustCompile(`\s+page=(\d+)(\s+|$)`)
		matches := re.FindStringSubmatch(messageText)
		if len(matches) < 2 {
			return 0
		}
		num, err := strconv.Atoi(matches[1])
		if err != nil {
			return 0
		}
		return num
	}()

	if strings.Contains(url, "instagram.com") {
		url, err = uc.mediaDown.GetInstagramDirectURL(url, page)
		if err != nil {
			return "", false, err
		}
	}

	mediaPath, mimeType, err := uc.mediaDown.DownloadFromURL(ctx, url)
	if err != nil {
		return "", false, err
	}

	isAnimated := strings.HasPrefix(mimeType, "video/") || strings.Contains(mimeType, "gif")
	return mediaPath, isAnimated, nil
}

func (uc *StickerUseCase) validateVideoDuration(ctx context.Context, senderJID waTypes.JID, mediaPath string, opt *valueobject.StickerOptions) bool {
	if opt.StartTime == "" {
		return true
	}

	duration, err := uc.mediaDown.GetDuration(mediaPath)
	if err != nil {
		if errors.Is(err, valueobject.ErrNotVideo) {
			uc.waClient.SendMessageToJID(ctx, senderJID, "Not a video but given start time", true)
		} else {
			uc.waClient.SendMessageToJID(ctx, senderJID, "Server error: failed to get duration", true)
		}
		return false
	}

	start := valueobject.ParseTimeFromString(opt.StartTime)
	end := valueobject.ParseTimeFromString(opt.EndTime)

	if start > duration {
		uc.waClient.SendMessageToJID(ctx, senderJID, fmt.Sprintf("Start Time (%.0fs) exceeds media duration (%.0fs)", start, duration), true)
		return false
	}
	if opt.EndTime != "" && end > duration {
		uc.waClient.SendMessageToJID(ctx, senderJID, fmt.Sprintf("End Time (%.0fs) exceeds media duration (%.0fs)", end, duration), true)
		return false
	}

	return true
}

func (uc *StickerUseCase) handleMediaError(ctx context.Context, senderJID waTypes.JID, err error) error {
	switch {
	case errors.Is(err, valueobject.ErrNotSupportedLink):
		uc.waClient.SendMessageToJID(ctx, senderJID, "Link not supported", true)
	case errors.Is(err, valueobject.ErrNoLinkProvided):
		uc.waClient.SendMessageToJID(ctx, senderJID, "No Link Provided", true)
	case errors.Is(err, valueobject.ErrPageNumberExceeded):
		uc.waClient.SendMessageToJID(ctx, senderJID, "Page Number Exceed the Available Pages", true)
	case errors.Is(err, valueobject.ErrPageNumberNotGiven):
		uc.waClient.SendMessageToJID(ctx, senderJID, "No Page Number Given, type page=<number>", true)
	default:
		uc.waClient.SendMessageToJID(ctx, senderJID, "Invalid Media / Link", true)
	}
	return err
}

package commonHandlers

import (
	"context"
	"errors"
	"fmt"
	"os"
	"regexp"
	"strconv"
	"strings"
	"time"

	"wa-bot/state"
	"wa-bot/utils"
)

func StickerHandler(s *state.MessageState) {
	if s.UserRole != "OWNER" && s.UserRole != "COMMON" {
		s.Reply("Invalid Command")
		return
	}
	s.Reply("⏳ Loading...")

	ctx, cancel := context.WithCancel(context.Background())
	s.AddUserToState("processing", cancel)

	go func() {
		defer s.ClearUserState()
		defer cancel()

		opt, err := parseStickerOptions(s.MessageText)
		if err != nil {
			s.Reply(err.Error())
			return
		}

		if opt.StartTime != "" && opt.EndTime != "" {
			if err := validateTimeRange(opt); err != nil {
				s.Reply(err.Error())
				return
			}
		}

		mediaPath, isVideo, err := getMedia(ctx, s, s.MessageText)
		defer os.Remove(mediaPath)
		if err != nil {
			handleMediaError(ctx, s, err)
			return
		}
		opt.IsVideo = isVideo

		if utils.IsCanceledGoroutine(ctx) {
			return
		}

		if !validateVideoDuration(ctx, s, mediaPath, opt) {
			return
		}

		if err := sendMediaAsSticker(ctx, s, mediaPath, opt); err != nil {
			if errors.Is(err, utils.ErrorNotUnder1MB) {
				s.Reply(
					"Failed to convert media under 1MB. Consider trying one of the following:\n" +
					"- Lower the quality with: quality=<0-100>\n" +
					"- Reduce the video duration: start=MM:SS end=MM:SS\n" +
					"- Reduce the video FPS: fps=<1-60>",
				)
			} else {
				utils.LogNoCancelErr(ctx, err, "error:")
				s.ReplyNoCancelError(ctx, err, "Server error: failed to convert sticker")
			}
		}
	}()
}

func parseStickerOptions(messageText string) (*utils.StickerOptions, error) {
	opt := &utils.StickerOptions{}
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

func validateTimeRange(opt *utils.StickerOptions) error {
	if opt.StartTime == "" && opt.EndTime != "" {
		return errors.New("End Time given, but Start Time not")
	}
	if (opt.StartTime != "" && !utils.IsValidTimeFormat(opt.StartTime)) ||
		(opt.EndTime != "" && !utils.IsValidTimeFormat(opt.EndTime)) {
		return errors.New("Invalid time format. Use MM:SS, e.g., start=00:10 end=00:20")
	}
	if utils.ParseTimeFromString(opt.StartTime) >= utils.ParseTimeFromString(opt.EndTime) {
		return errors.New("Start time must be earlier than end time")
	}
	return nil
}

func getMedia(ctx context.Context, s *state.MessageState, messageText string) (string, bool, error) {
	if s.VMessage.GetImageMessage() != nil || s.VMessage.GetVideoMessage() != nil {
		return getWaMedia(s)
	}
	return getMediaFromUrl(ctx, messageText)
}

func validateVideoDuration(ctx context.Context, s *state.MessageState, path string, opt *utils.StickerOptions) bool {
	if opt.StartTime == "" {
		return true
	}

	duration, err := utils.GetMediaDuration(path)
	if err != nil {
		if errors.Is(err, utils.ErrorNotVideo) {
			s.ReplyNoCancelError(ctx, err, "Not a video but given start time")
		} else {
			s.ReplyNoCancelError(ctx, err, "Server error: failed to convert sticker")
		}
		utils.LogNoCancelErr(ctx, err, "error:")
		return false
	}

	start := utils.ParseTimeFromString(opt.StartTime)
	end := utils.ParseTimeFromString(opt.EndTime)

	if start > duration {
		s.Reply(fmt.Sprintf("Start Time (%.0fs) exceeds media duration (%.0fs)", start, duration))
		return false
	}
	if opt.EndTime != "" && end > duration {
		s.Reply(fmt.Sprintf("End Time (%.0fs) exceeds media duration (%.0fs)", end, duration))
		return false
	}

	return true
}

func handleMediaError(ctx context.Context, s *state.MessageState, err error) {
	utils.LogNoCancelErr(ctx, err, "Error getting media:")
	switch {
	case errors.Is(err, ErrorNotSupportedLink):
		s.ReplyNoCancelError(ctx, err, "Link not supported")
	case errors.Is(err, ErrorNoLinkProvided):
		s.ReplyNoCancelError(ctx, err, "No Link Provided")
	case errors.Is(err, utils.ErrorPageNumberExceeded):
		s.ReplyNoCancelError(ctx, err, "Page Number Exceed the Available Pages")
	case errors.Is(err, utils.ErrorPageNumberNotGiven):
		s.ReplyNoCancelError(ctx, err, "No Page Number Given, type page=<number>")
	default:
		s.ReplyNoCancelError(ctx, err, "Invalid Media / Link")
	}
}

func getWaMedia(s *state.MessageState) (string, bool, error) {
	data, isVideo, err := s.GetDownloadableMedia()
	if err != nil {
		return "", false, err
	}

	mediaPath := fmt.Sprintf("media/%d", time.Now().UnixMilli())

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
	}

	if strings.Contains(url, "instagram.com") {
		igpage := page()
		url, err = utils.GetInstagramDirectURL(url, igpage)
		if err != nil {
			return "", false, err
		}
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

func sendMediaAsSticker(ctx context.Context, s *state.MessageState, mediaPath string, opt *utils.StickerOptions) error {
	var err error

	webpPath, err := utils.ConvertToWebp(ctx, mediaPath, opt)
	defer os.Remove(webpPath)
	if err != nil {
		if errors.Is(err, utils.ErrorNotUnder1MB) {
			return err
		} else {
			return fmt.Errorf("convert to WebP: %w", err)
		}
	}

	author := os.Getenv("APP_NAME")
	finalWebpPath, err := utils.WriteWebpExifFile(ctx, webpPath, "+62 812-3436-3620", author)
	defer os.Remove(finalWebpPath)
	if err != nil {
		return fmt.Errorf("write EXIF: %w", err)
	}

	webpData, err := os.ReadFile(finalWebpPath)
	if utils.IsCanceledGoroutine(ctx) { return nil }
	if err != nil {
		return fmt.Errorf("read WebP: %w", err)
	}

	uploadedData, err := s.UploadToWhatsapp(ctx, webpData, "image")
	if err != nil {
		return fmt.Errorf("upload to WhatsApp: %w", err)
	}

	err = s.SendStickerMessage(ctx, uploadedData, opt.IsVideo)
	if err != nil {
		return fmt.Errorf("send sticker: %w", err)
	}

	return nil
}

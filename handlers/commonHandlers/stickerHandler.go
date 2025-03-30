package commonHandlers

import (
	"context"
	"errors"
	"fmt"
	"os"
	"strconv"
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

		var mediaPath string
		opt := &utils.StickerOptions{}
		var err error

		opt.NoCrop = strings.Contains(strings.ToLower(s.MessageText), "nocrop")

		parts := strings.Fields(s.MessageText)
		for _, part := range parts {
			if strings.HasPrefix(part, "start=") {
				opt.StartTime = strings.TrimPrefix(part, "start=")
			}
			if strings.HasPrefix(part, "end=") {
				opt.EndTime = strings.TrimPrefix(part, "end=")
			}
			if strings.HasPrefix(part, "fps=") {
				fpsStr := strings.TrimPrefix(part, "fps=")
				opt.FPS, err = strconv.Atoi(fpsStr)
				if err != nil || opt.FPS < 1 || opt.FPS > 60 {
					s.Reply("FPS must be between 1 and 60")
					return
				}
			}
			if strings.HasPrefix(part, "direction=") {
				rawDirection := strings.TrimPrefix(part, "direction=")
				dParts := strings.Split(rawDirection, "-")
				side := dParts[0]

				if side != "up" && side != "down" && side != "left" && side != "right" {
					s.Reply("Direction invalid. Use up, down, left, or right (with optional -0 to -50)")
					return
				}

				if len(dParts) == 2 {
					percentStr := dParts[1]
					if _, err := strconv.Atoi(percentStr); err != nil {
						s.Reply("Invalid direction format. Use for example: right or right-25")
						return
					}

					percent, _ := strconv.Atoi(percentStr)
					if percent < 0 || percent > 50 {
						s.Reply("Direction offset must be between 0 and 50")
						return
					}
				}

				opt.Direction = rawDirection
			}
		}

		if opt.StartTime == "" && opt.EndTime != "" {
			s.Reply("End Time given, but Start Time not")
			return
		}

		if (opt.StartTime != "" && !utils.IsValidTimeFormat(opt.StartTime)) || (opt.EndTime != "" && !utils.IsValidTimeFormat(opt.EndTime)) {
			s.Reply("Invalid time format. Use MM:SS, e.g., start=00:10 end=00:20")
			return
		}

		if opt.StartTime != "" && opt.EndTime != "" {
			if utils.ParseTimeFromString(opt.StartTime) >= utils.ParseTimeFromString(opt.EndTime) {
				s.Reply("Start time must be earlier than end time")
				return
			}
		}


		if s.VMessage.GetImageMessage() != nil || s.VMessage.GetVideoMessage() != nil {
			mediaPath, opt.IsVideo, err = getWaMedia(s)
		} else {
			mediaPath, opt.IsVideo, err = getMediaFromUrl(ctx, s.MessageText)
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

		if opt.StartTime != "" {
			duration, err := utils.GetMediaDuration(mediaPath)
			if err != nil {
				if errors.Is(err, utils.ErrorNotVideo) {
					s.ReplyNoCancelError(ctx, err, "Not a video but given start time")
				} else {
					s.ReplyNoCancelError(ctx, err, "Server error: failed to convert sticker")
				}
				utils.LogNoCancelErr(ctx, err, "error:")

				return
			}

			startTimeInSecond :=  utils.ParseTimeFromString(opt.StartTime)
			endTimeInSecond := utils.ParseTimeFromString(opt.EndTime)

			if startTimeInSecond > duration {
				s.Reply(fmt.Sprintf("Start Time (%.0fs) exceeds media duration (%.0fs)", startTimeInSecond, duration))
				return
			}

			if opt.EndTime != "" && utils.ParseTimeFromString(opt.EndTime) > duration {
				s.Reply(fmt.Sprintf("End Time (%.0fs) exceeds media duration (%.0fs)", endTimeInSecond, duration))
				return
			}
		}

		err = sendMediaAsSticker(ctx, s, mediaPath, opt)
		if err != nil {
			utils.LogNoCancelErr(ctx, err, "error:")
			s.ReplyNoCancelError(ctx, err, "Server error: failed to convert sticker")
		}
	}()
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

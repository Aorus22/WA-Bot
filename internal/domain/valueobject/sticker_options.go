package valueobject

import (
	"errors"
	"regexp"
	"strconv"
	"strings"
)

var (
	ErrNotUnder1MB        = errors.New("file size not under 1MB")
	ErrNotSupportedLink   = errors.New("link not supported")
	ErrNoLinkProvided     = errors.New("no link provided")
	ErrPageNumberExceeded = errors.New("page number exceeded")
	ErrPageNumberNotGiven = errors.New("page number not given")
	ErrNotVideo           = errors.New("not a video")
)

type StickerOptions struct {
	NoCrop     bool
	Quality    int
	StartTime  string
	EndTime    string
	Direction  string
	FPS        int
	IsAnimated bool
}

func NewStickerOptions() *StickerOptions {
	return &StickerOptions{
		Quality: 100,
		FPS:     15,
	}
}

func IsValidTimeFormat(timeStr string) bool {
	parts := strings.Split(timeStr, ":")
	if len(parts) != 2 {
		return false
	}

	_, err := strconv.Atoi(parts[0])
	if err != nil {
		return false
	}

	_, err = strconv.Atoi(parts[1])
	if err != nil {
		return false
	}

	return true
}

func ParseTimeFromString(timeStr string) float64 {
	if timeStr == "" {
		return 0
	}

	parts := strings.Split(timeStr, ":")
	if len(parts) != 2 {
		return 0
	}

	minutes, err := strconv.ParseFloat(parts[0], 64)
	if err != nil {
		return 0
	}

	seconds, err := strconv.ParseFloat(parts[1], 64)
	if err != nil {
		return 0
	}

	return minutes*60 + seconds
}

func GetLinkFromString(messageText string) (string, error) {
	re := regexp.MustCompile(`https?://[^\s]+`)
	matches := re.FindStringSubmatch(messageText)

	if len(matches) == 0 {
		return "", ErrNoLinkProvided
	}

	return matches[0], nil
}

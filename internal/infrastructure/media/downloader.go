package media

import (
	"context"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"os/exec"
	"regexp"
	"strconv"
	"strings"
	"time"

	"github.com/aorus22/instagramdl"

	"wa-bot/internal/domain/valueobject"
)

type MediaDownloader struct {
	converter *FFmpegConverter
}

func NewMediaDownloader() *MediaDownloader {
	return &MediaDownloader{
		converter: NewFFmpegConverter(),
	}
}

var ErrorNotSupportedLink = errors.New("link not supported")

func (m *MediaDownloader) DownloadFromURL(ctx context.Context, url string) (string, string, error) {
	currentTime := fmt.Sprintf("%d", time.Now().UnixMilli())
	mediaPath := "media/" + currentTime

	tryCommands := []struct {
		cmd *exec.Cmd
	}{
		{
			cmd: exec.CommandContext(ctx, "yt-dlp",
				"-o", mediaPath,
				"--no-playlist",
				"-f", "best",
				url,
			),
		},
		{
			cmd: exec.CommandContext(ctx, "gallery-dl",
				"-D", "media",
				"-f", currentTime,
				url,
			),
		},
	}

	for _, tc := range tryCommands {
		err := tc.cmd.Run()
		if err != nil {
			continue
		}

		mimeType, err := m.converter.GetMimeType(mediaPath)
		if err != nil {
			os.Remove(mediaPath)
			continue
		}

		if strings.HasPrefix(mimeType, "image/") || strings.HasPrefix(mimeType, "video/") {
			return mediaPath, mimeType, nil
		} else {
			os.Remove(mediaPath)
		}
	}

	req, err := http.NewRequestWithContext(ctx, "GET", url, nil)
	if err != nil {
		return mediaPath, "", err
	}

	client := &http.Client{}
	resp, err := client.Do(req)
	if err != nil {
		return mediaPath, "", err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return mediaPath, "", fmt.Errorf("failed to fetch media, status: %d", resp.StatusCode)
	}

	file, err := os.Create(mediaPath)
	if err != nil {
		return mediaPath, "", err
	}
	defer file.Close()

	_, err = io.Copy(file, resp.Body)
	if err != nil {
		return mediaPath, "", err
	}

	mimeType, err := m.converter.GetMimeType(mediaPath)
	if err != nil {
		os.Remove(mediaPath)
		return mediaPath, "", err
	}

	if !strings.HasPrefix(mimeType, "image/") && !strings.HasPrefix(mimeType, "video/") {
		os.Remove(mediaPath)
		return mediaPath, "", ErrorNotSupportedLink
	}

	return mediaPath, mimeType, nil
}

func (m *MediaDownloader) GetInstagramDirectURL(url string, page int) (string, error) {
	urls, err := instagramdl.GetInstagramMediaURLs(url)
	if err != nil || len(urls) == 0 {
		return "", fmt.Errorf("failed to get direct url")
	}

	if len(urls) > 1 {
		if page <= 0 {
			return "", errors.New("no instagram page number given")
		}
		if page > len(urls) {
			return "", errors.New("given page exceeded")
		}
		return urls[page-1], nil
	}

	return urls[0], nil
}

func (m *MediaDownloader) GetLinkFromString(input string) (string, error) {
	urlRegex := regexp.MustCompile(`^(https?:\/\/)?([\w-]+\.)+[\w-]+(:\d+)?(\/[\w\-\.~!*'();:@&=+$,/?%#]*)?$`)
	words := strings.Split(input, " ")
	for _, word := range words {
		if urlRegex.MatchString(word) {
			return word, nil
		}
	}
	return "", fmt.Errorf("no link found / invalid link")
}

func (m *MediaDownloader) GetPageFromMessage(messageText string) int {
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

func (m *MediaDownloader) GetDuration(path string) (float64, error) {
	return m.converter.GetDuration(path)
}

func (m *MediaDownloader) GetMimeType(filePath string) (string, error) {
	return m.converter.GetMimeType(filePath)
}

func (m *MediaDownloader) IsValidTimeFormat(t string) bool {
	return m.converter.IsValidTimeFormat(t)
}

func (m *MediaDownloader) ParseTimeFromString(t string) float64 {
	return m.converter.ParseTimeFromString(t)
}

func (m *MediaDownloader) WriteWebpExif(ctx context.Context, input, packName, author string) (string, error) {
	return m.converter.WriteWebpExif(ctx, input, packName, author)
}

func (m *MediaDownloader) WriteWebpExifBytes(ctx context.Context, input []byte, packName, author string) ([]byte, error) {
	return input, nil
}

func (m *MediaDownloader) ConvertToWebP(ctx context.Context, input string, opt *valueobject.StickerOptions) (string, error) {
	return m.converter.ConvertToWebP(ctx, input, opt)
}

func (m *MediaDownloader) ConvertToWebpBytes(ctx context.Context, input []byte, opt *valueobject.StickerOptions) ([]byte, error) {
	return input, nil
}

func (m *MediaDownloader) GetMediaDuration(media []byte) (float64, error) {
	return 0, nil
}

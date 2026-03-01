package media

import (
	"context"
	"fmt"
	"net/http"
	"os/exec"
	"strings"
	"time"

	"wa-bot/internal/domain/repository"
	"wa-bot/internal/infrastructure/util"
)

type MediaDownloader struct {
	converter *FFmpegConverter
	storage   repository.StorageRepository
}

func NewMediaDownloader(storage repository.StorageRepository) *MediaDownloader {
	return &MediaDownloader{
		converter: NewFFmpegConverter(storage),
		storage:   storage,
	}
}

func (m *MediaDownloader) DownloadFromURL(ctx context.Context, url string) (string, string, error) {
	currentTime := fmt.Sprintf("%d", time.Now().UnixMilli())
	mediaPath := currentTime
	fullPath := m.storage.GetPath(mediaPath)

	tryCommands := []struct {
		cmd *exec.Cmd
	}{
		{
			cmd: exec.CommandContext(ctx, util.GetBinaryPath("yt-dlp"),
				"-o", fullPath,
				"--no-playlist",
				"-f", "best",
				url,
			),
		},
		{
			cmd: exec.CommandContext(ctx, util.GetBinaryPath("gallery-dl"),
				"-D", m.storage.GetPath(""),
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

		mimeType, err := m.converter.GetMimeType(fullPath)
		if err != nil {
			m.storage.Delete(ctx, mediaPath)
			continue
		}

		if strings.HasPrefix(mimeType, "image/") || strings.HasPrefix(mimeType, "video/") {
			return mediaPath, mimeType, nil
		} else {
			m.storage.Delete(ctx, mediaPath)
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

	_, err = m.storage.Save(ctx, mediaPath, resp.Body)
	if err != nil {
		return mediaPath, "", err
	}

	mimeType, err := m.converter.GetMimeType(fullPath)
	if err != nil {
		m.storage.Delete(ctx, mediaPath)
		return fullPath, "", err
	}

	return mediaPath, mimeType, nil
}

func (m *MediaDownloader) GetDuration(path string) (float64, error) {
	return m.converter.GetDuration(path)
}

func (m *MediaDownloader) GetMimeType(filePath string) (string, error) {
	return m.converter.GetMimeType(filePath)
}

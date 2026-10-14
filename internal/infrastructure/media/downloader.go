package media

import (
	"context"
	"fmt"
	"io"
	"io/fs"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
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

// DownloadFromURL fetches url into the media storage and returns the key it was
// stored under and its MIME type. The external downloaders write into a private
// scratch folder; only a verified image or video is copied into the storage.
func (m *MediaDownloader) DownloadFromURL(ctx context.Context, url string) (string, string, error) {
	currentTime := fmt.Sprintf("%d", time.Now().UnixMilli())
	mediaPath := currentTime

	scratch, cleanup, err := m.storage.WorkTemp("download")
	if err != nil {
		return mediaPath, "", err
	}
	defer cleanup()
	outPath := filepath.Join(scratch, currentTime)

	tryCommands := []*exec.Cmd{
		exec.CommandContext(ctx, util.GetBinaryPath("yt-dlp"),
			"-o", outPath,
			"--no-playlist",
			"-f", "best",
			url,
		),
		exec.CommandContext(ctx, util.GetBinaryPath("gallery-dl"),
			"-D", scratch,
			"-f", currentTime,
			url,
		),
	}

	for _, cmd := range tryCommands {
		if err := cmd.Run(); err != nil {
			continue
		}
		produced := firstFile(scratch)
		if produced == "" {
			continue
		}
		mimeType, err := m.converter.GetMimeType(produced)
		if err != nil || !(strings.HasPrefix(mimeType, "image/") || strings.HasPrefix(mimeType, "video/")) {
			os.Remove(produced)
			continue
		}
		if err := m.saveFile(ctx, mediaPath, produced); err != nil {
			return mediaPath, "", err
		}
		return mediaPath, mimeType, nil
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

	if _, err = m.storage.Save(ctx, mediaPath, resp.Body); err != nil {
		return mediaPath, "", err
	}

	// The type is read from the stored object's first bytes.
	reader, err := m.storage.Get(ctx, mediaPath)
	if err != nil {
		return mediaPath, "", err
	}
	head := make([]byte, 512)
	n, _ := io.ReadFull(reader, head)
	reader.Close()
	if n == 0 {
		m.storage.Delete(ctx, mediaPath)
		return mediaPath, "", fmt.Errorf("downloaded file is empty")
	}
	return mediaPath, http.DetectContentType(head[:n]), nil
}

func (m *MediaDownloader) saveFile(ctx context.Context, key, file string) error {
	f, err := os.Open(file)
	if err != nil {
		return err
	}
	defer f.Close()
	_, err = m.storage.Save(ctx, key, f)
	return err
}

// firstFile is the first regular file found under dir ("" if none).
func firstFile(dir string) string {
	found := ""
	_ = filepath.WalkDir(dir, func(p string, d fs.DirEntry, err error) error {
		if err == nil && found == "" && d.Type().IsRegular() {
			found = p
		}
		return nil
	})
	return found
}

func (m *MediaDownloader) GetDuration(path string) (float64, error) {
	return m.converter.GetDuration(path)
}

func (m *MediaDownloader) GetMimeType(filePath string) (string, error) {
	return m.converter.GetMimeType(filePath)
}

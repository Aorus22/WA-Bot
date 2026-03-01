package media

import (
	"errors"
	"net/http"
	"os"
	"os/exec"
	"strconv"
	"strings"

	"wa-bot/internal/domain/repository"
	"wa-bot/internal/infrastructure/util"
)

type FFmpegConverter struct {
	storage repository.StorageRepository
}

func NewFFmpegConverter(storage repository.StorageRepository) *FFmpegConverter {
	return &FFmpegConverter{
		storage: storage,
	}
}

func (f *FFmpegConverter) GetDuration(filePath string) (float64, error) {
	cmd := exec.Command(util.GetBinaryPath("ffprobe"), "-v", "error", "-show_entries", "format=duration",
		"-of", "default=noprint_wrappers=1:nokey=1", filePath)

	output, err := cmd.Output()
	if err != nil {
		return 0, err
	}

	durationStr := strings.TrimSpace(string(output))
	duration, err := strconv.ParseFloat(durationStr, 64)
	if err != nil {
		return 0, errors.New("not a video or media file")
	}

	return duration, nil
}

func (f *FFmpegConverter) GetMimeType(filePath string) (string, error) {
	file, err := os.Open(filePath)
	if err != nil {
		return "", err
	}
	defer file.Close()

	// Only the first 512 bytes are used to sniff the content type.
	buffer := make([]byte, 512)
	_, err = file.Read(buffer)
	if err != nil {
		return "", err
	}

	contentType := http.DetectContentType(buffer)
	return contentType, nil
}

package utils

import (
	"bytes"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"time"
)

func ConvertToWebp(mediaPath string, nocrop bool) (string, error) {
	defer os.Remove(mediaPath)

	ffmpegExec, err := GetFFMPEGExecutable()
	if err != nil {
		return "", err
	}

	webpPath := filepath.Join("media", fmt.Sprintf("output_%d.webp", time.Now().UnixMilli()))

	var cmd *exec.Cmd
	qualityLevels := []int{80, 60, 40, 20}

	for _, quality := range qualityLevels {
		if nocrop {
			cmd = exec.Command(
				ffmpegExec,
				"-i", mediaPath,
				"-vf", "scale=512:512:force_original_aspect_ratio=decrease,pad=512:512:(ow-iw)/2:(oh-ih)/2:color=0x00000000@0",
				"-quality", fmt.Sprintf("%d", quality),
				"-pix_fmt", "rgba",
				"-y", webpPath,
			)
		} else {
			cmd = exec.Command(
				ffmpegExec,
				"-i", mediaPath,
				"-vf", "crop=min(iw\\,ih):min(iw\\,ih),scale=512:512",
				"-quality", fmt.Sprintf("%d", quality),
				"-pix_fmt", "rgba",
				"-y", webpPath,
			)
		}

		var stderr bytes.Buffer
		cmd.Stderr = &stderr
		err := cmd.Run()
		if err != nil {
			fmt.Println("FFmpeg failed:", stderr.String())
			continue
		}

		info, err := os.Stat(webpPath)
		if err == nil && info.Size() <= 1024*1024 {
			return webpPath, nil
		}
	}

	os.Remove(webpPath)
	return "", fmt.Errorf("failed to convert to webp under 1MB")
}

package utils

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"
)

func ConvertToWebp(ctx context.Context, mediaPath string, nocrop bool) (string, error) {
	defer os.Remove(mediaPath)

	ffmpegExec, err := GetFFMPEGExecutable()
	if err != nil {
		return "", err
	}

	webpPath := filepath.Join("media", fmt.Sprintf("output_%d.webp", time.Now().UnixMilli()))
	qualityLevels := []int{80, 50, 20, 5}

	mimeType, err := GetMimeType(mediaPath)
	if err != nil {
		return "", fmt.Errorf("failed to get MIME type: %w", err)
	}
	isImage := strings.HasPrefix(mimeType, "image/")

	for _, quality := range qualityLevels {
		var args []string
		args = append(args, "-i", mediaPath)

		if !isImage {
			if nocrop {
				args = append(args, "-vf", "fps=10,scale=512:512:force_original_aspect_ratio=decrease,pad=512:512:(ow-iw)/2:(oh-ih)/2:color=0x00000000@0")
			} else {
				args = append(args, "-vf", "fps=10,crop=min(iw\\,ih):min(iw\\,ih),scale=512:512")
			}
			args = append(args, "-t", "30")
		} else {
			if nocrop {
				args = append(args, "-vf", "scale=512:512:force_original_aspect_ratio=decrease,pad=512:512:(ow-iw)/2:(oh-ih)/2:color=0x00000000@0")
			} else {
				args = append(args, "-vf", "crop=min(iw\\,ih):min(iw\\,ih),scale=512:512")
			}
		}

		args = append(args,
			"-quality", fmt.Sprintf("%d", quality),
			"-pix_fmt", "rgba",
			"-y", webpPath,
		)

		cmd := exec.CommandContext(ctx, ffmpegExec, args...)

		var stderr bytes.Buffer
		cmd.Stderr = &stderr

		err := cmd.Run()
		if err != nil {
			if errors.Is(err, context.Canceled) || errors.Is(err, context.DeadlineExceeded) {
				return "", err
			} else {
				fmt.Println("FFmpeg failed:", stderr.String())
				continue
			}
		}

		info, err := os.Stat(webpPath)
		if err == nil && info.Size() <= 1024*1024 {
			return webpPath, nil
		}
	}

	return "", fmt.Errorf("failed to convert to webp under 1MB")
}

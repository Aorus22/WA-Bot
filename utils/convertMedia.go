package utils

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"time"
)

func ConvertToWebp(ctx context.Context, isVideo bool, mediaPath string, nocrop bool, startTime string, endTime string, direction string, fps int) (string, error) {
	webpPath := filepath.Join("media", fmt.Sprintf("output_%d.webp", time.Now().UnixMilli()))
	qualityLevels := []int{80, 50, 20, 5, 1}

	if fps == 0 {
		fps = 15
	}

	getCropFilter := func() string {
		baseCrop := "crop=min(iw\\,ih):min(iw\\,ih)"
		switch direction {
		case "up":
			return baseCrop + ":0:0"
		case "down":
			return baseCrop + ":0:(ih-min(iw\\,ih))"
		case "left":
			return baseCrop + ":0:0"
		case "right":
			return baseCrop + ":(iw-min(iw\\,ih)):0"
		default:
			return baseCrop
		}
	}

	for _, quality := range qualityLevels {
		var args []string
		args = append(args, "-i", mediaPath)

		if isVideo {
			if startTime != "" {
				args = append(args, "-ss", startTime)
			}
			if endTime != "" {
				args = append(args, "-to", endTime)
			} else {
				args = append(args, "-t", "30")
			}
			if nocrop {
				args = append(args, "-vf", fmt.Sprintf("fps=%d,%s", fps,
					"scale=512:512:force_original_aspect_ratio=decrease,pad=512:512:(ow-iw)/2:(oh-ih)/2:color=0x00000000@0"))
			} else {
				args = append(args, "-vf", fmt.Sprintf("fps=%d,%s,scale=512:512", fps, getCropFilter()))
			}
		} else {
			if nocrop {
				args = append(args, "-vf", "scale=512:512:force_original_aspect_ratio=decrease,pad=512:512:(ow-iw)/2:(oh-ih)/2:color=0x00000000@0")
			} else {
				args = append(args, "-vf", getCropFilter() + ",scale=512:512")
			}
		}

		args = append(args,
			"-quality", fmt.Sprintf("%d", quality),
			"-pix_fmt", "rgba",
			"-y", webpPath,
		)

		cmd := exec.CommandContext(ctx, "ffmpeg", args...)
		var stderr bytes.Buffer
		cmd.Stderr = &stderr

		err := cmd.Run()
		if err != nil {
			if errors.Is(err, context.Canceled) || errors.Is(err, context.DeadlineExceeded) {
				return webpPath, err
			}
			fmt.Println("FFmpeg failed:", stderr.String())
			continue
		}

		info, err := os.Stat(webpPath)
		if err == nil && info.Size() <= 1024*1024 {
			return webpPath, nil
		}
	}

	return webpPath, fmt.Errorf("failed to convert to webp under 1MB")
}

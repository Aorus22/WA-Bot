package utils

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"time"
)

type StickerOptions struct {
	NoCrop    bool
	Quality   int
	StartTime string
	EndTime   string
	Direction string
	FPS       int
	IsVideo   bool
}

var ErrorNotUnder1MB = errors.New("failed to convert to webp under 1MB")

func ConvertToWebp(ctx context.Context, mediaPath string, opt *StickerOptions) (string, error) {
	webpPath := filepath.Join("media", fmt.Sprintf("output_%d.webp", time.Now().UnixMilli()))

	if opt.FPS == 0 {
		opt.FPS = 15
	}

	if opt.Quality == 0 {
		opt.Quality = 100
	}

	parseDirection := func() (string, int) {
		parts := strings.Split(opt.Direction, "-")
		side := parts[0]
		level := 0

		if len(parts) == 2 {
			if n, err := strconv.Atoi(parts[1]); err == nil {
				level = n
			}
		}

		return side, level
	}

	getCropFilter := func() string {
		base := "crop=min(iw\\,ih):min(iw\\,ih)"
		side, percent := parseDirection()

		ratio := float64(percent) / 100

		switch side {
		case "up":
			return fmt.Sprintf("%s:0:round((ih-min(iw\\,ih))*(1-%f))", base, ratio)
		case "down":
			return fmt.Sprintf("%s:0:round((ih-min(iw\\,ih))*%f)", base, ratio)
		case "left":
			return fmt.Sprintf("%s:round((iw-min(iw\\,ih))*%f):0", base, ratio)
		case "right":
			return fmt.Sprintf("%s:round((iw-min(iw\\,ih))*(1-%f)):0", base, ratio)
		default:
			return base
		}
	}

	var args []string
	args = append(args, "-i", mediaPath)

	if opt.IsVideo {
		if opt.StartTime != "" {
			args = append(args, "-ss", opt.StartTime)
		}
		if opt.EndTime != "" {
			args = append(args, "-to", opt.EndTime)
		} else {
			args = append(args, "-t", "30")
		}
		if opt.NoCrop {
			args = append(args, "-vf", fmt.Sprintf("fps=%d,%s", opt.FPS,
				"scale=512:512:force_original_aspect_ratio=decrease,pad=512:512:(ow-iw)/2:(oh-ih)/2:color=0x00000000@0"))
		} else {
			args = append(args, "-vf", fmt.Sprintf("fps=%d,%s,scale=512:512", opt.FPS, getCropFilter()))
		}
	} else {
		if opt.NoCrop {
			args = append(args, "-vf", "scale=512:512:force_original_aspect_ratio=decrease,pad=512:512:(ow-iw)/2:(oh-ih)/2:color=0x00000000@0")
		} else {
			args = append(args, "-vf", getCropFilter()+",scale=512:512")
		}
	}

	args = append(args,
		"-quality", fmt.Sprintf("%d", opt.Quality),
		"-y", webpPath,
	)

	cmd := exec.CommandContext(ctx, "ffmpeg", args...)
	var stderr bytes.Buffer
	cmd.Stderr = &stderr

	err := cmd.Run()
	if err != nil {
		if strings.Contains(err.Error(), "signal: killed") {
			return webpPath, context.Canceled
		}

		if strings.Contains(err.Error(), "exit status 1") {
			return webpPath, context.Canceled
		}

		if !errors.Is(err, context.Canceled) && !errors.Is(err, context.DeadlineExceeded) {
			fmt.Println("FFmpeg failed:", stderr.String())
		}

		return webpPath, err
	}

	info, err := os.Stat(webpPath)
	if err == nil && info.Size() <= 1024*1024 {
		return webpPath, nil
	}

	return webpPath, ErrorNotUnder1MB
}

package media

import (
	"bytes"
	"context"
	"encoding/binary"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"time"

	"wa-bot/internal/domain/valueobject"
)

type FFmpegConverter struct{}

func NewFFmpegConverter() *FFmpegConverter {
	return &FFmpegConverter{}
}

func (f *FFmpegConverter) ConvertToWebP(ctx context.Context, mediaPath string, opt *valueobject.StickerOptions) (string, error) {
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

	if opt.IsAnimated {
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
		"-pix_fmt", "rgba",
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

	return webpPath, valueobject.ErrNotUnder1MB
}

func (f *FFmpegConverter) GetDuration(filePath string) (float64, error) {
	cmd := exec.Command("ffprobe", "-v", "error", "-show_entries", "format=duration",
		"-of", "default=noprint_wrappers=1:nokey=1", filePath)

	output, err := cmd.Output()
	if err != nil {
		return 0, err
	}

	durationStr := strings.TrimSpace(string(output))
	duration, err := strconv.ParseFloat(durationStr, 64)
	if err != nil {
		return 0, valueobject.ErrNotVideo
	}

	return duration, nil
}

func (f *FFmpegConverter) GetMimeType(filePath string) (string, error) {
	cmd := exec.Command("file", "--mime-type", filePath)
	output, err := cmd.Output()
	if err != nil {
		return "", err
	}

	parts := strings.Fields(string(output))
	if len(parts) >= 2 {
		return parts[1], nil
	}

	return "", errors.New("failed to detect mime type")
}

func (f *FFmpegConverter) IsValidTimeFormat(t string) bool {
	if t == "" {
		return false
	}
	parts := strings.Split(t, ":")
	if len(parts) != 2 {
		return false
	}
	sec, err := strconv.Atoi(parts[1])
	if err != nil || sec < 0 || sec > 59 {
		return false
	}
	_, err = strconv.Atoi(parts[0])
	return err == nil
}

func (f *FFmpegConverter) ParseTimeFromString(t string) float64 {
	if t == "" {
		return 0
	}
	parts := strings.Split(t, ":")
	min, _ := strconv.Atoi(parts[0])
	sec, _ := strconv.Atoi(parts[1])
	return float64(min*60 + sec)
}

func (f *FFmpegConverter) WriteWebpExif(ctx context.Context, inputPath string, packName, author string) (string, error) {
	timestamp := time.Now().Unix()
	filenameBase := fmt.Sprintf("%d_convert", timestamp)

	outputPath := filepath.Join("media", filenameBase+"_output.webp")
	exifPath := filepath.Join("media", filenameBase+"_meta.exif")
	defer os.Remove(exifPath)

	var b bytes.Buffer
	startingBytes := []byte{0x49, 0x49, 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00, 0x01, 0x00, 0x41, 0x57, 0x07, 0x00}
	endingBytes := []byte{0x16, 0x00, 0x00, 0x00}

	meta := map[string]interface{}{
		"sticker-pack-id":        "site.alyza.custompack",
		"sticker-pack-name":      packName,
		"sticker-pack-publisher": author,
	}
	jsonBytes, err := json.Marshal(meta)
	if err != nil {
		return "", err
	}

	lenBuf := make([]byte, 4)
	binary.LittleEndian.PutUint32(lenBuf, uint32(len(jsonBytes)))

	b.Write(startingBytes)
	b.Write(lenBuf)
	b.Write(endingBytes)
	b.Write(jsonBytes)

	if err := os.WriteFile(exifPath, b.Bytes(), 0644); err != nil {
		return "", err
	}

	cmd := exec.CommandContext(ctx, "webpmux", "-set", "exif", exifPath, inputPath, "-o", outputPath)
	if err := cmd.Run(); err != nil {
		return "", err
	}

	return outputPath, nil
}

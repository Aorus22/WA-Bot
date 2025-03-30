package utils

import (
	"context"
	"bytes"
	"encoding/binary"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
	"time"
)

func Contains(slice []string, item string) bool {
	for _, v := range slice {
		if strings.EqualFold(v, item) {
			return true
		}
	}
	return false
}

func GetLinkFromString(input string) (string, error) {
	urlRegex := regexp.MustCompile(`^(https?:\/\/)?([\w-]+\.)+[\w-]+(:\d+)?(\/[\w\-\.~!*'();:@&=+$,/?%#]*)?$`)
	words := strings.Split(input, " ")
	for _, word := range words {
		if urlRegex.MatchString(word) {
			return word, nil
		}
	}
	return "", fmt.Errorf("no link found / invalid link")
}

func DownloadMediaFromURL(ctx context.Context, url string) (string, error) {
    currentTime := fmt.Sprintf("%d", time.Now().UnixMilli())
	mediaPath := "media/" + currentTime

    cmd := exec.CommandContext(ctx, "yt-dlp",
        "-o", mediaPath,
        "--no-playlist",
        "-f", "best",
        url,
    )
    err := cmd.Run()
    if err == nil {
        return mediaPath, nil
    }

    cmd = exec.CommandContext(ctx, "gallery-dl",
        "-D", "media",
        "-f", currentTime,
        url,
    )
    err = cmd.Run()
    if err == nil {
        return mediaPath, nil
    }

    req, err := http.NewRequestWithContext(ctx, "GET", url, nil)
    if err != nil {
        return mediaPath, err
    }

    client := &http.Client{}
    resp, err := client.Do(req)
    if err != nil {
        return mediaPath, err
    }
    defer resp.Body.Close()

    if resp.StatusCode != http.StatusOK {
        return mediaPath, fmt.Errorf("failed to fetch media, status: %d", resp.StatusCode)
    }

    file, err := os.Create(mediaPath)
    if err != nil {
        return mediaPath, err
    }
    defer file.Close()

    _, err = io.Copy(file, resp.Body)
    if err != nil {
        return mediaPath, err
    }

    return mediaPath, nil
}

// func GetMimeType(filePath string) (string, error) {
// 	file, err := os.Open(filePath)
// 	if err != nil {
// 		return "", err
// 	}
// 	defer file.Close()

// 	buffer := make([]byte, 512)
// 	_, err = file.Read(buffer)
// 	if err != nil {
// 		return "", err
// 	}

// 	mimeType := http.DetectContentType(buffer)
// 	return mimeType, nil
// }

func GetMimeType(filePath string) (string, error) {
	cmd := exec.Command("file", "--mime-type", "-b", filePath)

	var out bytes.Buffer
	cmd.Stdout = &out

	err := cmd.Run()
	if err != nil {
		return "", err
	}

	mimeType := strings.TrimSpace(out.String())
	return mimeType, nil
}

func GetFFMPEGExecutable() (string, error){
	if os.Getenv("ENV") == "PRODUCTION" {
		exePath, err := os.Executable()
		if err != nil {
			return "", fmt.Errorf("failed to get executable path: %w", err)
		}
		exeDir := filepath.Dir(exePath)

		ffmpegPath := filepath.Join(exeDir, "ffmpeg")
		if _, err := os.Stat(ffmpegPath); os.IsNotExist(err) {
			return "", fmt.Errorf("ffmpeg not found")
		}
		_ = os.Chmod(ffmpegPath, 0755)

		return ffmpegPath, nil

	} else {
		return "ffmpeg", nil
	}
}

func WriteWebpExifFile(ctx context.Context, inputPath string, packName, author string) (string, error) {
	timestamp := time.Now().Unix()
	filenameBase := fmt.Sprintf("%d_convert", timestamp)

	outputPath := filepath.Join("media", filenameBase+"_output.webp")
	exifPath := filepath.Join("media", filenameBase+"_meta.exif")
	defer os.Remove(exifPath)

	var b bytes.Buffer
	startingBytes := []byte{0x49, 0x49, 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00, 0x01, 0x00, 0x41, 0x57, 0x07, 0x00}
	endingBytes := []byte{0x16, 0x00, 0x00, 0x00}

	meta := map[string]any{
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

func IsCanceledGoroutine(ctx context.Context) bool {
	select {
	case <-ctx.Done():
		return true
	default:
		return false
	}
}

func LogNoCancelErr(ctx context.Context, err error, msg string) bool {
    if err != nil {
        if !errors.Is(err, context.Canceled) && !errors.Is(err, context.DeadlineExceeded) {
            fmt.Println(msg, err)
        }
        return true
    }
    return false
}

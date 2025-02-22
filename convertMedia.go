package main

import (
	"bytes"
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"time"

	"go.mau.fi/whatsmeow"
	waProto "go.mau.fi/whatsmeow/binary/proto"
	waTypes "go.mau.fi/whatsmeow/types"
	"google.golang.org/protobuf/proto"
)

func convertMediaToSticker(client *whatsmeow.Client, senderJID waTypes.JID, mediaPath string, crop bool, isAnimated bool){	
	ffmpegExec := "ffmpeg"

	webpPath := filepath.Join("media", fmt.Sprintf("output_%d.webp", time.Now().UnixMilli()))

	env := os.Getenv("ENV")
	if env == "PRODUCTION" {
		exePath, err := os.Executable()
		if err != nil {
			fmt.Println("Failed to get executable path:", err)
			return
		}
		exeDir := filepath.Dir(exePath)
	
		ffmpegPath := filepath.Join(exeDir, "ffmpeg")
		
		if _, err := os.Stat(ffmpegPath); os.IsNotExist(err) {
			fmt.Println("FFmpeg not found in project directory:", ffmpegPath)
			return
		}
		_ = os.Chmod(ffmpegPath, 0755)

		ffmpegExec = ffmpegPath
		webpPath = filepath.Join(exeDir, "media", fmt.Sprintf("output_%d.webp", time.Now().UnixMilli()))
	} 
	
	var cmd *exec.Cmd
	qualityLevels := []int{80, 60, 40, 20}

	for _, quality := range qualityLevels {
		if crop {
			cmd = exec.Command(
				ffmpegExec,
				"-i", mediaPath,
				"-vf", "crop=min(iw\\,ih):min(iw\\,ih),scale=512:512",
				"-c:v", "libwebp",
				"-quality", fmt.Sprintf("%d", quality),
				"-pix_fmt", "rgba",
				"-y", webpPath,
			)
		} else {
			cmd = exec.Command(
				ffmpegExec,
				"-i", mediaPath,
				"-vf", "scale=512:512:force_original_aspect_ratio=decrease,pad=512:512:(ow-iw)/2:(oh-ih)/2:color=0x00000000@0",
				"-c:v", "libwebp",
				"-quality", fmt.Sprintf("%d", quality),
				"-pix_fmt", "rgba",
				"-y", webpPath,
			)
		}

		var stderr bytes.Buffer
		cmd.Stderr = &stderr
		err := cmd.Run()
		if err != nil {
			fmt.Println("Failed to convert image:", stderr.String())
			return
		}

		fileInfo, err := os.Stat(webpPath)
		if err != nil {
			fmt.Println("Failed to read WebP file size:", err)
			return
		}

		if fileInfo.Size() <= 1024*1024 {
			break
		}
	}

	webpData, err := os.ReadFile(webpPath)
	if err != nil {
		fmt.Println("Failed to read WebP file:", err)
		return
	}

	uploaded, err := client.Upload(context.Background(), webpData, whatsmeow.MediaImage)
	if err != nil {
		fmt.Println("Failed to upload sticker:", err)
		return
	}

	_, err = client.SendMessage(context.Background(), senderJID, &waProto.Message{
		StickerMessage: &waProto.StickerMessage{
			Mimetype:    proto.String("image/webp"),
			URL:         proto.String(uploaded.URL),
			DirectPath:  proto.String(uploaded.DirectPath),
			MediaKey:    uploaded.MediaKey,
			FileEncSHA256: uploaded.FileEncSHA256,
			FileSHA256:  uploaded.FileSHA256,
			FileLength:  proto.Uint64(uploaded.FileLength),
			IsAnimated: proto.Bool(isAnimated),
		},
	})

	if err != nil {
		fmt.Println("Failed to send sticker:", err)
		_, err = client.SendMessage(context.Background(), senderJID, &waProto.Message{
			Conversation: proto.String("Gagal dalam membuat sticker"),
		})
	}

	_ = os.Remove(mediaPath)
	_ = os.Remove(webpPath)
}
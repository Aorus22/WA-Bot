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

func convertImageToSticker(client *whatsmeow.Client, senderJID waTypes.JID, imagePath string, crop bool){	
	exePath, err := os.Executable()
	if err != nil {
		fmt.Println("Failed to get executable path:", err)
		return
	}
	exeDir := filepath.Dir(exePath)

	ffmpegPath := filepath.Join(exeDir, "ffmpeg")
	webpPath := filepath.Join(exeDir, "images", fmt.Sprintf("output_%d.webp", time.Now().UnixMilli()))

	if _, err := os.Stat(ffmpegPath); os.IsNotExist(err) {
		fmt.Println("FFmpeg not found in project directory:", ffmpegPath)
		return
	}
	_ = os.Chmod(ffmpegPath, 0755) 

	var cmd *exec.Cmd
	if crop {
		cmd = exec.Command(
			"ffmpeg", 
			"-i", imagePath, 
			"-vf", 
			"crop=min(iw\\,ih):min(iw\\,ih),scale=512:512", 
			"-c:v", "libwebp", 
			"-quality", "80", 
			"-pix_fmt", "rgba", 
			"-y", webpPath,
		)
	} else {
		cmd = exec.Command(
			"ffmpeg", 
			"-i", imagePath, 
			"-vf", 
			"scale=512:512:force_original_aspect_ratio=decrease,pad=512:512:(ow-iw)/2:(oh-ih)/2:color=0x00000000@0", 
			"-c:v", "libwebp", 
			"-quality", "80", 
			"-pix_fmt", "rgba", 
			"-y", webpPath,
		)
	}

	var stderr bytes.Buffer
	cmd.Stderr = &stderr
	err = cmd.Run()
	if err != nil {
		fmt.Println("Failed to convert image:", stderr.String())
		return
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
		},
	})

	if err != nil {
		fmt.Println("Failed to send sticker:", err)
		_, err = client.SendMessage(context.Background(), senderJID, &waProto.Message{
			Conversation: proto.String("Gagal dalam membuat sticker"),
		})
	}

	_ = os.Remove(imagePath)
	_ = os.Remove(webpPath)
}
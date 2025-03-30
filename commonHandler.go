package main

import (
	"context"
	"fmt"
	"os"
	"strings"
	"time"

	"go.mau.fi/whatsmeow"
	waProto "go.mau.fi/whatsmeow/binary/proto"
	"go.mau.fi/whatsmeow/proto/waE2E"
	waTypes "go.mau.fi/whatsmeow/types"
	"google.golang.org/protobuf/proto"
	// "github.com/davecgh/go-spew/spew"
)

func checkHandler(client *whatsmeow.Client, senderJID waTypes.JID){
	_, err := client.SendMessage(context.Background(), senderJID, &waProto.Message{
		Conversation: proto.String("Hello, World!"),
	})
	if err != nil {
		fmt.Println("Failed to send message:", err)
	}
}

func listgroupsHandler(client *whatsmeow.Client, senderJID waTypes.JID){
	allowedSender := os.Getenv("ALLOWED_SENDER")
	if senderJID.String() != allowedSender {
		fmt.Println("Sender not allowed for !listgroups command.")
		return
	}

	groups, err := client.GetJoinedGroups()
	if err != nil {
		fmt.Println("Error fetching joined groups:", err)
		return
	}

	responseText := "📌 *Daftar Grup:*\n\n"
	for _, group := range groups {
		responseText += fmt.Sprintf("📂 *%s*\n📎 ID: %s\n", group.Name, group.JID.String())

		_, err := client.GetGroupInfo(group.JID)
		if err != nil {
			fmt.Println("Failed to get group info for", group.JID.String(), ":", err)
			continue
		}
	}

	_, err = client.SendMessage(context.Background(), senderJID, &waProto.Message{
		Conversation: proto.String(responseText),
	})
	if err != nil {
		fmt.Println("Failed to send group list message:", err)
	}	
}

func stickerHandler(
	client *whatsmeow.Client, 
	senderJID waTypes.JID, 
	vMessage *waProto.Message, 
	messageText string,
	){
	crop := strings.Contains(strings.ToLower(messageText), "crop")

	if vMessage.GetImageMessage() != nil {
		convertImageToStickerSubHandler(client, senderJID, vMessage.GetImageMessage(), crop)
	} else if vMessage.GetVideoMessage() != nil {
		convertVideoToStickerSubHandler(client, senderJID, vMessage.GetVideoMessage(), crop)
	} else {
		linkToStickerSubHandler(client, senderJID, messageText, crop)
	}

}

func convertImageToStickerSubHandler(
	client *whatsmeow.Client, 
	senderJID waTypes.JID, 
	waImageMessage *waE2E.ImageMessage, 
	crop bool,
	){
	data, err := client.Download(waImageMessage)
	if err != nil {
		fmt.Println("Failed to download image:", err)
		return
	}
	
	imagePath := fmt.Sprintf("media/%d.jpg", time.Now().UnixMilli())
	err = os.WriteFile(imagePath, data, 0644)
	if err != nil {
		fmt.Println("Failed to save image:", err)
		return
	}

	convertMediaToSticker(client, senderJID, imagePath, crop, false)
}

func convertVideoToStickerSubHandler(
	client *whatsmeow.Client, 
	senderJID waTypes.JID, 
	waVideoMessage *waE2E.VideoMessage, 
	crop bool,
){
	data, err := client.Download(waVideoMessage)
	if err != nil {
		fmt.Println("Failed to download video:", err)
		return
	}
	
	videoPath := fmt.Sprintf("media/%d.mp4", time.Now().UnixMilli())
	err = os.WriteFile(videoPath, data, 0644)
	if err != nil {
		fmt.Println("Failed to save image:", err)
		return
	}

	convertMediaToSticker(client, senderJID, videoPath, crop, true)
}

func linkToStickerSubHandler(
	client *whatsmeow.Client, 
	senderJID waTypes.JID, 
	messageText string,
	crop bool,
) {
	url, err := getLinkFromString(messageText)
	if err != nil {
		return
	}

	mediaPath, err := downloadMediaFromURL(url)
	if err != nil {
		return
	}

	mimeType, err := getMimeType(mediaPath)
	if err != nil {
		return
	}

	isVideo := true
	if strings.HasPrefix(mimeType, "image/") {
		isVideo = false
	} else if strings.HasPrefix(mimeType, "video/") {
		isVideo = true
	}

	fmt.Println("halo")
	convertMediaToSticker(client, senderJID, mediaPath, crop, isVideo)
}
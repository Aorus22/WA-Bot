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
		convertToStickerSubHandler(client, senderJID, vMessage.GetImageMessage(), crop)
	} else {
		linkToStickerSubHandler(client, senderJID, messageText, crop)
	}

}

func convertToStickerSubHandler(
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
	
	imagePath := fmt.Sprintf("images/image_%d.jpg", time.Now().UnixMilli())
	err = os.WriteFile(imagePath, data, 0644)
	if err != nil {
		fmt.Println("Gagal menyimpan gambar:", err)
		return
	}

	convertImageToSticker(client, senderJID, imagePath, crop)
}

func linkToStickerSubHandler(
	client *whatsmeow.Client, 
	senderJID waTypes.JID, 
	messageText string,
	crop bool,
	){
	url, err := getLinkFromString(messageText)
	if err != nil {
		return
	}

	imagePath, err := downloadImageFromURL(url)
	if err != nil {
		return
	}

	convertImageToSticker(client, senderJID, imagePath, crop)
}
package main

import (
	"context"
	"fmt"
	"os"

	"go.mau.fi/whatsmeow"
	"google.golang.org/protobuf/proto"
	waTypes "go.mau.fi/whatsmeow/types"
	waProto "go.mau.fi/whatsmeow/binary/proto"
)

func checkHandler(client *whatsmeow.Client, senderJID waTypes.JID){
	_, err := client.SendMessage(context.Background(), senderJID, &waProto.Message{
		Conversation: proto.String("Hello, World!" + senderJID.String()),
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
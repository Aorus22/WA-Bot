package main

import (
	"context"
	"fmt"
	"os"
	"strings"
	"time"
	"regexp"
	"strconv"
	"sync"

	"go.mau.fi/whatsmeow"
	"google.golang.org/protobuf/proto"
	waTypes "go.mau.fi/whatsmeow/types"
	waProto "go.mau.fi/whatsmeow/binary/proto"
)

var userState = struct {
	sync.Mutex
	pending map[string]time.Time
}{pending: make(map[string]time.Time)}

func tokenHandler(client *whatsmeow.Client, senderJID waTypes.JID){
	groupJIDs := strings.Split(os.Getenv("GROUP_JIDS"), ",")

	userRegistered := false

	for _, groupJIDStr := range groupJIDs {
		targetGroupJID, err := waTypes.ParseJID(groupJIDStr)
		if err != nil {
			fmt.Println("Invalid group JID:", err)
			continue
		}

		groupInfo, err := client.GetGroupInfo(targetGroupJID)
		if err != nil {
			fmt.Println("Failed to get group info for", groupJIDStr, ":", err)
			continue
		}

		for _, participant := range groupInfo.Participants {
			if participant.JID.String() == senderJID.String() {
				userRegistered = true
				break
			}
		}

		if userRegistered {
			break
		}
	}

	if !userRegistered{	
		return
	}

	userState.Lock()
	userState.pending[senderJID.String()] = time.Now() 
	userState.Unlock()

	_, err := client.SendMessage(context.Background(), senderJID, &waProto.Message{
		Conversation: proto.String("Silakan masukkan nama lengkap Anda."),
	})
	if err != nil {
		fmt.Println("Failed to send message:", err)
	}
}

func getNameHandler(client *whatsmeow.Client, senderJID waTypes.JID, messageText string){
	userState.Lock()
	startTime, exists := userState.pending[senderJID.String()]
	userState.Unlock()

	if !exists {
		return
	}

	timeoutStr := os.Getenv("TIMEOUT_NAMA")

	timeout, err := strconv.Atoi(timeoutStr)
	if err != nil {
		timeout = 2
	}
	
	if time.Since(startTime) > time.Duration(timeout)*time.Minute {
		userState.Lock()
		delete(userState.pending, senderJID.String())
		userState.Unlock()

		_, err := client.SendMessage(context.Background(), senderJID, &waProto.Message{
			Conversation: proto.String("⏳ Waktu habis! Silakan ketik *!token* lagi."),
		})
		if err != nil {
			fmt.Println("Failed to send timeout message:", err)
		}
		return
	}

	var validNameRegex = regexp.MustCompile(`^[a-zA-Z' ]+$`)
	fmt.Println(messageText)
	if !validNameRegex.MatchString(messageText) {
		_, err := client.SendMessage(context.Background(), senderJID, &waProto.Message{
			Conversation: proto.String("⚠️ Nama Invalid"),
		})
		if err != nil {
			fmt.Println("Failed to send validation message:", err)
		}

		userState.Lock()
		delete(userState.pending, senderJID.String())
		userState.Unlock()
		return
	}

	nis := strings.Split(senderJID.String(), "@")[0]
	nama := messageText

	userState.Lock()
	delete(userState.pending, senderJID.String())
	userState.Unlock()

	status, token, err := fetchTokenData(nama, nis)
	if err != nil {
		fmt.Println("Failed to fetch token:", err)
		return
	}

	var responseText string
	if status == "new" {
		responseText = "✅ Token baru Anda adalah:"
	} else if status == "update" {
		responseText = "Token lama telah tidak berlaku. Ini token baru anda:"
	} else {
		responseText = "Gagal mendapatkan token."
	}

	_, err = client.SendMessage(context.Background(), senderJID, &waProto.Message{
		Conversation: proto.String(responseText),
	})
	if err != nil {
		fmt.Println("Failed to send message:", err)
		return
	}

	_, err = client.SendMessage(context.Background(), senderJID, &waProto.Message{
		Conversation: proto.String(token),
	})
	if err != nil {
		fmt.Println("Failed to send message:", err)
	}
}
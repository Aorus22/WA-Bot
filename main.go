package main

import (
	"bufio"
	"context"
	"fmt"
	"os"
	"os/signal"
	"regexp"
	"strings"
	"syscall"
	"time"

	"github.com/davecgh/go-spew/spew"
	"github.com/joho/godotenv"
	_ "github.com/mattn/go-sqlite3"
	"github.com/mdp/qrterminal"
	"go.mau.fi/whatsmeow"
	"go.mau.fi/whatsmeow/store/sqlstore"
	"go.mau.fi/whatsmeow/types"
	"go.mau.fi/whatsmeow/types/events"
	waLog "go.mau.fi/whatsmeow/util/log"
)

func eventHandler(evt interface{}, client *whatsmeow.Client) {
	switch v := evt.(type) {
	case *events.Message:
		adminGroupsEnv := os.Getenv("ADMIN_GROUPS")
		adminGroups := strings.Split(adminGroupsEnv, ",")

		if v.Info.IsGroup {
			groupJID := v.Info.Chat.String()
			if !contains(adminGroups, groupJID) {
				return
			}
		}

		// if v.Info.IsGroup {
		// 	return
		// }

		msgTime := v.Info.Timestamp
		now := time.Now()

		if now.Sub(msgTime).Seconds() > 10 {
			return
		}

		spew.Config.Indent = "\n"
		// spew.Config.MaxDepth = 2
		// spew.Printf("messageContextInfo: %#v\n\n", v.Message)

		var senderJID types.JID
		isFromGroup := false

		if v.Info.IsGroup {
			senderJID = v.Info.Chat.ToNonAD()
			isFromGroup = true
		} else {
			senderJID = v.Info.Sender.ToNonAD()
		}

		var messageText string
		if v.Message.ExtendedTextMessage != nil && v.Message.ExtendedTextMessage.Text != nil {
			messageText = *v.Message.ExtendedTextMessage.Text
		} else if v.Message.ImageMessage != nil {
			messageText = *v.Message.ImageMessage.Caption
		} else if v.Message.VideoMessage != nil {
			messageText = *v.Message.VideoMessage.Caption
		} else {
			messageText = v.Message.GetConversation()
		}

		fmt.Println(senderJID.UserInt(), "=>", messageText)

		commandList := []string{
			"!check",
			"!listgroups",
			"!token",
			"!sticker",
			"!pdf",
		}

		if contains(commandList, messageText) {
			userState.Lock()
			_, exists := userState.pending[senderJID.String()]
			if exists {
				delete(userState.pending, senderJID.String())
			}
			userState.Unlock()
		}

		stickerRegex := regexp.MustCompile(`^!sticker(\s+\S+)*$`)
		pdfRegex := regexp.MustCompile(`^!pdf\s+\S+$`)
		answerPdfRegex := regexp.MustCompile(`^!answer(\s+\S+)*$`)

		if messageText == "!check" {
			checkHandler(client, senderJID)
		} else if messageText == "!listgroups" {
			listgroupsHandler(client, senderJID)
		} else if messageText == "!token" {
			tokenHandler(client, senderJID)
		} else if messageText == "!listmapel" {
			listMapelHandler(client, isFromGroup, senderJID)
		} else if pdfRegex.MatchString(messageText) {
			sendPDFHandler(client, isFromGroup, senderJID, v.Message, messageText)
		} else if answerPdfRegex.MatchString(messageText) {
			sendPDFHandler(client, isFromGroup, senderJID, v.Message, messageText)
		} else if stickerRegex.MatchString(messageText) {
			stickerHandler(client, senderJID, v.Message, messageText)
		} else {
			getNameHandler(client, senderJID, messageText)
		}
	}
}

func getAuth(client *whatsmeow.Client) {
	reader := bufio.NewReader(os.Stdin)
	fmt.Println("Select login method:")
    fmt.Println("1. QR Code")
    fmt.Println("2. Pair Code")
    fmt.Print("Choice: ")
	choice, _ := reader.ReadString('\n')
    choice = strings.TrimSpace(choice)

	switch choice {
	case "1":
		qrChan, _ := client.GetQRChannel(context.Background())
		err := client.Connect()
		if err != nil {
			panic(err)
		}
		for evt := range qrChan {
			if evt.Event == "code" {
				qrterminal.GenerateHalfBlock(evt.Code, qrterminal.L, os.Stdout)
			} else {
				fmt.Println("Login event:", evt.Event)
			}
		}

	case "2":
		fmt.Print("Enter phone number: ")
		phoneNumber, _ := reader.ReadString('\n')
		phoneNumber = strings.TrimSpace(phoneNumber)
		if !strings.HasPrefix(phoneNumber, "+") {
			phoneNumber = "+" + phoneNumber
		}

		err := client.Connect()
		if err != nil {
			panic(err)
		}

		pairCode, err := client.PairPhone(phoneNumber, true, whatsmeow.PairClientChrome, "Chrome (Windows)")
		if err != nil {
				panic(err)
			}
			
		fmt.Println("Your Pair Code:", pairCode)

	default:
        fmt.Println("Invalid choice")
        return
	}
}

func main() {
	err := godotenv.Load()
	if err != nil {
		fmt.Println("Error loading .env file")
	}

	logLevel := os.Getenv("LOG_LEVEL")
	if logLevel == "" {
		logLevel = "DEBUG"
	}

	dbLog := waLog.Stdout("Database", logLevel, true)
	container, err := sqlstore.New("sqlite3", "file:bimalord-bot-session.db?_foreign_keys=on", dbLog)
	if err != nil {
		panic(err)
	}

	deviceStore, err := container.GetFirstDevice()
	if err != nil {
		panic(err)
	}

	clientLog := waLog.Stdout("Client", logLevel, true)
	client := whatsmeow.NewClient(deviceStore, clientLog)
	client.AddEventHandler(func(evt interface{}) {
		eventHandler(evt, client)
	})

	if client.Store.ID == nil {
		getAuth(client)
	} else {
		err = client.Connect()
		fmt.Println("Successfully authenticated")
		if err != nil {
			panic(err)
		}
	}

	c := make(chan os.Signal, 1)
	signal.Notify(c, os.Interrupt, syscall.SIGTERM)
	<-c

	client.Disconnect()
}
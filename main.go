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

	"github.com/joho/godotenv"
	_ "github.com/mattn/go-sqlite3"
	"github.com/mdp/qrterminal"
	"go.mau.fi/whatsmeow"
	"go.mau.fi/whatsmeow/store/sqlstore"
	"go.mau.fi/whatsmeow/types"
	"go.mau.fi/whatsmeow/types/events"
	waLog "go.mau.fi/whatsmeow/util/log"

	Admin "wa-bot/handlers/adminHandlers"
	Common "wa-bot/handlers/commonHandlers"
	"wa-bot/utils"
	"wa-bot/state"
)

var (
	dbUrl = "file:wa-bot-session.db?_foreign_keys=on"
	waClient *whatsmeow.Client
)

func eventHandler(evt any, client *whatsmeow.Client) {
	switch v := evt.(type) {
	case *events.Message:

		if v.Info.IsGroup {
			allowed :=utils.IsFromAllowedGroups(&v.Info)
			if !allowed { return }
		}

		msgTime := v.Info.Timestamp
		now := time.Now()
		if now.Sub(msgTime).Seconds() > 10 { return }

		var senderJID types.JID
		isFromGroup := false
		if v.Info.IsGroup {
			senderJID = v.Info.Chat.ToNonAD()
			isFromGroup = true
		} else {
			senderJID = v.Info.Sender.ToNonAD()
		}

		if senderJID.UserInt() == 13135550002 { return }

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

		message_state := state.NewMessageContext(client, v.Message, senderJID, messageText, isFromGroup)

		fmt.Printf("%s [%s] %d => %s\n",
			func() string {
				if message_state.IsFromGroup {
					return "[Group]"
				}
				return ""
			}(),
			message_state.UserRole,
			message_state.SenderJID.UserInt(),
			message_state.MessageText,
		)

		if message_state.CheckUserState() != "" {
			if strings.HasPrefix(messageText, "!cancel") {
				Common.CancelHandler(message_state)
				return
			} else if strings.HasPrefix(messageText, "!") {
				message_state.Reply("There is another process, !cancel to cancel it")
				return
			}
		}

		stickerRegex := regexp.MustCompile(`^!sticker(\s+\S+)*$`)
		pdfRegex := regexp.MustCompile(`^!pdf\s+\S+$`)
		answerPdfRegex := regexp.MustCompile(`^!answer(\s+\S+)*$`)
		geminiRegex := regexp.MustCompile(`^!gemini(\s+\S+)*$`)

		switch {
		case message_state.MessageText == "!check":
			Common.CheckHandler(message_state)

		case message_state.MessageText == "!listgroups":
			Admin.ListgroupsHandler(message_state)

		case message_state.MessageText == "!token":
			Admin.TokenHandler(message_state)

		case message_state.MessageText == "!listmapel":
			Admin.ListMapelHandler(message_state)

		case pdfRegex.MatchString(message_state.MessageText), answerPdfRegex.MatchString(message_state.MessageText):
			Admin.SendPDFHandler(message_state)

		case geminiRegex.MatchString(message_state.MessageText):
			Admin.GeminiHandler(message_state)

		case stickerRegex.MatchString(message_state.MessageText):
			Common.StickerHandler(message_state)

		case message_state.MessageText == "!help":
			Common.GetCommandListHandler(message_state)

		default:
			if message_state.CheckUserState() == "PendingToken" {
				Admin.GetNameHandler(message_state)
				return
			}

			if strings.HasPrefix(message_state.MessageText, "!") {
				message_state.Reply("Invalid Command")
				return
			}

			if message_state.UserRole == "COMMON" {
				message_state.Reply("!help to see the command list")
			} else if message_state.UserRole == "USER" {
				message_state.Reply("!help untuk melihat list command")
			}
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

func init() {
	err := godotenv.Load()
	if err != nil {
		fmt.Println("Error loading .env file")
	}
}

func main() {
	logLevel := os.Getenv("LOG_LEVEL")
	if logLevel == "" {
		logLevel = "DEBUG"
	}

	dbLog := waLog.Stdout("Database", logLevel, true)
	container, err := sqlstore.New("sqlite3", dbUrl, dbLog)
	if err != nil {
		panic(err)
	}

	deviceStore, err := container.GetFirstDevice()
	if err != nil {
		panic(err)
	}

	clientLog := waLog.Stdout("Client", logLevel, true)
	waClient = whatsmeow.NewClient(deviceStore, clientLog)
	waClient.AddEventHandler(func(evt any) {
		eventHandler(evt, waClient)
	})

	setupCron()

	if waClient.Store.ID == nil {
		getAuth(waClient)
	} else {
		err = waClient.Connect()
		fmt.Println("Successfully authenticated")
		if err != nil {
			panic(err)
		}
	}

	c := make(chan os.Signal, 1)
	signal.Notify(c, os.Interrupt, syscall.SIGTERM)
	<-c

	waClient.Disconnect()
}

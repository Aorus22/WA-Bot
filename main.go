package main

import (
	"context"
	"fmt"
	"os"
	"os/signal"
	"syscall"
	"time"

	_ "github.com/mattn/go-sqlite3"
	"github.com/joho/godotenv"
	"github.com/mdp/qrterminal"
	"go.mau.fi/whatsmeow"
	"go.mau.fi/whatsmeow/store/sqlstore"
	"go.mau.fi/whatsmeow/types/events"
	waLog "go.mau.fi/whatsmeow/util/log"
)

func eventHandler(evt interface{}, client *whatsmeow.Client) {
	switch v := evt.(type) {
	case *events.Message:
		if v.Info.IsGroup {
			return
		}

        msgTime := v.Info.Timestamp
        now := time.Now()

        if now.Sub(msgTime).Seconds() > 10 {
            return
        }

		var messageText string
		if v.Message.ExtendedTextMessage != nil && v.Message.ExtendedTextMessage.Text != nil {
			messageText = *v.Message.ExtendedTextMessage.Text
		} else {
			messageText = v.Message.GetConversation()
		}

		senderJID := v.Info.Sender.ToNonAD()

		commandList := []string{
			"!check", 
			"!listgroups", 
			"!token",
		}

		if contains(commandList, messageText) {
			userState.Lock()
			_, exists := userState.pending[senderJID.String()];
			if exists {
				delete(userState.pending, senderJID.String())
			}
			userState.Unlock()
		}

		if messageText == "!check" {
			checkHandler(client, senderJID)
		} else if messageText == "!listgroups" {
			listgroupsHandler(client, senderJID)
		} else if messageText == "!token" {
			tokenHandler(client, senderJID)
		} else {
			getNameHandler(client, senderJID, messageText)
		}
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
		qrChan, _ := client.GetQRChannel(context.Background())
		err = client.Connect()
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
	} else {
		err = client.Connect()
		if err != nil {
			panic(err)
		}
	}

	c := make(chan os.Signal, 1)
	signal.Notify(c, os.Interrupt, syscall.SIGTERM)
	<-c

	client.Disconnect()
}

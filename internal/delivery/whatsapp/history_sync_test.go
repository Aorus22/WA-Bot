package whatsapp

import (
	"strings"
	"testing"
	"time"

	waE2E "go.mau.fi/whatsmeow/proto/waE2E"
	waTypes "go.mau.fi/whatsmeow/types"
	"go.mau.fi/whatsmeow/types/events"
	"google.golang.org/protobuf/proto"
)

func TestProjectHistoricalEventUsesLazyMediaURL(t *testing.T) {
	chat, _ := waTypes.ParseJID("628123@s.whatsapp.net")
	evt := &events.Message{
		Info: waTypes.MessageInfo{
			MessageSource: waTypes.MessageSource{Chat: chat, Sender: chat},
			ID:            "history-image",
			Timestamp:     time.UnixMilli(1234),
		},
		Message: &waE2E.Message{ImageMessage: &waE2E.ImageMessage{Caption: proto.String("old photo")}},
	}
	msg := projectHistoricalEvent(evt, chat.String())
	if msg == nil {
		t.Fatal("projectHistoricalEvent returned nil")
	}
	if msg.Type != "image" || msg.Content != "old photo" || msg.Status != "received" {
		t.Fatalf("unexpected projection: %#v", msg)
	}
	if !strings.Contains(msg.MediaURL, "/chats/628123@s.whatsapp.net/messages/history-image/media") {
		t.Fatalf("unexpected lazy media URL: %s", msg.MediaURL)
	}
}

func TestProjectHistoricalEventPreservesOutgoingDirection(t *testing.T) {
	chat, _ := waTypes.ParseJID("120363000000@g.us")
	evt := &events.Message{
		Info: waTypes.MessageInfo{
			MessageSource: waTypes.MessageSource{Chat: chat, IsFromMe: true, IsGroup: true},
			ID:            "history-text",
			Timestamp:     time.UnixMilli(5678),
		},
		Message: &waE2E.Message{Conversation: proto.String("hello")},
	}
	msg := projectHistoricalEvent(evt, chat.String())
	if msg == nil || msg.From != "me" || msg.To != chat.String() || msg.Status != "sent" {
		t.Fatalf("unexpected outgoing projection: %#v", msg)
	}
}

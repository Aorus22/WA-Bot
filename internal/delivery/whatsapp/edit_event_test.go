package whatsapp

import (
	"path/filepath"
	"sync"
	"testing"
	"time"

	"go.mau.fi/whatsmeow/proto/waCommon"
	"go.mau.fi/whatsmeow/proto/waE2E"
	waTypes "go.mau.fi/whatsmeow/types"
	"go.mau.fi/whatsmeow/types/events"
	"google.golang.org/protobuf/proto"

	"wa-bot/internal/domain/repository"
)

type broadcastRecorder struct {
	mu     sync.Mutex
	events []map[string]interface{}
	types  []string
}

func (b *broadcastRecorder) BroadcastMessage(msgType string, payload interface{}) {
	b.mu.Lock()
	defer b.mu.Unlock()
	b.types = append(b.types, msgType)
	if m, ok := payload.(map[string]interface{}); ok {
		b.events = append(b.events, m)
	} else if m, ok := payload.(map[string]string); ok {
		converted := map[string]interface{}{}
		for k, v := range m {
			converted[k] = v
		}
		b.events = append(b.events, converted)
	} else {
		b.events = append(b.events, nil)
	}
}

func (b *broadcastRecorder) SaveAndBroadcastMessage(*repository.Message) {}
func (b *broadcastRecorder) UpdateMessageStatus(string, string)          {}

func editFixture(t *testing.T) (*WhatsAppEventHandler, *repository.MessageStore, *broadcastRecorder) {
	t.Helper()
	store, err := repository.NewMessageStore(filepath.Join(t.TempDir(), "messages.db"))
	if err != nil {
		t.Fatal(err)
	}
	recorder := &broadcastRecorder{}
	h := &WhatsAppEventHandler{msgStore: store, httpServer: recorder}
	if err := store.SaveMessage(&repository.Message{
		ID: "orig", ChatID: "111@lid", From: "me", To: "111@lid", Content: "d",
		Timestamp: time.Now().UnixMilli(), Status: "sent", Type: "text",
	}); err != nil {
		t.Fatal(err)
	}
	return h, store, recorder
}

func protocolEvent(isFromMe bool, pm *waE2E.ProtocolMessage) *events.Message {
	chat, _ := waTypes.ParseJID("111@lid")
	return &events.Message{
		Info: waTypes.MessageInfo{
			ID:            "envelope",
			MessageSource: waTypes.MessageSource{Chat: chat, Sender: chat, IsFromMe: isFromMe},
			Timestamp:     time.Now(),
		},
		Message: &waE2E.Message{ProtocolMessage: pm},
	}
}

// An edit made on the phone arrives at the linked device as a protocol message.
func TestEditFromThePhoneUpdatesTheMessageAndKeepsTheOldText(t *testing.T) {
	h, store, recorder := editFixture(t)
	h.handleMessage(protocolEvent(true, &waE2E.ProtocolMessage{
		Key:           &waCommon.MessageKey{ID: proto.String("orig")},
		Type:          waE2E.ProtocolMessage_MESSAGE_EDIT.Enum(),
		EditedMessage: &waE2E.Message{Conversation: proto.String("dvv")},
	}))

	stored, err := store.GetMessageByID("orig")
	if err != nil {
		t.Fatal(err)
	}
	if stored.Content != "dvv" {
		t.Fatalf("content = %q, want the edited text", stored.Content)
	}
	if stored.Extra == nil || len(stored.Extra.Edits) != 1 || stored.Extra.Edits[0].Content != "d" {
		t.Fatalf("history not kept: %+v", stored.Extra)
	}
	// The newest message of the chat was edited, so the chat list preview follows.
	if len(recorder.types) != 2 || recorder.types[0] != "chat_update" || recorder.types[1] != "message_edited" {
		t.Fatalf("broadcasts = %v, want chat_update then message_edited", recorder.types)
	}
	if recorder.events[0]["lastMsg"] != "dvv" {
		t.Fatalf("chat_update = %v, want lastMsg dvv", recorder.events[0])
	}
}

func TestRevokeDeletesTheMessage(t *testing.T) {
	h, store, recorder := editFixture(t)
	h.handleMessage(protocolEvent(false, &waE2E.ProtocolMessage{
		Key:  &waCommon.MessageKey{ID: proto.String("orig")},
		Type: waE2E.ProtocolMessage_REVOKE.Enum(),
	}))
	if _, err := store.GetMessageByID("orig"); err == nil {
		t.Fatal("a revoked message must be gone")
	}
	if len(recorder.types) != 1 || recorder.types[0] != "message_deleted" {
		t.Fatalf("broadcasts = %v, want one message_deleted", recorder.types)
	}
}

// If the envelope carries the id of the message it edits, the duplicate check
// must not swallow it.
func TestEditWhoseEnvelopeIdIsTheTargetIsStillApplied(t *testing.T) {
	h, store, _ := editFixture(t)
	evt := protocolEvent(true, &waE2E.ProtocolMessage{
		Key:           &waCommon.MessageKey{ID: proto.String("orig")},
		Type:          waE2E.ProtocolMessage_MESSAGE_EDIT.Enum(),
		EditedMessage: &waE2E.Message{ExtendedTextMessage: &waE2E.ExtendedTextMessage{Text: proto.String("dvv")}},
	})
	evt.Info.ID = "orig"
	h.handleMessage(evt)

	stored, err := store.GetMessageByID("orig")
	if err != nil || stored.Content != "dvv" {
		t.Fatalf("stored = %+v, err = %v", stored, err)
	}
}

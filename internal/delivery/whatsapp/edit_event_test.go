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

func TestRevokeKeepsTheMessageAndFlagsIt(t *testing.T) {
	h, store, recorder := editFixture(t)
	h.handleMessage(protocolEvent(false, &waE2E.ProtocolMessage{
		Key:  &waCommon.MessageKey{ID: proto.String("orig")},
		Type: waE2E.ProtocolMessage_REVOKE.Enum(),
	}))
	msg, err := store.GetMessageByID("orig")
	if err != nil {
		t.Fatal("a revoked message must be kept")
	}
	if msg.RevokedAt == 0 || msg.Content != "d" {
		t.Fatalf("revoked message = %+v, want original content and a revoke time", msg)
	}
	if len(recorder.types) != 1 || recorder.types[0] != "message_revoked" {
		t.Fatalf("broadcasts = %v, want one message_revoked", recorder.types)
	}
	if recorder.events[0]["id"] != "orig" || recorder.events[0]["revokedAt"] != msg.RevokedAt {
		t.Fatalf("event = %v", recorder.events[0])
	}

	// Our own echo of the same revoke must not move the time.
	first := msg.RevokedAt
	h.handleMessage(protocolEvent(true, &waE2E.ProtocolMessage{
		Key:  &waCommon.MessageKey{ID: proto.String("orig")},
		Type: waE2E.ProtocolMessage_REVOKE.Enum(),
	}))
	if again, _ := store.GetMessageByID("orig"); again.RevokedAt != first {
		t.Fatalf("revoke time moved: %d -> %d", first, again.RevokedAt)
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

func TestGroupStatusForWaitsForEveryMember(t *testing.T) {
	cases := []struct {
		delivered, read, total int
		want                   string
	}{
		{0, 0, 4, "sent"},
		{3, 1, 4, "sent"},
		{4, 1, 4, "delivered"},
		{4, 3, 4, "delivered"},
		{4, 4, 4, "read"},
		{2, 2, 0, "sent"}, // unknown member list: never claim more than sent
	}
	for _, c := range cases {
		if got := groupStatusFor(c.delivered, c.read, c.total); got != c.want {
			t.Errorf("groupStatusFor(%d,%d,%d) = %s, want %s", c.delivered, c.read, c.total, got, c.want)
		}
	}
}

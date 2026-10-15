package whatsapp

import (
	"path/filepath"
	"sync"
	"testing"
	"time"

	"go.mau.fi/whatsmeow/proto/waCommon"
	"go.mau.fi/whatsmeow/proto/waE2E"
	"go.mau.fi/whatsmeow/proto/waSyncAction"
	waTypes "go.mau.fi/whatsmeow/types"
	"go.mau.fi/whatsmeow/types/events"
	"google.golang.org/protobuf/proto"

	"wa-bot/internal/domain/repository"
)

// savingRecorder is a broadcastRecorder that also keeps the messages saved
// through the HTTP layer.
type savingRecorder struct {
	broadcastRecorder
	mu2   sync.Mutex
	saved []*repository.Message
}

func (s *savingRecorder) SaveAndBroadcastMessage(m *repository.Message) {
	s.mu2.Lock()
	defer s.mu2.Unlock()
	s.saved = append(s.saved, m)
}

func flagsFixture(t *testing.T) (*WhatsAppEventHandler, *repository.MessageStore, *savingRecorder) {
	t.Helper()
	store, err := repository.NewMessageStore(filepath.Join(t.TempDir(), "messages.db"))
	if err != nil {
		t.Fatal(err)
	}
	rec := &savingRecorder{}
	h := &WhatsAppEventHandler{msgStore: store, httpServer: rec}
	if err := store.SaveMessage(&repository.Message{
		ID: "orig", ChatID: "628111@s.whatsapp.net", From: "628111@s.whatsapp.net", To: "me", Content: "hello",
		Timestamp: time.Now().UnixMilli(), Status: "received", Type: "text",
	}); err != nil {
		t.Fatal(err)
	}
	return h, store, rec
}

func chatMessage(msg *waE2E.Message, fromMe bool) *events.Message {
	chat, _ := waTypes.ParseJID("628111@s.whatsapp.net")
	return &events.Message{
		Info: waTypes.MessageInfo{
			ID:            "envelope",
			MessageSource: waTypes.MessageSource{Chat: chat, Sender: chat, IsFromMe: fromMe},
			Timestamp:     time.Now(),
		},
		Message: msg,
	}
}

func TestStarFromThePhoneIsStoredAndBroadcast(t *testing.T) {
	h, store, rec := flagsFixture(t)
	chat, _ := waTypes.ParseJID("628111@s.whatsapp.net")

	h.HandleEvent(&events.Star{ChatJID: chat, MessageID: "orig", Timestamp: time.Now(), Action: &waSyncAction.StarAction{Starred: proto.Bool(true)}})
	if msg, _ := store.GetMessageByID("orig"); msg == nil || !msg.Starred {
		t.Fatalf("message should be starred: %+v", msg)
	}
	if len(rec.types) != 1 || rec.types[0] != "message_starred" || rec.events[0]["starred"] != true {
		t.Fatalf("broadcast = %v %v", rec.types, rec.events)
	}

	h.HandleEvent(&events.Star{ChatJID: chat, MessageID: "orig", Timestamp: time.Now(), Action: &waSyncAction.StarAction{Starred: proto.Bool(false)}})
	if msg, _ := store.GetMessageByID("orig"); msg == nil || msg.Starred {
		t.Fatalf("message should be unstarred but kept: %+v", msg)
	}
}

func pinEvent(kind waE2E.PinInChatMessage_Type, secs uint32, fromMe bool) *events.Message {
	msg := &waE2E.Message{PinInChatMessage: &waE2E.PinInChatMessage{
		Key:               &waCommon.MessageKey{ID: proto.String("orig")},
		Type:              kind.Enum(),
		SenderTimestampMS: proto.Int64(time.Now().UnixMilli()),
	}}
	if secs > 0 {
		msg.MessageContextInfo = &waE2E.MessageContextInfo{MessageAddOnDurationInSecs: proto.Uint32(secs)}
	}
	return chatMessage(msg, fromMe)
}

func TestPinInChatIsStoredNotShownAsABubbleAndCanBeUnpinned(t *testing.T) {
	h, store, rec := flagsFixture(t)

	h.HandleEvent(pinEvent(waE2E.PinInChatMessage_PIN_FOR_ALL, 86400, false))
	pins, _ := store.GetActivePins("628111@s.whatsapp.net")
	if len(pins) != 1 || pins[0].MessageID != "orig" || pins[0].PinnedBy != "628111@s.whatsapp.net" {
		t.Fatalf("pins = %+v", pins)
	}
	if got := pins[0].ExpiresAt - pins[0].PinnedAt; got != 86400*1000 {
		t.Fatalf("pin lifetime = %dms", got)
	}
	if len(rec.saved) != 0 {
		t.Fatal("a pin is not a chat bubble")
	}
	if len(rec.types) != 1 || rec.types[0] != "chat_pins_changed" {
		t.Fatalf("broadcast = %v", rec.types)
	}

	// Our own echo: pinned by "me", no duration means the 7 day default.
	h.HandleEvent(pinEvent(waE2E.PinInChatMessage_PIN_FOR_ALL, 0, true))
	pins, _ = store.GetActivePins("628111@s.whatsapp.net")
	if len(pins) != 1 || pins[0].PinnedBy != "me" || pins[0].ExpiresAt-pins[0].PinnedAt != 7*24*3600*1000 {
		t.Fatalf("echo pins = %+v", pins)
	}

	h.HandleEvent(pinEvent(waE2E.PinInChatMessage_UNPIN_FOR_ALL, 0, false))
	if pins, _ = store.GetActivePins("628111@s.whatsapp.net"); len(pins) != 0 {
		t.Fatalf("after unpin = %+v", pins)
	}
	if msg, _ := store.GetMessageByID("orig"); msg == nil {
		t.Fatal("the pinned message itself must be kept")
	}
}

func TestDisappearingTimerChangesAreTrackedAndNothingIsDeleted(t *testing.T) {
	h, store, rec := flagsFixture(t)

	h.HandleEvent(chatMessage(&waE2E.Message{ProtocolMessage: &waE2E.ProtocolMessage{
		Type:                waE2E.ProtocolMessage_EPHEMERAL_SETTING.Enum(),
		EphemeralExpiration: proto.Uint32(604800),
	}}, false))
	if got := store.GetChatDisappearing("628111@s.whatsapp.net"); got != 604800 {
		t.Fatalf("timer = %d", got)
	}
	if len(rec.saved) != 0 {
		t.Fatal("a timer change is not a chat bubble")
	}
	if len(rec.events) != 1 || rec.events[0]["disappearingSecs"] != uint32(604800) {
		t.Fatalf("broadcast = %v", rec.events)
	}

	h.HandleEvent(chatMessage(&waE2E.Message{ProtocolMessage: &waE2E.ProtocolMessage{
		Type:                waE2E.ProtocolMessage_EPHEMERAL_SETTING.Enum(),
		EphemeralExpiration: proto.Uint32(0),
	}}, false))
	if got := store.GetChatDisappearing("628111@s.whatsapp.net"); got != 0 {
		t.Fatalf("timer should be off, got %d", got)
	}
	if msg, _ := store.GetMessageByID("orig"); msg == nil {
		t.Fatal("existing messages must be kept when the timer changes")
	}
}

func TestIncomingDisappearingMessageIsStampedAndKept(t *testing.T) {
	h, store, rec := flagsFixture(t)
	ev := chatMessage(&waE2E.Message{ExtendedTextMessage: &waE2E.ExtendedTextMessage{
		Text:        proto.String("secret"),
		ContextInfo: &waE2E.ContextInfo{Expiration: proto.Uint32(86400)},
	}}, false)
	ev.Info.ID = "ephemeral1"

	h.showMessage(ev, ev.Info.Chat, "secret", "628111@s.whatsapp.net", "", "")
	if len(rec.saved) != 1 {
		t.Fatalf("saved = %d", len(rec.saved))
	}
	d := rec.saved[0].Extra.Disappearing
	if d == nil || d.Seconds != 86400 || d.ExpiresAt != ev.Info.Timestamp.UnixMilli()+86400*1000 {
		t.Fatalf("disappearing = %+v", d)
	}
	if got := store.GetChatDisappearing("628111@s.whatsapp.net"); got != 86400 {
		t.Fatalf("an incoming timed message reveals the chat timer, got %d", got)
	}
}

func TestReplyToAStatusKeepsASnapshotOfIt(t *testing.T) {
	h, store, rec := flagsFixture(t)
	if err := store.SaveStatus(&repository.StatusEntry{
		ID: "st1", Sender: "628111@s.whatsapp.net", Content: "my story", Type: "text",
		Timestamp: time.Now().UnixMilli(), ExpiresAt: time.Now().Add(time.Hour).UnixMilli(),
	}, nil); err != nil {
		t.Fatal(err)
	}
	ev := chatMessage(&waE2E.Message{ExtendedTextMessage: &waE2E.ExtendedTextMessage{
		Text: proto.String("nice!"),
		ContextInfo: &waE2E.ContextInfo{
			StanzaID:  proto.String("st1"),
			RemoteJID: proto.String("status@broadcast"),
		},
	}}, false)
	ev.Info.ID = "reply1"

	h.showMessage(ev, ev.Info.Chat, "nice!", "628111@s.whatsapp.net", "", "")
	if len(rec.saved) != 1 || rec.saved[0].Extra == nil || rec.saved[0].Extra.StatusReply == nil {
		t.Fatalf("saved = %+v", rec.saved)
	}
	sr := rec.saved[0].Extra.StatusReply
	if sr.StatusID != "st1" || sr.Content != "my story" || sr.Type != "text" {
		t.Fatalf("status reply = %+v", sr)
	}
}

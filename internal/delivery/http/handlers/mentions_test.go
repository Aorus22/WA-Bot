package handlers

import (
	"context"
	"github.com/gorilla/mux"
	"net/http/httptest"
	"path/filepath"
	"testing"
	"wa-bot/internal/domain/entity"

	"wa-bot/internal/domain/repository"
)

type mentionReadHub struct{ unread interface{} }

func (h *mentionReadHub) BroadcastMessage(kind string, payload interface{}) {
	if kind == "chat_update" {
		h.unread = payload.(map[string]interface{})["unread"]
	}
}
func (*mentionReadHub) SendMessageToUser(string, string, interface{}) {}
func (*mentionReadHub) Run()                                          {}

func TestReadingChatClearsMentionAndBroadcastsToOtherWindows(t *testing.T) {
	store, err := repository.NewMessageStore(filepath.Join(t.TempDir(), "messages.db"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = store.Close() })
	chatID, own := "123@g.us", "own@s.whatsapp.net"
	if err := store.SaveMessage(&repository.Message{ID: "mention", ChatID: chatID, From: "peer", Status: "received", Type: "text", Extra: &repository.MessageExtra{Mentions: []string{own}}}); err != nil {
		t.Fatal(err)
	}
	hub := &mentionReadHub{}
	handler := NewChatHandler(&Handler{msgRepo: store, hub: hub})
	request := mux.SetURLVars(httptest.NewRequest("POST", "/chats/123/read", nil), map[string]string{"id": chatID})
	response := httptest.NewRecorder()
	handler.MarkAsRead(response, request)
	if response.Code != 200 || hub.unread != 0 {
		t.Fatalf("read response=%d, broadcast unread=%v", response.Code, hub.unread)
	}
	chats, err := store.UnreadMentionChats([]string{own})
	if err != nil || chats[chatID] {
		t.Fatalf("mention was not cleared: %v %v", chats, err)
	}
}

func TestMentionEnrichmentResolvesLIDAndOnlyMarksIncomingOwnMentions(t *testing.T) {
	store, err := repository.NewMessageStore(filepath.Join(t.TempDir(), "messages.db"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = store.Close() })
	phone, lid := "628111@s.whatsapp.net", "242622887161958@lid"
	if err := store.SaveLIDMapping(lid, phone); err != nil {
		t.Fatal(err)
	}
	if err := store.SaveContact(&repository.Contact{ID: phone, JID: phone, Name: "Nama kontak"}); err != nil {
		t.Fatal(err)
	}
	h := &Handler{msgRepo: store}
	for _, status := range []string{"received", "sent"} {
		msg := &repository.Message{ChatID: "123@g.us", Status: status, Extra: &repository.MessageExtra{Mentions: []string{lid}}}
		h.enrichMessageMentions(msg, []string{phone})
		if msg.Extra.MentionNames[lid] != "Nama kontak" {
			t.Fatalf("name not resolved: %#v", msg.Extra)
		}
		if msg.Extra.MentionsMe != (status == "received") {
			t.Fatalf("mentionsMe wrong for %s", status)
		}
	}
	msg := &repository.Message{ChatID: "123@g.us", Status: "received", Extra: &repository.MessageExtra{Mentions: []string{"someone@s.whatsapp.net"}}}
	h.enrichMessageMentions(msg, []string{phone})
	if msg.Extra.MentionsMe {
		t.Fatal("mention of another member was marked as own")
	}
	if len(msg.Extra.MentionNames) != 0 {
		t.Fatal("invented a name for an unknown contact")
	}
	unknownPhone, unknownLID := "628222@s.whatsapp.net", "555@lid"
	if err := store.SaveLIDMapping(unknownLID, unknownPhone); err != nil {
		t.Fatal(err)
	}
	msg.Extra.Mentions = []string{unknownLID}
	h.enrichMessageMentions(msg, []string{phone})
	if msg.Extra.MentionNames[unknownLID] != "+628222" {
		t.Fatal("unknown contact still exposes its internal LID")
	}
}

func TestApplyMentionNamesKeepsUnresolvedNumbers(t *testing.T) {
	extra := &repository.MessageExtra{
		Mentions:     []string{"123@lid", "456@lid"},
		MentionNames: map[string]string{"123@lid": "Zoë Putri", "456@lid": "  "},
	}
	got := applyMentionNames("hi @123, @456 and @12345", extra)
	if want := "hi @Zoë Putri, @456 and @12345"; got != want {
		t.Fatalf("got %q, want %q", got, want)
	}
	if got := applyMentionNames("@123", nil); got != "@123" {
		t.Fatalf("nil extra changed the text: %q", got)
	}
}

type receiptsOff struct{}

func (receiptsOff) Get(context.Context, string) (string, error)     { return "false", nil }
func (receiptsOff) Set(context.Context, string, string) error       { return nil }
func (receiptsOff) List(context.Context) (map[string]string, error) { return nil, nil }
func (receiptsOff) Delete(context.Context, string) error            { return nil }

func TestOpeningChatKeepsItUnreadWhenReadReceiptsAreOff(t *testing.T) {
	store, err := repository.NewMessageStore(filepath.Join(t.TempDir(), "messages.db"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = store.Close() })
	chatID := "628123@s.whatsapp.net"
	if err := store.SaveMessage(&repository.Message{ID: "in1", ChatID: chatID, From: chatID, Status: "received", Type: "text", Content: "hi"}); err != nil {
		t.Fatal(err)
	}
	hub := &mentionReadHub{unread: "untouched"}
	handler := NewChatHandler(&Handler{msgRepo: store, hub: hub, settingsRepo: receiptsOff{}})

	post := func(url string) {
		request := mux.SetURLVars(httptest.NewRequest("POST", url, nil), map[string]string{"id": chatID})
		response := httptest.NewRecorder()
		handler.MarkAsRead(response, request)
		if response.Code != 200 {
			t.Fatalf("%s: status %d", url, response.Code)
		}
	}
	post("/chats/x/read")
	if hub.unread != "untouched" {
		t.Fatalf("opening the chat cleared it: %v", hub.unread)
	}
	post("/chats/x/read?explicit=1")
	if hub.unread != 0 {
		t.Fatalf("an explicit mark as read must clear it: %v", hub.unread)
	}
}

func TestCallPeerDropsTheDeviceAndFindsTheName(t *testing.T) {
	store, err := repository.NewMessageStore(filepath.Join(t.TempDir(), "messages.db"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = store.Close() })
	phone, lid := "628111@s.whatsapp.net", "242622887161958@lid"
	if err := store.SaveLIDMapping(lid, phone); err != nil {
		t.Fatal(err)
	}
	if err := store.SaveContact(&repository.Contact{ID: phone, JID: phone, Name: "Nama kontak"}); err != nil {
		t.Fatal(err)
	}
	h := &Handler{msgRepo: store}
	ch := &CallHandler{handler: h}
	id, name := ch.peerOf(h.newNameResolver(), &entity.CallLog{Target: "242622887161958:75@lid"})
	if id != phone || name != "Nama kontak" {
		t.Fatalf("peer = %q %q", id, name)
	}
}

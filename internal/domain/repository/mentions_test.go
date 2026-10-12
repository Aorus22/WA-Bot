package repository

import "testing"

func TestUnreadMentionTracksUnreadIncomingWindowAndAliases(t *testing.T) {
	store := newTestMessageStore(t)
	phone, lid, chatID := "628111@s.whatsapp.net", "242622887161958@lid", "123@g.us"
	if err := store.SaveLIDMapping(lid, phone); err != nil {
		t.Fatal(err)
	}
	write := func(id, status string, timestamp int64, mentions ...string) {
		t.Helper()
		msg := &Message{ID: id, ChatID: chatID, From: "peer", Content: "message", Timestamp: timestamp, Status: status, Type: "text"}
		if len(mentions) > 0 {
			msg.Extra = &MessageExtra{Mentions: mentions}
		}
		if err := store.SaveMessage(msg); err != nil {
			t.Fatal(err)
		}
	}
	want := func(mentioned bool) {
		t.Helper()
		chats, err := store.UnreadMentionChats([]string{phone})
		if err != nil || chats[chatID] != mentioned {
			t.Fatalf("unread mention = %v, want %v, err=%v", chats[chatID], mentioned, err)
		}
	}
	write("mention", "received", 1, lid)
	want(true)
	write("plain", "received", 2)
	write("outgoing", "sent", 3, phone)
	want(true)
	if err := store.MarkAsRead(chatID); err != nil {
		t.Fatal(err)
	}
	want(false)
	write("after-read", "received", 4)
	want(false)
	write("other-member", "received", 5, "other@s.whatsapp.net")
	want(false)
	write("own-phone", "received", 6, phone)
	want(true)
	if _, _, err := store.MarkMessageDeleted("own-phone", 7); err != nil {
		t.Fatal(err)
	}
	want(false)
}

func TestMentionDisplayNameUsesContactAliasesBeforePushNames(t *testing.T) {
	store := newTestMessageStore(t)
	phone, lid := "628111@s.whatsapp.net", "242622887161958@lid"
	if err := store.SaveLIDMapping(lid, phone); err != nil {
		t.Fatal(err)
	}
	if err := store.SaveContact(&Contact{ID: phone, JID: phone, Name: "Saved name"}); err != nil {
		t.Fatal(err)
	}
	if err := store.SaveMessage(&Message{ID: "push", ChatID: "123@g.us", From: lid, SenderName: "Push name", Content: "hello", Type: "text"}); err != nil {
		t.Fatal(err)
	}
	name, err := store.GetContactName(lid)
	if err != nil || name != "Saved name" {
		t.Fatalf("name = %q, err=%v", name, err)
	}
}

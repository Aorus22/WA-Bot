package repository

import (
	"testing"
	"time"
)

func seedMessages(t *testing.T, s *MessageStore, chat string, ids ...string) {
	t.Helper()
	if _, err := s.db.Exec(`INSERT OR IGNORE INTO chats (id, name) VALUES (?, 'c')`, chat); err != nil {
		t.Fatal(err)
	}
	for i, id := range ids {
		msg := &Message{ID: id, ChatID: chat, From: "2@s.whatsapp.net", To: "me", Content: id, Timestamp: int64(i + 1), Status: "received", Type: "text"}
		if err := s.SaveMessage(msg); err != nil {
			t.Fatal(err)
		}
	}
}

func TestStarSurvivesMessageRewriteAndKeepsHistory(t *testing.T) {
	s := newTestStore(t)
	seedMessages(t, s, "1@s.whatsapp.net", "m1", "m2")

	if err := s.SetStarred("1@s.whatsapp.net", "m1", true, 100); err != nil {
		t.Fatal(err)
	}
	if err := s.SetStarred("1@s.whatsapp.net", "m2", true, 200); err != nil {
		t.Fatal(err)
	}

	// History sync rewrites the row with INSERT OR REPLACE; the star must stay.
	seedMessages(t, s, "1@s.whatsapp.net", "m1")

	got, err := s.GetStarredMessages("", 10, 0)
	if err != nil || len(got) != 2 || got[0].ID != "m2" || got[1].ID != "m1" || !got[0].Starred || got[0].StarredAt != 200 {
		t.Fatalf("starred = %+v err=%v", got, err)
	}

	if err := s.SetStarred("1@s.whatsapp.net", "m2", false, 300); err != nil {
		t.Fatal(err)
	}
	got, _ = s.GetStarredMessages("1@s.whatsapp.net", 10, 0)
	if len(got) != 1 || got[0].ID != "m1" {
		t.Fatalf("after unstar = %+v", got)
	}
	if msg, _ := s.GetMessageByID("m2"); msg == nil || msg.Starred {
		t.Fatalf("m2 must be stored and unstarred: %+v", msg)
	}
	if page, _ := s.GetStarredMessages("", 10, 100); len(page) != 0 {
		t.Fatalf("paging before 100 = %+v", page)
	}
	var rows int
	_ = s.db.QueryRow(`SELECT COUNT(*) FROM message_stars`).Scan(&rows)
	if rows != 2 {
		t.Fatalf("star rows are kept, got %d", rows)
	}
}

func TestPinsExpireAndUnpinWithoutDeleting(t *testing.T) {
	s := newTestStore(t)
	chat := "1@s.whatsapp.net"
	seedMessages(t, s, chat, "m1", "m2", "m3")
	now := time.Now().UnixMilli()

	if err := s.PinMessage(chat, "m1", "me", now-1000, now+60_000); err != nil {
		t.Fatal(err)
	}
	if err := s.PinMessage(chat, "m2", "2@s.whatsapp.net", now, now+60_000); err != nil {
		t.Fatal(err)
	}
	if err := s.PinMessage(chat, "m3", "me", now-5000, now-1); err != nil { // already expired
		t.Fatal(err)
	}

	pins, err := s.GetActivePins(chat)
	if err != nil || len(pins) != 2 || pins[0].MessageID != "m2" || pins[0].Message == nil || pins[0].Message.Content != "m2" {
		t.Fatalf("pins = %+v err=%v", pins, err)
	}

	if err := s.UnpinMessage("m2", now+1); err != nil {
		t.Fatal(err)
	}
	pins, _ = s.GetActivePins(chat)
	if len(pins) != 1 || pins[0].MessageID != "m1" {
		t.Fatalf("after unpin = %+v", pins)
	}

	// Re-pinning brings it back.
	_ = s.PinMessage(chat, "m2", "me", now+2, now+60_000)
	if pins, _ = s.GetActivePins(chat); len(pins) != 2 {
		t.Fatalf("after re-pin = %+v", pins)
	}
	var rows int
	_ = s.db.QueryRow(`SELECT COUNT(*) FROM chat_pins`).Scan(&rows)
	if rows != 3 {
		t.Fatalf("pin rows are kept, got %d", rows)
	}
}

func TestChatDisappearingTimer(t *testing.T) {
	s := newTestStore(t)
	chat := "1@s.whatsapp.net"
	seedMessages(t, s, chat, "m1")

	if changed, _ := s.SetChatDisappearing(chat, 86400); !changed {
		t.Fatal("first set should change")
	}
	if changed, _ := s.SetChatDisappearing(chat, 86400); changed {
		t.Fatal("same value is not a change")
	}
	if got := s.GetChatDisappearing(chat); got != 86400 {
		t.Fatalf("timer = %d", got)
	}
	chats, _ := s.GetChats()
	if len(chats) != 1 || chats[0].DisappearingSecs != 86400 {
		t.Fatalf("chats = %+v", chats)
	}

	// Disappearing messages are never removed from our DB.
	if err := s.SaveMessage(&Message{ID: "m2", ChatID: chat, From: "me", To: chat, Content: "x", Timestamp: 5, Status: "sent", Type: "text",
		Extra: &MessageExtra{Disappearing: &DisappearingMeta{Seconds: 86400, ExpiresAt: 1}}}); err != nil {
		t.Fatal(err)
	}
	msgs, _ := s.GetMessages(chat, 10, 0, 0)
	if len(msgs) != 2 || msgs[1].Extra == nil || msgs[1].Extra.Disappearing == nil || msgs[1].Extra.Disappearing.Seconds != 86400 {
		t.Fatalf("messages = %+v", msgs)
	}
}

func TestRevokedMessageIsKeptAndSurvivesRewrite(t *testing.T) {
	s := newTestStore(t)
	chat := "1@s.whatsapp.net"
	seedMessages(t, s, chat, "m1")

	if stored, err := s.MarkMessageRevoked("ghost", 5); err != nil || stored {
		t.Fatalf("unknown message: stored=%v err=%v", stored, err)
	}
	if stored, err := s.MarkMessageRevoked("m1", 500); err != nil || !stored {
		t.Fatalf("stored=%v err=%v", stored, err)
	}
	_, _ = s.MarkMessageRevoked("m1", 900) // echo keeps the first time

	// History sync rewrites the row; the flag must stay.
	seedMessages(t, s, chat, "m1")

	msgs, _ := s.GetMessages(chat, 10, 0, 0)
	if len(msgs) != 1 || msgs[0].RevokedAt != 500 || msgs[0].Content != "m1" {
		t.Fatalf("messages = %+v", msgs)
	}
}

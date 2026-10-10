package repository

import (
	"path/filepath"
	"testing"
)

func newTestStore(t *testing.T) *MessageStore {
	t.Helper()
	s, err := NewMessageStore(filepath.Join(t.TempDir(), "msgs.db"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { s.Close() })
	return s
}

func save(t *testing.T, s *MessageStore, id string, ts int64) {
	t.Helper()
	if err := s.SaveMessage(&Message{ID: id, ChatID: "123@s.whatsapp.net", From: "123@s.whatsapp.net", To: "me", Content: id, Timestamp: ts, Status: "received", Type: "text"}); err != nil {
		t.Fatal(err)
	}
}

func visible(t *testing.T, s *MessageStore) int {
	t.Helper()
	msgs, err := s.GetMessages("123@s.whatsapp.net", 50, 0, 0)
	if err != nil {
		t.Fatal(err)
	}
	return len(msgs)
}

func rowCount(t *testing.T, s *MessageStore) int {
	t.Helper()
	var n int
	if err := s.db.QueryRow(`SELECT COUNT(*) FROM messages`).Scan(&n); err != nil {
		t.Fatal(err)
	}
	return n
}

func TestDeleteForMeKeepsRowAndRefreshesPreview(t *testing.T) {
	s := newTestStore(t)
	save(t, s, "m1", 1000)
	save(t, s, "m2", 2000)

	chatID, last, err := s.MarkMessageDeleted("m2", 3000)
	if err != nil || chatID != "123@s.whatsapp.net" || last != "m1" {
		t.Fatalf("got %q %q, %v", chatID, last, err)
	}
	if visible(t, s) != 1 || rowCount(t, s) != 2 {
		t.Fatalf("visible=%d rows=%d", visible(t, s), rowCount(t, s))
	}
	chats, _ := s.GetChats()
	if len(chats) != 1 || chats[0].LastMsg != "m1" {
		t.Fatalf("preview not refreshed: %+v", chats)
	}
	if id, _, _ := s.MarkMessageDeleted("m2", 3000); id != "" {
		t.Fatal("second delete should be a no-op")
	}
}

func TestClearChatKeepsChatEmpty(t *testing.T) {
	s := newTestStore(t)
	save(t, s, "m1", 1000)
	save(t, s, "m2", 2000)
	if err := s.MarkChatCleared("123@s.whatsapp.net", 2000); err != nil {
		t.Fatal(err)
	}
	chats, _ := s.GetChats()
	if len(chats) != 1 || chats[0].LastMsg != "" || chats[0].Unread != 0 {
		t.Fatalf("chat should stay, empty: %+v", chats)
	}
	if visible(t, s) != 0 || rowCount(t, s) != 2 {
		t.Fatalf("visible=%d rows=%d", visible(t, s), rowCount(t, s))
	}
}

func TestDeleteChatHidesAndRevivesOnNewerMessage(t *testing.T) {
	s := newTestStore(t)
	save(t, s, "m1", 1000)

	if ok, err := s.MarkChatDeleted("123@s.whatsapp.net", 500); err != nil || ok {
		t.Fatalf("stale delete must be ignored: %v %v", ok, err)
	}
	if ok, err := s.MarkChatDeleted("123@s.whatsapp.net", 2000); err != nil || !ok {
		t.Fatalf("delete failed: %v %v", ok, err)
	}
	if chats, _ := s.GetChats(); len(chats) != 0 {
		t.Fatalf("chat should be hidden: %+v", chats)
	}
	if rowCount(t, s) != 1 {
		t.Fatal("messages must be kept")
	}

	save(t, s, "m2", 3000)
	chats, _ := s.GetChats()
	if len(chats) != 1 {
		t.Fatalf("chat should be revived: %+v", chats)
	}
	if visible(t, s) != 1 {
		t.Fatalf("only the new message should be visible, got %d", visible(t, s))
	}
}

func TestSaveContactKeepsAbout(t *testing.T) {
	s := newTestStore(t)
	if err := s.SaveContactAbout("123@s.whatsapp.net", "busy"); err != nil {
		t.Fatal(err)
	}
	if err := s.SaveContact(&Contact{ID: "123@s.whatsapp.net", Name: "Bob", JID: "123@s.whatsapp.net"}); err != nil {
		t.Fatal(err)
	}
	cs, _ := s.GetContacts()
	if len(cs) != 1 || cs[0].About != "busy" || cs[0].Name != "Bob" {
		t.Fatalf("%+v", cs)
	}
}

func TestApplyPushNameRespectsAddressBook(t *testing.T) {
	s := newTestStore(t)
	save(t, s, "m1", 1000)
	jid := "123@s.whatsapp.net"
	_, _ = s.db.Exec(`UPDATE chats SET name = 'Old Push' WHERE id = ?`, jid)
	_ = s.SaveContact(&Contact{ID: jid, Name: "Old Push", JID: jid})

	renamed, err := s.ApplyPushName(jid, "Old Push", "New Push")
	if err != nil || !renamed {
		t.Fatalf("%v %v", renamed, err)
	}
	// Name set by the user is not overwritten.
	_, _ = s.db.Exec(`UPDATE chats SET name = 'Mama' WHERE id = ?`, jid)
	if renamed, _ := s.ApplyPushName(jid, "New Push", "Third"); renamed {
		t.Fatal("address-book chat name must be kept")
	}
}

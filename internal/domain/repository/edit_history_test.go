package repository

import "testing"

func TestApplyEditKeepsEveryEarlierVersion(t *testing.T) {
	s := newTestStore(t)
	if _, err := s.db.Exec(`INSERT INTO messages (id, chat_id, sender_id, receiver_id, content, timestamp, status, msg_type)
		VALUES ('m1', 'c@s.whatsapp.net', 'me', 'c@s.whatsapp.net', 'first', 1000, 'sent', 'text')`); err != nil {
		t.Fatal(err)
	}

	extra, err := s.ApplyEdit("m1", "second", 2000)
	if err != nil {
		t.Fatal(err)
	}
	if len(extra.Edits) != 1 || extra.Edits[0].Content != "first" || extra.Edits[0].Timestamp != 1000 || extra.EditedAt != 2000 {
		t.Fatalf("after one edit: %+v", extra)
	}

	extra, err = s.ApplyEdit("m1", "third", 3000)
	if err != nil {
		t.Fatal(err)
	}
	want := []MessageEdit{{Content: "first", Timestamp: 1000}, {Content: "second", Timestamp: 2000}}
	if len(extra.Edits) != 2 || extra.Edits[0] != want[0] || extra.Edits[1] != want[1] || extra.EditedAt != 3000 {
		t.Fatalf("after two edits: %+v", extra)
	}

	// The same text again is not another version.
	if extra, _ = s.ApplyEdit("m1", "third", 4000); len(extra.Edits) != 2 || extra.EditedAt != 3000 {
		t.Fatalf("repeat changed the history: %+v", extra)
	}

	stored, err := s.GetMessageByID("m1")
	if err != nil {
		t.Fatal(err)
	}
	if stored.Content != "third" || stored.Extra == nil || len(stored.Extra.Edits) != 2 {
		t.Fatalf("stored message: %+v", stored)
	}

	if _, err := s.ApplyEdit("missing", "x", 1); err == nil {
		t.Fatal("editing an unknown message must fail")
	}
}

func TestSyncChatPreviewOnlyFollowsTheNewestMessage(t *testing.T) {
	s := newTestStore(t)
	const chat = "c@s.whatsapp.net"
	for _, m := range []struct {
		id, content string
		at          int64
	}{{"old", "older", 1000}, {"new", "newest", 2000}} {
		if _, err := s.db.Exec(`INSERT INTO messages (id, chat_id, sender_id, receiver_id, content, timestamp, status, msg_type)
			VALUES (?, ?, 'me', ?, ?, ?, 'sent', 'text')`, m.id, chat, chat, m.content, m.at); err != nil {
			t.Fatal(err)
		}
	}
	if _, err := s.db.Exec(`INSERT INTO chats (id, name, last_msg, last_time, is_group) VALUES (?, 'C', 'newest', 2000, 0)`, chat); err != nil {
		t.Fatal(err)
	}

	if _, changed := s.SyncChatPreview("old", "older, edited"); changed {
		t.Error("editing an older message must not touch the preview")
	}
	if id, changed := s.SyncChatPreview("new", "newest, edited"); !changed || id != chat {
		t.Errorf("editing the newest message must update the preview (id=%q changed=%v)", id, changed)
	}
	var preview string
	if err := s.db.QueryRow(`SELECT last_msg FROM chats WHERE id = ?`, chat).Scan(&preview); err != nil || preview != "newest, edited" {
		t.Errorf("preview = %q, err = %v", preview, err)
	}
}

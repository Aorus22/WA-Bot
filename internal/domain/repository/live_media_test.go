package repository

import "testing"

func TestLiveMediaWithoutAFileGetsADownloadSource(t *testing.T) {
	s := newTestStore(t)
	video := &Message{ID: "v1", ChatID: "1@s.whatsapp.net", From: "1@s.whatsapp.net", To: "me",
		Content: "[Video]", Timestamp: 1000, Status: "received", Type: "video", RawProto: []byte{1, 2}}
	text := &Message{ID: "t1", ChatID: "1@s.whatsapp.net", From: "1@s.whatsapp.net", To: "me",
		Content: "hi", Timestamp: 2000, Status: "received", Type: "text", RawProto: []byte{3}}
	stored := &Message{ID: "i1", ChatID: "1@s.whatsapp.net", From: "1@s.whatsapp.net", To: "me",
		Content: "[Image]", Timestamp: 3000, Status: "received", Type: "image", MediaURL: "/media/a.jpg", RawProto: []byte{4}}
	for _, m := range []*Message{video, text, stored} {
		if err := s.SaveMessage(m); err != nil {
			t.Fatal(err)
		}
	}
	rows, err := s.LiveMediaWithoutSource()
	if err != nil {
		t.Fatal(err)
	}
	if len(rows) != 1 || rows[0].ID != "v1" || string(rows[0].Proto) != string([]byte{1, 2}) {
		t.Fatalf("want only the video without a file, got %+v", rows)
	}
	if err := s.SetMediaSource("v1", []byte{9}, "/chats/x/messages/v1/media"); err != nil {
		t.Fatal(err)
	}
	raw, url, err := s.GetRawMessage("v1")
	if err != nil || string(raw) != string([]byte{9}) || url != "/chats/x/messages/v1/media" {
		t.Fatalf("source not stored: %v %q %v", raw, url, err)
	}
	if rows, _ := s.LiveMediaWithoutSource(); len(rows) != 0 {
		t.Fatalf("repaired message listed again: %+v", rows)
	}
	// Saving the message again (an edit, a reaction) keeps its source.
	video.MediaURL = url
	if err := s.SaveMessage(video); err != nil {
		t.Fatal(err)
	}
	if raw, _, _ := s.GetRawMessage("v1"); string(raw) != string([]byte{9}) {
		t.Fatalf("source lost on save: %v", raw)
	}
}

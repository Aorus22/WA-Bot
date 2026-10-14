package repository

import (
	"fmt"
	"path/filepath"
	"testing"
)

func newTestMessageStore(t *testing.T) *MessageStore {
	t.Helper()
	store, err := NewMessageStore(filepath.Join(t.TempDir(), "messages.db"))
	if err != nil {
		t.Fatalf("NewMessageStore: %v", err)
	}
	t.Cleanup(func() { _ = store.Close() })
	return store
}

func TestHistoryStagingIsInvisibleAndImportsFiftyPerBatch(t *testing.T) {
	store := newTestMessageStore(t)
	chatID := "628123@s.whatsapp.net"
	staged := make([]StagedHistoryMessage, 0, 75)
	for i := 1; i <= 75; i++ {
		staged = append(staged, StagedHistoryMessage{
			ChatID: chatID, MessageID: fmt.Sprintf("m-%03d", i), Timestamp: int64(i), Raw: []byte{byte(i)},
		})
	}
	if err := store.StageHistoryConversation(HistoryConversation{
		ChatID: chatID, Name: "History Contact", UnreadCount: 7,
	}, staged); err != nil {
		t.Fatalf("StageHistoryConversation: %v", err)
	}

	chats, err := store.GetChats()
	if err != nil {
		t.Fatal(err)
	}
	if len(chats) != 0 {
		t.Fatalf("staging leaked into visible chats: %#v", chats)
	}

	first, err := store.PendingHistory(chatID, 50)
	if err != nil {
		t.Fatal(err)
	}
	if len(first) != 50 {
		t.Fatalf("first batch = %d, want 50", len(first))
	}
	added, err := store.ImportHistoryBatch(chatID, first, projectTestHistory(first))
	if err != nil {
		t.Fatal(err)
	}
	if added != 50 {
		t.Fatalf("added = %d, want 50", added)
	}

	chats, _ = store.GetChats()
	if len(chats) != 1 || chats[0].Unread != 7 || chats[0].LastTime != 75 {
		t.Fatalf("unexpected imported chat: %#v", chats)
	}
	remaining, _ := store.PendingHistory(chatID, 50)
	if len(remaining) != 25 {
		t.Fatalf("remaining = %d, want 25", len(remaining))
	}
	added, err = store.ImportHistoryBatch(chatID, remaining, projectTestHistory(remaining))
	if err != nil || added != 25 {
		t.Fatalf("second import: added=%d err=%v", added, err)
	}
	remaining, _ = store.PendingHistory(chatID, 50)
	if len(remaining) != 0 {
		t.Fatalf("remaining after second import = %d", len(remaining))
	}
}

func TestHistoryImportNeverOverwritesExistingMessageOrChatPreview(t *testing.T) {
	store := newTestMessageStore(t)
	chatID := "628456@s.whatsapp.net"
	if err := store.SaveMessage(&Message{
		ID: "same-id", ChatID: chatID, From: chatID, To: "me", Content: "current",
		Timestamp: 10_000, Status: "received", Type: "text", ChatName: "Current Name",
	}); err != nil {
		t.Fatal(err)
	}
	staged := []StagedHistoryMessage{{ChatID: chatID, MessageID: "same-id", Timestamp: 100, Raw: []byte("raw")}}
	if err := store.StageHistoryConversation(HistoryConversation{ChatID: chatID, Name: "Old Name", UnreadCount: 99}, staged); err != nil {
		t.Fatal(err)
	}
	imports := []HistoryImportMessage{{
		Message: Message{ID: "same-id", ChatID: chatID, From: chatID, To: "me", Content: "old replacement", Timestamp: 100, Status: "received", Type: "text"},
		Raw:     []byte("raw"),
	}}
	added, err := store.ImportHistoryBatch(chatID, staged, imports)
	if err != nil {
		t.Fatal(err)
	}
	if added != 0 {
		t.Fatalf("duplicate import added %d rows", added)
	}
	messages, _ := store.GetMessages(chatID, 10, 0, 0)
	if len(messages) != 1 || messages[0].Content != "current" {
		t.Fatalf("existing message was overwritten: %#v", messages)
	}
	chats, _ := store.GetChats()
	if len(chats) != 1 || chats[0].Name != "Current Name" || chats[0].LastMsg != "current" || chats[0].LastTime != 10_000 || chats[0].Unread != 1 {
		t.Fatalf("existing chat regressed: %#v", chats)
	}
}

func projectTestHistory(staged []StagedHistoryMessage) []HistoryImportMessage {
	out := make([]HistoryImportMessage, 0, len(staged))
	for _, item := range staged {
		out = append(out, HistoryImportMessage{
			Message: Message{
				ID: item.MessageID, ChatID: item.ChatID, From: item.ChatID, To: "me",
				Content: item.MessageID, Timestamp: item.Timestamp, Status: "received", Type: "text",
			},
			Raw: item.Raw,
		})
	}
	return out
}

func TestHistoryJobKeepsItsPlace(t *testing.T) {
	store := newTestMessageStore(t)
	if err := store.BeginHistoryJob([]string{"c", "a", "b", "d"}); err != nil {
		t.Fatal(err)
	}
	// Chats are visited in the order they were given, not alphabetically.
	pending, _ := store.PendingHistoryJobChats()
	if fmt.Sprint(pending) != "[c a b d]" {
		t.Fatalf("pending: %v", pending)
	}

	store.RecordHistoryJobChat("c", 30, true, "")
	store.RecordHistoryJobChat("a", 20, false, "") // stopped midway: not done, but what it added counts
	store.RecordHistoryJobChat("a", 5, true, "exhausted")
	store.RecordHistoryJobChat("b", 0, true, "timeout")

	pending, _ = store.PendingHistoryJobChats()
	if fmt.Sprint(pending) != "[d]" {
		t.Fatalf("pending after progress: %v", pending)
	}
	totals, err := store.HistoryJobTotals()
	if err != nil {
		t.Fatal(err)
	}
	want := HistoryJobTotals{Chats: 4, Done: 3, Added: 55, WithNew: 2, Exhausted: 2, Timeouts: 1}
	if totals != want {
		t.Fatalf("totals %+v, want %+v", totals, want)
	}

	// A new job replaces the old one.
	store.BeginHistoryJob(nil)
	if totals, _ := store.HistoryJobTotals(); totals != (HistoryJobTotals{}) {
		t.Fatalf("old job survived: %+v", totals)
	}
}

func TestHistorySyncStatusRoundTripsItsOptionsAndTime(t *testing.T) {
	store := newTestMessageStore(t)
	now := int64(1700000000000)
	in := HistorySyncStatus{State: "interrupted", StartedAt: &now, ChatsSkipped: 7, ActiveMs: 123456,
		Options: HistorySyncOptions{All: true, IncludeExhausted: true}, Errors: []HistorySyncError{}}
	if err := store.SetHistorySyncStatus(in); err != nil {
		t.Fatal(err)
	}
	out, err := store.GetHistorySyncStatus()
	if err != nil {
		t.Fatal(err)
	}
	if !out.Options.All || !out.Options.IncludeExhausted || out.ChatsSkipped != 7 || out.ActiveMs != 123456 || out.State != "interrupted" {
		t.Fatalf("round trip: %+v", out)
	}
}

func TestFinishedChatCountIsWhatASyncSkips(t *testing.T) {
	store := newTestMessageStore(t)
	chatID := "628123@s.whatsapp.net"
	if err := store.StageHistoryConversation(HistoryConversation{ChatID: chatID, Name: "A"}, []StagedHistoryMessage{
		{ChatID: chatID, MessageID: "m1", Timestamp: 1, Raw: []byte{1}},
	}); err != nil {
		t.Fatal(err)
	}
	if n := store.FinishedChatCount(); n != 0 {
		t.Fatalf("nothing is finished yet: %d", n)
	}
	store.MarkHistoryExhausted(chatID)
	// Finished, but messages are still waiting to be imported: a sync does not skip it.
	if n := store.FinishedChatCount(); n != 0 {
		t.Fatalf("a chat with staged messages is not skipped: %d", n)
	}
	staged, _ := store.PendingHistory(chatID, 10)
	if _, err := store.ImportHistoryBatch(chatID, staged, nil); err != nil {
		t.Fatal(err)
	}
	if n := store.FinishedChatCount(); n != 1 {
		t.Fatalf("finished chat not counted: %d", n)
	}
}

func TestHistoryMediaSelection(t *testing.T) {
	store := newTestMessageStore(t)
	chatID := "628123@s.whatsapp.net"
	var imports []HistoryImportMessage
	for i, kind := range []string{"image", "image", "ptt", "audio", "video", "document", "text"} {
		id := fmt.Sprintf("m%d", i)
		url := fmt.Sprintf("/chats/%s/messages/%s/media", chatID, id)
		if kind == "text" {
			url = ""
		}
		imports = append(imports, HistoryImportMessage{
			Message: Message{ID: id, ChatID: chatID, From: chatID, To: "me", Content: kind, Timestamp: int64(100 + i), Status: "received", Type: kind, MediaURL: url},
			Raw:     []byte(id),
		})
	}
	// An already downloaded one is not selected.
	imports = append(imports, HistoryImportMessage{
		Message: Message{ID: "cached", ChatID: chatID, From: chatID, To: "me", Content: "x", Timestamp: 200, Status: "received", Type: "image", MediaURL: "/media/history/x.jpg"},
		Raw:     []byte("cached"),
	})
	if _, err := store.ImportHistoryBatch(chatID, nil, imports); err != nil {
		t.Fatal(err)
	}

	avail, expired := store.AvailableHistoryMedia()
	if expired["image"] != 0 {
		t.Fatalf("nothing is expired yet: %v", expired)
	}
	if avail["image"] != 2 || avail["audio"] != 2 || avail["video"] != 1 || avail["document"] != 1 || avail["sticker"] != 0 {
		t.Fatalf("available: %v", avail)
	}
	opts := HistoryMediaOptions{Types: []string{"audio", "video"}}
	if n := store.CountHistoryMedia(opts, 1); n != 3 {
		t.Fatalf("audio (incl. voice notes) and video: %d", n)
	}
	if n := store.CountHistoryMedia(HistoryMediaOptions{}, 1); n != 0 {
		t.Fatalf("nothing chosen selects nothing: %d", n)
	}

	// Newest first, in pages that continue after the cursor.
	ts, id := FirstHistoryMediaCursor()
	page, err := store.NextHistoryMedia(opts, 1, ts, id, 2)
	if err != nil || len(page) != 2 || page[0].MessageID != "m4" || page[1].MessageID != "m3" {
		t.Fatalf("first page: %v %v", page, err)
	}
	page, _ = store.NextHistoryMedia(opts, 1, page[1].Timestamp, page[1].MessageID, 2)
	if len(page) != 1 || page[0].MessageID != "m2" {
		t.Fatalf("second page: %v", page)
	}

	// Gone media is left alone; a failure only for the job it happened in.
	store.MarkHistoryMedia("m4", "unavailable", "gone", 1)
	store.MarkHistoryMedia("m3", "failed", "net", 1)
	if n := store.CountHistoryMedia(opts, 1); n != 1 {
		t.Fatalf("in the job of the failure: %d", n)
	}
	if n := store.CountHistoryMedia(opts, 2); n != 2 {
		t.Fatalf("in the next job (m3 is tried again, m4 is not): %d", n)
	}
	if n := store.CountHistoryMedia(HistoryMediaOptions{Types: opts.Types, RetryUnavailable: true}, 2); n != 3 {
		t.Fatalf("retrying unavailable: %d", n)
	}
	if u, f, m := store.HistoryMediaFailures(1); u != 1 || f != 1 || m != 0 {
		t.Fatalf("failures: %d %d %d", u, f, m)
	}

	if err := store.HistoryMediaDownloaded("m3", "/media/history/m3.jpg"); err != nil {
		t.Fatal(err)
	}
	if u, f, _ := store.HistoryMediaFailures(1); u != 1 || f != 0 {
		t.Fatalf("a download clears its failure: %d %d", u, f)
	}
}

func TestDatabaseSizeGrowsWithMessages(t *testing.T) {
	store := newTestMessageStore(t)
	before, err := store.DatabaseSize()
	if err != nil || before <= 0 {
		t.Fatalf("an empty database still has pages: %d %v", before, err)
	}
	chatID := "628123@s.whatsapp.net"
	var staged []StagedHistoryMessage
	for i := 0; i < 200; i++ {
		staged = append(staged, StagedHistoryMessage{ChatID: chatID, MessageID: fmt.Sprintf("m%d", i), Timestamp: int64(i), Raw: make([]byte, 4096)})
	}
	if err := store.StageHistoryConversation(HistoryConversation{ChatID: chatID}, staged); err != nil {
		t.Fatal(err)
	}
	after, _ := store.DatabaseSize()
	if after <= before {
		t.Fatalf("size did not grow with 800 KB of staged history: %d -> %d", before, after)
	}
}

func TestHistoryMediaSelectionWithThePhone(t *testing.T) {
	store := newTestMessageStore(t)
	chatID := "628123@s.whatsapp.net"
	var imports []HistoryImportMessage
	for i, id := range []string{"new", "gone", "lost"} {
		imports = append(imports, HistoryImportMessage{
			Message: Message{ID: id, ChatID: chatID, From: chatID, To: "me", Content: "x", Timestamp: int64(100 + i), Status: "received", Type: "image",
				MediaURL: fmt.Sprintf("/chats/%s/messages/%s/media", chatID, id)},
			Raw: []byte(id),
		})
	}
	if _, err := store.ImportHistoryBatch(chatID, nil, imports); err != nil {
		t.Fatal(err)
	}
	store.MarkHistoryMedia("gone", "unavailable", "404", 1) // the servers no longer have it
	store.MarkHistoryMedia("lost", "missing", "phone", 1)   // the phone does not have it either

	plain := HistoryMediaOptions{Types: []string{"image"}}
	phone := HistoryMediaOptions{Types: []string{"image"}, AskPhone: true}
	retry := HistoryMediaOptions{Types: []string{"image"}, RetryUnavailable: true}
	// A later job (id 2): only the new file, unless the phone is asked, which
	// also takes up what the servers lost; what the phone lacks needs a retry.
	if n := store.CountHistoryMedia(plain, 2); n != 1 {
		t.Fatalf("plain: %d", n)
	}
	if n := store.CountHistoryMedia(phone, 2); n != 2 {
		t.Fatalf("asking the phone: %d", n)
	}
	if n := store.CountHistoryMedia(retry, 2); n != 3 {
		t.Fatalf("retrying: %d", n)
	}

	available, expired := store.AvailableHistoryMedia()
	if available["image"] != 1 || expired["image"] != 1 {
		t.Fatalf("on offer: available %v expired %v (what the phone lacks is in neither)", available, expired)
	}
	if u, f, m := store.HistoryMediaFailures(1); u != 1 || f != 0 || m != 1 {
		t.Fatalf("failures: %d %d %d", u, f, m)
	}

	// Forgetting the "no answer" failures makes them to do again.
	store.MarkHistoryMedia("new", "failed", "the phone did not answer", 2)
	if n := store.CountHistoryMedia(plain, 2); n != 0 {
		t.Fatalf("a failure is left alone within its job: %d", n)
	}
	if err := store.ForgetHistoryMediaFailures(2, "the phone did not answer"); err != nil {
		t.Fatal(err)
	}
	if n := store.CountHistoryMedia(plain, 2); n != 1 {
		t.Fatalf("after forgetting: %d", n)
	}
}

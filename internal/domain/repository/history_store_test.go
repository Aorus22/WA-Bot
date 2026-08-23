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

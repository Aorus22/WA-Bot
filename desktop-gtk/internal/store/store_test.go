package store

import (
	"testing"

	"wa-bot-desktop/internal/api"
)

func TestChatsSortPinnedBeforeRecent(t *testing.T) {
	pinnedAt := int64(100)
	s := New()
	s.ReplaceChats([]api.Chat{
		{ID: "recent", LastTime: 500},
		{ID: "pinned", LastTime: 10, PinnedAt: &pinnedAt},
	})
	chats := s.Chats()
	if len(chats) != 2 || chats[0].ID != "pinned" {
		t.Fatalf("unexpected chat order: %#v", chats)
	}
}

func TestPatchChatState(t *testing.T) {
	s := New()
	s.ReplaceChats([]api.Chat{{ID: "chat", LastTime: 1, MuteMode: "off"}})
	until := int64(1234)
	s.PatchChatState(api.ChatState{
		ChatID: "chat", Archived: true, MuteMode: "until", MutedUntil: &until,
	})
	chat, ok := s.Chat("chat")
	if !ok || !chat.Archived || chat.MuteMode != "until" || chat.MutedUntil == nil || *chat.MutedUntil != until {
		t.Fatalf("unexpected patched chat: %#v", chat)
	}
}

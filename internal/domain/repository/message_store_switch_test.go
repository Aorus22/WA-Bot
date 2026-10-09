package repository

import (
	"path/filepath"
	"testing"
)

func countChats(t *testing.T, s *MessageStore) int {
	t.Helper()
	var n int
	if err := s.db.QueryRow("SELECT count(*) FROM chats").Scan(&n); err != nil {
		t.Fatalf("count chats: %v", err)
	}
	return n
}

func TestSwitchKeepsDataPerDatabase(t *testing.T) {
	dir := t.TempDir()
	pathA := filepath.Join(dir, "accounts", "111", "database", "messages.db")
	pathB := filepath.Join(dir, "accounts", "222", "database", "messages.db")

	store, err := NewMessageStore(filepath.Join(dir, "pending.db"))
	if err != nil {
		t.Fatalf("NewMessageStore: %v", err)
	}
	t.Cleanup(func() { _ = store.Close() })

	if err := store.Switch(pathA); err != nil {
		t.Fatalf("Switch A: %v", err)
	}
	if _, err := store.db.Exec("INSERT INTO chats (id, name) VALUES ('a@s.whatsapp.net', 'A')"); err != nil {
		t.Fatalf("insert A: %v", err)
	}

	if err := store.Switch(pathB); err != nil {
		t.Fatalf("Switch B: %v", err)
	}
	if got := countChats(t, store); got != 0 {
		t.Fatalf("account B sees %d chats from another account, want 0", got)
	}

	if err := store.Switch(pathA); err != nil {
		t.Fatalf("Switch back to A: %v", err)
	}
	if got := countChats(t, store); got != 1 {
		t.Fatalf("account A has %d chats after switching back, want 1", got)
	}
}

package handlers

import (
	"path/filepath"
	"testing"
	"time"

	"wa-bot/internal/domain/repository"
)

type notificationTestHub struct {
	payloads []map[string]interface{}
}

func (h *notificationTestHub) BroadcastMessage(kind string, payload interface{}) {
	if kind == "new_message" {
		h.payloads = append(h.payloads, payload.(map[string]interface{}))
	}
}
func (*notificationTestHub) SendMessageToUser(string, string, interface{}) {}
func (*notificationTestHub) Run()                                          {}

func TestNotificationSuppressionIsBroadcastFromStoredChatState(t *testing.T) {
	now := time.Now().UnixMilli()
	future, past := now+60000, now-60000
	for _, tc := range []struct {
		name       string
		archived   bool
		muteMode   string
		mutedUntil *int64
		silent     bool
	}{
		{"normal", false, "off", nil, false},
		{"archived", true, "off", nil, true},
		{"muted forever", false, "forever", nil, true},
		{"muted until future", false, "until", &future, true},
		{"muted until unknown", false, "until", nil, true},
		{"expired mute", false, "until", &past, false},
		{"archived with expired mute", true, "until", &past, true},
	} {
		t.Run(tc.name, func(t *testing.T) {
			store, err := repository.NewMessageStore(filepath.Join(t.TempDir(), "messages.db"))
			if err != nil {
				t.Fatal(err)
			}
			t.Cleanup(func() { _ = store.Close() })
			chatID := "123@g.us"
			seed := &repository.Message{ID: "seed", ChatID: chatID, From: "peer", Content: "seed", Timestamp: now, Type: "text"}
			if err := store.SaveMessage(seed); err != nil {
				t.Fatal(err)
			}
			state := repository.ChatState{ChatID: chatID, Archived: tc.archived, MuteMode: tc.muteMode, MutedUntil: tc.mutedUntil}
			if err := store.UpdateChatState(state); err != nil {
				t.Fatal(err)
			}
			hub := &notificationTestHub{}
			handler := &Handler{msgRepo: store, hub: hub}
			incoming := &repository.Message{ID: "incoming", ChatID: chatID, From: "peer", Content: "hello", Timestamp: now, Type: "text"}
			handler.SaveAndBroadcastMessage(incoming)
			if len(hub.payloads) != 1 {
				t.Fatalf("broadcast count = %d, want 1", len(hub.payloads))
			}
			if got := hub.payloads[0]["notificationSuppressed"]; got != tc.silent {
				t.Fatalf("notificationSuppressed = %v, want %v", got, tc.silent)
			}
			// Automatic unarchive after this event must not change its policy.
			state.Archived = false
			if err := store.UpdateChatState(state); err != nil {
				t.Fatal(err)
			}
			if hub.payloads[0]["notificationSuppressed"] != tc.silent {
				t.Fatal("broadcast policy changed after unarchive")
			}
			handler.SaveAndBroadcastMessage(incoming)
			if len(hub.payloads) != 1 {
				t.Fatal("duplicate message was broadcast again")
			}
		})
	}
}

func TestNotificationSuppressionIncludesNewChatAliases(t *testing.T) {
	phone, lid := "628123@s.whatsapp.net", "987@lid"
	for _, existing := range []string{phone, lid} {
		for _, mode := range []string{"archive", "mute"} {
			t.Run(existing+"/"+mode, func(t *testing.T) {
				store, err := repository.NewMessageStore(filepath.Join(t.TempDir(), "messages.db"))
				if err != nil {
					t.Fatal(err)
				}
				t.Cleanup(func() { _ = store.Close() })
				if err := store.SaveLIDMapping(lid, phone); err != nil {
					t.Fatal(err)
				}
				if err := store.SaveMessage(&repository.Message{ID: "seed", ChatID: existing, Content: "seed", Type: "text"}); err != nil {
					t.Fatal(err)
				}
				state := repository.ChatState{ChatID: existing, MuteMode: "off", Archived: mode == "archive"}
				if mode == "mute" {
					state.MuteMode = "forever"
				}
				if err := store.UpdateChatState(state); err != nil {
					t.Fatal(err)
				}
				incomingID := lid
				if existing == lid {
					incomingID = phone
				}
				hub := &notificationTestHub{}
				handler := &Handler{msgRepo: store, hub: hub}
				handler.SaveAndBroadcastMessage(&repository.Message{ID: "new-alias", ChatID: incomingID, From: "peer", Content: "hello", Type: "text"})
				if len(hub.payloads) != 1 || hub.payloads[0]["notificationSuppressed"] != true {
					t.Fatalf("new alias bypassed suppression: %#v", hub.payloads)
				}
			})
		}
	}
}

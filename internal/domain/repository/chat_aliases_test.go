package repository

import (
	"reflect"
	"testing"
)

// The app lists a migrated chat under the phone number, but the phone knows
// it by its LID: app-state changes (archive, pin, mute) must reach that form.
func TestChatAliasesIncludeTheLIDTheChatIsStoredUnder(t *testing.T) {
	s := newTestStore(t)
	const lid, pn = "111@lid", "628111@s.whatsapp.net"
	if err := s.SaveLIDMapping(lid, pn); err != nil {
		t.Fatal(err)
	}
	if _, err := s.db.Exec(`INSERT INTO chats (id, name, is_group) VALUES (?, 'A', 0)`, lid); err != nil {
		t.Fatal(err)
	}

	for _, ask := range []string{pn, lid} {
		if got := s.ChatAliases(ask); !reflect.DeepEqual(got, []string{lid}) {
			t.Errorf("ChatAliases(%q) = %v, want only the stored LID", ask, got)
		}
	}

	if _, err := s.db.Exec(`INSERT INTO chats (id, name, is_group) VALUES (?, 'A', 0)`, pn); err != nil {
		t.Fatal(err)
	}
	if got := s.ChatAliases(pn); !reflect.DeepEqual(got, []string{lid, pn}) {
		t.Errorf("both stored forms expected, LID first, got %v", got)
	}
	if got := s.ChatAliases("999@s.whatsapp.net"); len(got) != 0 {
		t.Errorf("unknown chat has aliases: %v", got)
	}
}

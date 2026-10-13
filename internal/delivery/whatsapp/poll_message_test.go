package whatsapp

import (
	"testing"

	"go.mau.fi/whatsmeow/proto/waE2E"
	"google.golang.org/protobuf/proto"
)

func TestPollCreationFindsEveryPollField(t *testing.T) {
	poll := &waE2E.PollCreationMessage{Name: proto.String("Lunch?")}
	cases := map[string]*waE2E.Message{
		"v1": {PollCreationMessage: poll},
		"v2": {PollCreationMessageV2: poll},
		"v3": {PollCreationMessageV3: poll},
		"v4": {PollCreationMessageV4: &waE2E.FutureProofMessage{Message: &waE2E.Message{PollCreationMessageV3: poll}}},
		"v5": {PollCreationMessageV5: poll},
		"v6": {PollCreationMessageV6: poll},
	}
	for name, msg := range cases {
		if got := pollCreation(msg); got == nil || got.GetName() != "Lunch?" {
			t.Errorf("%s: poll not found", name)
		}
	}
	if pollCreation(&waE2E.Message{Conversation: proto.String("hi")}) != nil || pollCreation(nil) != nil {
		t.Error("non-poll message reported as a poll")
	}
}

func TestProtocolMessageKinds(t *testing.T) {
	revoke := &waE2E.ProtocolMessage{Type: waE2E.ProtocolMessage_REVOKE.Enum()}
	edit := &waE2E.ProtocolMessage{Type: waE2E.ProtocolMessage_MESSAGE_EDIT.Enum()}
	other := &waE2E.ProtocolMessage{Type: waE2E.ProtocolMessage_HISTORY_SYNC_NOTIFICATION.Enum()}

	if !isRevoke(revoke) || isEdit(revoke) {
		t.Error("a revoke must be a revoke and not an edit")
	}
	if !isEdit(edit) || isRevoke(edit) {
		t.Error("an edit must be an edit and not a revoke (it used to delete the message)")
	}
	if isRevoke(other) || isEdit(other) || isRevoke(nil) || isEdit(nil) {
		t.Error("other protocol messages are neither")
	}
	// The wire values, so a renumbered enum is noticed.
	if waE2E.ProtocolMessage_REVOKE != 0 || waE2E.ProtocolMessage_MESSAGE_EDIT != 14 {
		t.Errorf("REVOKE=%d MESSAGE_EDIT=%d", waE2E.ProtocolMessage_REVOKE, waE2E.ProtocolMessage_MESSAGE_EDIT)
	}
}

func TestEditedTextLooksThroughWrappers(t *testing.T) {
	text := proto.String("dvv")
	cases := map[string]*waE2E.Message{
		"conversation": {Conversation: text},
		"extended":     {ExtendedTextMessage: &waE2E.ExtendedTextMessage{Text: text}},
		"caption":      {ImageMessage: &waE2E.ImageMessage{Caption: text}},
		"protocol":     {ProtocolMessage: &waE2E.ProtocolMessage{EditedMessage: &waE2E.Message{Conversation: text}}},
		"envelope":     {EditedMessage: &waE2E.FutureProofMessage{Message: &waE2E.Message{Conversation: text}}},
		"ephemeral":    {EphemeralMessage: &waE2E.FutureProofMessage{Message: &waE2E.Message{Conversation: text}}},
		"nested":       {EditedMessage: &waE2E.FutureProofMessage{Message: &waE2E.Message{ProtocolMessage: &waE2E.ProtocolMessage{EditedMessage: &waE2E.Message{Conversation: text}}}}},
	}
	for name, msg := range cases {
		if got := editedText(msg); got != "dvv" {
			t.Errorf("%s: got %q", name, got)
		}
	}
	if editedText(nil) != "" || editedText(&waE2E.Message{}) != "" {
		t.Error("nothing to find must give an empty string")
	}
}

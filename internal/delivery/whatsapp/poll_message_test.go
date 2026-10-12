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

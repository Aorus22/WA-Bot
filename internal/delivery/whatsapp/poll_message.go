package whatsapp

import "go.mau.fi/whatsmeow/proto/waE2E"

// pollCreation returns the poll a message creates, if any. WhatsApp has
// shipped polls under several message fields over time (the original, then
// V2 for announcement-group polls, V3 which phones send by default today, V4
// wrapped in a future-proof envelope, and V5/V6), so checking only the
// original field drops most real polls.
func pollCreation(msg *waE2E.Message) *waE2E.PollCreationMessage {
	if msg == nil {
		return nil
	}
	for _, poll := range []*waE2E.PollCreationMessage{
		msg.GetPollCreationMessage(),
		msg.GetPollCreationMessageV2(),
		msg.GetPollCreationMessageV3(),
		msg.GetPollCreationMessageV5(),
		msg.GetPollCreationMessageV6(),
	} {
		if poll != nil {
			return poll
		}
	}
	if wrapped := msg.GetPollCreationMessageV4().GetMessage(); wrapped != nil && wrapped != msg {
		return pollCreation(wrapped)
	}
	return nil
}

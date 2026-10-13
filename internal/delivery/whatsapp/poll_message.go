package whatsapp

import (
	"go.mau.fi/whatsmeow/proto/waE2E"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/reflect/protoreflect"
)

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

// isRevoke is a "delete for everyone"; isEdit is an edited message. They are
// protocol message types 0 and 14: compare against the generated constants,
// never numbers (the two were once swapped, deleting every edited message).
func isRevoke(pm *waE2E.ProtocolMessage) bool {
	return pm != nil && pm.GetType() == waE2E.ProtocolMessage_REVOKE
}

func isEdit(pm *waE2E.ProtocolMessage) bool {
	return pm != nil && pm.GetType() == waE2E.ProtocolMessage_MESSAGE_EDIT
}

// editedText is the text a message carries once edited: its body, or its
// caption when it is media. An edit can arrive wrapped (inside a protocol
// message, an edited-message envelope, an ephemeral wrapper), so those are
// looked through.
func editedText(msg *waE2E.Message) string {
	if msg == nil {
		return ""
	}
	switch {
	case msg.GetConversation() != "":
		return msg.GetConversation()
	case msg.GetExtendedTextMessage().GetText() != "":
		return msg.GetExtendedTextMessage().GetText()
	case msg.GetImageMessage().GetCaption() != "":
		return msg.GetImageMessage().GetCaption()
	case msg.GetVideoMessage().GetCaption() != "":
		return msg.GetVideoMessage().GetCaption()
	case msg.GetDocumentMessage().GetCaption() != "":
		return msg.GetDocumentMessage().GetCaption()
	}
	for _, inner := range []*waE2E.Message{
		msg.GetProtocolMessage().GetEditedMessage(),
		msg.GetEditedMessage().GetMessage(),
		msg.GetEphemeralMessage().GetMessage(),
		msg.GetDeviceSentMessage().GetMessage(),
		msg.GetViewOnceMessage().GetMessage(),
		msg.GetDocumentWithCaptionMessage().GetMessage(),
	} {
		if text := editedText(inner); text != "" {
			return text
		}
	}
	return ""
}

// populatedFields names the fields set on a message, for diagnostics.
func populatedFields(msg proto.Message) []string {
	if msg == nil {
		return nil
	}
	var names []string
	msg.ProtoReflect().Range(func(field protoreflect.FieldDescriptor, _ protoreflect.Value) bool {
		names = append(names, string(field.Name()))
		return true
	})
	return names
}

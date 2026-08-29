package whatsapp

import (
	"context"
	"fmt"
	"sort"
	"strings"
	"time"

	"go.mau.fi/whatsmeow"
	waE2E "go.mau.fi/whatsmeow/proto/waE2E"
	waTypes "go.mau.fi/whatsmeow/types"
)

// ChannelInfo is the flattened channel (newsletter) descriptor served to
// clients.
type ChannelInfo struct {
	JID          string `json:"jid"`
	Name         string `json:"name"`
	Description  string `json:"description"`
	Subscribers  int    `json:"subscribers"`
	InviteCode   string `json:"inviteCode,omitempty"`
	Avatar       string `json:"avatar,omitempty"`
	Muted        bool   `json:"muted"`
	Verified     bool   `json:"verified"`
	SubscribedAt int64  `json:"subscribedAt,omitempty"`
}

// ChannelMessage is one channel post.
type ChannelMessage struct {
	ID         string         `json:"id"`
	ServerID   int64          `json:"serverId"`
	Type       string         `json:"type"`
	Timestamp  int64          `json:"timestamp"`
	ViewsCount int            `json:"viewsCount"`
	Reactions  map[string]int `json:"reactions,omitempty"`
	Content    string         `json:"content,omitempty"`
	MediaType  string         `json:"mediaType,omitempty"` // image|video when the post carries media
}

func channelFromMeta(meta *waTypes.NewsletterMetadata, muted bool) *ChannelInfo {
	info := &ChannelInfo{
		JID:         meta.ID.String(),
		Name:        meta.ThreadMeta.Name.Text,
		Description: meta.ThreadMeta.Description.Text,
		Subscribers: meta.ThreadMeta.SubscriberCount,
		InviteCode:  meta.ThreadMeta.InviteCode,
		Muted:       muted,
		Verified:    meta.ThreadMeta.VerificationState == waTypes.NewsletterVerificationStateVerified,
	}
	if meta.ThreadMeta.Picture != nil {
		info.Avatar = meta.ThreadMeta.Picture.URL
	}
	return info
}

// ListChannels returns all followed channels, refreshing mute states.
func (w *WhatsAppClient) ListChannels(ctx context.Context) ([]*ChannelInfo, error) {
	metas, err := w.client.GetSubscribedNewsletters(ctx)
	if err != nil {
		return nil, err
	}
	out := make([]*ChannelInfo, 0, len(metas))
	for _, meta := range metas {
		muted := meta.ViewerMeta != nil && meta.ViewerMeta.Mute != waTypes.NewsletterMuteOff
		out = append(out, channelFromMeta(meta, muted))
	}
	return out, nil
}

// PreviewChannel peeks at a channel via invite code or link without following.
func (w *WhatsAppClient) PreviewChannel(ctx context.Context, linkOrCode string) (*ChannelInfo, error) {
	key := strings.TrimSpace(linkOrCode)
	key = strings.TrimPrefix(key, "https://whatsapp.com/channel/")
	key = strings.TrimPrefix(key, "https://www.whatsapp.com/channel/")
	key = strings.TrimSuffix(key, "/")
	if key == "" {
		return nil, fmt.Errorf("empty invite code")
	}
	meta, err := w.client.GetNewsletterInfoWithInvite(ctx, key)
	if err != nil {
		return nil, err
	}
	return channelFromMeta(meta, false), nil
}

// FollowChannel joins a channel via invite code or link.
func (w *WhatsAppClient) FollowChannel(ctx context.Context, linkOrCode string) (*ChannelInfo, error) {
	preview, err := w.PreviewChannel(ctx, linkOrCode)
	if err != nil {
		return nil, err
	}
	jid, err := waTypes.ParseJID(preview.JID)
	if err != nil {
		return nil, err
	}
	if err := w.client.FollowNewsletter(ctx, jid); err != nil {
		return nil, err
	}
	return preview, nil
}

// UnfollowChannel leaves a channel.
func (w *WhatsAppClient) UnfollowChannel(ctx context.Context, channelID string) error {
	jid, err := waTypes.ParseJID(channelID)
	if err != nil {
		return err
	}
	return w.client.UnfollowNewsletter(ctx, jid)
}

// SetChannelMuted toggles channel notifications.
func (w *WhatsAppClient) SetChannelMuted(ctx context.Context, channelID string, muted bool) error {
	jid, err := waTypes.ParseJID(channelID)
	if err != nil {
		return err
	}
	return w.client.NewsletterToggleMute(ctx, jid, muted)
}

// GetChannelMessages fetches channel posts (newest first, paginated by
// server id).
func (w *WhatsAppClient) GetChannelMessages(ctx context.Context, channelID string, count int, before int64) ([]*ChannelMessage, []*waTypes.NewsletterMessage, error) {
	jid, err := waTypes.ParseJID(channelID)
	if err != nil {
		return nil, nil, err
	}
	if count <= 0 || count > 50 {
		count = 30
	}
	params := &whatsmeow.GetNewsletterMessagesParams{Count: count}
	if before > 0 {
		params.Before = waTypes.MessageServerID(before)
	}
	msgs, err := w.client.GetNewsletterMessages(ctx, jid, params)
	if err != nil {
		return nil, nil, err
	}
	// WhatsApp returns channel posts newest-first; normalize to ascending
	// (oldest -> newest) so clients can render and page them like a chat.
	sort.SliceStable(msgs, func(i, j int) bool {
		return msgs[i].MessageServerID < msgs[j].MessageServerID
	})
	out := make([]*ChannelMessage, 0, len(msgs))
	for _, m := range msgs {
		out = append(out, channelMessageOf(m))
	}
	return out, msgs, nil
}

// ReactChannelMessage reacts (empty emoji removes) to a channel post.
func (w *WhatsAppClient) ReactChannelMessage(ctx context.Context, channelID string, serverID int64, messageID, emoji string) error {
	jid, err := waTypes.ParseJID(channelID)
	if err != nil {
		return err
	}
	return w.client.NewsletterSendReaction(ctx, jid, waTypes.MessageServerID(serverID), emoji, messageID)
}

// SubscribeChannelLive subscribes to live reaction/view updates; returns the
// subscription duration after which it must be renewed.
func (w *WhatsAppClient) SubscribeChannelLive(ctx context.Context, channelID string) (time.Duration, error) {
	jid, err := waTypes.ParseJID(channelID)
	if err != nil {
		return 0, err
	}
	return w.client.NewsletterSubscribeLiveUpdates(ctx, jid)
}

// ChannelMessageFromRaw converts a whatsmeow NewsletterMessage.
func ChannelMessageFromRaw(m *waTypes.NewsletterMessage) *ChannelMessage {
	return channelMessageOf(m)
}

// ChannelMessageFromEvent converts an incoming newsletter chat message into
// the channel post payload (ServerID only present in live updates/feed).
func ChannelMessageFromEvent(id string, ts time.Time, message *waE2E.Message) *ChannelMessage {
	msg := &ChannelMessage{
		ID:        id,
		Timestamp: ts.UnixMilli(),
	}
	if message != nil {
		switch {
		case message.GetConversation() != "":
			msg.Content = message.GetConversation()
		case message.GetExtendedTextMessage() != nil:
			msg.Content = message.GetExtendedTextMessage().GetText()
		case message.GetImageMessage() != nil:
			msg.MediaType = "image"
			msg.Content = message.GetImageMessage().GetCaption()
		case message.GetVideoMessage() != nil:
			msg.MediaType = "video"
			msg.Content = message.GetVideoMessage().GetCaption()
		}
	}
	return msg
}

func channelMessageOf(m *waTypes.NewsletterMessage) *ChannelMessage {
	msg := &ChannelMessage{
		ID:         m.MessageID,
		ServerID:   int64(m.MessageServerID),
		Type:       m.Type,
		Timestamp:  m.Timestamp.UnixMilli(),
		ViewsCount: m.ViewsCount,
		Reactions:  m.ReactionCounts,
	}
	if m.Message != nil {
		switch {
		case m.Message.GetConversation() != "":
			msg.Content = m.Message.GetConversation()
		case m.Message.GetExtendedTextMessage() != nil:
			msg.Content = m.Message.GetExtendedTextMessage().GetText()
		case m.Message.GetImageMessage() != nil:
			msg.MediaType = "image"
			msg.Content = m.Message.GetImageMessage().GetCaption()
		case m.Message.GetVideoMessage() != nil:
			msg.MediaType = "video"
			msg.Content = m.Message.GetVideoMessage().GetCaption()
		}
	}
	return msg
}

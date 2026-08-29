package api

import (
	"context"
	"net/url"
	"strconv"
)

// ChannelInfo is a followed WhatsApp channel (newsletter).
type ChannelInfo struct {
	JID         string `json:"jid"`
	Name        string `json:"name"`
	Description string `json:"description"`
	Subscribers int    `json:"subscribers"`
	InviteCode  string `json:"inviteCode,omitempty"`
	Avatar      string `json:"avatar,omitempty"`
	Muted       bool   `json:"muted"`
	Verified    bool   `json:"verified"`
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
	MediaType  string         `json:"mediaType,omitempty"`
}

// ListChannels returns the followed channels.
func (c *Client) ListChannels(ctx context.Context) ([]ChannelInfo, error) {
	var channels []ChannelInfo
	if err := c.doJSON(ctx, "GET", "/api/channels", nil, &channels); err != nil {
		return nil, err
	}
	return channels, nil
}

// PreviewChannel peeks at a channel via invite link without following.
func (c *Client) PreviewChannel(ctx context.Context, link string) (*ChannelInfo, error) {
	var info ChannelInfo
	if err := c.doJSON(ctx, "GET", "/api/channels/preview?url="+url.QueryEscape(link), nil, &info); err != nil {
		return nil, err
	}
	return &info, nil
}

// FollowChannel joins a channel via invite link.
func (c *Client) FollowChannel(ctx context.Context, link string) (*ChannelInfo, error) {
	var out struct {
		Status  string       `json:"status"`
		Channel *ChannelInfo `json:"channel"`
	}
	if err := c.doJSON(ctx, "POST", "/api/channels", map[string]any{"url": link}, &out); err != nil {
		return nil, err
	}
	return out.Channel, nil
}

// UnfollowChannel leaves a channel.
func (c *Client) UnfollowChannel(ctx context.Context, channelID string) error {
	return c.doJSON(ctx, "DELETE", "/api/channels/"+url.PathEscape(channelID), nil, nil)
}

// MuteChannel toggles channel notifications.
func (c *Client) MuteChannel(ctx context.Context, channelID string, muted bool) error {
	return c.doJSON(ctx, "POST", "/api/channels/"+url.PathEscape(channelID)+"/mute", map[string]any{"muted": muted}, nil)
}

// GetChannelMessages fetches the channel post feed.
func (c *Client) GetChannelMessages(ctx context.Context, channelID string, count int, before int64) ([]ChannelMessage, error) {
	q := url.Values{}
	if count <= 0 {
		count = 30
	}
	q.Set("count", strconv.Itoa(count))
	if before > 0 {
		q.Set("before", strconv.FormatInt(before, 10))
	}
	var msgs []ChannelMessage
	path := "/api/channels/" + url.PathEscape(channelID) + "/messages?" + q.Encode()
	if err := c.doJSON(ctx, "GET", path, nil, &msgs); err != nil {
		return nil, err
	}
	return msgs, nil
}

// ReactChannelMessage reacts (empty emoji removes) to a channel post.
func (c *Client) ReactChannelMessage(ctx context.Context, channelID string, serverID int64, messageID, emoji string) error {
	path := "/api/channels/" + url.PathEscape(channelID) + "/messages/" + strconv.FormatInt(serverID, 10) + "/react"
	return c.doJSON(ctx, "POST", path, map[string]any{"emoji": emoji, "messageId": messageID}, nil)
}

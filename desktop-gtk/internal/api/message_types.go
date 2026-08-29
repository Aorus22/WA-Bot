package api

import (
	"context"
	"net/url"
)

// ReactionEntry aggregates everyone who reacted with one emoji.
type ReactionEntry struct {
	Emoji   string   `json:"emoji"`
	Senders []string `json:"senders"`
}

type PollOption struct {
	Name string `json:"name"`
}

type PollMeta struct {
	Question    string              `json:"question"`
	Options     []PollOption        `json:"options"`
	MultiSelect bool                `json:"multiSelect"`
	Votes       map[string][]string `json:"votes,omitempty"`
}

type LocationMeta struct {
	Latitude     float64 `json:"latitude"`
	Longitude    float64 `json:"longitude"`
	Name         string  `json:"name,omitempty"`
	Address      string  `json:"address,omitempty"`
	Live         bool    `json:"live,omitempty"`
	ThumbnailURL string  `json:"thumbnailUrl,omitempty"`
}

type ContactEntry struct {
	DisplayName string `json:"displayName"`
	VCard       string `json:"vcard,omitempty"`
}

type ContactMeta struct {
	DisplayName string         `json:"displayName,omitempty"`
	Contacts    []ContactEntry `json:"contacts,omitempty"`
}

type ViewOnceMeta struct {
	MediaType string `json:"mediaType"`
	Viewed    bool   `json:"viewed"`
}

type LinkPreviewMeta struct {
	URL          string `json:"url"`
	Title        string `json:"title,omitempty"`
	Description  string `json:"description,omitempty"`
	ThumbnailURL string `json:"thumbnailUrl,omitempty"`
}

// MessageExtra carries type-specific metadata for poll/location/contact/
// view-once/gif/link-preview messages.
type MessageExtra struct {
	Poll        *PollMeta        `json:"poll,omitempty"`
	Location    *LocationMeta    `json:"location,omitempty"`
	Contact     *ContactMeta     `json:"contact,omitempty"`
	ViewOnce    *ViewOnceMeta    `json:"viewOnce,omitempty"`
	GIF         bool             `json:"gif,omitempty"`
	LinkPreview *LinkPreviewMeta `json:"linkPreview,omitempty"`
}

// PollVoteCounts computes per-option vote counts from the stored vote map.
func (p *PollMeta) VoteCounts() map[string]int {
	counts := make(map[string]int, len(p.Options))
	for _, opts := range p.Votes {
		for _, o := range opts {
			counts[o]++
		}
	}
	return counts
}

// MyVote returns the options this account voted for, or nil.
func (p *PollMeta) MyVote() []string { return p.Votes["me"] }

// SendPoll creates a poll in the chat (POST /api/chats/{chatId}/poll).
func (c *Client) SendPoll(ctx context.Context, chatID, question string, options []string, multiSelect bool) error {
	body := map[string]any{
		"secret":      "default-secret",
		"question":    question,
		"options":     options,
		"multiSelect": multiSelect,
	}
	return c.doJSON(ctx, "POST", "/api/chats/"+url.PathEscape(chatID)+"/poll", body, nil)
}

// SendPollVote votes on a poll message
// (POST /api/chats/{chatId}/messages/{msgId}/vote). An empty options slice
// retracts the vote.
func (c *Client) SendPollVote(ctx context.Context, chatID, msgID string, options []string) error {
	path := "/api/chats/" + url.PathEscape(chatID) + "/messages/" + url.PathEscape(msgID) + "/vote"
	return c.doJSON(ctx, "POST", path, map[string]any{"options": options}, nil)
}

// SendLocation shares a static or live location
// (POST /api/chats/{chatId}/location).
func (c *Client) SendLocation(ctx context.Context, chatID string, latitude, longitude float64, name, address string, live bool, caption string) error {
	body := map[string]any{
		"secret":    "default-secret",
		"latitude":  latitude,
		"longitude": longitude,
		"name":      name,
		"address":   address,
		"live":      live,
		"caption":   caption,
	}
	return c.doJSON(ctx, "POST", "/api/chats/"+url.PathEscape(chatID)+"/location", body, nil)
}

// SendContact shares a contact card (POST /api/chats/{chatId}/contact).
func (c *Client) SendContact(ctx context.Context, chatID, displayName, phone string) error {
	body := map[string]any{
		"secret":      "default-secret",
		"displayName": displayName,
		"phone":       phone,
	}
	return c.doJSON(ctx, "POST", "/api/chats/"+url.PathEscape(chatID)+"/contact", body, nil)
}

// SendReaction reacts to (or with empty emoji, removes the reaction on) a
// message (POST /api/chats/{chatId}/messages/{msgId}/react). author is the
// sender of the reacted-to message; empty means own message.
func (c *Client) SendReaction(ctx context.Context, chatID, msgID, emoji, author string) error {
	path := "/api/chats/" + url.PathEscape(chatID) + "/messages/" + url.PathEscape(msgID) + "/react"
	return c.doJSON(ctx, "POST", path, map[string]any{"emoji": emoji, "from": author}, nil)
}

// ForwardMessage forwards a message to other chats
// (POST /api/chats/{chatId}/messages/{msgId}/forward).
func (c *Client) ForwardMessage(ctx context.Context, chatID, msgID string, targets []string) error {
	path := "/api/chats/" + url.PathEscape(chatID) + "/messages/" + url.PathEscape(msgID) + "/forward"
	return c.doJSON(ctx, "POST", path, map[string]any{"targets": targets}, nil)
}

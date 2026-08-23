package api

import (
	"bytes"
	"context"
	"fmt"
	"io"
	"mime/multipart"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strconv"
)

// Chat is one entry in GET /api/chats.
// Fields match the backend's repository.Chat JSON shape.
type Chat struct {
	ID       string `json:"id"`
	Name     string `json:"name"`
	Avatar   string `json:"avatar"`
	LastMsg  string `json:"lastMsg"`
	LastTime int64  `json:"lastTime"`
	Unread   int    `json:"unread"`
	IsActive bool   `json:"isActive"`
	IsGroup  bool   `json:"isGroup"`
}

// Message is one entry in GET /api/chats/{id}/messages.
// Fields match the backend's repository.Message JSON shape.
type Message struct {
	ID          string `json:"id"`
	ChatID      string `json:"chatId"`
	From        string `json:"from"`
	To          string `json:"to"`
	Content     string `json:"content"`
	Timestamp   int64  `json:"timestamp"`
	Status      string `json:"status"`
	Type        string `json:"type"`
	MediaURL    string `json:"mediaUrl,omitempty"`
	IsAutomatic bool   `json:"isAutomatic"`
	SenderName  string `json:"senderName,omitempty"`
	ChatName    string `json:"chatName,omitempty"`
	ReplyToID   string `json:"replyToId,omitempty"`
}

// ListChats fetches chats from GET /api/chats, optionally paginated.
func (c *Client) ListChats(ctx context.Context, limit int, search string) ([]Chat, error) {
	q := url.Values{}
	if limit > 0 {
		q.Set("limit", strconv.Itoa(limit))
	}
	if search != "" {
		q.Set("search", search)
	}
	path := "/api/chats"
	if encoded := q.Encode(); encoded != "" {
		path += "?" + encoded
	}
	var chats []Chat
	if err := c.doJSON(ctx, "GET", path, nil, &chats); err != nil {
		return nil, err
	}
	return chats, nil
}

// GetMessages fetches the message history for a chat from
// GET /api/chats/{id}/messages. The backend returns the newest `limit`
// messages older than `before` (unix millis), newest first — callers must
// reverse for display order.
func (c *Client) GetMessages(ctx context.Context, chatID string, limit int, before int64) ([]Message, error) {
	q := url.Values{}
	if limit > 0 {
		q.Set("limit", strconv.Itoa(limit))
	}
	if before > 0 {
		q.Set("before", strconv.FormatInt(before, 10))
	}
	return c.getMessages(ctx, chatID, q)
}

// GetMessagesAfter fetches up to `limit` messages newer than `after`
// (unix millis). The backend returns them in ascending order already.
func (c *Client) GetMessagesAfter(ctx context.Context, chatID string, limit int, after int64) ([]Message, error) {
	q := url.Values{}
	if limit > 0 {
		q.Set("limit", strconv.Itoa(limit))
	}
	if after > 0 {
		q.Set("after", strconv.FormatInt(after, 10))
	}
	return c.getMessages(ctx, chatID, q)
}

// MarkRead marks all messages in the chat as read (POST /api/chats/{id}/read).
func (c *Client) MarkRead(ctx context.Context, chatID string) error {
	return c.doJSON(ctx, "POST", "/api/chats/"+url.PathEscape(chatID)+"/read", map[string]any{}, nil)
}

// SendTyping forwards a typing presence indicator for the chat
// (POST /api/chats/{chatId}/typing, body {"isTyping": bool}).
func (c *Client) SendTyping(ctx context.Context, chatID string, isTyping bool) error {
	body := map[string]bool{"isTyping": isTyping}
	return c.doJSON(ctx, "POST", "/api/chats/"+url.PathEscape(chatID)+"/typing", body, nil)
}

func (c *Client) getMessages(ctx context.Context, chatID string, q url.Values) ([]Message, error) {
	path := "/api/chats/" + url.PathEscape(chatID) + "/messages"
	if encoded := q.Encode(); encoded != "" {
		path += "?" + encoded
	}
	var msgs []Message
	if err := c.doJSON(ctx, "GET", path, nil, &msgs); err != nil {
		return nil, err
	}
	return msgs, nil
}

// sendRequest is the body shape for POST /api/send-message.
type sendRequest struct {
	Secret  string `json:"secret"`
	Target  string `json:"target"`
	Message string `json:"message"`
}

// SendText sends a text message to the given chat (target = JID).
// The backend's `secret` defaults to "default-secret" if API_SECRET is not set
// in the backend's environment. The desktop app's backend is launched with no
// API_SECRET, so we send the default.
func (c *Client) SendText(ctx context.Context, target, message string) error {
	body := sendRequest{Secret: "default-secret", Target: target, Message: message}
	return c.doJSON(ctx, "POST", "/api/send-message", body, nil)
}

// SendMedia sends a media file (image/document) to the given chat.
// mediaType must be one of "image", "video", "document" (per backend validation).
// filePath is a local file path; the file is uploaded as multipart/form-data.
func (c *Client) SendMedia(ctx context.Context, target, filePath, mediaType, caption string) error {
	file, err := os.Open(filePath)
	if err != nil {
		return fmt.Errorf("open file: %w", err)
	}
	defer file.Close()

	var buf bytes.Buffer
	mw := multipart.NewWriter(&buf)
	_ = mw.WriteField("secret", "default-secret")
	_ = mw.WriteField("target", target)
	if mediaType != "" {
		_ = mw.WriteField("type", mediaType)
	}
	if caption != "" {
		_ = mw.WriteField("message", caption)
	}
	fw, err := mw.CreateFormFile("file", filepath.Base(filePath))
	if err != nil {
		return fmt.Errorf("create form file: %w", err)
	}
	if _, err := io.Copy(fw, file); err != nil {
		return fmt.Errorf("copy file: %w", err)
	}
	_ = mw.Close()

	u, err := url.Parse(c.baseURL)
	if err != nil {
		return fmt.Errorf("invalid base url: %w", err)
	}
	u.Path = "/api/send-media"

	req, err := http.NewRequestWithContext(ctx, "POST", u.String(), &buf)
	if err != nil {
		return fmt.Errorf("build request: %w", err)
	}
	req.Header.Set("Content-Type", mw.FormDataContentType())
	req.Header.Set("Accept", "application/json")

	resp, err := c.http.Do(req)
	if err != nil {
		return fmt.Errorf("http: %w", err)
	}
	defer resp.Body.Close()
	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		body, _ := io.ReadAll(resp.Body)
		return &APIError{Status: resp.StatusCode, Body: string(body), Path: "/api/send-media"}
	}
	return nil
}

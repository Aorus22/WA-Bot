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
)

// StatusEntry is one status (story) update.
type StatusEntry struct {
	ID         string `json:"id"`
	Sender     string `json:"sender"`
	SenderName string `json:"senderName,omitempty"`
	Content    string `json:"content,omitempty"`
	MediaURL   string `json:"mediaUrl,omitempty"`
	Type       string `json:"type"`
	Timestamp  int64  `json:"timestamp"`
	ExpiresAt  int64  `json:"expiresAt"`
	Viewed     bool   `json:"viewed"`
}

// StatusGroup is one sender's active statuses.
type StatusGroup struct {
	Sender     string        `json:"sender"`
	Name       string        `json:"name,omitempty"`
	Avatar     string        `json:"avatar,omitempty"`
	AllViewed  bool          `json:"allViewed"`
	Statuses   []StatusEntry `json:"statuses"`
	LatestTime int64         `json:"latestTime"`
}

// ListStatuses fetches active statuses grouped per sender.
func (c *Client) ListStatuses(ctx context.Context) ([]StatusGroup, error) {
	var groups []StatusGroup
	if err := c.doJSON(ctx, "GET", "/api/statuses", nil, &groups); err != nil {
		return nil, err
	}
	return groups, nil
}

// PostStatusText publishes a text status.
func (c *Client) PostStatusText(ctx context.Context, text string, background uint32) error {
	body := map[string]any{"text": text, "background": background}
	return c.doJSON(ctx, "POST", "/api/statuses/text", body, nil)
}

// PostStatusMedia publishes an image/video status from a local file.
func (c *Client) PostStatusMedia(ctx context.Context, kind, filePath, caption string) error {
	file, err := os.Open(filePath)
	if err != nil {
		return fmt.Errorf("open file: %w", err)
	}
	defer file.Close()

	var buf bytes.Buffer
	mw := multipart.NewWriter(&buf)
	_ = mw.WriteField("type", kind)
	_ = mw.WriteField("caption", caption)
	fw, err := mw.CreateFormFile("file", filePath)
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
	u.Path = "/api/statuses/media"

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
		return &APIError{Status: resp.StatusCode, Body: string(body), Path: "/api/statuses/media"}
	}
	return nil
}

// MarkStatusViewed flags a status as viewed (sends the read receipt).
func (c *Client) MarkStatusViewed(ctx context.Context, statusID string) error {
	return c.doJSON(ctx, "POST", "/api/statuses/"+url.PathEscape(statusID)+"/viewed", map[string]any{}, nil)
}

// StatusMediaURL resolves the on-demand media endpoint for a status.
func (c *Client) StatusMediaURL(statusID string) string {
	return c.baseURL + "/statuses/" + url.PathEscape(statusID) + "/media"
}

// StatusBackgrounds is the WhatsApp text-status background palette (ARGB).
var StatusBackgrounds = []uint32{
	0xFF075E54, 0xFF128C7E, 0xFF777A77, 0xFF2C3E50, 0xFF6A3080,
	0xFFC43E00, 0xFFD4A017, 0xFF0E5A8A, 0xFF1F4E79, 0xFF4A1C5C,
}

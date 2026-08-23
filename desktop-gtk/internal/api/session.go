package api

import (
	"context"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
)

// GetQRCode fetches the latest cached pairing QR from GET /api/qr-code.
// Returns "" when the backend has no QR yet.
func (c *Client) GetQRCode(ctx context.Context) (string, error) {
	var qr struct {
		Code string `json:"code"`
	}
	if err := c.doJSON(ctx, "GET", "/api/qr-code", nil, &qr); err != nil {
		return "", err
	}
	return qr.Code, nil
}

// Logout disconnects the WhatsApp session on the backend (POST /api/logout).
func (c *Client) Logout(ctx context.Context) error {
	return c.doJSON(ctx, "POST", "/api/logout", map[string]any{}, nil)
}

// MediaURL resolves a stored mediaUrl ("/media/<file>" or absolute http(s))
// into an absolute backend URL. Mirrors the web UI's getMediaUrl(): root
// relative paths live under the /api prefix and the filename is
// percent-encoded individually.
func (c *Client) MediaURL(mediaURL string) string {
	switch {
	case mediaURL == "":
		return ""
	case strings.HasPrefix(mediaURL, "http://"),
		strings.HasPrefix(mediaURL, "https://"),
		strings.HasPrefix(mediaURL, c.baseURL):
		return mediaURL
	case !strings.HasPrefix(mediaURL, "/"):
		return c.baseURL + "/api/" + url.PathEscape(mediaURL)
	case mediaURL == "/api" || strings.HasPrefix(mediaURL, "/api/"):
		return c.baseURL + mediaURL
	}
	i := strings.LastIndex(mediaURL, "/")
	file := mediaURL[i+1:]
	return c.baseURL + "/api" + mediaURL[:i] + "/" + url.PathEscape(file)
}

// AvatarURL returns the absolute avatar proxy URL for a JID
// (GET /api/avatar/{jid}).
func (c *Client) AvatarURL(jid string) string {
	return c.baseURL + "/api/avatar/" + url.PathEscape(jid)
}

// FetchBytes downloads the given absolute URL and returns the raw body.
// Used for avatars and media files.
func (c *Client) FetchBytes(ctx context.Context, rawURL string) ([]byte, error) {
	u, err := url.Parse(rawURL)
	if err != nil {
		return nil, fmt.Errorf("invalid url %q: %w", rawURL, err)
	}
	req, err := http.NewRequestWithContext(ctx, "GET", u.String(), nil)
	if err != nil {
		return nil, fmt.Errorf("build request: %w", err)
	}
	resp, err := c.http.Do(req)
	if err != nil {
		return nil, fmt.Errorf("http: %w", err)
	}
	defer resp.Body.Close()
	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		body, _ := io.ReadAll(resp.Body)
		return nil, &APIError{Status: resp.StatusCode, Body: string(body), Path: u.Path}
	}
	return io.ReadAll(resp.Body)
}

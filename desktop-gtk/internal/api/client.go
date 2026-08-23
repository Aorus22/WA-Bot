// Package api is a thin HTTP client for the wa-bot-backend REST API.
// All v1.1 features reuse existing backend endpoints — see ../api/*.
package api

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"net/url"
	"time"
)

// Client is a JSON-over-HTTP client for the backend.
// BaseURL is http://127.0.0.1:<port> (no trailing slash).
type Client struct {
	baseURL string
	http    *http.Client
}

// NewClient constructs a Client. The caller passes the backend's bound port
// (discovered via the BACKEND_PORT: stdout handshake in the backend package).
func NewClient(port int) *Client {
	return &Client{
		baseURL: fmt.Sprintf("http://127.0.0.1:%d", port),
		http: &http.Client{
			Timeout: 10 * time.Second,
		},
	}
}

// BaseURL returns the base URL the client talks to.
func (c *Client) BaseURL() string { return c.baseURL }

// doJSON executes an HTTP request, decodes the JSON response into out (if non-nil),
// and returns a typed APIError for non-2xx responses.
func (c *Client) doJSON(ctx context.Context, method, path string, body any, out any) error {
	// Split path into path + raw query so we don't URL-encode the '?'.
	// Callers may pass "/api/chats?limit=200&search=foo" — we preserve that.
	u, err := url.Parse(c.baseURL)
	if err != nil {
		return fmt.Errorf("invalid base url: %w", err)
	}
	pPath, pQuery, _ := splitPathQuery(path)
	u.Path = pPath
	u.RawQuery = pQuery

	var reqBody io.Reader
	if body != nil {
		b, err := json.Marshal(body)
		if err != nil {
			return fmt.Errorf("marshal body: %w", err)
		}
		reqBody = bytes.NewReader(b)
	}

	req, err := http.NewRequestWithContext(ctx, method, u.String(), reqBody)
	if err != nil {
		return fmt.Errorf("build request: %w", err)
	}
	if body != nil {
		req.Header.Set("Content-Type", "application/json")
	}
	req.Header.Set("Accept", "application/json")

	resp, err := c.http.Do(req)
	if err != nil {
		return fmt.Errorf("http: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		body, _ := io.ReadAll(resp.Body)
		return &APIError{Status: resp.StatusCode, Body: string(body), Path: path}
	}

	if out != nil {
		raw, err := io.ReadAll(resp.Body)
		if err != nil {
			return fmt.Errorf("read body: %w", err)
		}
		if len(raw) == 0 {
			return fmt.Errorf("empty response body from %s", path)
		}
		log.Printf("doJSON %s %s -> %d bytes: %s", method, path, len(raw), string(raw[:min(200, len(raw))]))
		if err := json.Unmarshal(raw, out); err != nil {
			return fmt.Errorf("decode response: %w", err)
		}
	}
	return nil
}

// splitPathQuery splits "/path?query" into ("/path", "query", true).
func splitPathQuery(p string) (string, string, bool) {
	for i := 0; i < len(p); i++ {
		if p[i] == '?' {
			return p[:i], p[i+1:], true
		}
	}
	return p, "", false
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}

// APIError is a typed error for non-2xx responses from the backend.
type APIError struct {
	Status int
	Body   string
	Path   string
}

func (e *APIError) Error() string {
	return fmt.Sprintf("api %s: status %d: %s", e.Path, e.Status, e.Body)
}

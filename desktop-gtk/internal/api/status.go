package api

import "context"

// Health is the response from GET /api/health.
type Health struct {
	Status  string `json:"status"`
	Service string `json:"service"`
}

// Status is the response from GET /api/status.
// The current backend returns only {"isLoggedIn": bool}; other fields are
// kept optional for forward compatibility.
type Status struct {
	LoggedIn    bool   `json:"isLoggedIn"`
	Connected   bool   `json:"connected,omitempty"`
	Phone       string `json:"phone,omitempty"`
	Device      string `json:"device,omitempty"`
	Platform    string `json:"platform,omitempty"`
	LoginAt     string `json:"login_at,omitempty"`
	ChatCount   int    `json:"chat_count,omitempty"`
	UnreadCount int    `json:"unread_count,omitempty"`
}

// GetHealth pings the backend's /api/health endpoint.
// Returns nil if the backend responds with 2xx; an error otherwise.
func (c *Client) GetHealth(ctx context.Context) error {
	return c.doJSON(ctx, "GET", "/api/health", nil, nil)
}

// GetStatus fetches the system status from /api/status.
func (c *Client) GetStatus(ctx context.Context) (*Status, error) {
	var s Status
	if err := c.doJSON(ctx, "GET", "/api/status", nil, &s); err != nil {
		return nil, err
	}
	return &s, nil
}

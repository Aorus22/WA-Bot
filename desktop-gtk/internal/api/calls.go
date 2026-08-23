package api

import (
	"context"
	"net/url"
	"strconv"
)

// CallLog mirrors the backend's entity.CallLog JSON shape
// (GET /api/calls/history).
type CallLog struct {
	ID           string   `json:"id"`
	MeowCallID   string   `json:"meow_call_id"`
	Direction    string   `json:"direction"`
	CallType     string   `json:"call_type"`
	Target       string   `json:"target"`
	GroupJID     string   `json:"group_jid,omitempty"`
	Participants []string `json:"participants,omitempty"`
	Source       string   `json:"source"`
	MediaMode    string   `json:"media_mode"`
	Status       string   `json:"status"`
	ErrorMessage string   `json:"error_message,omitempty"`
	StartedAt    int64    `json:"started_at"`
	AnsweredAt   *int64   `json:"answered_at,omitempty"`
	EndedAt      *int64   `json:"ended_at,omitempty"`
	DurationMS   *int64   `json:"duration_ms,omitempty"`
	CreatedAt    int64    `json:"created_at"`
}

// GetCallHistory fetches call logs newest first
// (GET /api/calls/history?limit&before&direction&status). `before` is the
// started_at (unix millis) cursor for older pages; direction/status filter
// when non-empty ("incoming"/"outgoing", "missed"/"failed"/...).
func (c *Client) GetCallHistory(ctx context.Context, limit int, before int64, direction, status string) ([]CallLog, error) {
	q := url.Values{}
	if limit > 0 {
		q.Set("limit", strconv.Itoa(limit))
	}
	if before > 0 {
		q.Set("before", strconv.FormatInt(before, 10))
	}
	if direction != "" {
		q.Set("direction", direction)
	}
	if status != "" {
		q.Set("status", status)
	}
	path := "/api/calls/history"
	if encoded := q.Encode(); encoded != "" {
		path += "?" + encoded
	}
	var resp struct {
		Logs []CallLog `json:"logs"`
	}
	if err := c.doJSON(ctx, "GET", path, nil, &resp); err != nil {
		return nil, err
	}
	return resp.Logs, nil
}

// CreateCall starts an outgoing call to target (POST /api/calls).
// callType is "audio" or "video" ("audio" when empty). The backend rings the
// peer; this client has no local call media.
func (c *Client) CreateCall(ctx context.Context, target, callType string) error {
	if callType == "" {
		callType = "audio"
	}
	body := map[string]string{"target": target, "type": callType}
	return c.doJSON(ctx, "POST", "/api/calls", body, nil)
}

// RejectCall declines a ringing call (POST /api/calls/{id}/reject).
func (c *Client) RejectCall(ctx context.Context, id string) error {
	return c.doJSON(ctx, "POST", "/api/calls/"+url.PathEscape(id)+"/reject", map[string]any{}, nil)
}

// HangupCall ends an active call (POST /api/calls/{id}/hangup).
func (c *Client) HangupCall(ctx context.Context, id string) error {
	return c.doJSON(ctx, "POST", "/api/calls/"+url.PathEscape(id)+"/hangup", map[string]any{}, nil)
}

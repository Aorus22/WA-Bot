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
)

// postFile uploads one file as multipart form data to an API path.
func (c *Client) postFile(ctx context.Context, path, filePath string) error {
	file, err := os.Open(filePath)
	if err != nil {
		return fmt.Errorf("open file: %w", err)
	}
	defer file.Close()

	var buf bytes.Buffer
	mw := multipart.NewWriter(&buf)
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
	u.Path = path

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
		return &APIError{Status: resp.StatusCode, Body: string(body), Path: path}
	}
	return nil
}

// GroupParticipantInfo is one member entry of a group.
type GroupParticipantInfo struct {
	JID          string `json:"jid"`
	Name         string `json:"name,omitempty"`
	IsAdmin      bool   `json:"isAdmin"`
	IsSuperAdmin bool   `json:"isSuperAdmin,omitempty"`
}

// GroupCache is the backend's cached group snapshot.
type GroupCache struct {
	JID              string                 `json:"jid"`
	Name             string                 `json:"name"`
	Description      string                 `json:"description,omitempty"`
	Owner            string                 `json:"owner,omitempty"`
	Locked           bool                   `json:"locked"`
	Announce         bool                   `json:"announce"`
	JoinApproval     bool                   `json:"joinApproval"`
	MemberAddMode    string                 `json:"memberAddMode,omitempty"`
	OwnRole          string                 `json:"ownRole"`
	ParticipantCount int                    `json:"participantCount"`
	Participants     []GroupParticipantInfo `json:"participants"`
	UpdatedAt        int64                  `json:"updatedAt"`
}

// IsAdmin reports whether this account administers the group.
func (g *GroupCache) IsAdmin() bool { return g.OwnRole == "admin" || g.OwnRole == "superadmin" }

type GroupPreview struct {
	JID              string `json:"jid"`
	Name             string `json:"name"`
	ParticipantCount int    `json:"participantCount"`
	Description      string `json:"description"`
}

// GetGroup fetches the cached group info (server-refreshed on miss).
func (c *Client) GetGroup(ctx context.Context, groupID string) (*GroupCache, error) {
	var g GroupCache
	if err := c.doJSON(ctx, "GET", "/api/groups/"+url.PathEscape(groupID), nil, &g); err != nil {
		return nil, err
	}
	return &g, nil
}

// UpdateGroupChanges describes a group settings patch; nil fields untouched.
type UpdateGroupChanges struct {
	Name          *string `json:"name,omitempty"`
	Description   *string `json:"description,omitempty"`
	Locked        *bool   `json:"locked,omitempty"`
	Announce      *bool   `json:"announce,omitempty"`
	JoinApproval  *bool   `json:"joinApproval,omitempty"`
	MemberAddMode *string `json:"memberAddMode,omitempty"`
}

// UpdateGroup applies group changes and returns the refreshed snapshot.
func (c *Client) UpdateGroup(ctx context.Context, groupID string, changes UpdateGroupChanges) (*GroupCache, error) {
	var g GroupCache
	err := c.doJSON(ctx, "PATCH", "/api/groups/"+url.PathEscape(groupID), changes, &g)
	if err != nil {
		return nil, err
	}
	return &g, nil
}

// SetGroupPhoto uploads a new group avatar.
func (c *Client) SetGroupPhoto(ctx context.Context, groupID, filePath string) error {
	return c.postFile(ctx, "/api/groups/"+url.PathEscape(groupID)+"/photo", filePath)
}

// UpdateGroupParticipants applies add/remove/promote/demote.
func (c *Client) UpdateGroupParticipants(ctx context.Context, groupID, action string, jids []string) (*GroupCache, error) {
	body := map[string]any{"action": action, "jids": jids}
	var g GroupCache
	err := c.doJSON(ctx, "POST", "/api/groups/"+url.PathEscape(groupID)+"/participants", body, &g)
	if err != nil {
		return nil, err
	}
	return &g, nil
}

// GetInviteLink fetches the invite link (optionally revoking the old one).
func (c *Client) GetInviteLink(ctx context.Context, groupID string, reset bool) (string, error) {
	q := ""
	if reset {
		q = "?reset=true"
	}
	var out struct {
		Status string `json:"status"`
		Link   string `json:"link"`
	}
	err := c.doJSON(ctx, "GET", "/api/groups/"+url.PathEscape(groupID)+"/invite-link"+q, nil, &out)
	return out.Link, err
}

// PreviewGroupLink peeks at a group via invite link without joining.
func (c *Client) PreviewGroupLink(ctx context.Context, link string) (*GroupPreview, error) {
	// The backend returns flattened fields under the same envelope.
	var raw map[string]any
	if err := c.doJSON(ctx, "GET", "/api/groups/preview?url="+url.QueryEscape(link), nil, &raw); err != nil {
		return nil, err
	}
	preview := &GroupPreview{}
	if v, ok := raw["jid"].(string); ok {
		preview.JID = v
	}
	if v, ok := raw["name"].(string); ok {
		preview.Name = v
	}
	if v, ok := raw["description"].(string); ok {
		preview.Description = v
	}
	if v, ok := raw["participantCount"].(float64); ok {
		preview.ParticipantCount = int(v)
	}
	return preview, nil
}

// JoinGroupWithLink joins a group via invite link, returning the group JID.
func (c *Client) JoinGroupWithLink(ctx context.Context, link string) (string, error) {
	var out struct {
		Status string `json:"status"`
		JID    string `json:"jid"`
	}
	err := c.doJSON(ctx, "POST", "/api/groups/join", map[string]any{"url": link}, &out)
	return out.JID, err
}

// CreateGroup creates a group and returns its snapshot.
func (c *Client) CreateGroup(ctx context.Context, name string, participants []string) (*GroupCache, error) {
	var out struct {
		Status string      `json:"status"`
		Group  *GroupCache `json:"group"`
	}
	err := c.doJSON(ctx, "POST", "/api/groups/create", map[string]any{"name": name, "participants": participants}, &out)
	return out.Group, err
}

// LeaveGroup removes this account from the group.
func (c *Client) LeaveGroup(ctx context.Context, groupID string) error {
	return c.doJSON(ctx, "POST", "/api/groups/"+url.PathEscape(groupID)+"/leave", map[string]any{}, nil)
}

type JoinRequest struct {
	JID  string `json:"jid"`
	Name string `json:"name"`
}

// GetJoinRequests lists pending join requests.
func (c *Client) GetJoinRequests(ctx context.Context, groupID string) ([]JoinRequest, error) {
	var out []JoinRequest
	err := c.doJSON(ctx, "GET", "/api/groups/"+url.PathEscape(groupID)+"/join-requests", nil, &out)
	return out, err
}

// UpdateJoinRequests approves or rejects pending join requests.
func (c *Client) UpdateJoinRequests(ctx context.Context, groupID, action string, jids []string) (*GroupCache, error) {
	var g GroupCache
	err := c.doJSON(ctx, "POST", "/api/groups/"+url.PathEscape(groupID)+"/join-requests", map[string]any{"action": action, "jids": jids}, &g)
	if err != nil {
		return nil, err
	}
	return &g, nil
}

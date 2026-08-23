package api

import (
	"context"
	"net/http"
	"net/url"
)

type ChatState struct {
	ChatID     string `json:"chatId"`
	Archived   bool   `json:"archived"`
	PinnedAt   *int64 `json:"pinnedAt"`
	MuteMode   string `json:"muteMode"`
	MutedUntil *int64 `json:"mutedUntil"`
}

type HistorySyncError struct {
	ChatID  string `json:"chatId,omitempty"`
	Message string `json:"message"`
}

type HistorySyncStatus struct {
	State           string             `json:"state"`
	PendingChats    int                `json:"pendingChats"`
	PendingMessages int                `json:"pendingMessages"`
	ChatsTotal      int                `json:"chatsTotal"`
	ChatsProcessed  int                `json:"chatsProcessed"`
	MessagesAdded   int                `json:"messagesAdded"`
	Errors          []HistorySyncError `json:"errors"`
	StartedAt       *int64             `json:"startedAt"`
	FinishedAt      *int64             `json:"finishedAt"`
	LastRunAt       *int64             `json:"lastRunAt"`
}

func (c *Client) PinChat(ctx context.Context, chatID string, pinned bool) (*ChatState, error) {
	var state ChatState
	err := c.doJSON(ctx, http.MethodPost, "/api/chats/"+url.PathEscape(chatID)+"/pin", map[string]bool{"pinned": pinned}, &state)
	return &state, err
}

func (c *Client) ArchiveChat(ctx context.Context, chatID string, archived bool) (*ChatState, error) {
	var state ChatState
	err := c.doJSON(ctx, http.MethodPost, "/api/chats/"+url.PathEscape(chatID)+"/archive", map[string]bool{"archived": archived}, &state)
	return &state, err
}

func (c *Client) MuteChat(ctx context.Context, chatID, mode string) (*ChatState, error) {
	var state ChatState
	err := c.doJSON(ctx, http.MethodPost, "/api/chats/"+url.PathEscape(chatID)+"/mute", map[string]string{"mode": mode}, &state)
	return &state, err
}

func (c *Client) GetHistorySyncStatus(ctx context.Context) (*HistorySyncStatus, error) {
	var status HistorySyncStatus
	err := c.doJSON(ctx, http.MethodGet, "/api/history-sync/status", nil, &status)
	return &status, err
}

func (c *Client) StartHistorySync(ctx context.Context) (*HistorySyncStatus, error) {
	var status HistorySyncStatus
	err := c.doJSON(ctx, http.MethodPost, "/api/history-sync", map[string]any{}, &status)
	return &status, err
}

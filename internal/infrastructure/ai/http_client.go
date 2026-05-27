package ai

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"time"
)

// AIClient dispatches incoming WhatsApp messages to the Python AI companion server.
// Fire-and-forget — no response parsing needed (Python calls Go REST API directly for actions).
type AIClient struct {
	baseURL string
	client  *http.Client
}

func NewAIClient(serverURL string) *AIClient {
	if serverURL == "" {
		return nil // companion mode disabled when URL empty
	}
	return &AIClient{
		baseURL: serverURL,
		client:  &http.Client{Timeout: 10 * time.Second},
	}
}

type aiRequest struct {
	ChatID     string `json:"chat_id"`
	SenderJID  string `json:"sender_jid"`
	SenderName string `json:"sender_name"`
	Text       string `json:"text"`
	IsGroup    bool   `json:"is_group"`
	Timestamp  int64  `json:"timestamp"`
	MsgID      string `json:"msg_id"`
}

// Dispatch sends the incoming message to the Python AI server.
// Fire-and-forget — errors are logged, never propagated.
func (c *AIClient) Dispatch(ctx context.Context, chatID, senderJID, senderName, text, msgID string, isGroup bool) {
	if c == nil {
		return
	}

	body, err := json.Marshal(aiRequest{
		ChatID:     chatID,
		SenderJID:  senderJID,
		SenderName: senderName,
		Text:       text,
		IsGroup:    isGroup,
		Timestamp:  time.Now().UnixMilli(),
		MsgID:      msgID,
	})
	if err != nil {
		return
	}

	req, err := http.NewRequestWithContext(ctx, "POST", c.baseURL+"/chat", bytes.NewReader(body))
	if err != nil {
		return
	}
	req.Header.Set("Content-Type", "application/json")

	resp, err := c.client.Do(req)
	if err != nil {
		fmt.Printf("[AI] dispatch error: %v\n", err)
		return
	}
	resp.Body.Close()
}

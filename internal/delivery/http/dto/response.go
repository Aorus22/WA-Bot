package dto

import "time"

type MessageResponse struct {
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

type ChatResponse struct {
	ID          string    `json:"id"`
	Name        string    `json:"name"`
	LastMessage string    `json:"lastMessage"`
	UnreadCount int       `json:"unreadCount"`
	UpdatedAt   time.Time `json:"updatedAt"`
}

type ContactResponse struct {
	JID  string `json:"jid"`
	Name string `json:"name"`
}

type TriggerResponse struct {
	ID          string `json:"id"`
	Pattern     string `json:"pattern"`
	Script      string `json:"script"`
	Enabled     bool   `json:"enabled"`
	Description string `json:"description,omitempty"`
	CreatedAt   int64  `json:"createdAt,omitempty"`
	UpdatedAt   int64  `json:"updatedAt,omitempty"`
}

type StickerFavoriteResponse struct {
	ID         string `json:"id"`
	MessageID  string `json:"messageId"`
	MediaURL   string `json:"mediaUrl"`
	IsAnimated bool   `json:"isAnimated"`
	CreatedAt  int64  `json:"createdAt"`
}

type StatusResponse struct {
	IsLoggedIn bool `json:"isLoggedIn"`
}

type SuccessResponse struct {
	Status string `json:"status"`
	ID     string `json:"id,omitempty"`
}

type ErrorResponse struct {
	Error string `json:"error"`
	Code  string `json:"code,omitempty"`
}

type HealthResponse struct {
	Status    string         `json:"status"`
	WhatsApp  WhatsAppHealth `json:"whatsapp"`
	Messages  ServiceHealth  `json:"messages"`
	Triggers  ServiceHealth  `json:"triggers"`
	Timestamp int64          `json:"timestamp"`
}

type WhatsAppHealth struct {
	Connected bool   `json:"connected"`
	Error     string `json:"error,omitempty"`
}

type ServiceHealth struct {
	Available bool   `json:"available"`
	Error     string `json:"error,omitempty"`
}

type TriggerTestResponse struct {
	Matched bool                   `json:"matched"`
	Result  map[string]interface{} `json:"result"`
	Error   string                 `json:"error,omitempty"`
}

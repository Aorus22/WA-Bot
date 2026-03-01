package entity

import (
	"time"

	waProto "go.mau.fi/whatsmeow/proto/waE2E"
)

type Message struct {
	ID        string
	SenderJID string
	ChatID    string
	Text      string
	Media     *Media
	VMessage  *waProto.Message
	Timestamp time.Time
	IsGroup   bool
}

type MediaType string

const (
	MediaTypeImage    MediaType = "image"
	MediaTypeVideo    MediaType = "video"
	MediaTypeGif      MediaType = "gif"
	MediaTypeDocument MediaType = "document"
)

type Media struct {
	Data       []byte
	Type       MediaType
	IsAnimated bool
}

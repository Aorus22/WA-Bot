package dto

import "wa-bot/internal/domain/entity"

type SendMessageRequest struct {
	Secret  string `json:"secret" validate:"required"`
	Target  string `json:"target" validate:"required"`
	Message string `json:"message" validate:"required"`
}

type SendMediaRequest struct {
	Secret    string `json:"secret" validate:"required"`
	Target    string `json:"target" validate:"required"`
	Message   string `json:"message"`
	MediaType string `json:"type" validate:"required,oneof=image video document audio ptt voice audio-ptt"`
	File      []byte `json:"-"`
	Filename  string `json:"-"`
}

type SendStickerRequest struct {
	Secret     string `json:"secret" validate:"required"`
	Target     string `json:"target" validate:"required"`
	MediaURL   string `json:"mediaUrl" validate:"required"`
	IsAnimated bool   `json:"isAnimated"`
}

type BulkSendSameRequest struct {
	Secret  string   `json:"secret" validate:"required"`
	Targets []string `json:"targets" validate:"required,min=1"`
	Message string   `json:"message" validate:"required"`
}

type BulkSendMessage struct {
	Targets string `json:"targets" validate:"required"`
	Message string `json:"message" validate:"required"`
}

type BulkSendDifferentRequest struct {
	Secret   string            `json:"secret" validate:"required"`
	Messages []BulkSendMessage `json:"messages" validate:"required,min=1"`
}

type EditMessageRequest struct {
	Content string `json:"content" validate:"required"`
}

type ReplyMessageRequest struct {
	Content string `json:"content" validate:"required"`
}

type ReactMessageRequest struct {
	Emoji string `json:"emoji" validate:"required"`
	// From is the sender of the reacted-to message (needed to react to other
	// people's messages in groups). Empty means own message.
	From string `json:"from"`
}

type SendPollRequest struct {
	Secret      string   `json:"secret"`
	Question    string   `json:"question" validate:"required"`
	Options     []string `json:"options" validate:"required,min=2"`
	MultiSelect bool     `json:"multiSelect"`
}

type PollVoteRequest struct {
	Options []string `json:"options" validate:"required"`
}

type SendLocationRequest struct {
	Secret    string  `json:"secret"`
	Latitude  float64 `json:"latitude" validate:"required"`
	Longitude float64 `json:"longitude" validate:"required"`
	Name      string  `json:"name"`
	Address   string  `json:"address"`
	Live      bool    `json:"live"`
	Caption   string  `json:"caption"`
}

type SendContactItem struct {
	DisplayName string `json:"displayName" validate:"required"`
	Phone       string `json:"phone"`
	VCard       string `json:"vcard"`
}

type SendContactRequest struct {
	Secret      string            `json:"secret"`
	DisplayName string            `json:"displayName"`
	Phone       string            `json:"phone"`
	VCard       string            `json:"vcard"`
	Contacts    []SendContactItem `json:"contacts"`
}

type ForwardMessageRequest struct {
	Targets []string `json:"targets" validate:"required,min=1"`
}

type TypingRequest struct {
	IsTyping bool `json:"isTyping"`
}

type FavoriteStickerRequest struct {
	Secret     string `json:"secret" validate:"required"`
	MessageID  string `json:"messageId" validate:"required"`
	MediaURL   string `json:"mediaUrl" validate:"required"`
	IsAnimated bool   `json:"isAnimated"`
}

type CreateTriggerRequest entity.Trigger

type UpdateTriggerRequest entity.Trigger

type TestTriggerRequest struct {
	Pattern string `json:"pattern" validate:"required"`
	Script  string `json:"script" validate:"required"`
	Message string `json:"message" validate:"required"`
}

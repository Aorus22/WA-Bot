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
	MediaType string `json:"type" validate:"required,oneof=image video document"`
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

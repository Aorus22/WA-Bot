package repository

import (
	"context"
	"wa-bot/internal/domain/entity"
)

type WebhookRepository interface {
	GetAllWebhooks(ctx context.Context) ([]*entity.Webhook, error)
	GetWebhookByID(ctx context.Context, id string) (*entity.Webhook, error)
	GetWebhookByPath(ctx context.Context, path string) (*entity.Webhook, error)
	CreateWebhook(ctx context.Context, webhook *entity.Webhook) error
	UpdateWebhook(ctx context.Context, webhook *entity.Webhook) error
	DeleteWebhook(ctx context.Context, id string) error
	DeleteAllWebhooks(ctx context.Context) error
}

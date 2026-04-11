package repository

import (
	"context"
	"wa-bot/internal/domain/entity"
)

type WebhookLogRepository interface {
	CreateWebhookLog(ctx context.Context, log *entity.WebhookLog) error
	GetAllWebhookLogs(ctx context.Context, webhookID string, limit int, offset int) ([]*entity.WebhookLog, error)
	GetWebhookLogCount(ctx context.Context, webhookID string) (int, error)
	DeleteAllWebhookLogs(ctx context.Context) error
}

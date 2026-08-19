package repository

import (
	"context"

	"wa-bot/internal/domain/entity"
)

// APIKeyRepository persists API keys.
type APIKeyRepository interface {
	CreateAPIKey(ctx context.Context, key *entity.APIKey) error
	GetAPIKeys(ctx context.Context) ([]*entity.APIKey, error)
	FindAPIKeyByHash(ctx context.Context, hash string) (*entity.APIKey, error)
	TouchAPIKey(ctx context.Context, id string, lastUsedAt int64) error
	RevokeAPIKey(ctx context.Context, id string, revokedAt int64) error
	DeleteAPIKey(ctx context.Context, id string) error
}

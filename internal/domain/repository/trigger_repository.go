package repository

import (
	"context"
	"wa-bot/internal/domain/entity"
)

type TriggerRepository interface {
	GetAll(ctx context.Context) ([]*entity.Trigger, error)
	GetByID(ctx context.Context, id string) (*entity.Trigger, error)
	Create(ctx context.Context, trigger *entity.Trigger) error
	Update(ctx context.Context, trigger *entity.Trigger) error
	Delete(ctx context.Context, id string) error
}

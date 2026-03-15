package repository

import (
	"context"
	"wa-bot/internal/domain/entity"
)

type CronJobRepository interface {
	GetAllCron(ctx context.Context) ([]*entity.CronJob, error)
	GetCronByID(ctx context.Context, id string) (*entity.CronJob, error)
	CreateCron(ctx context.Context, job *entity.CronJob) error
	UpdateCron(ctx context.Context, job *entity.CronJob) error
	DeleteCron(ctx context.Context, id string) error
	DeleteAllCron(ctx context.Context) error
}

package repository

import (
	"context"
)

type AIRepository interface {
	GenerateAnswer(ctx context.Context, filepath, mapel string) (string, error)
}

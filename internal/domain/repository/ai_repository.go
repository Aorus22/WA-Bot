package repository

import (
	"context"
)

type AIRepository interface {
	GenerateAnswer(ctx context.Context, modelName, filepath, mapel string) (string, error)
	GenerateText(ctx context.Context, modelName, prompt string) (string, error)
}

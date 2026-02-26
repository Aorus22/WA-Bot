package repository

import (
	"context"
	"io"
)

type StorageRepository interface {
	Save(ctx context.Context, path string, reader io.Reader) (string, error)
	Get(ctx context.Context, path string) (io.ReadCloser, error)
	Delete(ctx context.Context, path string) error
	GetPath(path string) string
	Exists(path string) bool
}

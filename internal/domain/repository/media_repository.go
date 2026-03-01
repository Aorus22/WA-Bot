package repository

import (
	"context"
)

type MediaRepository interface {
	DownloadFromURL(ctx context.Context, url string) (string, string, error)
	GetDuration(path string) (float64, error)
	GetMimeType(filePath string) (string, error)
}

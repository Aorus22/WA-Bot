package repository

import (
	"context"
	"wa-bot/internal/domain/valueobject"
)

type MediaRepository interface {
	ConvertToWebP(ctx context.Context, input string, opt *valueobject.StickerOptions) (string, error)
	DownloadFromURL(ctx context.Context, url string) (string, string, error)
	GetDuration(path string) (float64, error)
	WriteWebpExif(ctx context.Context, input, packName, author string) (string, error)
	GetInstagramDirectURL(url string, page int) (string, error)
	GetMimeType(filePath string) (string, error)
	IsValidTimeFormat(t string) bool
	ParseTimeFromString(t string) float64
}

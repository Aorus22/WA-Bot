package repository

import (
	"context"
)

type APIRepository interface {
	FetchToken(ctx context.Context, nama, nis string) (string, string, error)
	FetchPDF(ctx context.Context, mapel string, answerKey map[string]string) (string, error)
	FetchMapelList(ctx context.Context) ([]string, error)
	GetToken(name string) (string, error)
	FetchMapel() ([]string, error)
	FetchPDFWithAnswer(mapel, answer string) ([]byte, error)
}

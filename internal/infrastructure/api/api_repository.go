package api

import (
	"context"
	"io"
)

type APIRepository struct {
	pdfClient   *PDFClient
	tokenClient *TokenClient
}

func NewAPIRepository(pdfClient *PDFClient, tokenClient *TokenClient) *APIRepository {
	return &APIRepository{
		pdfClient:   pdfClient,
		tokenClient: tokenClient,
	}
}

func (a *APIRepository) FetchToken(ctx context.Context, nama, nis string) (string, string, error) {
	return a.tokenClient.FetchToken(ctx, nama, nis)
}

func (a *APIRepository) FetchPDF(ctx context.Context, mapel string, answerKey map[string]string) (string, error) {
	return a.pdfClient.FetchPDF(ctx, mapel, answerKey)
}

func (a *APIRepository) FetchMapelList(ctx context.Context) ([]string, error) {
	return a.pdfClient.FetchMapelList(ctx)
}

func (a *APIRepository) GetToken(name string) (string, error) {
	token, _, err := a.tokenClient.FetchToken(context.Background(), name, "")
	return token, err
}

func (a *APIRepository) FetchMapel() ([]string, error) {
	return a.pdfClient.FetchMapelList(context.Background())
}

func (a *APIRepository) FetchPDFWithAnswer(mapel, answer string) ([]byte, error) {
	answerKey := map[string]string{}
	ctx := context.Background()
	path, err := a.pdfClient.FetchPDF(ctx, mapel, answerKey)
	if err != nil {
		return nil, err
	}

	reader, err := a.pdfClient.storage.Get(ctx, path)
	if err != nil {
		return nil, err
	}
	defer reader.Close()

	return io.ReadAll(reader)
}

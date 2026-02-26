package api

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"time"

	"wa-bot/internal/domain/repository"
)

type PDFClient struct {
	baseURL string
	client  *http.Client
	storage repository.StorageRepository
}

func NewPDFClient(baseURL string, storage repository.StorageRepository) *PDFClient {
	return &PDFClient{
		baseURL: baseURL,
		client:  &http.Client{},
		storage: storage,
	}
}

func (p *PDFClient) FetchPDF(ctx context.Context, mapel string, answerKey map[string]string) (string, error) {
	url := fmt.Sprintf("%s/pdf/%s", p.baseURL, mapel)

	filename := fmt.Sprintf("soal_%d.pdf", time.Now().Unix())

	var req *http.Request
	var err error

	if answerKey != nil {
		jsonData := map[string]map[string]map[string]string{
			"datakunci": {"kunci": answerKey},
		}

		jsonBody, err := json.Marshal(jsonData)
		if err != nil {
			return "", err
		}

		req, err = http.NewRequestWithContext(ctx, "POST", url, bytes.NewBuffer(jsonBody))
		if err != nil {
			return "", err
		}
		req.Header.Set("Content-Type", "application/json")
	} else {
		req, err = http.NewRequestWithContext(ctx, "GET", url, nil)
		if err != nil {
			return "", err
		}
	}

	resp, err := p.client.Do(req)
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()

	if _, err := p.storage.Save(ctx, filename, resp.Body); err != nil {
		return "", err
	}

	return filename, nil
}

func (p *PDFClient) FetchMapelList(ctx context.Context) ([]string, error) {
	url := fmt.Sprintf("%s/listmapel", p.baseURL)

	req, err := http.NewRequest("GET", url, nil)
	if err != nil {
		return nil, err
	}

	resp, err := p.client.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, err
	}

	var result struct {
		MapelList []string `json:"mapelList"`
	}

	err = json.Unmarshal(body, &result)
	if err != nil {
		return nil, err
	}

	return result.MapelList, nil
}

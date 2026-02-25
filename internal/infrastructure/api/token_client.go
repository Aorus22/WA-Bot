package api

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
)

type TokenClient struct {
	baseURL string
	client  *http.Client
}

func NewTokenClient(baseURL string) *TokenClient {
	return &TokenClient{
		baseURL: baseURL,
		client:  &http.Client{},
	}
}

func (t *TokenClient) FetchToken(ctx context.Context, nama, nis string) (string, string, error) {
	url := fmt.Sprintf("%s/token", t.baseURL)

	payload := map[string]string{
		"nama": nama,
		"nis":  nis,
	}
	jsonData, err := json.Marshal(payload)
	if err != nil {
		return "", "", err
	}

	req, err := http.NewRequestWithContext(ctx, "POST", url, bytes.NewBuffer(jsonData))
	if err != nil {
		return "", "", err
	}
	req.Header.Set("Content-Type", "application/json")

	resp, err := t.client.Do(req)
	if err != nil {
		return "", "", err
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return "", "", err
	}

	var result struct {
		Status string `json:"status"`
		Token  string `json:"token"`
	}
	err = json.Unmarshal(body, &result)
	if err != nil {
		return "", "", err
	}

	return result.Status, result.Token, nil
}

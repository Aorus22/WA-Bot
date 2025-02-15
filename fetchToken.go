package main

import (
	"encoding/json"
	"net/http"
	"io"
	"bytes"
	"os"
)

func fetchTokenData(nama, nis string) (string, string, error) {
	apiURL := os.Getenv("API_URL")
	payload := map[string]string{
		"nama": nama,
		"nis":  nis,
	}
	jsonData, err := json.Marshal(payload)
	if err != nil {
		return "", "", err
	}

	req, err := http.NewRequest("POST", apiURL, bytes.NewBuffer(jsonData))
	if err != nil {
		return "", "", err
	}
	req.Header.Set("Content-Type", "application/json")

	client := &http.Client{}
	resp, err := client.Do(req)
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
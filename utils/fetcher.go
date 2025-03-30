package utils

import (
	"context"
	"bytes"
	"encoding/json"
	"io"
	"fmt"
	"time"
	"net/http"
	"os"
	"path/filepath"
)

func FetchTokenData(ctx context.Context, nama, nis string) (string, string, error) {
	apiURL := os.Getenv("API_URL")
	payload := map[string]string{
		"nama": nama,
		"nis":  nis,
	}
	jsonData, err := json.Marshal(payload)
	if err != nil {
		return "", "", err
	}

	req, err := http.NewRequestWithContext(ctx, "POST", apiURL, bytes.NewBuffer(jsonData))
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

func FetchPDF(ctx context.Context, mapel string, dataKunci ...map[string]string) (string, error) {
	pdfURL := os.Getenv("PDF_URL")

	url := fmt.Sprintf("%s/pdf/%s", pdfURL, mapel)

	mediaFolder := "media"
	if _, err := os.Stat(mediaFolder); os.IsNotExist(err) {
		err = os.Mkdir(mediaFolder, 0755)
		if err != nil {
			return "", err
		}
	}

	filePath := filepath.Join(mediaFolder, fmt.Sprintf("soal_%d.pdf", time.Now().Unix()))

	var req *http.Request
	var err error

	if len(dataKunci) > 0 && dataKunci[0] != nil {
		jsonData := map[string]map[string]map[string]string{
			"datakunci": {"kunci": dataKunci[0]},
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

	client := &http.Client{}
	resp, err := client.Do(req)
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()

	out, err := os.Create(filePath)
	if err != nil {
		return "", err
	}
	defer out.Close()

	_, err = io.Copy(out, resp.Body)
	if err != nil {
		return "", err
	}

	return filePath, nil
}

func FetchMapel() ([]string, error) {
    pdfURL := os.Getenv("PDF_URL")
    url := fmt.Sprintf("%s/listmapel", pdfURL)

    req, err := http.NewRequest("GET", url, nil)
	if err != nil {
        return nil, err
    }

	client := &http.Client{}
	resp, err := client.Do(req)
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

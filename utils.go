package main

import (
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"regexp"
	"strings"
	"time"
)

func contains(slice []string, item string) bool {
	for _, v := range slice {
		if strings.EqualFold(v, item) {
			return true
		}
	}
	return false
} 

func getLinkFromString(input string) (string, error) {
	urlRegex := regexp.MustCompile(`^(https?:\/\/)?([\w-]+\.)+[\w-]+(:\d+)?(\/[\w\-\.~!*'();:@&=+$,/?%#]*)?$`)
	words := strings.Split(input, " ")
	for _, word := range words {
		if urlRegex.MatchString(word) {
			return word, nil
		}
	}
	return "", errors.New("no link found")
}

func downloadMediaFromURL(url string) (string, error) {
	resp, err := http.Get(url)
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return "", fmt.Errorf("failed to fetch media, status: %d", resp.StatusCode)
	}

	mediaPath := "media/" + fmt.Sprintf("%d", time.Now().UnixMilli())
	file, err := os.Create(mediaPath)
	if err != nil {
		return "", err
	}
	defer file.Close()

	_, err = io.Copy(file, resp.Body)
	if err != nil {
		return "", err
	}

	return mediaPath, nil
}

func getMimeType(filePath string) (string, error) {
	file, err := os.Open(filePath)
	if err != nil {
		return "", err
	}
	defer file.Close()

	buffer := make([]byte, 512)
	_, err = file.Read(buffer)
	if err != nil {
		return "", err
	}

	mimeType := http.DetectContentType(buffer)
	return mimeType, nil
}

func convertToJSON(input string) (map[string]string, error) {
	lines := strings.Split(input, "\n")

	dataKunci := make(map[string]string)

	for _, line := range lines {
		line = strings.TrimSpace(line) 
		if line == "-" || line == "" {
			continue 
		}

		parts := strings.SplitN(line, ".", 2) 
		if len(parts) == 2 {
			nomor := strings.TrimSpace(parts[0])
			jawaban := strings.TrimSpace(parts[1])
			dataKunci[nomor] = strings.ToUpper(jawaban)
		}
	}

	return dataKunci, nil
}
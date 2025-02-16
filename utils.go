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

func downloadImageFromURL(url string) (string, error) {
	resp, err := http.Get(url)
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return "", fmt.Errorf("failed to fetch image, status: %d", resp.StatusCode)
	}

	imagePath := "images/image_" + fmt.Sprintf("%d", time.Now().UnixMilli()) + ".jpg"
	file, err := os.Create(imagePath)
	if err != nil {
		return "", err
	}
	defer file.Close()

	_, err = io.Copy(file, resp.Body)
	if err != nil {
		return "", err
	}

	return imagePath, nil
}

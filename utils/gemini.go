package utils

import (
	"context"
	"fmt"
	"os"
	"regexp"
	"strings"

	"google.golang.org/api/option"
	"github.com/google/generative-ai-go/genai"
)

func sanitizeFileName(name string) string {
	name = strings.ToLower(name)

	re := regexp.MustCompile(`[^a-z0-9]+`)
	name = re.ReplaceAllString(name, "-")

	name = strings.Trim(name, "-")

	return name
}

func GenerateAnswerGemini(ctx context.Context, filepath string, mapel string) (string, error){
	apiKey := os.Getenv("GEMINI_API_KEY")
	if apiKey == "" {
		fmt.Println("GEMINI_API_KEY tidak ditemukan di .env")
		return "", fmt.Errorf("GEMINI_API_KEY tidak ditemukan di .env")
	}

	client, err := genai.NewClient(ctx, option.WithAPIKey(apiKey))
	if err != nil {
		fmt.Printf("Gagal membuat klien: %v\n", err)
		return "", err

	}
	defer client.Close()

	file, err := os.Open(filepath)
	if err != nil {
		fmt.Printf("Gagal membuka file: %v\n", err)
		return "", err
	}
	defer file.Close()

	model := client.GenerativeModel("gemini-2.0-flash")

	re := regexp.MustCompile(`[^a-z0-9]+`)
	file_name := re.ReplaceAllString(mapel, "")

	uploadedFile, err := client.UploadFile(ctx, file_name, file, nil)
	defer client.DeleteFile(ctx, file_name)

	if err != nil {
		if strings.Contains(err.Error(), "already exists") {
			fmt.Println("File sudah ada, mencoba menghapus dan mengunggah ulang...")

			err := client.DeleteFile(ctx, file_name)
			if err != nil {
				return "", err
			}

			uploadedFile, err = client.UploadFile(ctx, file_name, file, nil)
			if err != nil {
				fmt.Printf("Gagal mengunggah ulang file: %v\n", err)
				return "", err
			}
		} else {
			return "", err
		}
	}


	prompt := genai.Text("jawab ini dengan format gini, tanpa ada tambahan lainnya/n1.a/n2.b/n3.c")

	resp, err := model.GenerateContent(ctx, prompt, genai.FileData{URI: uploadedFile.URI})
	if err != nil {
		return "", err
	}

	answer := ""

	for _, cand := range resp.Candidates {
		for _, part := range cand.Content.Parts {
			if text, ok := part.(genai.Text); ok {
				answer += string(text)
			}
		}
	}

	return answer, nil
}


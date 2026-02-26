package ai

import (
	"context"
	"fmt"
	"regexp"
	"strings"

	"github.com/google/generative-ai-go/genai"
	"google.golang.org/api/option"

	"wa-bot/internal/domain/repository"
)

type GeminiService struct {
	client  *genai.Client
	apiKey  string
	storage repository.StorageRepository
}

func NewGeminiService(apiKey string, storage repository.StorageRepository) (*GeminiService, error) {
	client, err := genai.NewClient(context.Background(), option.WithAPIKey(apiKey))
	if err != nil {
		return nil, err
	}

	return &GeminiService{
		client:  client,
		apiKey:  apiKey,
		storage: storage,
	}, nil
}

func (g *GeminiService) GenerateAnswer(ctx context.Context, filepath, mapel string) (string, error) {
	if g.apiKey == "" {
		fmt.Println("GEMINI_API_KEY tidak ditemukan di .env")
		return "", fmt.Errorf("GEMINI_API_KEY tidak ditemukan di .env")
	}

	file, err := g.storage.Get(ctx, filepath)
	if err != nil {
		fmt.Printf("Gagal membuka file dari storage: %v\n", err)
		return "", err
	}
	defer file.Close()

	model := g.client.GenerativeModel("gemini-2.0-flash")

	re := regexp.MustCompile(`[^a-z0-9]+`)
	fileName := re.ReplaceAllString(mapel, "")

	uploadedFile, err := g.client.UploadFile(ctx, fileName, file, nil)
	defer g.client.DeleteFile(ctx, fileName)

	if err != nil {
		if strings.Contains(err.Error(), "already exists") {
			fmt.Println("File sudah ada, mencoba menghapus dan mengunggah ulang...")

			err := g.client.DeleteFile(ctx, fileName)
			if err != nil {
				return "", err
			}

			uploadedFile, err = g.client.UploadFile(ctx, fileName, file, nil)
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

func (g *GeminiService) Close() {
	g.client.Close()
}

package ai

import (
	"context"
	"fmt"
	"regexp"
	"strings"
	"time"

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

func (g *GeminiService) GenerateAnswer(ctx context.Context, modelName, filepath, mapel string) (string, error) {     
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

        model := g.client.GenerativeModel(modelName)
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

func (g *GeminiService) GenerateWithFile(ctx context.Context, modelName, promptStr, filepathStr string) (string, error) {
	if g.apiKey == "" {
		return "", fmt.Errorf("GEMINI_API_KEY not found")
	}

	// 1. Get file from storage
	file, err := g.storage.Get(ctx, filepathStr)
	if err != nil {
		return "", fmt.Errorf("failed to get file from storage: %w", err)
	}
	defer file.Close()

	// 2. Upload file to Gemini
	uploadName := fmt.Sprintf("upload-%d", time.Now().UnixNano())
	uploadedFile, err := g.client.UploadFile(ctx, "", file, &genai.UploadFileOptions{DisplayName: uploadName})
	if err != nil {
		fmt.Printf("[GEMINI] Upload error: %v\n", err)
		return "", fmt.Errorf("failed to upload file to gemini: %w", err)
	}
	fmt.Printf("[GEMINI] File uploaded: %s\n", uploadedFile.URI)
	defer g.client.DeleteFile(ctx, uploadedFile.Name)

	// 3. Generate content
	model := g.client.GenerativeModel(modelName)
	resp, err := model.GenerateContent(ctx, genai.Text(promptStr), genai.FileData{URI: uploadedFile.URI})
	if err != nil {
		fmt.Printf("[GEMINI] Generation error: %v\n", err)
		return "", fmt.Errorf("failed to generate content: %w", err)
	}

	answer := ""
	for _, cand := range resp.Candidates {
		if cand.Content != nil {
			for _, part := range cand.Content.Parts {
				if text, ok := part.(genai.Text); ok {
					answer += string(text)
				}
			}
		}
	}

	return answer, nil
}

func (g *GeminiService) Close() {
	g.client.Close()
}

func (g *GeminiService) GenerateText(ctx context.Context, modelName, prompt string) (string, error) {
	if g.apiKey == "" {
		return "", fmt.Errorf("GEMINI_API_KEY not found")
	}

	model := g.client.GenerativeModel(modelName)
	resp, err := model.GenerateContent(ctx, genai.Text(prompt))
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

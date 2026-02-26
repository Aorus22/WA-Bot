package usecase

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	waTypes "go.mau.fi/whatsmeow/types"

	"wa-bot/internal/domain/entity"
	"wa-bot/internal/domain/repository"
	"wa-bot/internal/infrastructure/ai"
	"wa-bot/internal/infrastructure/api"
	"wa-bot/internal/infrastructure/whatsapp"
)

type PDFUseCase struct {
	waClient      *whatsapp.WhatsAppClient
	apiRepo       *api.APIRepository
	geminiService *ai.GeminiService
	stateRepo     repository.UserStateRepository
}

func NewPDFUseCase(waClient *whatsapp.WhatsAppClient, apiRepo *api.APIRepository, geminiService *ai.GeminiService, stateRepo repository.UserStateRepository) *PDFUseCase {
	return &PDFUseCase{
		waClient:      waClient,
		apiRepo:       apiRepo,
		geminiService: geminiService,
		stateRepo:     stateRepo,
	}
}

func (uc *PDFUseCase) SendPDF(ctx context.Context, senderJID waTypes.JID, command string, role string, msg *entity.Message) error {
	isAllowed := role == "ADMIN" || role == "OWNER"
	if !isAllowed {
		uc.waClient.SendMessageToJID(ctx, senderJID, "Invalid Command", true)
		return nil
	}

	parts := strings.SplitN(command, "\n", 2)
	commandString := parts[0]
	answerBody := ""
	if len(parts) > 1 {
		answerBody = parts[1]
	}

	commandArray := strings.Split(commandString, " ")
	if len(commandArray) != 2 {
		uc.waClient.SendMessageToJID(ctx, senderJID, "Format perintah salah", true)
		return nil
	}

	cmd := commandArray[0]
	mapel := commandArray[1]

	uc.waClient.SendMessageToJID(ctx, senderJID, "⏳ Loading...", true)

	cancelCtx, cancel := context.WithCancel(context.Background())
	uc.stateRepo.AddUser(senderJID.String(), "processing", cancel)

	go func() {
		defer uc.stateRepo.ClearUserState(senderJID.String())
		defer cancel()

		listMapel, err := uc.apiRepo.FetchMapel()
		if err != nil {
			uc.waClient.SendMessageToJID(cancelCtx, senderJID, "Gagal mengambil daftar mapel.", true)
			return
		}

		if index, err := strconv.Atoi(mapel); err == nil {
			if index > 0 && index <= len(listMapel) {
				mapel = listMapel[index-1]
			} else {
				uc.waClient.SendMessageToJID(cancelCtx, senderJID, "Nomor mapel tidak valid.", true)
				return
			}
		} else if !uc.isValidMapel(mapel, listMapel) {
			uc.waClient.SendMessageToJID(cancelCtx, senderJID, "Mapel tidak valid.", true)
			return
		}

		pdfPath, source, err := uc.fetchPDF(cancelCtx, cmd, mapel, answerBody)
		if err != nil {
			uc.waClient.SendMessageToJID(cancelCtx, senderJID, "Gagal mengambil PDF", true)
			return
		}

		pdfData, err := os.ReadFile(pdfPath)
		if err != nil {
			uc.waClient.SendMessageToJID(cancelCtx, senderJID, "Gagal membaca PDF", true)
			return
		}

		mediaURL := "/" + filepath.ToSlash(pdfPath)
		err = uc.waClient.SendDocumentToJID(cancelCtx, senderJID, pdfData, fmt.Sprintf("%s (%s)", mapel, source), mediaURL, true)
		if err != nil {
			uc.waClient.SendMessageToJID(cancelCtx, senderJID, "Gagal mengirim PDF", true)
			return
		}
	}()

	return nil
}

func (uc *PDFUseCase) extractMapel(command string) string {
	parts := strings.Split(command, " ")
	if len(parts) >= 2 {
		return parts[1]
	}
	return ""
}

func (uc *PDFUseCase) isValidMapel(mapel string, listMapel []string) bool {
	for _, m := range listMapel {
		if m == mapel {
			return true
		}
	}
	return false
}

func (uc *PDFUseCase) fetchPDF(ctx context.Context, command, mapel, answerBody string) (string, string, error) {
	var source string
	var path string
	var err error

	switch command {
	case "!pdf":
		source = "Original"
		path, err = uc.apiRepo.FetchPDF(ctx, mapel, nil)
		if err != nil {
			return "", "", err
		}
	case "!answer":
		source = "With Answer"
		answerKey := uc.convertToJSON(answerBody)
		path, err = uc.apiRepo.FetchPDF(ctx, mapel, answerKey)
		if err != nil {
			return "", "", err
		}
	case "!gemini":
		source = "Gemini"
		originalPdfPath, fetchErr := uc.apiRepo.FetchPDF(ctx, mapel, nil)
		if fetchErr != nil {
			return "", "", fetchErr
		}
		defer os.Remove(originalPdfPath)

		answerBody, err = uc.geminiService.GenerateAnswer(ctx, originalPdfPath, mapel)
		if err != nil {
			return "", "", err
		}

		answerKey := uc.convertToJSON(answerBody)
		path, err = uc.apiRepo.FetchPDF(ctx, mapel, answerKey)
		if err != nil {
			return "", "", err
		}
	}

	return path, source, nil
}

func (uc *PDFUseCase) convertToJSON(input string) map[string]string {
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

	return dataKunci
}

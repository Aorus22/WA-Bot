package adminHandlers

import (
	"context"
	"fmt"
	"os"
	"strconv"
	"strings"
	"regexp"

	"github.com/google/generative-ai-go/genai"
	"github.com/joho/godotenv"
	"google.golang.org/api/option"

	"wa-bot/state"
	"wa-bot/utils"
)

func init() {
	err := godotenv.Load()
	if err != nil {
		fmt.Println("Error loading .env file")
	}
}

func sanitizeFileName(name string) string {
	name = strings.ToLower(name)

	re := regexp.MustCompile(`[^a-z0-9]+`)
	name = re.ReplaceAllString(name, "-")

	name = strings.Trim(name, "-")

	return name
}

func GeminiHandler(s *state.MessageState) {

	isAllowed := s.UserRole == "ADMIN" || s.UserRole == "OWNER"

	if !isAllowed {
		s.Reply("Invalid Command")
		return
	}

	mapel := strings.Split(s.MessageText, " ")[1]

	s.Reply("⏳ Loading...")

	listMapel, err := utils.FetchMapel()
	if err != nil {
		utils.LogNoCancelErr(context.Background(), err, "Error fetching mapel:")
		s.Reply("Gagal mengambil daftar mapel.")
		return
	}

	if index, err := strconv.Atoi(mapel); err == nil {
		if index > 0 && index <= len(listMapel) {
			mapel = listMapel[index-1]
		} else {
			s.Reply("Nomor mapel tidak valid.")
			return
		}
	} else if !utils.Contains(listMapel, mapel) {
		s.Reply("Mapel tidak valid.")
		return
	}

	ctx, cancel := context.WithCancel(context.Background())
	s.AddUserToState("processing", cancel)


	go func() {
		defer s.ClearUserState()
		defer cancel()

		pdfPath, err := utils.FetchPDF(ctx, mapel)
		defer os.Remove(pdfPath)
		if err != nil {
			utils.LogNoCancelErr(ctx, err, "Error fetching PDF:")
			s.ReplyNoCancelError(ctx, err, "Gagal mengambil PDF")
			return
		}

		apiKey := os.Getenv("GEMINI_API_KEY")
		if apiKey == "" {
			fmt.Println("GEMINI_API_KEY tidak ditemukan di .env")
			return
		}

		client, err := genai.NewClient(ctx, option.WithAPIKey(apiKey))
		if err != nil {
			fmt.Printf("Gagal membuat klien: %v\n", err)
			return
		}
		defer client.Close()

		file, err := os.Open(pdfPath)
		if err != nil {
			fmt.Printf("Gagal membuka file: %v\n", err)
			return
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

				delErr := client.DeleteFile(ctx, file_name)
				if delErr != nil {
					fmt.Printf("Gagal menghapus file yang sudah ada: %v\n", delErr)
					return
				}

				uploadedFile, err = client.UploadFile(ctx, file_name, file, nil)
				if err != nil {
					fmt.Printf("Gagal mengunggah ulang file: %v\n", err)
					s.Reply("Gagal mengunggah ulang file. Coba lagi")
					return
				}
			} else {
				fmt.Printf("Gagal mengunggah file: %v\n", err)
				return
			}
		}


		prompt := genai.Text("jawab ini dengan format gini, tanpa ada tambahan lainnya/n1.a/n2.b/n3.c")

		resp, err := model.GenerateContent(ctx, prompt, genai.FileData{URI: uploadedFile.URI})
		if err != nil {
			fmt.Printf("Gagal menghasilkan konten: %v\n", err)
			return
		}

		answer := ""

		for _, cand := range resp.Candidates {
			for _, part := range cand.Content.Parts {
				if text, ok := part.(genai.Text); ok {
					answer += string(text)
				}
			}
		}

		pdfAnswerPath, err := utils.FetchPDF(ctx, mapel, convertToJSON(answer))
		defer os.Remove(pdfAnswerPath)

		fileData, err := os.ReadFile(pdfAnswerPath)
		if utils.IsCanceledGoroutine(ctx) { return }
		if err != nil {
			utils.LogNoCancelErr(ctx, err, "Error reading file:")
			s.ReplyNoCancelError(ctx, err, "Gagal mengambil PDF")
			return
		}

		uploaded, err := s.UploadToWhatsapp(ctx, fileData, "document")
		if err != nil {
			utils.LogNoCancelErr(ctx, err, "Error uploading file:")
			s.ReplyNoCancelError(ctx, err, "Gagal mengambil PDF")
			return
		}

		err = s.SendDocumentMessage(ctx, uploaded, mapel)
		if err != nil {
			utils.LogNoCancelErr(ctx, err, "Error sending document message:")
			s.ReplyNoCancelError(ctx, err, "Gagal mengambil PDF")
			return
		}
	}()
}

package adminHandlers

import (
	"context"
	"os"
	"strconv"
	"strings"

	"wa-bot/state"
	"wa-bot/utils"
)

func SendPDFHandler(s *state.MessageState) {
	isAllowed := s.UserRole == "ADMIN" || s.UserRole == "OWNER"

	if !isAllowed {
		s.Reply("Invalid Command")
		return
	}

	messageArray := strings.Split(s.MessageText, " ")
	if len(messageArray) < 2 || len(messageArray) > 3 {
		s.Reply("Format perintah salah")
		return
	}

	mapel := messageArray[1]
	var answer string
	if len(messageArray) == 3 {
		answer = messageArray[2]
	}

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

		var pdfPath string
		var err error

		switch answer {
		case "":
			pdfPath, err = utils.FetchPDF(ctx, mapel)
		default:
			pdfPath, err = utils.FetchPDF(ctx, mapel, convertToJSON(answer))
		}
		if err != nil {
			utils.LogNoCancelErr(ctx, err, "Error fetching PDF:")
			s.ReplyNoCancelError(ctx, err, "Gagal mengambil PDF")
		}
		defer os.Remove(pdfPath)

		fileData, err := os.ReadFile(pdfPath)
		if utils.IsCanceledGoroutine(ctx) { return }
		if err != nil {
			utils.LogNoCancelErr(ctx, err, "Error reading file:")
			s.ReplyNoCancelError(ctx, err, "Gagal mengambil PDF")
		}

		uploaded, err := s.UploadToWhatsapp(ctx, fileData, "document")
		if err != nil {
			utils.LogNoCancelErr(ctx, err, "Error uploading file:")
			s.ReplyNoCancelError(ctx, err, "Gagal mengambil PDF")
		}

		err = s.SendDocumentMessage(ctx, uploaded, mapel)
		if err != nil {
			utils.LogNoCancelErr(ctx, err, "Error sending document message:")
			s.ReplyNoCancelError(ctx, err, "Gagal mengambil PDF")
		}
	}()
}

func convertToJSON(input string) map[string]string {
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
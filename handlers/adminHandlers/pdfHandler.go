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

	parts := strings.SplitN(s.MessageText, "\n", 2)
	commandString := parts[0]
	answerBody := ""
	if len(parts) > 1 {
		answerBody = parts[1]
	}

	commandArray := strings.Split(commandString, " ")
	if len(commandArray) != 2 {
		s.Reply("Format perintah salah")
		return
	}

	command := commandArray[0]
	mapel := commandArray[1]

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

		switch command{
		case "!pdf":
			pdfPath, err = utils.FetchPDF(ctx, mapel)
		case "!answer":
			pdfPath, err = utils.FetchPDF(ctx, mapel, convertToJSON(answerBody))
		}
		defer os.Remove(pdfPath)
		if err != nil {
			utils.LogNoCancelErr(ctx, err, "Error fetching PDF:")
			s.ReplyNoCancelError(ctx, err, "Gagal mengambil PDF")
			return
		}

		fileData, err := os.ReadFile(pdfPath)
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
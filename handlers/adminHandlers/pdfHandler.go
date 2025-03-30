package adminHandlers

import (
	goctx "context"
	"fmt"
	"os"
	"strconv"
	"strings"

	"wa-bot/context"
	"wa-bot/utils"
)

func SendPDFHandler(ctx *context.MessageContext) {
	isAllowed := ctx.UserRole == "ADMIN" || ctx.UserRole == "OWNER"

	if !isAllowed {
		ctx.Reply("Invalid Command")
		return
	}

	messageArray := strings.Split(ctx.MessageText, " ")
	if len(messageArray) < 2 || len(messageArray) > 3 {
		ctx.Reply("Format perintah salah")
		return
	}

	mapel := messageArray[1]
	var answer string
	if len(messageArray) == 3 {
		answer = messageArray[2]
	}

	ctx.Reply("⏳ Loading...")

	listMapel, err := utils.FetchMapel()
	if err != nil {
		ctx.Reply("Gagal mengambil daftar mapel.")
		return
	}

	if index, err := strconv.Atoi(mapel); err == nil {
		if index > 0 && index <= len(listMapel) {
			mapel = listMapel[index-1]
		} else {
			ctx.Reply("Nomor mapel tidak valid.")
			return
		}
	} else if !utils.Contains(listMapel, mapel) {
		ctx.Reply("Mapel tidak valid.")
		return
	}

	procCtx, cancel := goctx.WithCancel(goctx.Background())
	ctx.AddUserToState("processing", cancel)

	go func() {
		defer ctx.ClearUserState()

		var pdfPath string
		var err error

		if utils.IsCanceledGoroutine(procCtx) { return }

		switch answer {
		case "":
			pdfPath, err = utils.FetchPDF(mapel)
		default:
			pdfPath, err = utils.FetchPDF(mapel, convertToJSON(answer))
		}
		defer os.Remove(pdfPath)

		if err != nil {
			fmt.Println("Failed to fetch PDF:", err)
			ctx.Reply("Gagal mengambil PDF")
			return
		}

		if utils.IsCanceledGoroutine(procCtx) { return }

		fileData, err := os.ReadFile(pdfPath)
		if err != nil {
			fmt.Println("Failed to read PDF file:", err)
			return
		}

		if utils.IsCanceledGoroutine(procCtx) { return }

		uploaded, err := ctx.UploadToWhatsapp(fileData, "document")
		if err != nil {
			fmt.Println("Failed to upload PDF:", err)
			return
		}

		if utils.IsCanceledGoroutine(procCtx) { return }

		err = ctx.SendDocumentMessage(uploaded, mapel)
		if err != nil {
			fmt.Println("Failed to send PDF:", err)
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
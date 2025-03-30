package adminHandlers

import (
	"fmt"
	"os"
	"strconv"
	"strings"

	"wa-bot/context"
	"wa-bot/utils"
)

func SendPDFHandler(ctx *context.MessageContext) {
	isAllowed := ctx.UserRole == "ADMIN" || ctx.UserRole == "OWNER"

	if !ctx.IsFromGroup && !isAllowed {
		ctx.Reply("Invalid Command")
		return
	}
	ctx.Reply("⏳ Loading...")

	messageArray := strings.Split(ctx.MessageText, " ")
	if len(messageArray) < 2 && len(messageArray) > 3 {
		return
	}

	mapel := messageArray[1]
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

	if len(messageArray) == 3 {
		answer := messageArray[2]
		sendPDFMessage(ctx, mapel, answer)
	} else if len(messageArray) == 2 {
		sendPDFMessage(ctx, mapel, "")
	} else {
		ctx.Reply("Invalid Command")
	}
}

func sendPDFMessage(ctx *context.MessageContext, mapel string, answer string) {
	var pdfPath string
	var err error

	if answer == "" {
		pdfPath, err = utils.FetchPDF(mapel)
	} else {
		jsonAnswer, err := convertToJSON(answer)

		if err != nil {
			ctx.Reply("Format Jawaban Salah")
		}
		pdfPath, _ = utils.FetchPDF(mapel, jsonAnswer)
	}

	if err != nil {
		fmt.Println("Failed to fetch PDF:", err)
		ctx.Reply("Gagal mengambil PDF")
		return
	}

	fileData, err := os.ReadFile(pdfPath)
	if err != nil {
		fmt.Println("Failed to read PDF file:", err)
		return
	}

	uploaded, err := ctx.UploadToWhatsapp(fileData, "document")
	if err != nil {
		fmt.Println("Failed to upload PDF:", err)
		return
	}

	err = ctx.SendDocumentMessage(uploaded, mapel)

	if err != nil {
		fmt.Println("Failed to send PDF:", err)
	}

	err = os.Remove(pdfPath)
	if err != nil {
		fmt.Println("Failed to delete PDF file:", err)
	}
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
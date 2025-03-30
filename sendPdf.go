package main

import (
	"context"
	"fmt"
	"os"

	"go.mau.fi/whatsmeow"
	waProto "go.mau.fi/whatsmeow/binary/proto"
	waTypes "go.mau.fi/whatsmeow/types"
	"google.golang.org/protobuf/proto"
)

// func sendHTMLMessage(client *whatsmeow.Client, senderJID waTypes.JID) {
// 	resp, err := fetchSoal();

// 	fmt.Println(resp)

// 	if err != nil {
// 		fmt.Println("Failed to read HTML body:", err)
// 		_, _ = client.SendMessage(context.Background(), senderJID, &waProto.Message{
// 			Conversation: proto.String("Gagal membaca HTML"),
// 		})
// 		return
// 	}

// 	_, err = client.SendMessage(context.Background(), senderJID, &waProto.Message{
// 		Conversation: proto.String(resp),
// 	})
// 	if err != nil {
// 		fmt.Println("Failed to send HTML message:", err)
// 	}
// }

func sendPDFMessage(client *whatsmeow.Client, senderJID waTypes.JID, mapel string, answer string) {

	var pdfPath string
	var err error

	if answer == "" {
		pdfPath, err = fetchPDF(mapel)
	} else {
		jsonAnswer, err := convertToJSON(answer)

		if err != nil {
			client.SendMessage(context.Background(), senderJID, &waProto.Message{
				Conversation: proto.String("Format Jawaban Salah"),
			})
		}
		pdfPath, _ = fetchPDF(mapel, jsonAnswer)
	}

	// dataKunci := map[string]string{
	// 	"1": "A",
	// 	"2": "B",
	// 	"3": "C",
	// }

	// fmt.Println(dataKunci)
	// pdfPath, err = fetchPDF(mapel, dataKunci)

	if err != nil {
		fmt.Println("Failed to fetch PDF:", err)
		client.SendMessage(context.Background(), senderJID, &waProto.Message{
			Conversation: proto.String("Gagal mengambil PDF"),
		})
		return
	}

	fileData, err := os.ReadFile(pdfPath)
	if err != nil {
		fmt.Println("Failed to read PDF file:", err)
		return
	}

	uploaded, err := client.Upload(context.Background(), fileData, whatsmeow.MediaDocument)
	if err != nil {
		fmt.Println("Failed to upload PDF:", err)
		return
	}

	_, err = client.SendMessage(context.Background(), senderJID, &waProto.Message{
		DocumentMessage: &waProto.DocumentMessage{
			Title:        proto.String(mapel),
			Mimetype:     proto.String("application/pdf"),
			URL:          proto.String(uploaded.URL),
			DirectPath:   proto.String(uploaded.DirectPath),
			MediaKey:     uploaded.MediaKey,
			FileEncSHA256: uploaded.FileEncSHA256,
			FileSHA256:   uploaded.FileSHA256,
			FileLength:   proto.Uint64(uploaded.FileLength),
		},
	})
	if err != nil {
		fmt.Println("Failed to send PDF:", err)
	}

	err = os.Remove(pdfPath)
	if err != nil {
		fmt.Println("Failed to delete PDF file:", err)
	}
}
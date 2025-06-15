package main

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"net/http"
	"os"

	"github.com/gorilla/mux"
	"github.com/rs/cors"
	waProto "go.mau.fi/whatsmeow/proto/waE2E"
	waTypes "go.mau.fi/whatsmeow/types"
	"google.golang.org/protobuf/proto"
)

type sendRequest struct {
	Secret  string `json:"secret"`
	Target  string `json:"target"`
	Message string `json:"message"`
}

type bulkMessageRequest struct {
	Secret  string   `json:"secret"`
	Targets []string `json:"targets"`
	Message string   `json:"message"`
}

type bulkDifferentMessageRequest struct {
	Secret   string `json:"secret"`
	Messages []struct {
		Targets string `json:"targets"`
		Message string `json:"message"`
	} `json:"messages"`
}

func handleSendMessage(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	var req sendRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		w.WriteHeader(http.StatusBadRequest)
		w.Write([]byte(err.Error()))
		return
	}

	SECRET := os.Getenv("API_SECRET")
	if SECRET == "" {
		SECRET = "default-secret"
	}

	if req.Secret != SECRET {
		w.WriteHeader(http.StatusUnauthorized)
		w.Write([]byte("Unauthorized"))
		return
	}

	targetJID := waTypes.NewJID(req.Target, waTypes.DefaultUserServer)
	waClient.SendMessage(context.Background(), targetJID, &waProto.Message{
		Conversation: proto.String(req.Message),
	})

	w.WriteHeader(http.StatusOK)
	w.Write([]byte("Success"))
}

func handleBulkSendSameMessage(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	var req bulkMessageRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		w.WriteHeader(http.StatusBadRequest)
		w.Write([]byte(err.Error()))
		return
	}

	SECRET := os.Getenv("API_SECRET")
	if SECRET == "" {
		SECRET = "default-secret"
	}

	if req.Secret != SECRET {
		w.WriteHeader(http.StatusUnauthorized)
		w.Write([]byte("Unauthorized"))
		return
	}

	done := make(chan bool)
	for _, target := range req.Targets {
		go func(targetJID string) {
			jid := waTypes.NewJID(targetJID, waTypes.DefaultUserServer)
			waClient.SendMessage(context.Background(), jid, &waProto.Message{
				Conversation: proto.String(req.Message),
			})
			done <- true
		}(target)
	}

	for range req.Targets {
		<-done
	}

	w.WriteHeader(http.StatusOK)
	w.Write([]byte("Bulk same message sent successfully"))
}

func handleBulkSendDifferentMessages(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	var req bulkDifferentMessageRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		w.WriteHeader(http.StatusBadRequest)
		w.Write([]byte(err.Error()))
		return
	}

	SECRET := os.Getenv("API_SECRET")
	if SECRET == "" {
		SECRET = "default-secret"
	}

	if req.Secret != SECRET {
		w.WriteHeader(http.StatusUnauthorized)
		w.Write([]byte("Unauthorized"))
		return
	}

	done := make(chan bool)
	for _, msg := range req.Messages {
		go func(targetJID, message string) {
			jid := waTypes.NewJID(targetJID, waTypes.DefaultUserServer)
			waClient.SendMessage(context.Background(), jid, &waProto.Message{
				Conversation: proto.String(message),
			})
			done <- true
		}(msg.Targets, msg.Message)
	}

	for range req.Messages {
		<-done
	}

	w.WriteHeader(http.StatusOK)
	w.Write([]byte("Bulk different messages sent successfully"))
}

func init() {
	go func() {
		r := mux.NewRouter()
		r.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
			w.Write([]byte("Hello, World!"))
		}).Methods("GET")
		r.HandleFunc("/send-message", handleSendMessage).Methods("POST")
		r.HandleFunc("/send-bulk-same-message", handleBulkSendSameMessage).Methods("POST")
		r.HandleFunc("/send-bulk-different-messages", handleBulkSendDifferentMessages).Methods("POST")

		fmt.Println("Server running in port 3000")
		handler := cors.New(cors.Options{
			AllowedOrigins:   []string{"*"},
			AllowedMethods:   []string{"GET", "POST", "PUT", "DELETE", "OPTIONS"},
			AllowedHeaders:   []string{"*"},
			AllowCredentials: false,
		}).Handler(r)
		log.Fatal(http.ListenAndServe(":3000", handler))
	}()
}

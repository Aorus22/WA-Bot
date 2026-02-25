package http

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"os"

	"github.com/gorilla/mux"
	"github.com/rs/cors"

	"wa-bot/internal/domain/repository"
	whatsappInfra "wa-bot/internal/infrastructure/whatsapp"
)

type HTTPServer struct {
	client *whatsappInfra.WhatsAppClient
	config repository.ConfigRepository
	server *http.Server
}

func NewHTTPServer(client *whatsappInfra.WhatsAppClient, config repository.ConfigRepository) *HTTPServer {
	return &HTTPServer{
		client: client,
		config: config,
	}
}

func (s *HTTPServer) Start() error {
	r := mux.NewRouter()
	r.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		w.Write([]byte("Hello, World!"))
	}).Methods("GET")
	r.HandleFunc("/send-message", s.handleSendMessage).Methods("POST")
	r.HandleFunc("/send-bulk-same-message", s.handleBulkSendSameMessage).Methods("POST")
	r.HandleFunc("/send-bulk-different-messages", s.handleBulkSendDifferentMessages).Methods("POST")

	fmt.Println("Server running in port 3000")
	handler := cors.New(cors.Options{
		AllowedOrigins:   []string{"*"},
		AllowedMethods:   []string{"GET", "POST", "PUT", "DELETE", "OPTIONS"},
		AllowedHeaders:   []string{"*"},
		AllowCredentials: false,
	}).Handler(r)

	s.server = &http.Server{
		Addr:    ":3000",
		Handler: handler,
	}

	return s.server.ListenAndServe()
}

func (s *HTTPServer) Stop() {
	if s.server != nil {
		s.server.Shutdown(context.Background())
	}
}

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

func (s *HTTPServer) handleSendMessage(w http.ResponseWriter, r *http.Request) {
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

	s.client.SendMessage(context.Background(), req.Target, req.Message)

	w.WriteHeader(http.StatusOK)
	w.Write([]byte("Success"))
}

func (s *HTTPServer) handleBulkSendSameMessage(w http.ResponseWriter, r *http.Request) {
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
			s.client.SendMessage(context.Background(), targetJID, req.Message)
			done <- true
		}(target)
	}

	for range req.Targets {
		<-done
	}

	w.WriteHeader(http.StatusOK)
	w.Write([]byte("Bulk same message sent successfully"))
}

func (s *HTTPServer) handleBulkSendDifferentMessages(w http.ResponseWriter, r *http.Request) {
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
			s.client.SendMessage(context.Background(), targetJID, message)
			done <- true
		}(msg.Targets, msg.Message)
	}

	for range req.Messages {
		<-done
	}

	w.WriteHeader(http.StatusOK)
	w.Write([]byte("Bulk different messages sent successfully"))
}

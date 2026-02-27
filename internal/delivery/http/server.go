package http

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"mime"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/gorilla/mux"
	"github.com/rs/cors"

	"wa-bot/internal/domain/entity"
	"wa-bot/internal/domain/repository"
	whatsappInfra "wa-bot/internal/infrastructure/whatsapp"
)

type HTTPServer struct {
	client      *whatsappInfra.WhatsAppClient
	config      repository.ConfigRepository
	storage     repository.StorageRepository
	server      *http.Server
	hub         *WSHub
	msgRepo     *repository.MessageStore
	triggerRepo repository.TriggerRepository
	lua         LuaService
}

type LuaService interface {
	TestTrigger(ctx context.Context, pattern, script, message string) (map[string]interface{}, error)
}

func NewHTTPServer(client *whatsappInfra.WhatsAppClient, config repository.ConfigRepository, storage repository.StorageRepository) *HTTPServer {
	return &HTTPServer{
		client:  client,
		config:  config,
		storage: storage,
		hub:     NewWSHub(),
	}
}

func (s *HTTPServer) SetLuaService(lua LuaService) {
	s.lua = lua
}

func (s *HTTPServer) SetMessageRepo(repo *repository.MessageStore) {
	s.msgRepo = repo
}

func (s *HTTPServer) SetTriggerRepo(repo repository.TriggerRepository) {
	s.triggerRepo = repo
}

func (s *HTTPServer) GetHub() *WSHub {
	return s.hub
}

func (s *HTTPServer) Start() error {
	r := mux.NewRouter()

	// API routes (must be registered before frontend)
	api := r.PathPrefix("/api").Subrouter()
	api.HandleFunc("/send-message", s.handleSendMessage).Methods("POST", "OPTIONS")
	api.HandleFunc("/send-media", s.handleSendMedia).Methods("POST", "OPTIONS")
	api.HandleFunc("/send-sticker", s.handleSendSticker).Methods("POST", "OPTIONS")
	api.HandleFunc("/send-bulk-same-message", s.handleBulkSendSameMessage).Methods("POST", "OPTIONS")
	api.HandleFunc("/send-bulk-different-messages", s.handleBulkSendDifferentMessages).Methods("POST", "OPTIONS")
	api.HandleFunc("/chats", s.handleGetChats).Methods("GET")
	api.HandleFunc("/chats/{id}/messages", s.handleGetMessages).Methods("GET")
	api.HandleFunc("/chats/{id}/read", s.handleMarkAsRead).Methods("POST", "OPTIONS")
	api.HandleFunc("/stickers/favorites", s.handleGetFavoriteStickers).Methods("GET")
	api.HandleFunc("/stickers/favorite", s.handleFavoriteSticker).Methods("POST", "OPTIONS")
	api.HandleFunc("/stickers/favorites/{id}", s.handleDeleteFavoriteSticker).Methods("DELETE", "OPTIONS")
	api.HandleFunc("/contacts", s.handleGetContacts).Methods("GET")
	api.HandleFunc("/status", s.handleGetStatus).Methods("GET")
	api.HandleFunc("/logout", s.handleLogout).Methods("POST", "OPTIONS")

	// Trigger Management
	api.HandleFunc("/triggers", s.handleGetTriggers).Methods("GET")
	api.HandleFunc("/triggers", s.handleCreateTrigger).Methods("POST", "OPTIONS")
	api.HandleFunc("/triggers/test", s.handleTestTrigger).Methods("POST", "OPTIONS")
	api.HandleFunc("/triggers/{id}", s.handleUpdateTrigger).Methods("PUT", "OPTIONS")
	api.HandleFunc("/triggers/{id}", s.handleDeleteTrigger).Methods("DELETE", "OPTIONS")

	// Media files - custom handler to URL decode filenames
	api.PathPrefix("/media/").Handler(http.StripPrefix("/api/media/", http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// URL decode the filename
		filename := r.URL.Path
		decodedFilename, err := url.QueryUnescape(filename)
		if err != nil {
			decodedFilename = filename
		}
		fmt.Printf("Serving media: %s (decoded: %s)\n", filename, decodedFilename)

		// Check if file exists
		if !s.storage.Exists(decodedFilename) {
			http.NotFound(w, r)
			fmt.Printf("File not found in storage: %s\n", decodedFilename)
			return
		}

		// Set content type based on extension
		ext := strings.ToLower(filepath.Ext(decodedFilename))
		switch ext {
		case ".jpg", ".jpeg":
			w.Header().Set("Content-Type", "image/jpeg")
		case ".png":
			w.Header().Set("Content-Type", "image/png")
		case ".webp":
			w.Header().Set("Content-Type", "image/webp")
		case ".gif":
			w.Header().Set("Content-Type", "image/gif")
		case ".mp4":
			w.Header().Set("Content-Type", "video/mp4")
		case ".pdf":
			w.Header().Set("Content-Type", "application/pdf")
		}

		reader, err := s.storage.Get(r.Context(), decodedFilename)
		if err != nil {
			http.Error(w, "Failed to get file", http.StatusInternalServerError)
			return
		}
		defer reader.Close()

		io.Copy(w, reader)
	})))

	// Avatar proxy - proxy WhatsApp avatar URLs
	api.HandleFunc("/avatar/{jid}", s.handleAvatarProxy).Methods("GET")

	// WebSocket
	s.RegisterWSRoutes(r, s.hub)

	// Static files for frontend
	frontendPath := filepath.Join(".", "frontend", "my-app", "dist")

	r.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		if _, err := os.Stat(frontendPath); err == nil {
			// Serve index.html for root or non-file paths
			if r.URL.Path == "/" {
				w.Header().Set("Content-Type", "text/html")
				http.ServeFile(w, r, filepath.Join(frontendPath, "index.html"))
				return
			}

			// Serve other files with correct MIME type
			filePath := filepath.Join(frontendPath, filepath.Clean(r.URL.Path))

			// Check if file exists
			if _, err := os.Stat(filePath); os.IsNotExist(err) {
				// Fall back to index.html for SPA routing
				w.Header().Set("Content-Type", "text/html")
				http.ServeFile(w, r, filepath.Join(frontendPath, "index.html"))
				return
			}

			// Set correct MIME type
			ext := strings.ToLower(filepath.Ext(filePath))
			switch ext {
			case ".css":
				w.Header().Set("Content-Type", "text/css")
			case ".js":
				w.Header().Set("Content-Type", "application/javascript")
			case ".html":
				w.Header().Set("Content-Type", "text/html")
			case ".json":
				w.Header().Set("Content-Type", "application/json")
			case ".png":
				w.Header().Set("Content-Type", "image/png")
			case ".jpg", ".jpeg":
				w.Header().Set("Content-Type", "image/jpeg")
			case ".svg":
				w.Header().Set("Content-Type", "image/svg+xml")
			case ".ico":
				w.Header().Set("Content-Type", "image/x-icon")
			case ".woff":
				w.Header().Set("Content-Type", "font/woff")
			case ".woff2":
				w.Header().Set("Content-Type", "font/woff2")
			case ".ttf":
				w.Header().Set("Content-Type", "font/ttf")
			default:
				// Use mime package as fallback
				if mimeType := mime.TypeByExtension(ext); mimeType != "" {
					w.Header().Set("Content-Type", mimeType)
				}
			}

			http.ServeFile(w, r, filePath)
		} else {
			fmt.Println("Frontend dist not found at:", frontendPath, "- serving Hello World")
			w.Write([]byte("Hello, World!"))
		}
	})

	// Start hub in background
	go s.hub.Run()

	fmt.Println("Server running on port 3000")
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

func (s *HTTPServer) BroadcastMessage(msgType string, payload interface{}) {
	s.hub.Broadcast(WSMessage{
		Type:    msgType,
		Payload: payload,
	})
}

func (s *HTTPServer) LogSentMessage(msgID, chatID, from, to, content, msgType, mediaURL string, isAutomatic bool) {
	msg := &repository.Message{
		ID:          msgID,
		ChatID:      chatID,
		From:        from,
		To:          to,
		Content:     content,
		Timestamp:   time.Now().UnixMilli(),
		Status:      "sent",
		Type:        msgType,
		MediaURL:    mediaURL,
		IsAutomatic: isAutomatic,
	}
	s.SaveAndBroadcastMessage(msg)
}

func (s *HTTPServer) UpdateMessageStatus(msgID, status string) {
	if s.msgRepo != nil {
		if err := s.msgRepo.UpdateMessageStatus(msgID, status); err != nil {
			fmt.Printf("Failed to update message status: %v\n", err)
			return
		}

		// Broadcast via WebSocket
		if s.hub != nil {
			s.hub.Broadcast(WSMessage{
				Type: "message_status",
				Payload: map[string]interface{}{
					"id":     msgID,
					"status": status,
				},
			})
		}
	}
}

func (s *HTTPServer) SaveAndBroadcastMessage(msg *repository.Message) {
	if s.msgRepo != nil {
		if err := s.msgRepo.SaveMessage(msg); err != nil {
			fmt.Printf("Failed to save message: %v\n", err)
			return
		}
		fmt.Printf("\u2713 Saved message to database (auto=%v)\n", msg.IsAutomatic)

		// Broadcast via WebSocket
		if s.hub != nil {
			s.hub.Broadcast(WSMessage{
				Type: "new_message",
				Payload: map[string]interface{}{
					"id":          msg.ID,
					"chatId":      msg.ChatID,
					"from":        msg.From,
					"to":          msg.To,
					"content":     msg.Content,
					"timestamp":   msg.Timestamp,
					"status":      msg.Status,
					"type":        msg.Type,
					"mediaUrl":    msg.MediaURL,
					"isAutomatic": msg.IsAutomatic,
					"senderName":  msg.SenderName,
				},
			})
			fmt.Printf("\u2713 Broadcasted message via WebSocket\n")
		}
	}
}

func (s *HTTPServer) SendToUser(userID string, msgType string, payload interface{}) {
	s.hub.SendToUser(userID, WSMessage{
		Type:    msgType,
		Payload: payload,
	})
}

type sendRequest struct {
	Secret  string `json:"secret"`
	Target  string `json:"target"`
	Message string `json:"message"`
}

func (s *HTTPServer) handleSendMedia(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	// Parse multipart form
	if err := r.ParseMultipartForm(50 << 20); err != nil { // 50MB max
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(map[string]string{"error": "Failed to parse form: " + err.Error()})
		return
	}

	secret := r.FormValue("secret")
	target := r.FormValue("target")
	message := r.FormValue("message")
	mediaType := r.FormValue("type") // "image", "video", "document"

	SECRET := os.Getenv("API_SECRET")
	if SECRET == "" {
		SECRET = "default-secret"
	}

	if secret != SECRET {
		w.WriteHeader(http.StatusUnauthorized)
		json.NewEncoder(w).Encode(map[string]string{"error": "Unauthorized"})
		return
	}

	file, header, err := r.FormFile("file")
	if err != nil {
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(map[string]string{"error": "Failed to get file: " + err.Error()})
		return
	}
	defer file.Close()

	// Read file data
	data := make([]byte, header.Size)
	if _, err := file.Read(data); err != nil {
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "Failed to read file: " + err.Error()})
		return
	}

	fmt.Printf("📤 Sending %s to: %s\n", mediaType, target)

	os.MkdirAll("media", 0755)
	ext := filepath.Ext(header.Filename)
	if ext == "" {
		if mediaType == "image" {
			ext = ".jpg"
		} else if mediaType == "video" {
			ext = ".mp4"
		} else {
			ext = ".bin"
		}
	}
	safeJID := strings.ReplaceAll(target, "@", "_")
	safeJID = strings.ReplaceAll(safeJID, ".", "_")
	filename := fmt.Sprintf("sent_%d_%s%s", time.Now().UnixMilli(), safeJID, ext)
	mediaURL := fmt.Sprintf("/media/%s", filename)

	if _, err := s.storage.Save(r.Context(), filename, bytes.NewReader(data)); err != nil {
		fmt.Printf("Failed to persist sent media: %v\n", err)
		mediaURL = ""
	}

	ctx := context.Background()
	var sendErr error

	switch mediaType {
	case "image":
		sendErr = s.client.SendImage(ctx, target, data, message, mediaURL, false)
	case "video":
		sendErr = s.client.SendVideo(ctx, target, data, message, mediaURL, false)
	default: // document or anything else
		sendErr = s.client.SendDocument(ctx, target, data, header.Filename, mediaURL, false)
	}

	if sendErr != nil {
		fmt.Printf("\u23ec Failed to send media: %v\n", sendErr)
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": sendErr.Error()})
		return
	}

	fmt.Printf("\u2713 Media sent successfully to %s\n", target)

	w.WriteHeader(http.StatusOK)
	json.NewEncoder(w).Encode(map[string]string{"status": "success"})
}

func (s *HTTPServer) handleSendMessage(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	var req sendRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
		return
	}

	SECRET := os.Getenv("API_SECRET")
	if SECRET == "" {
		SECRET = "default-secret"
	}

	if req.Secret != SECRET {
		w.WriteHeader(http.StatusUnauthorized)
		json.NewEncoder(w).Encode(map[string]string{"error": "Unauthorized"})
		return
	}

	fmt.Printf("📤 Sending message to: %s | Content: %s\n", req.Target, req.Message)

	err := s.client.SendMessage(context.Background(), req.Target, req.Message, false)
	if err != nil {
		fmt.Printf("\u23ec Failed to send WhatsApp message: %v\n", err)
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
		return
	}

	fmt.Printf("\u2713 WhatsApp message sent to %s\n", req.Target)

	w.WriteHeader(http.StatusOK)
	json.NewEncoder(w).Encode(map[string]string{"status": "success"})
}

func (s *HTTPServer) handleBulkSendSameMessage(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	var req struct {
		Secret  string   `json:"secret"`
		Targets []string `json:"targets"`
		Message string   `json:"message"`
	}
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
			s.client.SendMessage(context.Background(), targetJID, req.Message, false)
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

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	var req struct {
		Secret   string `json:"secret"`
		Messages []struct {
			Targets string `json:"targets"`
			Message string `json:"message"`
		} `json:"messages"`
	}
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
			s.client.SendMessage(context.Background(), targetJID, message, false)
			done <- true
		}(msg.Targets, msg.Message)
	}

	for range req.Messages {
		<-done
	}

	w.WriteHeader(http.StatusOK)
	w.Write([]byte("Bulk different messages sent successfully"))
}

func (s *HTTPServer) handleGetFavoriteStickers(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if s.msgRepo == nil {
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "Message repository not configured"})
		return
	}

	stickers, err := s.msgRepo.GetFavoriteStickers()
	if err != nil {
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
		return
	}

	if stickers == nil {
		stickers = []map[string]interface{}{}
	}

	json.NewEncoder(w).Encode(stickers)
}

func (s *HTTPServer) handleFavoriteSticker(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	var req struct {
		Secret     string `json:"secret"`
		MessageID  string `json:"messageId"`
		MediaURL   string `json:"mediaUrl"`
		IsAnimated bool   `json:"isAnimated"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
		return
	}

	SECRET := os.Getenv("API_SECRET")
	if SECRET == "" {
		SECRET = "default-secret"
	}

	if req.Secret != SECRET {
		w.WriteHeader(http.StatusUnauthorized)
		json.NewEncoder(w).Encode(map[string]string{"error": "Unauthorized"})
		return
	}

	if s.msgRepo != nil {
		err := s.msgRepo.SaveFavoriteSticker(req.MessageID, req.MediaURL, req.IsAnimated)
		if err != nil {
			w.WriteHeader(http.StatusInternalServerError)
			json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
			return
		}
	}

	w.WriteHeader(http.StatusOK)
	json.NewEncoder(w).Encode(map[string]string{"status": "success"})
}

func (s *HTTPServer) handleDeleteFavoriteSticker(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	vars := mux.Vars(r)
	id := vars["id"]

	if s.msgRepo != nil {
		err := s.msgRepo.DeleteFavoriteSticker(id)
		if err != nil {
			w.WriteHeader(http.StatusInternalServerError)
			json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
			return
		}
	}

	w.WriteHeader(http.StatusOK)
	json.NewEncoder(w).Encode(map[string]string{"status": "success"})
}

func (s *HTTPServer) handleSendSticker(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	var req struct {
		Secret     string `json:"secret"`
		Target     string `json:"target"`
		MediaURL   string `json:"mediaUrl"`
		IsAnimated bool   `json:"isAnimated"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
		return
	}

	SECRET := os.Getenv("API_SECRET")
	if SECRET == "" {
		SECRET = "default-secret"
	}

	if req.Secret != SECRET {
		w.WriteHeader(http.StatusUnauthorized)
		json.NewEncoder(w).Encode(map[string]string{"error": "Unauthorized"})
		return
	}

	localPath := strings.TrimPrefix(req.MediaURL, "/")
	localPath = strings.TrimPrefix(localPath, "api/")

	fmt.Printf("📤 Sending sticker to: %s | Path: %s\n", req.Target, localPath)

	reader, err := s.storage.Get(context.Background(), localPath)
	if err != nil {
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "Failed to read sticker file: " + err.Error()})
		return
	}
	data, err := io.ReadAll(reader)
	reader.Close()
	if err != nil {
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "Failed to read sticker file: " + err.Error()})
		return
	}

	err = s.client.SendSticker(context.Background(), req.Target, data, req.IsAnimated, req.MediaURL, false)
	if err != nil {
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
		return
	}

	w.WriteHeader(http.StatusOK)
	json.NewEncoder(w).Encode(map[string]string{"status": "success"})
}

func (s *HTTPServer) handleGetStatus(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	isLoggedIn := false
	if s.client != nil {
		isLoggedIn = s.client.IsLoggedIn()
	}

	json.NewEncoder(w).Encode(map[string]interface{}{
		"isLoggedIn": isLoggedIn,
	})
}

func (s *HTTPServer) handleLogout(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	if s.client == nil {
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "WhatsApp client not initialized"})
		return
	}

	err := s.client.Logout()
	if err != nil {
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
		return
	}

	json.NewEncoder(w).Encode(map[string]string{"status": "success"})
}

func (s *HTTPServer) handleMarkAsRead(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	if s.msgRepo == nil {
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "Message repository not configured"})
		return
	}

	vars := mux.Vars(r)
	chatID := vars["id"]

	err := s.msgRepo.MarkAsRead(chatID)
	if err != nil {
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
		return
	}

	json.NewEncoder(w).Encode(map[string]string{"status": "success"})
}

func (s *HTTPServer) handleGetChats(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if s.msgRepo == nil {
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "Message repository not configured"})
		return
	}

	chats, err := s.msgRepo.GetChats()
	if err != nil {
		fmt.Printf("Error getting chats: %v\n", err)
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
		return
	}

	if chats == nil {
		chats = []repository.Chat{}
	}

	json.NewEncoder(w).Encode(chats)
}

func (s *HTTPServer) handleGetMessages(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if s.msgRepo == nil {
		w.WriteHeader(http.StatusNotImplemented)
		json.NewEncoder(w).Encode(map[string]string{"error": "Message repository not configured"})
		return
	}

	vars := mux.Vars(r)
	chatID := vars["id"]

	messages, err := s.msgRepo.GetMessages(chatID, 100)
	if err != nil {
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
		return
	}

	json.NewEncoder(w).Encode(messages)
}

func (s *HTTPServer) handleGetContacts(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if s.msgRepo == nil {
		w.WriteHeader(http.StatusNotImplemented)
		json.NewEncoder(w).Encode(map[string]string{"error": "Message repository not configured"})
		return
	}

	contacts, err := s.msgRepo.GetContacts()
	if err != nil {
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
		return
	}

	json.NewEncoder(w).Encode(contacts)
}

func (s *HTTPServer) handleAvatarProxy(w http.ResponseWriter, r *http.Request) {
	vars := mux.Vars(r)
	jid := vars["jid"]

	avatarURL, err := s.client.GetProfilePictureInfo(context.Background(), jid)
	if err != nil {
		http.Error(w, "Avatar not found", http.StatusNotFound)
		return
	}

	http.Redirect(w, r, avatarURL, http.StatusFound)
}

func generateID() string {
	return fmt.Sprintf("%d", time.Now().UnixMilli())
}

// Trigger Handlers
func (s *HTTPServer) handleGetTriggers(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	if s.triggerRepo == nil {
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "Repository not configured"})
		return
	}
	triggers, err := s.triggerRepo.GetAll(r.Context())
	if err != nil {
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
		return
	}
	if triggers == nil {
		triggers = []*entity.Trigger{}
	}
	json.NewEncoder(w).Encode(triggers)
}

func (s *HTTPServer) handleCreateTrigger(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}
	var t entity.Trigger
	if err := json.NewDecoder(r.Body).Decode(&t); err != nil {
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
		return
	}
	if t.ID == "" {
		t.ID = generateID()
	}
	if err := s.triggerRepo.Create(r.Context(), &t); err != nil {
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
		return
	}
	w.WriteHeader(http.StatusCreated)
	json.NewEncoder(w).Encode(t)
}

func (s *HTTPServer) handleUpdateTrigger(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}
	vars := mux.Vars(r)
	id := vars["id"]
	var t entity.Trigger
	if err := json.NewDecoder(r.Body).Decode(&t); err != nil {
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
		return
	}
	t.ID = id
	if err := s.triggerRepo.Update(r.Context(), &t); err != nil {
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
		return
	}
	json.NewEncoder(w).Encode(t)
}

func (s *HTTPServer) handleDeleteTrigger(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}
	vars := mux.Vars(r)
	id := vars["id"]
	if err := s.triggerRepo.Delete(r.Context(), id); err != nil {
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
		return
	}
	w.WriteHeader(http.StatusOK)
	json.NewEncoder(w).Encode(map[string]string{"status": "success"})
}

func (s *HTTPServer) handleTestTrigger(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	if s.lua == nil {
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "Lua service not initialized"})
		return
	}

	var req struct {
		Pattern string `json:"pattern"`
		Script  string `json:"script"`
		Message string `json:"message"`
	}

	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
		return
	}

	result, err := s.lua.TestTrigger(r.Context(), req.Pattern, req.Script, req.Message)
	if err != nil {
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
		return
	}

	json.NewEncoder(w).Encode(result)
}

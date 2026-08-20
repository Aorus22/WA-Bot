package http

import (
	"context"
	"encoding/json"
	"fmt"
	"net"
	"net/http"
	"sync"
	"time"

	"github.com/gorilla/mux"

	"wa-bot/internal/delivery/cron"
	"wa-bot/internal/delivery/http/handlers"
	"wa-bot/internal/delivery/http/middleware"
	"wa-bot/internal/delivery/http/routes"
	"wa-bot/internal/domain/repository"
	"wa-bot/internal/infrastructure/ai"
	"wa-bot/internal/infrastructure/call"
	whatsappInfra "wa-bot/internal/infrastructure/whatsapp"
)

type LuaService interface {
	TestTrigger(ctx context.Context, pattern, script, message string) (map[string]interface{}, error)
}

type HTTPServer struct {
	server        *http.Server
	handler       *handlers.Handler
	router        *routes.Router
	hub           *WSHub
	msgRepo       *repository.MessageStore
	config        repository.ConfigRepository
	cronScheduler *cron.CronScheduler
	latestQRCode  string
	qrMu          sync.RWMutex
	callMedia     http.HandlerFunc
}

func NewHTTPServer(client *whatsappInfra.WhatsAppClient, config repository.ConfigRepository, storage repository.StorageRepository) *HTTPServer {
	h := handlers.NewHandler(client, config, storage)
	r := routes.NewRouter(h, storage)
	hub := NewWSHub()

	return &HTTPServer{
		handler: h,
		router:  r,
		hub:     hub,
		config:  config,
	}
}

func (s *HTTPServer) SetLuaService(lua LuaService) {
	s.handler.SetLuaService(lua)
}

func (s *HTTPServer) SetMessageRepo(repo *repository.MessageStore) {
	s.msgRepo = repo
	s.handler.SetMessageRepo(repo)
}

func (s *HTTPServer) SetTriggerRepo(repo repository.TriggerRepository) {
	s.handler.SetTriggerRepo(repo)
}

func (s *HTTPServer) SetCronRepo(repo repository.CronJobRepository) {
	s.handler.SetCronRepo(repo)
}

func (s *HTTPServer) SetWebhookRepo(repo repository.WebhookRepository) {
	s.handler.SetWebhookRepo(repo)
}

func (s *HTTPServer) SetWebhookLogRepo(repo repository.WebhookLogRepository) {
	s.handler.SetWebhookLogRepo(repo)
}

func (s *HTTPServer) SetCronScheduler(cs *cron.CronScheduler) {
	s.cronScheduler = cs
	s.handler.SetCronScheduler(cs)
}

func (s *HTTPServer) SetGeminiService(gemini *ai.GeminiService) {
	s.handler.SetGeminiService(gemini)
}

func (s *HTTPServer) SetCallService(svc *call.CallService) {
	s.handler.SetCallService(svc)
}

func (s *HTTPServer) SetAPIKeyRepo(repo repository.APIKeyRepository) {
	s.handler.SetAPIKeyRepo(repo)
}

func (s *HTTPServer) SetTTSProvider(p call.TTSProvider) {
	s.handler.SetTTSProvider(p)
}

// SetCallMediaHandler registers the dedicated binary media WebSocket handler.
func (s *HTTPServer) SetCallMediaHandler(h http.HandlerFunc) {
	s.callMedia = h
	s.router.SetCallMediaHandler(h)
}

func (s *HTTPServer) GetHub() *WSHub {
	return s.hub
}

func (s *HTTPServer) Start() error {
	s.handler.SetHub(s.hub)
	s.router.SetWebSocketHandler(s.handleWebSocket(s.hub))
	s.router.SetQrHandler(s.handleQRCode)

	muxRouter := s.router.RegisterRoutes()

	go s.hub.Run()

	port := s.config.Get("PORT")
	if port == "" {
		port = ":3000"
	}

	// Use net.Listen to support auto-port (:0) for desktop mode
	ln, err := net.Listen("tcp", "0.0.0.0"+port)
	if err != nil {
		return fmt.Errorf("failed to listen: %w", err)
	}
	actualPort := ln.Addr().(*net.TCPAddr).Port

	s.server = &http.Server{
		Handler:      s.createHandler(muxRouter),
		ReadTimeout:  30 * time.Second,
		WriteTimeout: 30 * time.Second,
		IdleTimeout:  120 * time.Second,
	}

	// Print for parent process (Electron) to detect the port
	fmt.Printf("BACKEND_PORT:%d\n", actualPort)
	fmt.Printf("Server running on port %d\n", actualPort)
	return s.server.Serve(ln)
}

// StartWithListener starts the HTTP server using a pre-created listener
// (port is already bound, just set up routes and serve)
func (s *HTTPServer) StartWithListener(ln net.Listener) error {
	s.handler.SetHub(s.hub)
	s.router.SetWebSocketHandler(s.handleWebSocket(s.hub))
	s.router.SetQrHandler(s.handleQRCode)

	muxRouter := s.router.RegisterRoutes()

	go s.hub.Run()

	s.server = &http.Server{
		Handler:      s.createHandler(muxRouter),
		ReadTimeout:  30 * time.Second,
		WriteTimeout: 30 * time.Second,
		IdleTimeout:  120 * time.Second,
	}

	fmt.Printf("Server running on port %d\n", ln.Addr().(*net.TCPAddr).Port)
	return s.server.Serve(ln)
}

// BindPort listens on the configured port and returns the listener + actual port
// without starting the HTTP server (useful for printing BACKEND_PORT early)
func (s *HTTPServer) BindPort() (net.Listener, int, error) {
	port := s.config.Get("PORT")
	if port == "" {
		port = ":3000"
	}
	ln, err := net.Listen("tcp", "0.0.0.0"+port)
	if err != nil {
		return nil, 0, fmt.Errorf("failed to listen: %w", err)
	}
	actualPort := ln.Addr().(*net.TCPAddr).Port
	return ln, actualPort, nil
}

func (s *HTTPServer) Stop() {
	if s.server != nil {
		ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		_ = s.server.Shutdown(ctx)
	}
}

func (s *HTTPServer) BroadcastMessage(msgType string, payload interface{}) {
	// Cache latest QR code
	if msgType == "qr_code" {
		if m, ok := payload.(map[string]string); ok {
			s.qrMu.Lock()
			s.latestQRCode = m["code"]
			s.qrMu.Unlock()
		}
	}
	s.hub.Broadcast(WSMessage{
		Type:    msgType,
		Payload: payload,
	})
}

func (s *HTTPServer) handleQRCode(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	s.qrMu.RLock()
	defer s.qrMu.RUnlock()
	json.NewEncoder(w).Encode(map[string]string{
		"code": s.latestQRCode,
	})
}

func (s *HTTPServer) LogSentMessage(msgID, chatID, from, to, content, msgType, mediaURL string, isAutomatic bool, replyToID string) {
	s.handler.LogSentMessage(msgID, chatID, from, to, content, msgType, mediaURL, isAutomatic, replyToID)
}

func (s *HTTPServer) SaveAndBroadcastMessage(msg *repository.Message) {
	s.handler.SaveAndBroadcastMessage(msg)
}

func (s *HTTPServer) UpdateMessageStatus(msgID, status string) {
	s.handler.UpdateMessageStatus(msgID, status)
}

func (s *HTTPServer) SendToUser(userID string, msgType string, payload interface{}) {
	s.handler.SendToUser(userID, msgType, payload)
}

func (s *HTTPServer) createHandler(r *mux.Router) http.Handler {
	handler := http.Handler(r)
	handler = middleware.CORS(handler)
	handler = middleware.Recovery(handler)
	handler = middleware.Logging(handler)
	return handler
}

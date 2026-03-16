package http

import (
	"context"
	"fmt"
	"net/http"
	"time"

	"github.com/gorilla/mux"

	"wa-bot/internal/domain/repository"
	"wa-bot/internal/delivery/cron"
	"wa-bot/internal/delivery/http/handlers"
	"wa-bot/internal/delivery/http/middleware"
	"wa-bot/internal/delivery/http/routes"
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

func (s *HTTPServer) SetCronScheduler(cs *cron.CronScheduler) {
	s.cronScheduler = cs
	s.handler.SetCronScheduler(cs)
}

func (s *HTTPServer) GetHub() *WSHub {
	return s.hub
}

func (s *HTTPServer) Start() error {
	s.handler.SetHub(s.hub)
	s.router.SetWebSocketHandler(s.handleWebSocket(s.hub))

	muxRouter := s.router.RegisterRoutes()

	go s.hub.Run()

	port := s.config.Get("PORT")
	if port == "" {
		port = "3000"
	}

	s.server = &http.Server{
		Addr:         ":" + port,
		Handler:      s.createHandler(muxRouter),
		ReadTimeout:  30 * time.Second,
		WriteTimeout: 30 * time.Second,
		IdleTimeout:  120 * time.Second,
	}

	fmt.Printf("Server running on port %s\n", port)
	return s.server.ListenAndServe()
}

func (s *HTTPServer) Stop() {
	if s.server != nil {
		ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		_ = s.server.Shutdown(ctx)
	}
}

func (s *HTTPServer) BroadcastMessage(msgType string, payload interface{}) {
	s.hub.Broadcast(WSMessage{
		Type:    msgType,
		Payload: payload,
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

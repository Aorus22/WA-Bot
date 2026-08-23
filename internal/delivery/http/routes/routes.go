package routes

import (
	"io"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strings"

	"github.com/gorilla/mux"

	"wa-bot/internal/delivery/http/handlers"
	"wa-bot/internal/delivery/http/middleware"
	"wa-bot/internal/domain/repository"
	"wa-bot/internal/infrastructure/lua"
)

type Router struct {
	muxRouter *mux.Router
	handler   *handlers.Handler
	storage   repository.StorageRepository
	wsHandler http.HandlerFunc
	qrHandler http.HandlerFunc
	callMedia http.HandlerFunc
}

func NewRouter(h *handlers.Handler, storage repository.StorageRepository) *Router {
	return &Router{
		muxRouter: mux.NewRouter(),
		handler:   h,
		storage:   storage,
	}
}

func (r *Router) SetWebSocketHandler(handler http.HandlerFunc) {
	r.wsHandler = handler
}

func (r *Router) SetQrHandler(handler http.HandlerFunc) {
	r.qrHandler = handler
}

// SetCallMediaHandler registers the dedicated binary media WebSocket handler,
// served at the top-level (non-API) /ws/calls/{id}/media path.
func (r *Router) SetCallMediaHandler(handler http.HandlerFunc) {
	r.callMedia = handler
}

func (r *Router) RegisterRoutes() *mux.Router {
	messageHandler := handlers.NewMessageHandler(r.handler)
	chatHandler := handlers.NewChatHandler(r.handler)
	historySyncHandler := handlers.NewHistorySyncHandler(r.handler)
	triggerHandler := handlers.NewTriggerHandler(r.handler)

	// Type assertion needed because handler interface doesn't match concrete type directly in struct initialization
	luaSvc := r.handler.GetLuaService().(*lua.LuaService)

	cronHandler := handlers.NewCronHandler(r.handler.GetCronRepo(), r.handler.GetCronScheduler(), luaSvc)
	cronHandler.SetHandler(r.handler)
	stickerHandler := handlers.NewStickerHandler(r.handler)
	systemHandler := handlers.NewSystemHandler(r.handler)
	msgMgmtHandler := handlers.NewMessageManagementHandler(r.handler)
	aiHandler := handlers.NewAIHandler(r.handler)

	callHandler := handlers.NewCallHandler(r.handler)
	apiKeyHandler := handlers.NewAPIKeyHandler(r.handler)
	extCallHandler := handlers.NewExternalCallHandler(r.handler)

	settingsHandler := handlers.NewSettingsHandler(r.handler, r.handler.GetSettingsRepo())

	api := r.muxRouter.PathPrefix("/api").Subrouter()

	api.HandleFunc("/send-message", messageHandler.SendMessage).Methods("POST", "OPTIONS")
	api.HandleFunc("/send-media", messageHandler.SendMedia).Methods("POST", "OPTIONS")
	api.HandleFunc("/send-sticker", messageHandler.SendSticker).Methods("POST", "OPTIONS")
	api.HandleFunc("/send-bulk-same-message", messageHandler.BulkSendSame).Methods("POST", "OPTIONS")
	api.HandleFunc("/send-bulk-different-messages", messageHandler.BulkSendDifferent).Methods("POST", "OPTIONS")

	api.HandleFunc("/chats", chatHandler.GetChats).Methods("GET")
	api.HandleFunc("/chats/{id}/messages", chatHandler.GetMessages).Methods("GET")
	api.HandleFunc("/chats/{id}/search", chatHandler.SearchMessages).Methods("GET")
	api.HandleFunc("/chats/{id}/messages/{msgId}/context", chatHandler.GetMessageContext).Methods("GET")
	api.HandleFunc("/chats/{id}/media", chatHandler.GetChatMedia).Methods("GET")
	api.HandleFunc("/chats/{id}/docs", chatHandler.GetChatDocs).Methods("GET")
	api.HandleFunc("/chats/{id}/links", chatHandler.GetChatLinks).Methods("GET")
	api.HandleFunc("/chats/{id}/read", chatHandler.MarkAsRead).Methods("POST", "OPTIONS")
	api.HandleFunc("/chats/{id}/pin", chatHandler.PinChat).Methods("POST", "OPTIONS")
	api.HandleFunc("/chats/{id}/archive", chatHandler.ArchiveChat).Methods("POST", "OPTIONS")
	api.HandleFunc("/chats/{id}/mute", chatHandler.MuteChat).Methods("POST", "OPTIONS")
	api.HandleFunc("/chats/{id}/messages/{messageId}/media", chatHandler.GetHistoricalMedia).Methods("GET")
	api.HandleFunc("/history-sync", historySyncHandler.Start).Methods("POST", "OPTIONS")
	api.HandleFunc("/history-sync/status", historySyncHandler.Status).Methods("GET")

	api.HandleFunc("/chats/{chatId}/messages/{id}/delete", msgMgmtHandler.DeleteMessage).Methods("POST", "OPTIONS")
	api.HandleFunc("/chats/{chatId}/messages/{id}/edit", msgMgmtHandler.EditMessage).Methods("POST", "OPTIONS")
	api.HandleFunc("/chats/{chatId}/messages/{id}/reply", msgMgmtHandler.ReplyMessage).Methods("POST", "OPTIONS")
	api.HandleFunc("/chats/{chatId}/messages/{id}/react", messageHandler.SendReaction).Methods("POST", "OPTIONS")
	api.HandleFunc("/chats/{chatId}/typing", messageHandler.SendTyping).Methods("POST", "OPTIONS")

	api.HandleFunc("/stickers/favorites", stickerHandler.GetFavorites).Methods("GET")
	api.HandleFunc("/stickers/favorite", stickerHandler.FavoriteSticker).Methods("POST", "OPTIONS")
	api.HandleFunc("/stickers/favorites/{id}", stickerHandler.DeleteFavorite).Methods("DELETE", "OPTIONS")

	api.HandleFunc("/contacts", chatHandler.GetContacts).Methods("GET")

	api.HandleFunc("/status", systemHandler.GetStatus).Methods("GET")
	api.HandleFunc("/qr-code", r.qrHandler).Methods("GET")
	api.HandleFunc("/logout", systemHandler.Logout).Methods("POST", "OPTIONS")
	api.HandleFunc("/health", systemHandler.HealthCheck).Methods("GET")

	// DB-backed TTS + AI settings. No auth middleware for now, matching
	// /api/triggers and /api/cron.
	api.HandleFunc("/settings", settingsHandler.Get).Methods("GET")
	api.HandleFunc("/settings", settingsHandler.Update).Methods("PUT", "OPTIONS")

	api.HandleFunc("/triggers", triggerHandler.GetTriggers).Methods("GET")
	api.HandleFunc("/triggers", triggerHandler.CreateTrigger).Methods("POST", "OPTIONS")
	api.HandleFunc("/triggers/test", triggerHandler.TestTrigger).Methods("POST", "OPTIONS")
	api.HandleFunc("/triggers/{id}", triggerHandler.UpdateTrigger).Methods("PUT", "OPTIONS")
	api.HandleFunc("/triggers", triggerHandler.DeleteAllTriggers).Methods("DELETE", "OPTIONS")
	api.HandleFunc("/triggers/{id}", triggerHandler.DeleteTrigger).Methods("DELETE", "OPTIONS")

	api.HandleFunc("/cron", cronHandler.GetAll).Methods("GET")
	api.HandleFunc("/cron", cronHandler.Create).Methods("POST", "OPTIONS")
	api.HandleFunc("/cron", cronHandler.DeleteAll).Methods("DELETE", "OPTIONS")
	api.HandleFunc("/cron/test", cronHandler.Test).Methods("POST", "OPTIONS")
	api.HandleFunc("/cron/{id}", cronHandler.Update).Methods("PUT", "OPTIONS")
	api.HandleFunc("/cron/{id}", cronHandler.Delete).Methods("DELETE", "OPTIONS")

	webhookHandler := handlers.NewWebhookHandler(r.handler, r.handler.GetWebhookRepo(), r.handler.GetWebhookLogRepo(), luaSvc)

	api.HandleFunc("/webhooks", webhookHandler.GetAll).Methods("GET")
	api.HandleFunc("/webhooks", webhookHandler.Create).Methods("POST", "OPTIONS")
	api.HandleFunc("/webhooks/test", webhookHandler.Test).Methods("POST", "OPTIONS")
	api.HandleFunc("/webhooks/logs", webhookHandler.GetLogs).Methods("GET")
	api.HandleFunc("/webhooks/logs", webhookHandler.DeleteAllLogs).Methods("DELETE", "OPTIONS")
	api.HandleFunc("/webhooks/{id}", webhookHandler.Update).Methods("PUT", "OPTIONS")
	api.HandleFunc("/webhooks/{id}", webhookHandler.Delete).Methods("DELETE", "OPTIONS")
	api.HandleFunc("/webhooks", webhookHandler.DeleteAll).Methods("DELETE", "OPTIONS")
	api.HandleFunc("/docs", aiHandler.GetDocs).Methods("GET")
	api.HandleFunc("/ai/assistant", aiHandler.ChatAssistant).Methods("POST", "OPTIONS")

	// Call routes. Literal paths (active/history) are registered before the
	// {id} pattern to avoid mux matching conflicts.
	api.HandleFunc("/calls/active", callHandler.GetActiveCall).Methods("GET")
	api.HandleFunc("/calls/history", callHandler.GetHistory).Methods("GET")
	api.HandleFunc("/calls", callHandler.CreateCall).Methods("POST", "OPTIONS")
	api.HandleFunc("/calls/group", callHandler.CreateGroupCall).Methods("POST", "OPTIONS")
	api.HandleFunc("/calls/{id}/answer", callHandler.AnswerCall).Methods("POST", "OPTIONS")
	api.HandleFunc("/calls/{id}/reject", callHandler.RejectCall).Methods("POST", "OPTIONS")
	api.HandleFunc("/calls/{id}/hangup", callHandler.HangupCall).Methods("POST", "OPTIONS")
	api.HandleFunc("/calls/{id}/video/start", callHandler.StartVideo).Methods("POST", "OPTIONS")
	api.HandleFunc("/calls/{id}/video/accept", callHandler.AcceptVideo).Methods("POST", "OPTIONS")
	api.HandleFunc("/calls/{id}/video/reject", callHandler.RejectVideo).Methods("POST", "OPTIONS")
	api.HandleFunc("/calls/{id}/video/stop", callHandler.StopVideo).Methods("POST", "OPTIONS")
	api.HandleFunc("/calls/{id}/participants", callHandler.AddCallParticipants).Methods("POST", "OPTIONS")
	api.HandleFunc("/calls/{id}/ring", callHandler.RingCallParticipant).Methods("POST", "OPTIONS")

	// Internal API key management (PRD §40). These routes manage credentials
	// that mint calls:write keys, so they require the internal X-API-Secret
	// (API_SECRET env). Missing/misconfigured secret fails closed (401).
	api.HandleFunc("/api-keys", middleware.Auth(apiKeyHandler.List)).Methods("GET", "OPTIONS")
	api.HandleFunc("/api-keys", middleware.Auth(apiKeyHandler.Create)).Methods("POST", "OPTIONS")
	api.HandleFunc("/api-keys/{id}/revoke", middleware.Auth(apiKeyHandler.Revoke)).Methods("POST", "OPTIONS")
	api.HandleFunc("/api-keys/{id}", middleware.Auth(apiKeyHandler.Delete)).Methods("DELETE", "OPTIONS")

	// External call API (PRD §33-37), authenticated via bearer API keys. The
	// create + hangup routes require calls:write; status requires calls:read.
	apiKeyMW := middleware.NewAPIKeyMiddleware(r.handler.GetAPIKeyRepo())
	external := api.PathPrefix("/external/v1").Subrouter()
	external.HandleFunc("/calls", apiKeyMW.RequireScope("calls:write", extCallHandler.CreateCall)).Methods("POST", "OPTIONS")
	external.HandleFunc("/calls/{id}/hangup", apiKeyMW.RequireScope("calls:write", extCallHandler.HangupCall)).Methods("POST", "OPTIONS")
	external.HandleFunc("/calls/{id}", apiKeyMW.RequireScope("calls:read", extCallHandler.GetCallStatus)).Methods("GET", "OPTIONS")

	api.HandleFunc("/avatar/{jid}", systemHandler.AvatarProxy).Methods("GET")

	api.PathPrefix("/media/").Handler(http.StripPrefix("/api/media/", http.HandlerFunc(r.handleMediaFile)))

	if r.wsHandler != nil {
		r.muxRouter.HandleFunc("/ws", r.wsHandler)
	}

	if r.callMedia != nil {
		r.muxRouter.HandleFunc("/ws/calls/{id}/media", r.callMedia)
	}

	r.muxRouter.HandleFunc("/webhook/{path:.+}", webhookHandler.ExecuteWebhook)

	r.setupStaticRoutes()

	return r.muxRouter
}

func (r *Router) handleMediaFile(w http.ResponseWriter, req *http.Request) {
	filename := req.URL.Path
	decodedFilename, err := url.QueryUnescape(filename)
	if err != nil {
		decodedFilename = filename
	}

	if !r.storage.Exists(decodedFilename) {
		http.NotFound(w, req)
		return
	}

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
	case ".ogg":
		w.Header().Set("Content-Type", "audio/ogg")
	case ".mp3":
		w.Header().Set("Content-Type", "audio/mpeg")
	case ".m4a":
		w.Header().Set("Content-Type", "audio/mp4")
	case ".opus":
		w.Header().Set("Content-Type", "audio/ogg")
	case ".wav":
		w.Header().Set("Content-Type", "audio/wav")
	case ".webm":
		w.Header().Set("Content-Type", "audio/webm")
	}

	reader, err := r.storage.Get(req.Context(), decodedFilename)
	if err != nil {
		http.Error(w, "Failed to get file", http.StatusInternalServerError)
		return
	}
	defer reader.Close()

	io.Copy(w, reader)
}

func (r *Router) setupStaticRoutes() {
	frontendPath := filepath.Join(".", "frontend", "dist")

	r.muxRouter.PathPrefix("/").Handler(http.HandlerFunc(func(w http.ResponseWriter, req *http.Request) {
		if _, err := os.Stat(frontendPath); err == nil {
			filePath := filepath.Join(frontendPath, filepath.Clean(req.URL.Path))
			if info, err := os.Stat(filePath); err == nil && !info.IsDir() {
				http.ServeFile(w, req, filePath)
				return
			}
			http.ServeFile(w, req, filepath.Join(frontendPath, "index.html"))
		} else {
			w.Write([]byte("Frontend build not found"))
		}
	}))
}

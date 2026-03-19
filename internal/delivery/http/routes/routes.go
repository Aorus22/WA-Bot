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
	"wa-bot/internal/domain/repository"
	"wa-bot/internal/infrastructure/lua"
)

type Router struct {
	muxRouter *mux.Router
	handler   *handlers.Handler
	storage   repository.StorageRepository
	wsHandler http.HandlerFunc
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

func (r *Router) RegisterRoutes() *mux.Router {
	messageHandler := handlers.NewMessageHandler(r.handler)
	chatHandler := handlers.NewChatHandler(r.handler)
	triggerHandler := handlers.NewTriggerHandler(r.handler)
	
	// Type assertion needed because handler interface doesn't match concrete type directly in struct initialization
	luaSvc := r.handler.GetLuaService().(*lua.LuaService)
	
	cronHandler := handlers.NewCronHandler(r.handler.GetCronRepo(), r.handler.GetCronScheduler(), luaSvc)
	cronHandler.SetHandler(r.handler)
	stickerHandler := handlers.NewStickerHandler(r.handler)
	systemHandler := handlers.NewSystemHandler(r.handler)
	msgMgmtHandler := handlers.NewMessageManagementHandler(r.handler)
	aiHandler := handlers.NewAIHandler(r.handler)

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

	api.HandleFunc("/chats/{chatId}/messages/{id}/delete", msgMgmtHandler.DeleteMessage).Methods("POST", "OPTIONS")
	api.HandleFunc("/chats/{chatId}/messages/{id}/edit", msgMgmtHandler.EditMessage).Methods("POST", "OPTIONS")
	api.HandleFunc("/chats/{chatId}/messages/{id}/reply", msgMgmtHandler.ReplyMessage).Methods("POST", "OPTIONS")

	api.HandleFunc("/stickers/favorites", stickerHandler.GetFavorites).Methods("GET")
	api.HandleFunc("/stickers/favorite", stickerHandler.FavoriteSticker).Methods("POST", "OPTIONS")
	api.HandleFunc("/stickers/favorites/{id}", stickerHandler.DeleteFavorite).Methods("DELETE", "OPTIONS")

	api.HandleFunc("/contacts", chatHandler.GetContacts).Methods("GET")

	api.HandleFunc("/status", systemHandler.GetStatus).Methods("GET")
	api.HandleFunc("/logout", systemHandler.Logout).Methods("POST", "OPTIONS")
	api.HandleFunc("/health", systemHandler.HealthCheck).Methods("GET")

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

	api.HandleFunc("/docs", aiHandler.GetDocs).Methods("GET")
	api.HandleFunc("/ai/assistant", aiHandler.ChatAssistant).Methods("POST", "OPTIONS")

	api.HandleFunc("/avatar/{jid}", systemHandler.AvatarProxy).Methods("GET")

	api.PathPrefix("/media/").Handler(http.StripPrefix("/api/media/", http.HandlerFunc(r.handleMediaFile)))

	if r.wsHandler != nil {
		r.muxRouter.HandleFunc("/ws", r.wsHandler)
	}

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

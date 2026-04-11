package handlers

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/gorilla/mux"

	"wa-bot/internal/domain/repository"
	"wa-bot/internal/delivery/cron"
	whatsappInfra "wa-bot/internal/infrastructure/whatsapp"
	"wa-bot/internal/infrastructure/ai"
)

type LuaService interface {
	TestTrigger(ctx context.Context, pattern, script, message string) (map[string]interface{}, error)
}

type Handler struct {
	client        *whatsappInfra.WhatsAppClient
	config        repository.ConfigRepository
	storage       repository.StorageRepository
	msgRepo       *repository.MessageStore
	triggerRepo   repository.TriggerRepository
	cronRepo      repository.CronJobRepository
	webhookRepo    repository.WebhookRepository
	webhookLogRepo repository.WebhookLogRepository
	cronScheduler *cron.CronScheduler
	lua           LuaService
	gemini        *ai.GeminiService
	hub           any
}

func NewHandler(
	client *whatsappInfra.WhatsAppClient,
	config repository.ConfigRepository,
	storage repository.StorageRepository,
) *Handler {
	return &Handler{
		client:  client,
		config:  config,
		storage: storage,
	}
}

func (h *Handler) SetMessageRepo(repo *repository.MessageStore) {
	h.msgRepo = repo
}

func (h *Handler) SetTriggerRepo(repo repository.TriggerRepository) {
	h.triggerRepo = repo
}

func (h *Handler) SetCronRepo(repo repository.CronJobRepository) {
	h.cronRepo = repo
}

func (h *Handler) SetWebhookRepo(repo repository.WebhookRepository) {
	h.webhookRepo = repo
}

func (h *Handler) GetWebhookRepo() repository.WebhookRepository {
	return h.webhookRepo
}

func (h *Handler) SetWebhookLogRepo(repo repository.WebhookLogRepository) {
	h.webhookLogRepo = repo
}

func (h *Handler) GetWebhookLogRepo() repository.WebhookLogRepository {
	return h.webhookLogRepo
}

func (h *Handler) SetCronScheduler(scheduler *cron.CronScheduler) {
	h.cronScheduler = scheduler
}

func (h *Handler) SetLuaService(lua LuaService) {
	h.lua = lua
}

func (h *Handler) SetGeminiService(gemini *ai.GeminiService) {
	h.gemini = gemini
}

func (h *Handler) GetCronRepo() repository.CronJobRepository {
	return h.cronRepo
}

func (h *Handler) GetCronScheduler() *cron.CronScheduler {
	return h.cronScheduler
}

func (h *Handler) GetLuaService() LuaService {
	return h.lua
}

func (h *Handler) GetGeminiService() *ai.GeminiService {
	return h.gemini
}

func (h *Handler) SetHub(hub any) {
	h.hub = hub
}

func (h *Handler) GetHub() any {
	return h.hub
}

func (h *Handler) sendJSON(w http.ResponseWriter, data interface{}) {
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(data)
}

func (h *Handler) sendJSONWithStatus(w http.ResponseWriter, status int, data interface{}) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	json.NewEncoder(w).Encode(data)
}

func (h *Handler) sendError(w http.ResponseWriter, code int, msg string) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(code)
	json.NewEncoder(w).Encode(map[string]string{"error": msg})
}

func (h *Handler) sendSuccess(w http.ResponseWriter, data map[string]interface{}) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(http.StatusOK)
	if data == nil {
		data = make(map[string]interface{})
	}
	if _, ok := data["status"]; !ok {
		data["status"] = "success"
	}
	json.NewEncoder(w).Encode(data)
}

func (h *Handler) getJID(r *http.Request, key string) string {
	vars := mux.Vars(r)
	return vars[key]
}

func (h *Handler) getQueryParam(r *http.Request, key string) string {
	return r.URL.Query().Get(key)
}

func (h *Handler) readJSON(r *http.Request, v interface{}) error {
	return json.NewDecoder(r.Body).Decode(v)
}

func (h *Handler) readMultipartFile(r *http.Request, fieldName string) ([]byte, *multipartFileHeader, error) {
	if err := r.ParseMultipartForm(50 << 20); err != nil {
		return nil, nil, fmt.Errorf("failed to parse form: %w", err)
	}

	file, header, err := r.FormFile(fieldName)
	if err != nil {
		return nil, nil, fmt.Errorf("failed to get file: %w", err)
	}
	defer file.Close()

	data := make([]byte, header.Size)
	if _, err := file.Read(data); err != nil {
		return nil, nil, fmt.Errorf("failed to read file: %w", err)
	}

	return data, &multipartFileHeader{
		Filename: header.Filename,
		Size:     header.Size,
	}, nil
}

type multipartFileHeader struct {
	Filename string
	Size     int64
}

func (h *Handler) saveMediaFile(ctx context.Context, data []byte, target, filename string) (string, error) {
	os.MkdirAll("media", 0755)

	ext := filepath.Ext(filename)
	if ext == "" {
		ext = ".bin"
	}

	safeJID := strings.ReplaceAll(target, "@", "_")
	safeJID = strings.ReplaceAll(safeJID, ".", "_")
	savedFilename := fmt.Sprintf("sent_%d_%s%s", time.Now().UnixMilli(), safeJID, ext)
	mediaURL := fmt.Sprintf("/media/%s", savedFilename)

	if _, err := h.storage.Save(ctx, savedFilename, bytes.NewReader(data)); err != nil {
		return "", fmt.Errorf("failed to save media: %w", err)
	}

	return mediaURL, nil
}

func (h *Handler) LogSentMessage(msgID, chatID, from, to, content, msgType, mediaURL string, isAutomatic bool, replyToID string) {
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
		ReplyToID:   replyToID,
	}
	h.SaveAndBroadcastMessage(msg)
}

type wsHub interface {
	BroadcastMessage(msgType string, payload interface{})
	SendMessageToUser(userID string, msgType string, payload interface{})
	Run()
}

func (h *Handler) SaveAndBroadcastMessage(msg *repository.Message) {
	if h.msgRepo != nil {
		if err := h.msgRepo.SaveMessage(msg); err != nil {
			fmt.Printf("Failed to save message: %v\n", err)
			return
		}
		fmt.Printf("[DB] Saved message to database (auto=%v)\n", msg.IsAutomatic)

		if hub, ok := h.hub.(wsHub); ok && hub != nil {
			resolvedChatID := h.msgRepo.ResolveChatID(msg.ChatID)
			payload := map[string]interface{}{
				"id":          msg.ID,
				"chatId":      resolvedChatID,
				"from":        msg.From,
				"to":          msg.To,
				"content":     msg.Content,
				"timestamp":   msg.Timestamp,
				"status":      msg.Status,
				"type":        msg.Type,
				"mediaUrl":    msg.MediaURL,
				"isAutomatic": msg.IsAutomatic,
				"senderName":  msg.SenderName,
				"chatName":    msg.ChatName,
			}
			if msg.ReplyToID != "" {
				payload["replyToId"] = msg.ReplyToID
			}
			fmt.Printf("[WS] Broadcasted message via WebSocket (chatId=%s, resolved=%s, id=%s)\n", msg.ChatID, resolvedChatID, msg.ID)
			hub.BroadcastMessage("new_message", payload)
		}
	}
}

func (h *Handler) UpdateMessageStatus(msgID, status string) {
	if h.msgRepo != nil {
		if err := h.msgRepo.UpdateMessageStatus(msgID, status); err != nil {
			fmt.Printf("Failed to update message status: %v\n", err)
			return
		}

		if hub, ok := h.hub.(wsHub); ok && hub != nil {
			hub.BroadcastMessage("message_status", map[string]interface{}{
				"id":     msgID,
				"status": status,
			})
		}
	}
}

func (h *Handler) BroadcastMessage(msgType string, payload interface{}) {
	if hub, ok := h.hub.(wsHub); ok && hub != nil {
		hub.BroadcastMessage(msgType, payload)
	}
}

func (h *Handler) SendToUser(userID string, msgType string, payload interface{}) {
	if hub, ok := h.hub.(wsHub); ok && hub != nil {
		hub.SendMessageToUser(userID, msgType, payload)
	}
}

func generateID() string {
	return fmt.Sprintf("%d", time.Now().UnixMilli())
}

func readAllData(reader io.ReadCloser) ([]byte, error) {
	defer reader.Close()
	return io.ReadAll(reader)
}

type bytesReader interface {
	io.Reader
}

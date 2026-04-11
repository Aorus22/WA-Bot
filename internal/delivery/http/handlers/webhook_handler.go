package handlers

import (
	"bytes"
	"encoding/json"
	"io"
	"net/http"
	"net/url"
	"regexp"
	"strconv"
	"strings"
	"time"

	"github.com/gorilla/mux"
	"github.com/google/uuid"

	"wa-bot/internal/domain/entity"
	"wa-bot/internal/domain/repository"
	"wa-bot/internal/infrastructure/lua"
)

type WebhookHandler struct {
	handler        *Handler
	repo           repository.WebhookRepository
	webhookLogRepo repository.WebhookLogRepository
	luaService     *lua.LuaService
}

func NewWebhookHandler(handler *Handler, repo repository.WebhookRepository, webhookLogRepo repository.WebhookLogRepository, luaService *lua.LuaService) *WebhookHandler {
	return &WebhookHandler{
		handler:        handler,
		repo:           repo,
		webhookLogRepo: webhookLogRepo,
		luaService:     luaService,
	}
}

var validPathRegex = regexp.MustCompile(`^[a-zA-Z0-9._-]+$`)

func (h *WebhookHandler) GetAll(w http.ResponseWriter, r *http.Request) {
	webhooks, err := h.repo.GetAllWebhooks(r.Context())
	if err != nil {
		h.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}
	h.handler.sendJSON(w, webhooks)
}

func (h *WebhookHandler) Create(w http.ResponseWriter, r *http.Request) {
	var webhook entity.Webhook
	if err := h.handler.readJSON(r, &webhook); err != nil {
		h.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}

	if webhook.Path == "" {
		h.handler.sendError(w, http.StatusBadRequest, "path is required")
		return
	}

	if !validPathRegex.MatchString(webhook.Path) {
		h.handler.sendError(w, http.StatusBadRequest, "path must only contain alphanumeric characters, dots, hyphens, and underscores")
		return
	}

	webhook.ID = uuid.New().String()
	if err := h.repo.CreateWebhook(r.Context(), &webhook); err != nil {
		h.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	h.handler.sendJSONWithStatus(w, http.StatusCreated, webhook)
}

func (h *WebhookHandler) Update(w http.ResponseWriter, r *http.Request) {
	vars := mux.Vars(r)
	id := vars["id"]

	var webhook entity.Webhook
	if err := h.handler.readJSON(r, &webhook); err != nil {
		h.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}

	if webhook.Path != "" && !validPathRegex.MatchString(webhook.Path) {
		h.handler.sendError(w, http.StatusBadRequest, "path must only contain alphanumeric characters, dots, hyphens, and underscores")
		return
	}

	webhook.ID = id
	if err := h.repo.UpdateWebhook(r.Context(), &webhook); err != nil {
		h.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	h.handler.sendJSON(w, webhook)
}

func (h *WebhookHandler) Delete(w http.ResponseWriter, r *http.Request) {
	vars := mux.Vars(r)
	id := vars["id"]

	if err := h.repo.DeleteWebhook(r.Context(), id); err != nil {
		h.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	w.WriteHeader(http.StatusNoContent)
}

func (h *WebhookHandler) DeleteAll(w http.ResponseWriter, r *http.Request) {
	if err := h.repo.DeleteAllWebhooks(r.Context()); err != nil {
		h.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	w.WriteHeader(http.StatusNoContent)
}

func (h *WebhookHandler) Test(w http.ResponseWriter, r *http.Request) {
	var req struct {
		Path   string `json:"path"`
		Script string `json:"script"`
		Method string `json:"method"`
		Body   string `json:"body"`
	}
	if err := h.handler.readJSON(r, &req); err != nil {
		h.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}

	if req.Method == "" {
		req.Method = "POST"
	}

	mockReq, _ := http.NewRequest(req.Method, "/webhook/"+req.Path, strings.NewReader(req.Body))
	mockReq.Header.Set("Content-Type", "application/json")

	webhook := &entity.Webhook{
		Path:  req.Path,
		Script: req.Script,
	}

	status, result := h.luaService.ExecuteWebhook(r.Context(), webhook, mockReq)
	h.handler.sendJSONWithStatus(w, status, result)
}

func (h *WebhookHandler) ExecuteWebhook(w http.ResponseWriter, r *http.Request) {
	vars := mux.Vars(r)
	path := vars["path"]

	webhook, err := h.repo.GetWebhookByPath(r.Context(), path)
	if err != nil {
		h.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	if webhook == nil {
		h.handler.sendJSONWithStatus(w, http.StatusNotFound, map[string]string{"error": "webhook not found"})
		return
	}

	if !webhook.IsActive {
		h.handler.sendJSONWithStatus(w, http.StatusGone, map[string]string{"error": "webhook is disabled"})
		return
	}

	// Validate secret if set
	if webhook.Secret != "" {
		secret := r.Header.Get("X-Webhook-Secret")
		if secret == "" {
			secret = r.URL.Query().Get("secret")
		}
		if secret != webhook.Secret {
			h.handler.sendJSONWithStatus(w, http.StatusUnauthorized, map[string]string{"error": "invalid secret"})
			return
		}
	}

	// Read body bytes for logging and execution
	bodyBytes, _ := io.ReadAll(r.Body)

	// Handle form-encoded payload (e.g., GitHub Actions webhooks)
	if strings.HasPrefix(r.Header.Get("Content-Type"), "application/x-www-form-urlencoded") {
		form, err := url.ParseQuery(string(bodyBytes))
		if err == nil {
			if payload := form.Get("payload"); payload != "" {
				bodyBytes = []byte(payload)
			}
		}
	}
	r.Body = io.NopCloser(bytes.NewReader(bodyBytes))

	status, result := h.luaService.ExecuteWebhook(r.Context(), webhook, r)

	// Log the request
	headersMap := make(map[string]string)
	for k, vals := range r.Header {
		if len(vals) > 0 {
			headersMap[k] = vals[0]
		}
	}
	headersJSON, _ := json.Marshal(headersMap)

	sourceIP := r.RemoteAddr
	if idx := strings.LastIndex(sourceIP, ":"); idx != -1 {
		sourceIP = sourceIP[:idx]
	}

	log := &entity.WebhookLog{
		ID:          uuid.New().String(),
		WebhookID:   webhook.ID,
		WebhookPath: webhook.Path,
		SourceIP:    sourceIP,
		Method:      r.Method,
		Headers:     string(headersJSON),
		Body:        string(bodyBytes),
		QueryParams: r.URL.RawQuery,
		StatusCode:  status,
		CreatedAt:   time.Now().Unix(),
	}
	_ = h.webhookLogRepo.CreateWebhookLog(r.Context(), log)

	h.handler.sendJSONWithStatus(w, status, result)
}

func (h *WebhookHandler) GetLogs(w http.ResponseWriter, r *http.Request) {
	webhookID := r.URL.Query().Get("webhook_id")
	limit := 50
	offset := 0
	if l, err := strconv.Atoi(r.URL.Query().Get("limit")); err == nil && l > 0 {
		limit = l
	}
	if o, err := strconv.Atoi(r.URL.Query().Get("offset")); err == nil && o >= 0 {
		offset = o
	}

	logs, err := h.webhookLogRepo.GetAllWebhookLogs(r.Context(), webhookID, limit, offset)
	if err != nil {
		h.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	total, _ := h.webhookLogRepo.GetWebhookLogCount(r.Context(), webhookID)

	h.handler.sendJSON(w, map[string]interface{}{
		"logs":   logs,
		"total":  total,
		"limit":  limit,
		"offset": offset,
	})
}

func (h *WebhookHandler) DeleteAllLogs(w http.ResponseWriter, r *http.Request) {
	if err := h.webhookLogRepo.DeleteAllWebhookLogs(r.Context()); err != nil {
		h.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}
	w.WriteHeader(http.StatusNoContent)
}

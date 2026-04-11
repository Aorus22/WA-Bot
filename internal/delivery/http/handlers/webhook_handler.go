package handlers

import (
	"bytes"
	"io"
	"net/http"
	"regexp"
	"strings"

	"github.com/gorilla/mux"
	"github.com/google/uuid"

	"wa-bot/internal/domain/entity"
	"wa-bot/internal/domain/repository"
	"wa-bot/internal/infrastructure/lua"
)

type WebhookHandler struct {
	handler    *Handler
	repo       repository.WebhookRepository
	luaService *lua.LuaService
}

func NewWebhookHandler(handler *Handler, repo repository.WebhookRepository, luaService *lua.LuaService) *WebhookHandler {
	return &WebhookHandler{
		handler:    handler,
		repo:       repo,
		luaService: luaService,
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

	// Buffer body so it can be read by ExecuteWebhook
	var bodyBuf bytes.Buffer
	tee := io.TeeReader(r.Body, &bodyBuf)
	r.Body = io.NopCloser(tee)

	status, result := h.luaService.ExecuteWebhook(r.Context(), webhook, r)
	h.handler.sendJSONWithStatus(w, status, result)
}

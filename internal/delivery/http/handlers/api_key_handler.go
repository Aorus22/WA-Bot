package handlers

import (
	"crypto/rand"
	"encoding/base64"
	"encoding/hex"
	"fmt"
	"net/http"
	"time"

	"wa-bot/internal/delivery/http/middleware"
	"wa-bot/internal/domain/entity"
	"wa-bot/internal/domain/repository"
)

// maxAPIKeyBodyBytes bounds the size of the api-keys request body.
const maxAPIKeyBodyBytes = 64 << 10 // 64KB

// APIKeyHandler manages internal API keys (PRD §40).
type APIKeyHandler struct {
	handler *Handler
	repo    repository.APIKeyRepository
}

// NewAPIKeyHandler builds the API key handler from the shared container.
func NewAPIKeyHandler(h *Handler) *APIKeyHandler {
	return &APIKeyHandler{
		handler: h,
		repo:    h.GetAPIKeyRepo(),
	}
}

// List returns all API keys. The full key and its hash are never exposed.
func (kh *APIKeyHandler) List(w http.ResponseWriter, r *http.Request) {
	keys, err := kh.repo.GetAPIKeys(r.Context())
	if err != nil {
		kh.handler.sendError(w, http.StatusInternalServerError, "failed_to_list_api_keys")
		return
	}

	type keyView struct {
		ID         string   `json:"id"`
		Name       string   `json:"name"`
		Prefix     string   `json:"prefix"`
		Scopes     []string `json:"scopes"`
		IsActive   bool     `json:"is_active"`
		CreatedAt  int64    `json:"created_at"`
		LastUsedAt *int64   `json:"last_used_at,omitempty"`
		RevokedAt  *int64   `json:"revoked_at,omitempty"`
	}

	out := make([]keyView, 0, len(keys))
	for _, k := range keys {
		out = append(out, keyView{
			ID:         k.ID,
			Name:       k.Name,
			Prefix:     k.Prefix,
			Scopes:     k.Scopes,
			IsActive:   k.IsActive,
			CreatedAt:  k.CreatedAt,
			LastUsedAt: k.LastUsedAt,
			RevokedAt:  k.RevokedAt,
		})
	}
	kh.handler.sendJSON(w, map[string]interface{}{"api_keys": out})
}

// Create generates a new API key. The full key is returned only once.
func (kh *APIKeyHandler) Create(w http.ResponseWriter, r *http.Request) {
	r.Body = http.MaxBytesReader(w, r.Body, maxAPIKeyBodyBytes)
	var req entity.CreateAPIKeyRequest
	if err := kh.handler.readJSON(r, &req); err != nil {
		kh.handler.sendError(w, http.StatusBadRequest, "invalid request body")
		return
	}
	if req.Name == "" {
		kh.handler.sendError(w, http.StatusBadRequest, "name_required")
		return
	}
	scopes := req.Scopes
	if len(scopes) == 0 {
		scopes = []string{"calls:read", "calls:write"}
	}

	token, err := generateAPIKey()
	if err != nil {
		kh.handler.sendError(w, http.StatusInternalServerError, "failed_to_generate_key")
		return
	}
	keyID, err := generateAPIKeyID()
	if err != nil {
		kh.handler.sendError(w, http.StatusInternalServerError, "failed_to_generate_key")
		return
	}

	now := time.Now().UnixMilli()
	apiKey := &entity.APIKey{
		ID:        keyID,
		Name:      req.Name,
		Prefix:    token[:len("wab_")+4], // e.g. "wab_abcd"
		KeyHash:   middleware.HashAPIKey(token),
		Scopes:    scopes,
		IsActive:  true,
		CreatedAt: now,
	}
	if err := kh.repo.CreateAPIKey(r.Context(), apiKey); err != nil {
		kh.handler.sendError(w, http.StatusInternalServerError, "failed_to_create_api_key")
		return
	}

	kh.handler.sendJSONWithStatus(w, http.StatusCreated, entity.CreateAPIKeyResponse{
		ID:     apiKey.ID,
		Name:   apiKey.Name,
		Prefix: apiKey.Prefix,
		Key:    token,
	})
}

// Revoke deactivates an API key.
func (kh *APIKeyHandler) Revoke(w http.ResponseWriter, r *http.Request) {
	id := kh.handler.getJID(r, "id")
	if err := kh.repo.RevokeAPIKey(r.Context(), id, time.Now().UnixMilli()); err != nil {
		kh.handler.sendError(w, http.StatusInternalServerError, "failed_to_revoke_api_key")
		return
	}
	kh.handler.sendJSON(w, map[string]string{"status": "revoked"})
}

// Delete removes an API key permanently.
func (kh *APIKeyHandler) Delete(w http.ResponseWriter, r *http.Request) {
	id := kh.handler.getJID(r, "id")
	if err := kh.repo.DeleteAPIKey(r.Context(), id); err != nil {
		kh.handler.sendError(w, http.StatusInternalServerError, "failed_to_delete_api_key")
		return
	}
	kh.handler.sendJSON(w, map[string]string{"status": "deleted"})
}

// generateAPIKey creates a "wab_" prefixed key with >= 32 bytes of entropy.
func generateAPIKey() (string, error) {
	b := make([]byte, 32)
	if _, err := rand.Read(b); err != nil {
		return "", err
	}
	return "wab_" + base64.RawURLEncoding.EncodeToString(b), nil
}

// generateAPIKeyID builds a "key_" prefixed ID with a random 8-byte suffix so
// that two keys created in the same millisecond cannot collide on the primary key.
func generateAPIKeyID() (string, error) {
	b := make([]byte, 8)
	if _, err := rand.Read(b); err != nil {
		return "", err
	}
	return fmt.Sprintf("key_%d_%s", time.Now().UnixMilli(), hex.EncodeToString(b)), nil
}

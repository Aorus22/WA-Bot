package middleware

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"net/http"
	"strings"
	"time"

	"wa-bot/internal/domain/entity"
	"wa-bot/internal/domain/repository"
)

type apiKeyCtxKey struct{}

// HashAPIKey returns the SHA-256 hex digest of a key token. API keys are stored
// and matched only by this digest; the plaintext key is never persisted.
func HashAPIKey(token string) string {
	sum := sha256.Sum256([]byte(token))
	return hex.EncodeToString(sum[:])
}

// APIKeyMiddleware authenticates requests with a bearer API key and enforces
// scopes. It is built from the APIKeyRepository.
type APIKeyMiddleware struct {
	repo repository.APIKeyRepository
}

// NewAPIKeyMiddleware builds an API key auth middleware.
func NewAPIKeyMiddleware(repo repository.APIKeyRepository) *APIKeyMiddleware {
	return &APIKeyMiddleware{repo: repo}
}

// RequireScope wraps an HTTP handler with bearer-token authentication and a scope
// check. OPTIONS preflight requests pass through unauthenticated.
func (m *APIKeyMiddleware) RequireScope(scope string, next http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		if r.Method == http.MethodOptions {
			next(w, r)
			return
		}
		key, ok := m.authenticate(w, r)
		if !ok {
			return
		}
		if !hasScope(key.Scopes, scope) {
			writeAPIError(w, http.StatusForbidden, "api_scope_denied")
			return
		}
		ctx := context.WithValue(r.Context(), apiKeyCtxKey{}, key)
		next(w, r.WithContext(ctx))
	}
}

// authenticate validates the bearer token, checks activity/revocation, updates
// last_used_at, and returns the matched API key.
func (m *APIKeyMiddleware) authenticate(w http.ResponseWriter, r *http.Request) (*entity.APIKey, bool) {
	token, ok := bearerToken(r)
	if !ok {
		writeAPIError(w, http.StatusUnauthorized, "api_key_invalid")
		return nil, false
	}

	key, err := m.repo.FindAPIKeyByHash(r.Context(), HashAPIKey(token))
	if err != nil {
		writeAPIError(w, http.StatusInternalServerError, "internal_error")
		return nil, false
	}
	if key == nil || !key.IsActive {
		writeAPIError(w, http.StatusUnauthorized, "api_key_invalid")
		return nil, false
	}
	if key.RevokedAt != nil {
		writeAPIError(w, http.StatusUnauthorized, "api_key_revoked")
		return nil, false
	}

	// Touch last_used_at (best-effort; a failure should not block the request).
	now := time.Now().UnixMilli()
	_ = m.repo.TouchAPIKey(r.Context(), key.ID, now)
	return key, true
}

// bearerToken extracts the token from an "Authorization: Bearer <token>" header.
func bearerToken(r *http.Request) (string, bool) {
	auth := r.Header.Get("Authorization")
	parts := strings.SplitN(auth, " ", 2)
	if len(parts) != 2 || !strings.EqualFold(parts[0], "Bearer") || parts[1] == "" {
		return "", false
	}
	return parts[1], true
}

// hasScope reports whether scopes contains the required scope.
func hasScope(scopes []string, required string) bool {
	for _, s := range scopes {
		if s == required {
			return true
		}
	}
	return false
}

// APIKeyFromContext returns the authenticated API key stored in the request
// context (nil, false if the request was not authenticated via the middleware).
func APIKeyFromContext(ctx context.Context) (*entity.APIKey, bool) {
	key, ok := ctx.Value(apiKeyCtxKey{}).(*entity.APIKey)
	return key, ok
}

// APIKeyIDFromContext returns the authenticated API key ID, or "" if absent.
func APIKeyIDFromContext(ctx context.Context) string {
	if key, ok := APIKeyFromContext(ctx); ok && key != nil {
		return key.ID
	}
	return ""
}

// HasScopeFromContext reports whether the authenticated API key holds the given
// scope. Returns false when no key is present in the context.
func HasScopeFromContext(ctx context.Context, scope string) bool {
	key, ok := APIKeyFromContext(ctx)
	if !ok || key == nil {
		return false
	}
	return hasScope(key.Scopes, scope)
}

// writeAPIError writes a JSON error response.
func writeAPIError(w http.ResponseWriter, code int, msg string) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(code)
	_, _ = w.Write([]byte(`{"error":"` + msg + `"}`))
}

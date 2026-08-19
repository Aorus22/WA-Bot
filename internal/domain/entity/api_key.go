package entity

// APIKey is a persisted API key used to authenticate external API callers.
type APIKey struct {
	ID         string   `json:"id"`
	Name       string   `json:"name"`
	Prefix     string   `json:"prefix"`
	KeyHash    string   `json:"key_hash"`
	Scopes     []string `json:"scopes"`
	IsActive   bool     `json:"is_active"`
	CreatedAt  int64    `json:"created_at"`
	LastUsedAt *int64   `json:"last_used_at,omitempty"`
	RevokedAt  *int64   `json:"revoked_at,omitempty"`
}

// CreateAPIKeyRequest is the body for creating an API key.
type CreateAPIKeyRequest struct {
	Name   string   `json:"name"`
	Scopes []string `json:"scopes"`
}

// CreateAPIKeyResponse is returned once after creating an API key. The full
// key is only visible at creation time and is not stored.
type CreateAPIKeyResponse struct {
	ID     string `json:"id"`
	Name   string `json:"name"`
	Prefix string `json:"prefix"`
	Key    string `json:"key"`
}

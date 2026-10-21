package ai

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
	"sync"
	"time"

	"wa-bot/internal/domain/repository"
)

// Providers the text assistant can talk to (setting "ai_provider").
const (
	ProviderGemini = "gemini"
	ProviderOpenAI = "openai"
	ProviderClaude = "claude"
)

const (
	defaultOpenAIBase = "https://api.openai.com/v1"
	defaultClaudeBase = "https://api.anthropic.com"
	claudeAPIVersion  = "2023-06-01"
	claudeMaxTokens   = 4096
)

// TextRouter answers text prompts with whichever provider the settings name.
// Settings are read on every call, so changes apply without a restart.
type TextRouter struct {
	settings repository.SettingsRepository
	storage  repository.StorageRepository
	http     *http.Client

	mu     sync.Mutex
	gemini *GeminiService
	key    string
}

func NewTextRouter(settings repository.SettingsRepository, storage repository.StorageRepository) *TextRouter {
	return &TextRouter{settings: settings, storage: storage, http: &http.Client{Timeout: 120 * time.Second}}
}

func (r *TextRouter) get(ctx context.Context, key string) string {
	v, _ := r.settings.Get(ctx, key)
	return strings.TrimSpace(v)
}

// GenerateText sends prompt to the configured provider. A model set in the
// settings wins over modelName, which scripts pass for Gemini.
func (r *TextRouter) GenerateText(ctx context.Context, modelName, prompt string) (string, error) {
	provider := r.get(ctx, "ai_provider")
	if provider == "" {
		provider = ProviderGemini
	}
	model := r.get(ctx, "ai_model")
	switch provider {
	case ProviderGemini:
		key := r.get(ctx, "gemini_api_key")
		if key == "" {
			return "", fmt.Errorf("Gemini API key is not set")
		}
		g, err := r.geminiFor(key)
		if err != nil {
			return "", err
		}
		if model != "" {
			modelName = model
		}
		return g.GenerateText(ctx, modelName, prompt)
	case ProviderOpenAI:
		return r.openAI(ctx, model, prompt)
	case ProviderClaude:
		return r.claude(ctx, model, prompt)
	}
	return "", fmt.Errorf("unknown AI provider %q", provider)
}

func (r *TextRouter) geminiFor(key string) (*GeminiService, error) {
	r.mu.Lock()
	defer r.mu.Unlock()
	if r.gemini != nil && r.key == key {
		return r.gemini, nil
	}
	g, err := NewGeminiService(key, r.storage)
	if err != nil {
		return nil, err
	}
	if r.gemini != nil {
		r.gemini.Close()
	}
	r.gemini, r.key = g, key
	return g, nil
}

func (r *TextRouter) endpoint(ctx context.Context, fallback, path string) string {
	base := strings.TrimRight(r.get(ctx, "ai_base_url"), "/")
	if base == "" {
		base = fallback
	}
	// Accept a base that already ends in the API prefix.
	if strings.HasSuffix(base, "/v1") && strings.HasPrefix(path, "/v1/") {
		path = strings.TrimPrefix(path, "/v1")
	}
	return base + path
}

func (r *TextRouter) openAI(ctx context.Context, model, prompt string) (string, error) {
	key := r.get(ctx, "ai_api_key")
	if model == "" {
		return "", fmt.Errorf("set a model for the OpenAI-compatible provider")
	}
	body, _ := json.Marshal(map[string]interface{}{
		"model":    model,
		"messages": []map[string]string{{"role": "user", "content": prompt}},
	})
	url := r.endpoint(ctx, defaultOpenAIBase, "/chat/completions")
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, url, bytes.NewReader(body))
	if err != nil {
		return "", err
	}
	req.Header.Set("Content-Type", "application/json")
	if key != "" {
		req.Header.Set("Authorization", "Bearer "+key)
	}
	var out struct {
		Choices []struct {
			Message struct {
				Content string `json:"content"`
			} `json:"message"`
		} `json:"choices"`
	}
	if err := r.do(req, &out); err != nil {
		return "", err
	}
	if len(out.Choices) == 0 {
		return "", fmt.Errorf("the provider returned no answer")
	}
	return out.Choices[0].Message.Content, nil
}

func (r *TextRouter) claude(ctx context.Context, model, prompt string) (string, error) {
	key := r.get(ctx, "ai_api_key")
	if model == "" {
		return "", fmt.Errorf("set a model for the Claude-compatible provider")
	}
	body, _ := json.Marshal(map[string]interface{}{
		"model":      model,
		"max_tokens": claudeMaxTokens,
		"messages":   []map[string]string{{"role": "user", "content": prompt}},
	})
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, r.endpoint(ctx, defaultClaudeBase, "/v1/messages"), bytes.NewReader(body))
	if err != nil {
		return "", err
	}
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("anthropic-version", claudeAPIVersion)
	if key != "" {
		req.Header.Set("x-api-key", key)
	}
	var out struct {
		Content []struct {
			Type string `json:"type"`
			Text string `json:"text"`
		} `json:"content"`
	}
	if err := r.do(req, &out); err != nil {
		return "", err
	}
	var text strings.Builder
	for _, c := range out.Content {
		if c.Type == "text" {
			text.WriteString(c.Text)
		}
	}
	if text.Len() == 0 {
		return "", fmt.Errorf("the provider returned no answer")
	}
	return text.String(), nil
}

func (r *TextRouter) do(req *http.Request, out interface{}) error {
	resp, err := r.http.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	data, err := io.ReadAll(io.LimitReader(resp.Body, 4<<20))
	if err != nil {
		return err
	}
	if resp.StatusCode >= 300 {
		msg := strings.TrimSpace(string(data))
		if len(msg) > 300 {
			msg = msg[:300]
		}
		return fmt.Errorf("the provider answered %d: %s", resp.StatusCode, msg)
	}
	return json.Unmarshal(data, out)
}

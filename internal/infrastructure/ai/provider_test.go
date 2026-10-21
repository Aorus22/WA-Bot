package ai

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
)

type fakeSettings map[string]string

func (f fakeSettings) Get(_ context.Context, k string) (string, error) { return f[k], nil }
func (f fakeSettings) Set(_ context.Context, k, v string) error        { f[k] = v; return nil }
func (f fakeSettings) List(context.Context) (map[string]string, error) { return f, nil }
func (f fakeSettings) Delete(_ context.Context, k string) error        { delete(f, k); return nil }

func TestOpenAICompatible(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/v1/chat/completions" || r.Header.Get("Authorization") != "Bearer k" {
			t.Errorf("unexpected request %s %q", r.URL.Path, r.Header.Get("Authorization"))
		}
		var body struct{ Model string }
		_ = json.NewDecoder(r.Body).Decode(&body)
		if body.Model != "m1" {
			t.Errorf("model = %q", body.Model)
		}
		_, _ = w.Write([]byte(`{"choices":[{"message":{"content":"hi"}}]}`))
	}))
	defer srv.Close()
	r := NewTextRouter(fakeSettings{"ai_provider": "openai", "ai_base_url": srv.URL + "/v1/", "ai_api_key": "k", "ai_model": "m1"}, nil)
	got, err := r.GenerateText(context.Background(), "ignored", "yo")
	if err != nil || got != "hi" {
		t.Fatalf("got %q, %v", got, err)
	}
}

func TestClaudeCompatible(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/v1/messages" || r.Header.Get("x-api-key") != "k" || r.Header.Get("anthropic-version") == "" {
			t.Errorf("unexpected request %s", r.URL.Path)
		}
		_, _ = w.Write([]byte(`{"content":[{"type":"text","text":"a"},{"type":"text","text":"b"}]}`))
	}))
	defer srv.Close()
	r := NewTextRouter(fakeSettings{"ai_provider": "claude", "ai_base_url": srv.URL, "ai_api_key": "k", "ai_model": "m"}, nil)
	got, err := r.GenerateText(context.Background(), "", "yo")
	if err != nil || got != "ab" {
		t.Fatalf("got %q, %v", got, err)
	}
}

func TestProviderNeedsModelAndKnownName(t *testing.T) {
	r := NewTextRouter(fakeSettings{"ai_provider": "openai"}, nil)
	if _, err := r.GenerateText(context.Background(), "", "x"); err == nil {
		t.Fatal("a missing model should fail")
	}
	r = NewTextRouter(fakeSettings{"ai_provider": "nope"}, nil)
	if _, err := r.GenerateText(context.Background(), "", "x"); err == nil {
		t.Fatal("an unknown provider should fail")
	}
}

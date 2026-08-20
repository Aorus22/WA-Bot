package call

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"time"
)

const (
	// FishAudioAPIBase is the Fish Audio text-to-speech endpoint.
	FishAudioAPIBase = "https://api.fish.audio/v1/tts"
	// FishAudioTimeout bounds a single synthesis request.
	FishAudioTimeout = 30 * time.Second
)

var (
	// ErrTTSFailed indicates the TTS provider could not synthesize speech.
	ErrTTSFailed = errors.New("tts_failed")
)

// FishAudioConfig configures the Fish Audio TTS provider.
type FishAudioConfig struct {
	APIKey         string
	DefaultModel   string
	DefaultVoiceID string
}

// FishAudioTTSProvider synthesizes speech via the Fish Audio API (api.fish.audio).
type FishAudioTTSProvider struct {
	apiKey         string
	defaultModel   string
	defaultVoiceID string
	httpClient     httpDoer
}

// httpDoer is the minimal HTTP client contract used by FishAudioTTSProvider so
// tests can inject a fake transport.
type httpDoer interface {
	Do(req *http.Request) (*http.Response, error)
}

// NewFishAudioTTSProvider builds a Fish Audio TTS provider. The model defaults
// to the free developer tier s2.1-pro-free when empty.
func NewFishAudioTTSProvider(cfg FishAudioConfig) *FishAudioTTSProvider {
	if cfg.DefaultModel == "" {
		cfg.DefaultModel = "s2.1-pro-free"
	}
	return &FishAudioTTSProvider{
		apiKey:         cfg.APIKey,
		defaultModel:   cfg.DefaultModel,
		defaultVoiceID: cfg.DefaultVoiceID,
		httpClient:     &http.Client{Timeout: FishAudioTimeout},
	}
}

// DefaultVoice returns the configured default voice library ID.
func (p *FishAudioTTSProvider) DefaultVoice() string {
	return p.defaultVoiceID
}

// fishAudioError is the structured error body returned by the Fish Audio API.
type fishAudioError struct {
	Status  int    `json:"status"`
	Message string `json:"message"`
	Reason  string `json:"reason"`
}

func (e *fishAudioError) Error() string {
	if e.Reason != "" {
		return fmt.Sprintf("fish audio tts failed: %s (%s)", e.Message, e.Reason)
	}
	return fmt.Sprintf("fish audio tts failed: %s", e.Message)
}

// Synthesize renders req.Text into an MP3 temp file via the Fish Audio API and
// returns an AudioResult whose Cleanup removes the file after playback. The
// voice is selected by req.ReferenceID, falling back to the provider default.
func (p *FishAudioTTSProvider) Synthesize(ctx context.Context, req TTSRequest) (*AudioResult, error) {
	if p.apiKey == "" {
		return nil, fmt.Errorf("%w: fish audio api key is not configured", ErrTTSFailed)
	}
	referenceID := req.ReferenceID
	if referenceID == "" {
		referenceID = p.defaultVoiceID
	}

	// The model is selected via a header, not the body (defaults to s2.1-pro).
	bodyMap := map[string]interface{}{
		"text":        req.Text,
		"format":      "mp3",
		"mp3_bitrate": 128,
	}
	// Omit reference_id entirely when empty so Fish picks a built-in voice.
	if referenceID != "" {
		bodyMap["reference_id"] = referenceID
	}
	body, err := json.Marshal(bodyMap)
	if err != nil {
		return nil, fmt.Errorf("%w: %v", ErrTTSFailed, err)
	}

	httpReq, err := http.NewRequestWithContext(ctx, http.MethodPost, FishAudioAPIBase, bytes.NewReader(body))
	if err != nil {
		return nil, fmt.Errorf("%w: %v", ErrTTSFailed, err)
	}
	httpReq.Header.Set("Content-Type", "application/json")
	httpReq.Header.Set("Authorization", "Bearer "+p.apiKey)
	httpReq.Header.Set("model", p.defaultModel)

	resp, err := p.httpClient.Do(httpReq)
	if err != nil {
		return nil, fmt.Errorf("%w: %v", ErrTTSFailed, err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, p.decodeError(resp)
	}

	tmp, err := os.CreateTemp("", "wa-tts-*.mp3")
	if err != nil {
		return nil, fmt.Errorf("%w: %v", ErrTTSFailed, err)
	}
	path := tmp.Name()
	if _, err := io.Copy(tmp, resp.Body); err != nil {
		tmp.Close()
		_ = os.Remove(path)
		return nil, fmt.Errorf("%w: %v", ErrTTSFailed, err)
	}
	if err := tmp.Close(); err != nil {
		_ = os.Remove(path)
		return nil, fmt.Errorf("%w: %v", ErrTTSFailed, err)
	}

	return &AudioResult{
		Path:   path,
		Format: "mp3",
		Cleanup: func() {
			_ = os.Remove(path)
		},
	}, nil
}

// decodeError parses the Fish Audio JSON error body into a wrapped ErrTTSFailed.
func (p *FishAudioTTSProvider) decodeError(resp *http.Response) error {
	var apiErr fishAudioError
	if err := json.NewDecoder(io.LimitReader(resp.Body, 1<<20)).Decode(&apiErr); err != nil {
		return fmt.Errorf("%w: fish audio returned status %d", ErrTTSFailed, resp.StatusCode)
	}
	if apiErr.Message == "" {
		apiErr.Message = fmt.Sprintf("fish audio returned status %d", resp.StatusCode)
	}
	return fmt.Errorf("%w: %v", ErrTTSFailed, &apiErr)
}

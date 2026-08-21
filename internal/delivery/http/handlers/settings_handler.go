package handlers

import (
	"encoding/json"
	"net/http"

	"wa-bot/internal/domain/repository"
)

// SettingsHandler manages DB-backed application settings (TTS + AI config).
type SettingsHandler struct {
	handler      *Handler
	settingsRepo repository.SettingsRepository
}

// managedSettingsKeys are the only keys the settings API may read/write.
var managedSettingsKeys = []string{
	"gemini_api_key",
	"ai_server_url",
	"call_tts_provider",
	"call_tts_default_voice",
	"call_tts_fish_audio_key",
	"call_tts_fish_audio_model",
	"call_tts_fish_audio_voice_id",
}

// secretSettingsKeys are masked in API responses so raw secrets never leak.
var secretSettingsKeys = map[string]bool{
	"gemini_api_key":          true,
	"call_tts_fish_audio_key": true,
}

func NewSettingsHandler(h *Handler, repo repository.SettingsRepository) *SettingsHandler {
	return &SettingsHandler{handler: h, settingsRepo: repo}
}

// maskedSettings builds the API response: managed keys with secrets masked and
// hasGeminiKey/hasFishKey flags so the client knows whether a secret is set.
func (sh *SettingsHandler) maskedSettings(all map[string]string) map[string]interface{} {
	result := map[string]string{}
	hasGeminiKey := false
	hasFishKey := false
	for _, key := range managedSettingsKeys {
		val := all[key]
		if secretSettingsKeys[key] {
			if val != "" {
				result[key] = "••••••••"
				if key == "gemini_api_key" {
					hasGeminiKey = true
				} else {
					hasFishKey = true
				}
			} else {
				result[key] = ""
			}
		} else {
			result[key] = val
		}
	}
	return map[string]interface{}{
		"settings":     result,
		"hasGeminiKey": hasGeminiKey,
		"hasFishKey":   hasFishKey,
	}
}

func (sh *SettingsHandler) Get(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	if sh.settingsRepo == nil {
		sh.handler.sendError(w, http.StatusInternalServerError, "Settings repository not configured")
		return
	}

	all, err := sh.settingsRepo.List(r.Context())
	if err != nil {
		sh.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	sh.handler.sendJSON(w, sh.maskedSettings(all))
}

func (sh *SettingsHandler) Update(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	if sh.settingsRepo == nil {
		sh.handler.sendError(w, http.StatusInternalServerError, "Settings repository not configured")
		return
	}

	var updates map[string]string
	if err := json.NewDecoder(r.Body).Decode(&updates); err != nil {
		sh.handler.sendError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}

	allowed := make(map[string]bool, len(managedSettingsKeys))
	for _, k := range managedSettingsKeys {
		allowed[k] = true
	}

	for key := range updates {
		if !allowed[key] {
			sh.handler.sendError(w, http.StatusBadRequest, "unknown setting key: "+key)
			return
		}
	}

	if provider, ok := updates["call_tts_provider"]; ok {
		switch provider {
		case "", "fish", "edge":
		default:
			sh.handler.sendError(w, http.StatusBadRequest, "call_tts_provider must be one of: '', 'fish', 'edge'")
			return
		}
	}

	for key, value := range updates {
		if err := sh.settingsRepo.Set(r.Context(), key, value); err != nil {
			sh.handler.sendError(w, http.StatusInternalServerError, err.Error())
			return
		}
	}

	all, err := sh.settingsRepo.List(r.Context())
	if err != nil {
		sh.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	sh.handler.sendJSON(w, sh.maskedSettings(all))
}

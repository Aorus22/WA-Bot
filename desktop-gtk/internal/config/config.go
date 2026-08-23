// Package config persists small desktop-app preferences (currently just the
// selected theme) as JSON in the per-user data directory. Desktop-only by
// design: the backend's /api/settings is an AI/TTS whitelist, and the web UI
// keeps its own theme choice in localStorage too.
package config

import (
	"encoding/json"
	"os"
	"path/filepath"
)

// Config is the persisted desktop preference set.
type Config struct {
	Theme string `json:"theme"`
}

// Path returns <userDataDir>/settings.json.
func Path(userDataDir string) string {
	return filepath.Join(userDataDir, "settings.json")
}

// Load reads settings.json. A missing or invalid file yields an empty
// Config — callers substitute their own defaults (avoids an import cycle).
func Load(userDataDir string) Config {
	var cfg Config
	data, err := os.ReadFile(Path(userDataDir))
	if err != nil {
		return cfg
	}
	var parsed Config
	if err := json.Unmarshal(data, &parsed); err != nil {
		return cfg
	}
	return parsed
}

// Save writes settings.json atomically (temp file + rename).
func Save(userDataDir string, cfg Config) error {
	data, err := json.MarshalIndent(cfg, "", "  ")
	if err != nil {
		return err
	}
	dst := Path(userDataDir)
	tmp := dst + ".tmp"
	if err := os.WriteFile(tmp, data, 0o644); err != nil {
		return err
	}
	return os.Rename(tmp, dst)
}

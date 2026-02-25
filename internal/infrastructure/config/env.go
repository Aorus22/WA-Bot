package config

import (
	"os"
	"strconv"
)

type EnvConfig struct{}

func NewEnvConfig() *EnvConfig {
	return &EnvConfig{}
}

func (e *EnvConfig) Get(key string) string {
	return os.Getenv(key)
}

func (e *EnvConfig) GetInt(key string) int {
	val := os.Getenv(key)
	if val == "" {
		return 0
	}
	i, err := strconv.Atoi(val)
	if err != nil {
		return 0
	}
	return i
}

func (e *EnvConfig) GetBool(key string) bool {
	val := os.Getenv(key)
	return val == "true" || val == "1"
}

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

func (e *EnvConfig) GetRedisURL() string {
	val := os.Getenv("REDIS_URL")
	if val == "" {
		return "localhost:6379"
	}
	return val
}

func (e *EnvConfig) GetRedisPassword() string {
	return os.Getenv("REDIS_PASSWORD")
}

func (e *EnvConfig) GetRedisDB() int {
	return e.GetInt("REDIS_DB")
}

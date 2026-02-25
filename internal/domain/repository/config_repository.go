package repository

type ConfigRepository interface {
	Get(key string) string
	GetInt(key string) int
	GetBool(key string) bool
}

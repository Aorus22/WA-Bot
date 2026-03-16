package storage

import (
	"context"
	"time"

	"github.com/redis/go-redis/v9"
	"wa-bot/internal/infrastructure/config"
)

type RedisService struct {
	Client *redis.Client
}

func NewRedisService(cfg *config.EnvConfig) *RedisService {
	client := redis.NewClient(&redis.Options{
		Addr:     cfg.GetRedisURL(),
		Password: cfg.GetRedisPassword(),
		DB:       cfg.GetRedisDB(),
	})

	return &RedisService{
		Client: client,
	}
}

func (s *RedisService) Set(ctx context.Context, key string, value interface{}, expiration time.Duration) error {
	return s.Client.Set(ctx, key, value, expiration).Err()
}

func (s *RedisService) Get(ctx context.Context, key string) (string, error) {
	return s.Client.Get(ctx, key).Result()
}

func (s *RedisService) Del(ctx context.Context, key string) error {
	return s.Client.Del(ctx, key).Err()
}

func (s *RedisService) Exists(ctx context.Context, key string) (bool, error) {
	n, err := s.Client.Exists(ctx, key).Result()
	return n > 0, err
}

func (s *RedisService) HSet(ctx context.Context, key string, values ...interface{}) error {
	return s.Client.HSet(ctx, key, values...).Err()
}

func (s *RedisService) HGet(ctx context.Context, key, field string) (string, error) {
	return s.Client.HGet(ctx, key, field).Result()
}

func (s *RedisService) HDel(ctx context.Context, key string, fields ...string) error {
	return s.Client.HDel(ctx, key, fields...).Err()
}

func (s *RedisService) HGetAll(ctx context.Context, key string) (map[string]string, error) {
	return s.Client.HGetAll(ctx, key).Result()
}

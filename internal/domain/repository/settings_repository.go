package repository

import (
	"context"
	"fmt"
)

// SettingsRepository persists key/value application settings. Settings are the
// source of truth for TTS + AI configuration; env vars are only consulted once
// during the one-time seed migration, never at runtime.
type SettingsRepository interface {
	Get(ctx context.Context, key string) (string, error)
	Set(ctx context.Context, key, value string) error
	List(ctx context.Context) (map[string]string, error)
	Delete(ctx context.Context, key string) error
}

// AppSettingsRepository adapts AppStore's GetSetting/SetSetting/ListSettings/
// DeleteSetting methods to the SettingsRepository interface. A separate type is
// required because AppStore already owns a Delete(ctx, id) method for triggers.
type AppSettingsRepository struct {
	store *AppStore
}

func NewAppSettingsRepository(store *AppStore) *AppSettingsRepository {
	return &AppSettingsRepository{store: store}
}

func (r *AppSettingsRepository) Get(ctx context.Context, key string) (string, error) {
	return r.store.GetSetting(ctx, key)
}

func (r *AppSettingsRepository) Set(ctx context.Context, key, value string) error {
	return r.store.SetSetting(ctx, key, value)
}

func (r *AppSettingsRepository) List(ctx context.Context) (map[string]string, error) {
	return r.store.ListSettings(ctx)
}

func (r *AppSettingsRepository) Delete(ctx context.Context, key string) error {
	return r.store.DeleteSetting(ctx, key)
}

// SeedSettings inserts defaults for any missing settings keys. It is used for
// the one-time migration of legacy env vars into the DB-backed settings store.
// Keys whose default value is empty are skipped; existing values are never
// overwritten. Seeded keys are logged so operators can see the migration.
func SeedSettings(ctx context.Context, repo SettingsRepository, defaults map[string]string) error {
	for key, value := range defaults {
		if value == "" {
			continue
		}
		existing, err := repo.Get(ctx, key)
		if err != nil {
			return fmt.Errorf("failed to read setting %q: %w", key, err)
		}
		if existing != "" {
			continue
		}
		if err := repo.Set(ctx, key, value); err != nil {
			return fmt.Errorf("failed to seed setting %q: %w", key, err)
		}
		fmt.Printf("[SETTINGS] seeded %s from legacy env\n", key)
	}
	return nil
}

package repository

import (
	"context"
	"database/sql"
	"fmt"
	"sync"
	"time"
	"wa-bot/internal/domain/entity"

	_ "github.com/mattn/go-sqlite3"
)

type AppStore struct {
	db *sql.DB
	mu sync.RWMutex
}

func NewAppStore(dbPath string) (*AppStore, error) {
	db, err := sql.Open("sqlite3", dbPath)
	if err != nil {
		return nil, fmt.Errorf("failed to open app database: %w", err)
	}

	store := &AppStore{db: db}

	if err := store.init(); err != nil {
		return nil, fmt.Errorf("failed to initialize app database: %w", err)
	}

	return store, nil
}

func (s *AppStore) init() error {
	s.mu.Lock()
	defer s.mu.Unlock()

	queries := []string{
		`CREATE TABLE IF NOT EXISTS triggers (
			id TEXT PRIMARY KEY,
			name TEXT,
			pattern TEXT,
			script TEXT,
			priority INTEGER DEFAULT 0,
			is_active INTEGER DEFAULT 1,
			created_at INTEGER DEFAULT (strftime('%s', 'now')),
			updated_at INTEGER DEFAULT (strftime('%s', 'now'))
		)`,
		`CREATE TABLE IF NOT EXISTS cron_jobs (
			id TEXT PRIMARY KEY,
			name TEXT,
			schedule TEXT,
			script TEXT,
			is_active INTEGER DEFAULT 1,
			created_at INTEGER DEFAULT (strftime('%s', 'now')),
			updated_at INTEGER DEFAULT (strftime('%s', 'now'))
		)`,
			`CREATE TABLE IF NOT EXISTS webhooks (
				id TEXT PRIMARY KEY,
				name TEXT,
				path TEXT UNIQUE,
				script TEXT,
				secret TEXT DEFAULT '',
				is_active INTEGER DEFAULT 1,
				created_at INTEGER DEFAULT (strftime('%s', 'now')),
				updated_at INTEGER DEFAULT (strftime('%s', 'now'))
			)`,
	}

	for _, query := range queries {
		if _, err := s.db.Exec(query); err != nil {
			return fmt.Errorf("failed to create app table: %w", err)
		}
	}

	// Add priority column if it doesn't exist (for existing databases)
	_, _ = s.db.Exec("ALTER TABLE triggers ADD COLUMN priority INTEGER DEFAULT 0")

	return nil
}

// TriggerRepository implementation for AppStore
func (s *AppStore) GetAll(ctx context.Context) ([]*entity.Trigger, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	// Logic:
	// Tier 1: priority > 0 (sorted 1, 2, 3...)
	// Tier 2: priority = 0 (default)
	// Tier 3: priority < 0 (sorted -2, -1)
	query := `
		SELECT id, name, pattern, script, priority, is_active, created_at, updated_at 
		FROM triggers 
		ORDER BY 
			(CASE 
				WHEN priority > 0 THEN 1 
				WHEN priority = 0 THEN 2 
				ELSE 3 
			END) ASC, 
			priority ASC, 
			created_at DESC`
	rows, err := s.db.QueryContext(ctx, query)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var triggers []*entity.Trigger
	for rows.Next() {
		var t entity.Trigger
		var isActive int
		var createdAt, updatedAt int64
		if err := rows.Scan(&t.ID, &t.Name, &t.Pattern, &t.Script, &t.Priority, &isActive, &createdAt, &updatedAt); err != nil {
			return nil, err
		}
		t.IsActive = isActive == 1
		t.CreatedAt = time.Unix(createdAt, 0)
		t.UpdatedAt = time.Unix(updatedAt, 0)
		triggers = append(triggers, &t)
	}
	return triggers, nil
}

func (s *AppStore) GetByID(ctx context.Context, id string) (*entity.Trigger, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	query := `SELECT id, name, pattern, script, priority, is_active, created_at, updated_at FROM triggers WHERE id = ?`
	var t entity.Trigger
	var isActive int
	var createdAt, updatedAt int64
	err := s.db.QueryRowContext(ctx, query, id).Scan(&t.ID, &t.Name, &t.Pattern, &t.Script, &t.Priority, &isActive, &createdAt, &updatedAt)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	t.IsActive = isActive == 1
	t.CreatedAt = time.Unix(createdAt, 0)
	t.UpdatedAt = time.Unix(updatedAt, 0)
	return &t, nil
}

func (s *AppStore) Create(ctx context.Context, t *entity.Trigger) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	query := `INSERT INTO triggers (id, name, pattern, script, priority, is_active, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)`
	now := time.Now().Unix()
	_, err := s.db.ExecContext(ctx, query, t.ID, t.Name, t.Pattern, t.Script, t.Priority, func() int {
		if t.IsActive {
			return 1
		}
		return 0
	}(), now, now)
	return err
}

func (s *AppStore) Update(ctx context.Context, t *entity.Trigger) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	query := `UPDATE triggers SET name = ?, pattern = ?, script = ?, priority = ?, is_active = ?, updated_at = ? WHERE id = ?`
	_, err := s.db.ExecContext(ctx, query, t.Name, t.Pattern, t.Script, t.Priority, func() int {
		if t.IsActive {
			return 1
		}
		return 0
	}(), time.Now().Unix(), t.ID)
	return err
}

func (s *AppStore) Delete(ctx context.Context, id string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	query := `DELETE FROM triggers WHERE id = ?`
	_, err := s.db.ExecContext(ctx, query, id)
	return err
}

func (s *AppStore) DeleteCron(ctx context.Context, id string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	query := `DELETE FROM cron_jobs WHERE id = ?`
	_, err := s.db.ExecContext(ctx, query)
	return err
}

func (s *AppStore) DeleteAllCron(ctx context.Context) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	query := `DELETE FROM cron_jobs`
	_, err := s.db.ExecContext(ctx, query)
	return err
}

func (s *AppStore) DeleteAll(ctx context.Context) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	query := `DELETE FROM triggers`
	_, err := s.db.ExecContext(ctx, query)
	return err
}

// CronJobRepository implementation for AppStore
func (s *AppStore) GetAllCron(ctx context.Context) ([]*entity.CronJob, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	query := `SELECT id, name, schedule, script, is_active, created_at, updated_at FROM cron_jobs ORDER BY created_at DESC`
	rows, err := s.db.QueryContext(ctx, query)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var jobs []*entity.CronJob
	for rows.Next() {
		var j entity.CronJob
		var isActive int
		var createdAt, updatedAt int64
		if err := rows.Scan(&j.ID, &j.Name, &j.Schedule, &j.Script, &isActive, &createdAt, &updatedAt); err != nil {
			return nil, err
		}
		j.IsActive = isActive == 1
		j.CreatedAt = time.Unix(createdAt, 0)
		j.UpdatedAt = time.Unix(updatedAt, 0)
		jobs = append(jobs, &j)
	}
	return jobs, nil
}

func (s *AppStore) GetCronByID(ctx context.Context, id string) (*entity.CronJob, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	query := `SELECT id, name, schedule, script, is_active, created_at, updated_at FROM cron_jobs WHERE id = ?`
	var j entity.CronJob
	var isActive int
	var createdAt, updatedAt int64
	err := s.db.QueryRowContext(ctx, query, id).Scan(&j.ID, &j.Name, &j.Schedule, &j.Script, &isActive, &createdAt, &updatedAt)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	j.IsActive = isActive == 1
	j.CreatedAt = time.Unix(createdAt, 0)
	j.UpdatedAt = time.Unix(updatedAt, 0)
	return &j, nil
}

func (s *AppStore) CreateCron(ctx context.Context, j *entity.CronJob) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	query := `INSERT INTO cron_jobs (id, name, schedule, script, is_active, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)`
	now := time.Now().Unix()
	_, err := s.db.ExecContext(ctx, query, j.ID, j.Name, j.Schedule, j.Script, func() int {
		if j.IsActive {
			return 1
		}
		return 0
	}(), now, now)
	return err
}

func (s *AppStore) UpdateCron(ctx context.Context, j *entity.CronJob) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	query := `UPDATE cron_jobs SET name = ?, schedule = ?, script = ?, is_active = ?, updated_at = ? WHERE id = ?`
	_, err := s.db.ExecContext(ctx, query, j.Name, j.Schedule, j.Script, func() int {
		if j.IsActive {
			return 1
		}
		return 0
	}(), time.Now().Unix(), j.ID)
	return err
}

func (s *AppStore) Close() error {
	return s.db.Close()
}

// WebhookRepository implementation for AppStore
func (s *AppStore) GetAllWebhooks(ctx context.Context) ([]*entity.Webhook, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	query := `SELECT id, name, path, script, secret, is_active, created_at, updated_at FROM webhooks ORDER BY created_at DESC`
	rows, err := s.db.QueryContext(ctx, query)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var webhooks []*entity.Webhook
	for rows.Next() {
		var w entity.Webhook
		var isActive int
		var createdAt, updatedAt int64
		if err := rows.Scan(&w.ID, &w.Name, &w.Path, &w.Script, &w.Secret, &isActive, &createdAt, &updatedAt); err != nil {
			return nil, err
		}
		w.IsActive = isActive == 1
		w.CreatedAt = time.Unix(createdAt, 0)
		w.UpdatedAt = time.Unix(updatedAt, 0)
		webhooks = append(webhooks, &w)
	}
	return webhooks, nil
}

func (s *AppStore) GetWebhookByID(ctx context.Context, id string) (*entity.Webhook, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	query := `SELECT id, name, path, script, secret, is_active, created_at, updated_at FROM webhooks WHERE id = ?`
	var w entity.Webhook
	var isActive int
	var createdAt, updatedAt int64
	err := s.db.QueryRowContext(ctx, query, id).Scan(&w.ID, &w.Name, &w.Path, &w.Script, &w.Secret, &isActive, &createdAt, &updatedAt)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	w.IsActive = isActive == 1
	w.CreatedAt = time.Unix(createdAt, 0)
	w.UpdatedAt = time.Unix(updatedAt, 0)
	return &w, nil
}

func (s *AppStore) GetWebhookByPath(ctx context.Context, path string) (*entity.Webhook, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	query := `SELECT id, name, path, script, secret, is_active, created_at, updated_at FROM webhooks WHERE path = ?`
	var w entity.Webhook
	var isActive int
	var createdAt, updatedAt int64
	err := s.db.QueryRowContext(ctx, query, path).Scan(&w.ID, &w.Name, &w.Path, &w.Script, &w.Secret, &isActive, &createdAt, &updatedAt)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	w.IsActive = isActive == 1
	w.CreatedAt = time.Unix(createdAt, 0)
	w.UpdatedAt = time.Unix(updatedAt, 0)
	return &w, nil
}

func (s *AppStore) CreateWebhook(ctx context.Context, w *entity.Webhook) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	query := `INSERT INTO webhooks (id, name, path, script, secret, is_active, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)`
	now := time.Now().Unix()
	_, err := s.db.ExecContext(ctx, query, w.ID, w.Name, w.Path, w.Script, w.Secret, func() int {
		if w.IsActive {
			return 1
		}
		return 0
	}(), now, now)
	return err
}

func (s *AppStore) UpdateWebhook(ctx context.Context, w *entity.Webhook) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	query := `UPDATE webhooks SET name = ?, path = ?, script = ?, secret = ?, is_active = ?, updated_at = ? WHERE id = ?`
	_, err := s.db.ExecContext(ctx, query, w.Name, w.Path, w.Script, w.Secret, func() int {
		if w.IsActive {
			return 1
		}
		return 0
	}(), time.Now().Unix(), w.ID)
	return err
}

func (s *AppStore) DeleteWebhook(ctx context.Context, id string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	query := `DELETE FROM webhooks WHERE id = ?`
	_, err := s.db.ExecContext(ctx, query, id)
	return err
}

func (s *AppStore) DeleteAllWebhooks(ctx context.Context) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	query := `DELETE FROM webhooks`
	_, err := s.db.ExecContext(ctx, query)
	return err
}

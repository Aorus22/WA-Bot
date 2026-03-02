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

func (s *AppStore) DeleteAll(ctx context.Context) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	query := `DELETE FROM triggers`
	_, err := s.db.ExecContext(ctx, query)
	return err
}

func (s *AppStore) Close() error {
	return s.db.Close()
}

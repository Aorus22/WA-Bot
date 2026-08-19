package repository

import (
	"context"
	"database/sql"
	"encoding/json"
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
			description TEXT DEFAULT '',
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
			description TEXT DEFAULT '',
			is_active INTEGER DEFAULT 1,
			created_at INTEGER DEFAULT (strftime('%s', 'now')),
			updated_at INTEGER DEFAULT (strftime('%s', 'now'))
		)`,
		`CREATE TABLE IF NOT EXISTS webhooks (
				id TEXT PRIMARY KEY,
				name TEXT,
				path TEXT UNIQUE,
				script TEXT,
				description TEXT DEFAULT '',
				secret TEXT DEFAULT '',
				is_active INTEGER DEFAULT 1,
				created_at INTEGER DEFAULT (strftime('%s', 'now')),
				updated_at INTEGER DEFAULT (strftime('%s', 'now'))
			)`,
		`CREATE TABLE IF NOT EXISTS webhook_logs (
					id TEXT PRIMARY KEY,
					webhook_id TEXT,
					webhook_path TEXT,
					source_ip TEXT,
					method TEXT,
					headers TEXT,
					body TEXT,
					query_params TEXT,
					status_code INTEGER,
					created_at INTEGER DEFAULT (strftime('%s', 'now'))
				)`,
		`CREATE INDEX IF NOT EXISTS idx_webhook_logs_created_at ON webhook_logs(created_at DESC)`,
		`CREATE INDEX IF NOT EXISTS idx_webhook_logs_webhook_id ON webhook_logs(webhook_id)`,
		`CREATE TABLE IF NOT EXISTS api_keys (
				id TEXT PRIMARY KEY,
				name TEXT,
				key_prefix TEXT,
				key_hash TEXT UNIQUE,
				scopes TEXT DEFAULT '[]',
				is_active INTEGER DEFAULT 1,
				created_at INTEGER DEFAULT (strftime('%s', 'now')),
				last_used_at INTEGER,
				revoked_at INTEGER
			)`,
		`CREATE INDEX IF NOT EXISTS idx_api_keys_hash ON api_keys(key_hash)`,
		`CREATE TABLE IF NOT EXISTS call_logs (
				id TEXT PRIMARY KEY,
				meow_call_id TEXT DEFAULT '',
				direction TEXT,
				call_type TEXT,
				target TEXT,
				group_jid TEXT DEFAULT '',
				participants TEXT DEFAULT '[]',
				source TEXT,
				media_mode TEXT,
				status TEXT,
				error_message TEXT DEFAULT '',
				api_key_id TEXT DEFAULT '',
				started_at INTEGER,
				answered_at INTEGER,
				ended_at INTEGER,
				duration_ms INTEGER,
				created_at INTEGER DEFAULT (strftime('%s', 'now'))
			)`,
		`CREATE INDEX IF NOT EXISTS idx_call_logs_started ON call_logs(started_at DESC)`,
		`CREATE INDEX IF NOT EXISTS idx_call_logs_target ON call_logs(target)`,
		`CREATE INDEX IF NOT EXISTS idx_call_logs_status ON call_logs(status)`,
	}

	for _, query := range queries {
		if _, err := s.db.Exec(query); err != nil {
			return fmt.Errorf("failed to create app table: %w", err)
		}
	}

	// Add priority column if it doesn't exist (for existing databases)
	_, _ = s.db.Exec("ALTER TABLE triggers ADD COLUMN priority INTEGER DEFAULT 0")

	// Add description column if it doesn't exist (for existing databases)
	_, _ = s.db.Exec("ALTER TABLE triggers ADD COLUMN description TEXT DEFAULT ''")
	_, _ = s.db.Exec("ALTER TABLE cron_jobs ADD COLUMN description TEXT DEFAULT ''")
	_, _ = s.db.Exec("ALTER TABLE webhooks ADD COLUMN description TEXT DEFAULT ''")

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
		SELECT id, name, pattern, script, description, priority, is_active, created_at, updated_at 
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
		if err := rows.Scan(&t.ID, &t.Name, &t.Pattern, &t.Script, &t.Description, &t.Priority, &isActive, &createdAt, &updatedAt); err != nil {
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

	query := `SELECT id, name, pattern, script, description, priority, is_active, created_at, updated_at FROM triggers WHERE id = ?`
	var t entity.Trigger
	var isActive int
	var createdAt, updatedAt int64
	err := s.db.QueryRowContext(ctx, query, id).Scan(&t.ID, &t.Name, &t.Pattern, &t.Script, &t.Description, &t.Priority, &isActive, &createdAt, &updatedAt)
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

	query := `INSERT INTO triggers (id, name, pattern, script, description, priority, is_active, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`
	now := time.Now().Unix()
	_, err := s.db.ExecContext(ctx, query, t.ID, t.Name, t.Pattern, t.Script, t.Description, t.Priority, func() int {
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

	query := `UPDATE triggers SET name = ?, pattern = ?, script = ?, description = ?, priority = ?, is_active = ?, updated_at = ? WHERE id = ?`
	_, err := s.db.ExecContext(ctx, query, t.Name, t.Pattern, t.Script, t.Description, t.Priority, func() int {
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

	query := `SELECT id, name, schedule, script, description, is_active, created_at, updated_at FROM cron_jobs ORDER BY created_at DESC`
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
		if err := rows.Scan(&j.ID, &j.Name, &j.Schedule, &j.Script, &j.Description, &isActive, &createdAt, &updatedAt); err != nil {
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

	query := `SELECT id, name, schedule, script, description, is_active, created_at, updated_at FROM cron_jobs WHERE id = ?`
	var j entity.CronJob
	var isActive int
	var createdAt, updatedAt int64
	err := s.db.QueryRowContext(ctx, query, id).Scan(&j.ID, &j.Name, &j.Schedule, &j.Script, &j.Description, &isActive, &createdAt, &updatedAt)
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

	query := `INSERT INTO cron_jobs (id, name, schedule, script, description, is_active, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)`
	now := time.Now().Unix()
	_, err := s.db.ExecContext(ctx, query, j.ID, j.Name, j.Schedule, j.Script, j.Description, func() int {
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

	query := `UPDATE cron_jobs SET name = ?, schedule = ?, script = ?, description = ?, is_active = ?, updated_at = ? WHERE id = ?`
	_, err := s.db.ExecContext(ctx, query, j.Name, j.Schedule, j.Script, j.Description, func() int {
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

	query := `SELECT id, name, path, script, description, secret, is_active, created_at, updated_at FROM webhooks ORDER BY created_at DESC`
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
		if err := rows.Scan(&w.ID, &w.Name, &w.Path, &w.Script, &w.Description, &w.Secret, &isActive, &createdAt, &updatedAt); err != nil {
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

	query := `SELECT id, name, path, script, description, secret, is_active, created_at, updated_at FROM webhooks WHERE id = ?`
	var w entity.Webhook
	var isActive int
	var createdAt, updatedAt int64
	err := s.db.QueryRowContext(ctx, query, id).Scan(&w.ID, &w.Name, &w.Path, &w.Script, &w.Description, &w.Secret, &isActive, &createdAt, &updatedAt)
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

	query := `SELECT id, name, path, script, description, secret, is_active, created_at, updated_at FROM webhooks WHERE path = ?`
	var w entity.Webhook
	var isActive int
	var createdAt, updatedAt int64
	err := s.db.QueryRowContext(ctx, query, path).Scan(&w.ID, &w.Name, &w.Path, &w.Script, &w.Description, &w.Secret, &isActive, &createdAt, &updatedAt)
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

	query := `INSERT INTO webhooks (id, name, path, script, description, secret, is_active, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`
	now := time.Now().Unix()
	_, err := s.db.ExecContext(ctx, query, w.ID, w.Name, w.Path, w.Script, w.Description, w.Secret, func() int {
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

	query := `UPDATE webhooks SET name = ?, path = ?, script = ?, description = ?, secret = ?, is_active = ?, updated_at = ? WHERE id = ?`
	_, err := s.db.ExecContext(ctx, query, w.Name, w.Path, w.Script, w.Description, w.Secret, func() int {
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

// WebhookLogRepository implementation for AppStore

func (s *AppStore) CreateWebhookLog(ctx context.Context, log *entity.WebhookLog) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	query := `INSERT INTO webhook_logs (id, webhook_id, webhook_path, source_ip, method, headers, body, query_params, status_code, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
	_, err := s.db.ExecContext(ctx, query, log.ID, log.WebhookID, log.WebhookPath, log.SourceIP, log.Method, log.Headers, log.Body, log.QueryParams, log.StatusCode, log.CreatedAt)
	return err
}

func (s *AppStore) GetAllWebhookLogs(ctx context.Context, webhookID string, limit int, offset int) ([]*entity.WebhookLog, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	var query string
	var rows *sql.Rows
	var err error

	if webhookID != "" {
		query = `SELECT id, webhook_id, webhook_path, source_ip, method, headers, body, query_params, status_code, created_at FROM webhook_logs WHERE webhook_id = ? ORDER BY created_at DESC LIMIT ? OFFSET ?`
		rows, err = s.db.QueryContext(ctx, query, webhookID, limit, offset)
	} else {
		query = `SELECT id, webhook_id, webhook_path, source_ip, method, headers, body, query_params, status_code, created_at FROM webhook_logs ORDER BY created_at DESC LIMIT ? OFFSET ?`
		rows, err = s.db.QueryContext(ctx, query, limit, offset)
	}
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var logs []*entity.WebhookLog
	for rows.Next() {
		var l entity.WebhookLog
		if err := rows.Scan(&l.ID, &l.WebhookID, &l.WebhookPath, &l.SourceIP, &l.Method, &l.Headers, &l.Body, &l.QueryParams, &l.StatusCode, &l.CreatedAt); err != nil {
			return nil, err
		}
		logs = append(logs, &l)
	}
	return logs, nil
}

func (s *AppStore) GetWebhookLogCount(ctx context.Context, webhookID string) (int, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	var query string
	var row *sql.Row

	if webhookID != "" {
		query = `SELECT COUNT(*) FROM webhook_logs WHERE webhook_id = ?`
		row = s.db.QueryRowContext(ctx, query, webhookID)
	} else {
		query = `SELECT COUNT(*) FROM webhook_logs`
		row = s.db.QueryRowContext(ctx, query)
	}

	var count int
	if err := row.Scan(&count); err != nil {
		return 0, err
	}
	return count, nil
}

func (s *AppStore) DeleteAllWebhookLogs(ctx context.Context) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	query := `DELETE FROM webhook_logs`
	_, err := s.db.ExecContext(ctx, query)
	return err
}

// APIKeyRepository implementation for AppStore

func (s *AppStore) CreateAPIKey(ctx context.Context, key *entity.APIKey) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	scopes, err := json.Marshal(key.Scopes)
	if err != nil {
		return err
	}
	isActive := 0
	if key.IsActive {
		isActive = 1
	}
	query := `INSERT INTO api_keys (id, name, key_prefix, key_hash, scopes, is_active, created_at, last_used_at, revoked_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`
	_, err = s.db.ExecContext(ctx, query, key.ID, key.Name, key.Prefix, key.KeyHash, string(scopes), isActive, key.CreatedAt, key.LastUsedAt, key.RevokedAt)
	return err
}

func (s *AppStore) GetAPIKeys(ctx context.Context) ([]*entity.APIKey, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	query := `SELECT id, name, key_prefix, key_hash, scopes, is_active, created_at, last_used_at, revoked_at FROM api_keys ORDER BY created_at DESC`
	rows, err := s.db.QueryContext(ctx, query)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var keys []*entity.APIKey
	for rows.Next() {
		var k entity.APIKey
		var scopes string
		var isActive int
		if err := rows.Scan(&k.ID, &k.Name, &k.Prefix, &k.KeyHash, &scopes, &isActive, &k.CreatedAt, &k.LastUsedAt, &k.RevokedAt); err != nil {
			return nil, err
		}
		_ = json.Unmarshal([]byte(scopes), &k.Scopes)
		if k.Scopes == nil {
			k.Scopes = []string{}
		}
		k.IsActive = isActive == 1
		keys = append(keys, &k)
	}
	return keys, nil
}

func (s *AppStore) FindAPIKeyByHash(ctx context.Context, hash string) (*entity.APIKey, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	query := `SELECT id, name, key_prefix, key_hash, scopes, is_active, created_at, last_used_at, revoked_at FROM api_keys WHERE key_hash = ?`
	var k entity.APIKey
	var scopes string
	var isActive int
	err := s.db.QueryRowContext(ctx, query, hash).Scan(&k.ID, &k.Name, &k.Prefix, &k.KeyHash, &scopes, &isActive, &k.CreatedAt, &k.LastUsedAt, &k.RevokedAt)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	_ = json.Unmarshal([]byte(scopes), &k.Scopes)
	if k.Scopes == nil {
		k.Scopes = []string{}
	}
	k.IsActive = isActive == 1
	return &k, nil
}

func (s *AppStore) TouchAPIKey(ctx context.Context, id string, lastUsedAt int64) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	query := `UPDATE api_keys SET last_used_at = ? WHERE id = ?`
	_, err := s.db.ExecContext(ctx, query, lastUsedAt, id)
	return err
}

func (s *AppStore) RevokeAPIKey(ctx context.Context, id string, revokedAt int64) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	query := `UPDATE api_keys SET revoked_at = ?, is_active = 0 WHERE id = ?`
	_, err := s.db.ExecContext(ctx, query, revokedAt, id)
	return err
}

func (s *AppStore) DeleteAPIKey(ctx context.Context, id string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	query := `DELETE FROM api_keys WHERE id = ?`
	_, err := s.db.ExecContext(ctx, query, id)
	return err
}

// CallRepository implementation for AppStore

func (s *AppStore) CreateCallLog(ctx context.Context, log *entity.CallLog) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	participants, err := json.Marshal(log.Participants)
	if err != nil {
		return err
	}
	query := `INSERT INTO call_logs (id, meow_call_id, direction, call_type, target, group_jid, participants, source, media_mode, status, error_message, api_key_id, started_at, answered_at, ended_at, duration_ms, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
	_, err = s.db.ExecContext(ctx, query, log.ID, log.MeowCallID, log.Direction, log.CallType, log.Target, log.GroupJID, string(participants), log.Source, log.MediaMode, log.Status, log.ErrorMessage, log.APIKeyID, log.StartedAt, log.AnsweredAt, log.EndedAt, log.DurationMS, log.CreatedAt)
	return err
}

func (s *AppStore) UpdateCallStatus(ctx context.Context, id string, status entity.CallStatus, answeredAt *int64, endedAt *int64, durationMS *int64, meowCallID string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	query := `UPDATE call_logs SET status = ?, answered_at = COALESCE(?, answered_at), ended_at = COALESCE(?, ended_at), duration_ms = COALESCE(?, duration_ms), meow_call_id = CASE WHEN ? = '' THEN meow_call_id ELSE ? END WHERE id = ?`
	_, err := s.db.ExecContext(ctx, query, status, answeredAt, endedAt, durationMS, meowCallID, meowCallID, id)
	return err
}

func (s *AppStore) GetCallLog(ctx context.Context, id string) (*entity.CallLog, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	query := `SELECT id, meow_call_id, direction, call_type, target, group_jid, participants, source, media_mode, status, error_message, api_key_id, started_at, answered_at, ended_at, duration_ms, created_at FROM call_logs WHERE id = ?`
	var l entity.CallLog
	var participants string
	err := s.db.QueryRowContext(ctx, query, id).Scan(&l.ID, &l.MeowCallID, &l.Direction, &l.CallType, &l.Target, &l.GroupJID, &participants, &l.Source, &l.MediaMode, &l.Status, &l.ErrorMessage, &l.APIKeyID, &l.StartedAt, &l.AnsweredAt, &l.EndedAt, &l.DurationMS, &l.CreatedAt)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	_ = json.Unmarshal([]byte(participants), &l.Participants)
	if l.Participants == nil {
		l.Participants = []string{}
	}
	return &l, nil
}

func (s *AppStore) GetCallHistory(ctx context.Context, opts CallHistoryFilter) ([]*entity.CallLog, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	query := `SELECT id, meow_call_id, direction, call_type, target, group_jid, participants, source, media_mode, status, error_message, api_key_id, started_at, answered_at, ended_at, duration_ms, created_at FROM call_logs WHERE 1=1`
	var args []interface{}
	if opts.Before != nil {
		query += ` AND started_at < ?`
		args = append(args, *opts.Before)
	}
	if opts.Direction != "" {
		query += ` AND direction = ?`
		args = append(args, opts.Direction)
	}
	if opts.Type != "" {
		query += ` AND call_type = ?`
		args = append(args, opts.Type)
	}
	if opts.Status != "" {
		query += ` AND status = ?`
		args = append(args, opts.Status)
	}
	if opts.Target != "" {
		query += ` AND target = ?`
		args = append(args, opts.Target)
	}
	query += ` ORDER BY started_at DESC`

	limit := opts.Limit
	if limit <= 0 {
		limit = 50
	}
	query += ` LIMIT ?`
	args = append(args, limit)

	rows, err := s.db.QueryContext(ctx, query, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var logs []*entity.CallLog
	for rows.Next() {
		var l entity.CallLog
		var participants string
		if err := rows.Scan(&l.ID, &l.MeowCallID, &l.Direction, &l.CallType, &l.Target, &l.GroupJID, &participants, &l.Source, &l.MediaMode, &l.Status, &l.ErrorMessage, &l.APIKeyID, &l.StartedAt, &l.AnsweredAt, &l.EndedAt, &l.DurationMS, &l.CreatedAt); err != nil {
			return nil, err
		}
		_ = json.Unmarshal([]byte(participants), &l.Participants)
		if l.Participants == nil {
			l.Participants = []string{}
		}
		logs = append(logs, &l)
	}
	return logs, nil
}

func (s *AppStore) MarkInterruptedCalls(ctx context.Context) (int64, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	query := `UPDATE call_logs SET status = 'interrupted' WHERE status IN ('preparing', 'initiating', 'ringing', 'connecting', 'connected', 'ending') AND ended_at IS NULL`
	res, err := s.db.ExecContext(ctx, query)
	if err != nil {
		return 0, err
	}
	return res.RowsAffected()
}

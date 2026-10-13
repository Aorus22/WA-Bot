package repository

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	_ "github.com/mattn/go-sqlite3"
	waWeb "go.mau.fi/whatsmeow/proto/waWeb"
	"google.golang.org/protobuf/proto"
)

type MessageStore struct {
	db swapDB
	mu sync.RWMutex
}

// swapDB forwards to a *sql.DB that can be replaced at runtime, so one
// MessageStore pointer can follow the logged-in WhatsApp account while every
// holder of that pointer stays valid.
type swapDB struct {
	mu sync.RWMutex
	db *sql.DB
}

func (d *swapDB) cur() *sql.DB {
	d.mu.RLock()
	defer d.mu.RUnlock()
	return d.db
}

func (d *swapDB) swap(next *sql.DB) *sql.DB {
	d.mu.Lock()
	defer d.mu.Unlock()
	prev := d.db
	d.db = next
	return prev
}

func (d *swapDB) Exec(q string, args ...any) (sql.Result, error) { return d.cur().Exec(q, args...) }
func (d *swapDB) ExecContext(ctx context.Context, q string, args ...any) (sql.Result, error) {
	return d.cur().ExecContext(ctx, q, args...)
}
func (d *swapDB) Query(q string, args ...any) (*sql.Rows, error) { return d.cur().Query(q, args...) }
func (d *swapDB) QueryContext(ctx context.Context, q string, args ...any) (*sql.Rows, error) {
	return d.cur().QueryContext(ctx, q, args...)
}
func (d *swapDB) QueryRow(q string, args ...any) *sql.Row { return d.cur().QueryRow(q, args...) }
func (d *swapDB) QueryRowContext(ctx context.Context, q string, args ...any) *sql.Row {
	return d.cur().QueryRowContext(ctx, q, args...)
}
func (d *swapDB) Begin() (*sql.Tx, error) { return d.cur().Begin() }
func (d *swapDB) Close() error            { return d.cur().Close() }

// ReactionEntry aggregates everyone who reacted with one emoji.
type ReactionEntry struct {
	Emoji   string   `json:"emoji"`
	Senders []string `json:"senders"`
}

type PollOption struct {
	Name string `json:"name"`
}

type PollMeta struct {
	Question    string       `json:"question"`
	Options     []PollOption `json:"options"`
	MultiSelect bool         `json:"multiSelect"`
	// Votes maps sender JID -> option names they voted for.
	Votes map[string][]string `json:"votes,omitempty"`
}

type LocationMeta struct {
	Latitude     float64 `json:"latitude"`
	Longitude    float64 `json:"longitude"`
	Name         string  `json:"name,omitempty"`
	Address      string  `json:"address,omitempty"`
	Live         bool    `json:"live,omitempty"`
	ThumbnailURL string  `json:"thumbnailUrl,omitempty"`
}

type ContactEntry struct {
	DisplayName string `json:"displayName"`
	VCard       string `json:"vcard,omitempty"`
}

type ContactMeta struct {
	DisplayName string         `json:"displayName,omitempty"`
	Contacts    []ContactEntry `json:"contacts,omitempty"`
}

type ViewOnceMeta struct {
	MediaType string `json:"mediaType"`
	Viewed    bool   `json:"viewed"`
}

type LinkPreviewMeta struct {
	URL          string `json:"url"`
	Title        string `json:"title,omitempty"`
	Description  string `json:"description,omitempty"`
	ThumbnailURL string `json:"thumbnailUrl,omitempty"`
}

// MessageExtra carries type-specific metadata for poll/location/contact/
// view-once/gif/link-preview messages.
// MessageEdit is one earlier version of an edited message.
type MessageEdit struct {
	Content string `json:"content"`
	// Timestamp is when this version was written, in milliseconds.
	Timestamp int64 `json:"timestamp"`
}

type MessageExtra struct {
	// Edits holds the versions before the current one, oldest first; the
	// message's content is always the latest. EditedAt is when it was last edited.
	Edits        []MessageEdit     `json:"edits,omitempty"`
	EditedAt     int64             `json:"editedAt,omitempty"`
	Poll         *PollMeta         `json:"poll,omitempty"`
	Location     *LocationMeta     `json:"location,omitempty"`
	Contact      *ContactMeta      `json:"contact,omitempty"`
	ViewOnce     *ViewOnceMeta     `json:"viewOnce,omitempty"`
	GIF          bool              `json:"gif,omitempty"`
	LinkPreview  *LinkPreviewMeta  `json:"linkPreview,omitempty"`
	Mentions     []string          `json:"mentions,omitempty"`
	MentionNames map[string]string `json:"mentionNames,omitempty"`
	// MentionChats maps each mentioned JID to the one-to-one chat JID to open
	// for it (a LID resolved to its phone number when known).
	MentionChats map[string]string `json:"mentionChats,omitempty"`
	MentionsMe   bool              `json:"mentionsMe,omitempty"`
}

// MessageReceipt records a read receipt reported by WhatsApp for an outgoing
// group message. ReadAt is a Unix timestamp in milliseconds.
type MessageReceipt struct {
	MessageID string `json:"messageId"`
	ChatID    string `json:"chatId"`
	JID       string `json:"jid"`
	ReadAt    int64  `json:"readAt"`
}

type Message struct {
	ID          string          `json:"id"`
	ChatID      string          `json:"chatId"`
	From        string          `json:"from"`
	To          string          `json:"to"`
	Content     string          `json:"content"`
	Timestamp   int64           `json:"timestamp"`
	Status      string          `json:"status"`
	Type        string          `json:"type"`
	MediaURL    string          `json:"mediaUrl,omitempty"`
	IsAutomatic bool            `json:"isAutomatic"`
	SenderName  string          `json:"senderName,omitempty"`
	ChatName    string          `json:"chatName,omitempty"`
	ReplyToID   string          `json:"replyToId,omitempty"`
	Forwarded   bool            `json:"forwarded,omitempty"`
	Reactions   []ReactionEntry `json:"reactions,omitempty"`
	Extra       *MessageExtra   `json:"extra,omitempty"`
	// RawProto is the serialized waE2E.Message of incoming messages; kept for
	// lossless forwarding. Not exposed over JSON.
	RawProto []byte `json:"-"`
}

type Chat struct {
	ID               string `json:"id"`
	Name             string `json:"name"`
	Avatar           string `json:"avatar"`
	LastMsg          string `json:"lastMsg"`
	LastTime         int64  `json:"lastTime"`
	Unread           int    `json:"unread"`
	IsActive         bool   `json:"isActive"`
	IsGroup          bool   `json:"isGroup"`
	Archived         bool   `json:"archived"`
	PinnedAt         *int64 `json:"pinnedAt"`
	MuteMode         string `json:"muteMode"`
	MutedUntil       *int64 `json:"mutedUntil"`
	HasUnreadMention bool   `json:"hasUnreadMention"`
}

type Contact struct {
	ID     string `json:"id"`
	Name   string `json:"name"`
	JID    string `json:"jid"`
	Avatar string `json:"avatar"`
	About  string `json:"about,omitempty"`
}

func NewMessageStore(dbPath string) (*MessageStore, error) {
	db, err := sql.Open("sqlite3", dbPath)
	if err != nil {
		return nil, fmt.Errorf("failed to open database: %w", err)
	}

	store := &MessageStore{}
	store.db.db = db

	if err := store.init(); err != nil {
		return nil, fmt.Errorf("failed to initialize database: %w", err)
	}

	return store, nil
}

// Switch points the store at another database file (one per WhatsApp account),
// creating its parent directory and schema when needed. Callers keep using the
// same *MessageStore. On error the current database stays active.
func (s *MessageStore) Switch(dbPath string) error {
	if dir := filepath.Dir(dbPath); dir != "." {
		if err := os.MkdirAll(dir, 0o755); err != nil {
			return fmt.Errorf("failed to create database directory: %w", err)
		}
	}
	next, err := sql.Open("sqlite3", "file:"+dbPath+"?_foreign_keys=on")
	if err != nil {
		return fmt.Errorf("failed to open database: %w", err)
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	prev := s.db.swap(next)
	if err := s.initLocked(); err != nil {
		s.db.swap(prev)
		_ = next.Close()
		return fmt.Errorf("failed to initialize database: %w", err)
	}
	if prev != nil {
		_ = prev.Close()
	}
	return nil
}

func (s *MessageStore) init() error {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.initLocked()
}

// initLocked creates/migrates the schema; the caller holds s.mu.
func (s *MessageStore) initLocked() error {
	queries := []string{
		`CREATE TABLE IF NOT EXISTS contacts (
                        id TEXT PRIMARY KEY,
                        name TEXT,
                        jid TEXT UNIQUE,
                        avatar TEXT,
                        created_at INTEGER DEFAULT (strftime('%s', 'now')),
                        updated_at INTEGER DEFAULT (strftime('%s', 'now'))
                )`,
		`CREATE TABLE IF NOT EXISTS chats (
                        id TEXT PRIMARY KEY,
                        name TEXT,
                        avatar TEXT,
                        last_msg TEXT,
                        last_time INTEGER,
                        unread INTEGER DEFAULT 0,
                        is_active INTEGER DEFAULT 0,
                        is_group INTEGER DEFAULT 0,
			archived INTEGER DEFAULT 0,
			pinned_at INTEGER,
			mute_mode TEXT DEFAULT 'off',
			muted_until INTEGER,
                        created_at INTEGER DEFAULT (strftime('%s', 'now')),
                        updated_at INTEGER DEFAULT (strftime('%s', 'now'))
                )`,
		`CREATE TABLE IF NOT EXISTS messages (
                        id TEXT PRIMARY KEY,
                        chat_id TEXT,
                        sender_id TEXT,
                        receiver_id TEXT,
                        content TEXT,
                        timestamp INTEGER,
                        status TEXT DEFAULT 'sent',
                        msg_type TEXT DEFAULT 'text',
                        media_url TEXT,
                        is_automatic INTEGER DEFAULT 0,
                        metadata TEXT,
			raw_message BLOB,
                        forwarded INTEGER DEFAULT 0,
                        reactions TEXT,
                        extra_meta TEXT,
                        fwd_proto BLOB,
                        created_at INTEGER DEFAULT (strftime('%s', 'now')),
                        FOREIGN KEY (chat_id) REFERENCES chats(id) ON DELETE CASCADE
                )`,
		`CREATE TABLE IF NOT EXISTS message_read_receipts (
			message_id TEXT NOT NULL,
			chat_id TEXT NOT NULL,
			participant_jid TEXT NOT NULL,
			read_at INTEGER NOT NULL,
			PRIMARY KEY (message_id, participant_jid)
		)`,
		`CREATE INDEX IF NOT EXISTS idx_message_read_receipts_chat ON message_read_receipts(chat_id, message_id)`,
		`CREATE TABLE IF NOT EXISTS favorite_stickers (
                        id TEXT PRIMARY KEY,
                        media_url TEXT,
                        is_animated INTEGER DEFAULT 0,
                        created_at INTEGER DEFAULT (strftime('%s', 'now'))
                )`,
		`CREATE TABLE IF NOT EXISTS lid_mapping (
                        lid TEXT PRIMARY KEY,
                        pn_jid TEXT
                )`,
		`CREATE TABLE IF NOT EXISTS history_staged_conversations (
			chat_id TEXT PRIMARY KEY,
			name TEXT,
			unread_count INTEGER DEFAULT 0,
			archived INTEGER DEFAULT 0,
			pinned_at INTEGER,
			mute_end INTEGER,
			updated_at INTEGER DEFAULT (strftime('%s', 'now'))
		)`,
		`CREATE TABLE IF NOT EXISTS history_staged_messages (
			chat_id TEXT NOT NULL,
			message_id TEXT NOT NULL,
			timestamp INTEGER NOT NULL,
			raw_message BLOB NOT NULL,
			imported INTEGER DEFAULT 0,
			staged_at INTEGER DEFAULT (strftime('%s', 'now')),
			PRIMARY KEY (chat_id, message_id)
		)`,
		`CREATE TABLE IF NOT EXISTS history_sync_notifications (
			id TEXT PRIMARY KEY,
			processed_at INTEGER NOT NULL
		)`,
		`CREATE TABLE IF NOT EXISTS history_sync_runs (
			id INTEGER PRIMARY KEY CHECK (id = 1),
			state TEXT NOT NULL DEFAULT 'idle',
			chats_total INTEGER DEFAULT 0,
			chats_processed INTEGER DEFAULT 0,
			messages_added INTEGER DEFAULT 0,
			errors_json TEXT DEFAULT '[]',
			started_at INTEGER,
			finished_at INTEGER,
			last_run_at INTEGER
		)`,
		`INSERT OR IGNORE INTO history_sync_runs (id, state) VALUES (1, 'idle')`,
		`CREATE INDEX IF NOT EXISTS idx_messages_chat_id ON messages(chat_id)`,
		`CREATE INDEX IF NOT EXISTS idx_messages_timestamp ON messages(timestamp DESC)`,
		`CREATE INDEX IF NOT EXISTS idx_history_staged_pending ON history_staged_messages(chat_id, imported, timestamp DESC)`,
		`CREATE TRIGGER IF NOT EXISTS update_chat_timestamp
                        AFTER INSERT ON messages
                        BEGIN
                                UPDATE chats SET last_msg = NEW.content, last_time = NEW.timestamp, updated_at = strftime('%s', 'now')
				WHERE id = NEW.chat_id AND COALESCE(last_time, 0) <= NEW.timestamp;
                        END`,
	}

	for _, query := range queries {
		if _, err := s.db.Exec(query); err != nil {
			return fmt.Errorf("failed to create table: %w", err)
		}
	}

	// Migration: Add is_automatic column if it doesn't exist
	_, _ = s.db.Exec("ALTER TABLE messages ADD COLUMN is_automatic INTEGER DEFAULT 0")
	_, _ = s.db.Exec("ALTER TABLE messages ADD COLUMN sender_name TEXT")
	_, _ = s.db.Exec("ALTER TABLE chats ADD COLUMN unread INTEGER DEFAULT 0")
	_, _ = s.db.Exec("ALTER TABLE chats ADD COLUMN archived INTEGER DEFAULT 0")
	_, _ = s.db.Exec("ALTER TABLE chats ADD COLUMN pinned_at INTEGER")
	_, _ = s.db.Exec("ALTER TABLE chats ADD COLUMN mute_mode TEXT DEFAULT 'off'")
	_, _ = s.db.Exec("ALTER TABLE chats ADD COLUMN muted_until INTEGER")
	_, _ = s.db.Exec("ALTER TABLE messages ADD COLUMN raw_message BLOB")
	_, _ = s.db.Exec("ALTER TABLE messages ADD COLUMN forwarded INTEGER DEFAULT 0")
	_, _ = s.db.Exec("ALTER TABLE messages ADD COLUMN reactions TEXT")
	_, _ = s.db.Exec("ALTER TABLE messages ADD COLUMN extra_meta TEXT")
	_, _ = s.db.Exec("ALTER TABLE messages ADD COLUMN fwd_proto BLOB")
	// Soft-delete markers (ms epoch). Deleting a chat/message on the phone only
	// flags the row here; the data is kept.
	_, _ = s.db.Exec("ALTER TABLE messages ADD COLUMN deleted_at INTEGER")
	_, _ = s.db.Exec("ALTER TABLE chats ADD COLUMN deleted_at INTEGER")
	_, _ = s.db.Exec("ALTER TABLE contacts ADD COLUMN about TEXT")
	_, _ = s.db.Exec("UPDATE chats SET unread = 0 WHERE unread IS NULL")
	_, _ = s.db.Exec("UPDATE chats SET archived = 0 WHERE archived IS NULL")
	_, _ = s.db.Exec("UPDATE chats SET mute_mode = 'off' WHERE mute_mode IS NULL OR mute_mode = ''")
	// Older databases already have the unconditional version of this trigger.
	// Recreate it so importing old history can never move a chat preview backwards.
	_, _ = s.db.Exec("DROP TRIGGER IF EXISTS update_chat_timestamp")
	_, _ = s.db.Exec(`CREATE TRIGGER update_chat_timestamp
		AFTER INSERT ON messages
		BEGIN
			UPDATE chats SET last_msg = NEW.content, last_time = NEW.timestamp, updated_at = strftime('%s', 'now')
			WHERE id = NEW.chat_id AND COALESCE(last_time, 0) <= NEW.timestamp;
		END`)

	// Starred stickers synced from the phone are tagged source='wa'; the
	// ones saved from this app keep the default 'custom'.
	_, _ = s.db.Exec(`ALTER TABLE favorite_stickers ADD COLUMN source TEXT DEFAULT 'custom'`)

	// Migration: Ensure favorite_stickers has the 'id' column (handling old implementation)
	var tableExists bool
	err := s.db.QueryRow("SELECT count(*) FROM sqlite_master WHERE type='table' AND name='favorite_stickers'").Scan(&tableExists)
	if err == nil && tableExists {
		// Check if 'id' column exists
		var idExists bool
		rows, err := s.db.Query("PRAGMA table_info(favorite_stickers)")
		if err == nil {
			for rows.Next() {
				var cid int
				var name, dtype string
				var notnull, pk int
				var dflt_value interface{}
				if err := rows.Scan(&cid, &name, &dtype, &notnull, &dflt_value, &pk); err == nil {
					if name == "id" {
						idExists = true
						break
					}
				}
			}
			rows.Close()
		}

		if !idExists {
			// Easiest fix: drop and recreate since it's a new feature and data is transient
			_, _ = s.db.Exec("DROP TABLE favorite_stickers")
			_, _ = s.db.Exec(`CREATE TABLE favorite_stickers (
                                id TEXT PRIMARY KEY,
                                media_url TEXT,
                                is_animated INTEGER DEFAULT 0,
                                created_at INTEGER DEFAULT (strftime('%s', 'now'))
                        )`)
		}
	}

	return nil
}

func (s *MessageStore) SaveLIDMapping(lid, pnJID string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	fmt.Printf("[DB] Saving LID mapping: %s -> %s\n", lid, pnJID)
	_, err := s.db.Exec(`
                INSERT OR REPLACE INTO lid_mapping (lid, pn_jid)
                VALUES (?, ?)
        `, lid, pnJID)

	return err
}

func (s *MessageStore) ResolveChatID(chatID string) string {
	s.mu.RLock()
	defer s.mu.RUnlock()

	if strings.HasSuffix(chatID, "@lid") {
		var pnJID string
		err := s.db.QueryRow("SELECT pn_jid FROM lid_mapping WHERE lid = ?", chatID).Scan(&pnJID)
		if err == nil && pnJID != "" {
			return pnJID
		}
	} else if strings.HasSuffix(chatID, "@s.whatsapp.net") || strings.HasSuffix(chatID, "@g.us") {
		var lid string
		err := s.db.QueryRow("SELECT lid FROM lid_mapping WHERE pn_jid = ?", chatID).Scan(&lid)
		if err == nil && lid != "" {
			return chatID
		}
	}

	return chatID
}

// messageColumns is the shared SELECT list for the messages table. Every
// query that returns message rows must use it so scans stay in sync.
const messageColumns = `id, chat_id,
	CASE WHEN sender_id LIKE '%@lid' THEN COALESCE((SELECT pn_jid FROM lid_mapping WHERE lid = sender_id), sender_id) ELSE sender_id END as sender_id,
	receiver_id, content, timestamp, status, msg_type, ifnull(media_url, '') as media_url,
	is_automatic, ifnull(sender_name, '') as sender_name, ifnull(metadata, '') as reply_to_id,
	ifnull(forwarded, 0) as forwarded, ifnull(reactions, '') as reactions, ifnull(extra_meta, '') as extra_meta`

func decodeMessageMeta(msg *Message, forwarded int, reactionsJSON, extraJSON string) {
	msg.Forwarded = forwarded == 1
	if reactionsJSON != "" {
		var reactions []ReactionEntry
		if json.Unmarshal([]byte(reactionsJSON), &reactions) == nil {
			msg.Reactions = reactions
		}
	}
	if extraJSON != "" {
		var extra MessageExtra
		if json.Unmarshal([]byte(extraJSON), &extra) == nil && (extra.Poll != nil || extra.Location != nil || extra.Contact != nil || extra.ViewOnce != nil || extra.GIF || extra.LinkPreview != nil || len(extra.Mentions) > 0 || len(extra.Edits) > 0) {
			msg.Extra = &extra
		}
	}
}

// scanMessages reads rows produced by a messageColumns query.
func scanMessages(rows *sql.Rows) ([]Message, error) {
	defer rows.Close()

	var messages []Message
	for rows.Next() {
		var msg Message
		var isAuto, forwarded int
		var reactionsJSON, extraJSON string
		err := rows.Scan(
			&msg.ID,
			&msg.ChatID,
			&msg.From,
			&msg.To,
			&msg.Content,
			&msg.Timestamp,
			&msg.Status,
			&msg.Type,
			&msg.MediaURL,
			&isAuto,
			&msg.SenderName,
			&msg.ReplyToID,
			&forwarded,
			&reactionsJSON,
			&extraJSON,
		)
		if err != nil {
			return nil, err
		}
		msg.IsAutomatic = isAuto == 1
		decodeMessageMeta(&msg, forwarded, reactionsJSON, extraJSON)
		messages = append(messages, msg)
	}
	return messages, rows.Err()
}

func (s *MessageStore) SaveMessage(msg *Message) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	tx, err := s.db.Begin()
	if err != nil {
		return err
	}
	defer tx.Rollback()

	// Insert or update chat
	var chatExists bool
	err = tx.QueryRow("SELECT COUNT(*) > 0 FROM chats WHERE id = ?", msg.ChatID).Scan(&chatExists)
	if err != nil {
		return err
	}

	if !chatExists {
		name := msg.ChatName
		if name == "" {
			name = msg.SenderName
		}
		if name == "" {
			name = msg.ChatID
		}
		isGroup := 0
		if strings.HasSuffix(msg.ChatID, "@g.us") {
			isGroup = 1
		}
		unread := 0
		if msg.Status == "received" {
			unread = 1
		}
		_, err = tx.Exec(`
                        INSERT INTO chats (id, name, last_msg, last_time, unread, is_active, is_group)
                        VALUES (?, ?, ?, ?, ?, 1, ?)
                `, msg.ChatID, name, msg.Content, msg.Timestamp, unread, isGroup)
	} else {
		// Update chat name if provided
		if msg.ChatName != "" {
			if msg.Status == "received" {
				_, err = tx.Exec(`
                                        UPDATE chats SET name = ?, last_msg = ?, last_time = ?, unread = ifnull(unread, 0) + 1, updated_at = ?
                                        WHERE id = ?
                                `, msg.ChatName, msg.Content, msg.Timestamp, time.Now().Unix(), msg.ChatID)
			} else {
				_, err = tx.Exec(`
                                        UPDATE chats SET name = ?, last_msg = ?, last_time = ?, updated_at = ?
                                        WHERE id = ?
                                `, msg.ChatName, msg.Content, msg.Timestamp, time.Now().Unix(), msg.ChatID)
			}
		} else {
			if msg.Status == "received" {
				_, err = tx.Exec(`
                                        UPDATE chats SET last_msg = ?, last_time = ?, unread = ifnull(unread, 0) + 1, updated_at = ?
                                        WHERE id = ?
                                `, msg.Content, msg.Timestamp, time.Now().Unix(), msg.ChatID)
			} else {
				_, err = tx.Exec(`
                                        UPDATE chats SET last_msg = ?, last_time = ?, updated_at = ?
                                        WHERE id = ?
                                `, msg.Content, msg.Timestamp, time.Now().Unix(), msg.ChatID)
			}
		}
	}
	if err != nil {
		return err
	}

	// A message newer than a phone-side delete brings the chat back.
	if _, err = tx.Exec(`UPDATE chats SET deleted_at = NULL WHERE id = ? AND deleted_at IS NOT NULL AND deleted_at < ?`, msg.ChatID, msg.Timestamp); err != nil {
		return err
	}

	// Insert message
	var reactionsJSON any
	if msg.Reactions != nil {
		b, _ := json.Marshal(msg.Reactions)
		reactionsJSON = string(b)
	}
	var extraJSON any
	if msg.Extra != nil {
		b, _ := json.Marshal(msg.Extra)
		extraJSON = string(b)
	}
	_, err = tx.Exec(`
                INSERT OR REPLACE INTO messages (id, chat_id, sender_id, receiver_id, content, timestamp, status, msg_type, media_url, is_automatic, sender_name, metadata, forwarded, reactions, extra_meta, fwd_proto)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        `, msg.ID, msg.ChatID, msg.From, msg.To, msg.Content, msg.Timestamp, msg.Status, msg.Type, msg.MediaURL, func() int {
		if msg.IsAutomatic {
			return 1
		}
		return 0
	}(), msg.SenderName, msg.ReplyToID, func() int {
		if msg.Forwarded {
			return 1
		}
		return 0
	}(), reactionsJSON, extraJSON, msg.RawProto)
	if err != nil {
		return err
	}

	return tx.Commit()
}

// HasMessage reports whether a message ID has already been persisted. WhatsApp
// may echo a message sent from another linked device after the send request has
// already been logged locally.
func (s *MessageStore) HasMessage(messageID string) bool {
	s.mu.RLock()
	defer s.mu.RUnlock()

	var exists bool
	if err := s.db.QueryRow(`SELECT EXISTS(SELECT 1 FROM messages WHERE id = ?)`, messageID).Scan(&exists); err != nil {
		return false
	}
	return exists
}
func (s *MessageStore) GetMessages(chatID string, limit int, before int64, after int64) ([]Message, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	var query string
	var args []interface{}

	baseQuery := `
		WITH linked_chats AS (
			SELECT ? as id
			UNION
			SELECT lid FROM lid_mapping WHERE pn_jid = ?
			UNION
			SELECT pn_jid FROM lid_mapping WHERE lid = ?
		)
		SELECT ` + messageColumns + `
		FROM messages
		WHERE chat_id IN (SELECT id FROM linked_chats WHERE id IS NOT NULL)
		AND deleted_at IS NULL`

	if before > 0 {
		query = baseQuery + ` AND timestamp < ? ORDER BY timestamp DESC, rowid DESC LIMIT ?`
		args = []interface{}{chatID, chatID, chatID, before, limit}
	} else if after > 0 {
		query = baseQuery + ` AND timestamp > ? ORDER BY timestamp ASC, rowid ASC LIMIT ?`
		args = []interface{}{chatID, chatID, chatID, after, limit}
	} else {
		query = baseQuery + ` ORDER BY timestamp DESC, rowid DESC LIMIT ?`
		args = []interface{}{chatID, chatID, chatID, limit}
	}

	rows, err := s.db.Query(query, args...)
	if err != nil {
		return nil, err
	}

	messages, err := scanMessages(rows)
	if err != nil {
		return nil, err
	}

	// Reverse to get chronological order if we were fetching "before" or the latest
	if after == 0 {
		for i, j := 0, len(messages)-1; i < j; i, j = i+1, j-1 {
			messages[i], messages[j] = messages[j], messages[i]
		}
	}

	return messages, nil
}

func (s *MessageStore) SearchMessages(chatID string, query string, limit int) ([]Message, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	sql := `
		WITH linked_chats AS (
			SELECT ? as id
			UNION
			SELECT lid FROM lid_mapping WHERE pn_jid = ?
			UNION
			SELECT pn_jid FROM lid_mapping WHERE lid = ?
		)
		SELECT ` + messageColumns + `
		FROM messages
		WHERE chat_id IN (SELECT id FROM linked_chats WHERE id IS NOT NULL)
		AND deleted_at IS NULL
		AND content LIKE ?
		ORDER BY timestamp DESC
		LIMIT ?
	`
	args := []interface{}{chatID, chatID, chatID, "%" + query + "%", limit}

	rows, err := s.db.Query(sql, args...)
	if err != nil {
		return nil, err
	}
	return scanMessages(rows)
}

func (s *MessageStore) GetMessageContext(chatID string, messageID string, limit int) ([]Message, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	var targetTimestamp int64
	err := s.db.QueryRow("SELECT timestamp FROM messages WHERE id = ?", messageID).Scan(&targetTimestamp)
	if err != nil {
		return nil, err
	}

	half := limit / 2

	// Get messages before
	before, err := s.GetMessages(chatID, half, targetTimestamp, 0)
	if err != nil {
		return nil, err
	}

	// Get target message
	var targetMsg Message
	var isAuto, fwd int
	var reactionsJSON, extraJSON string
	err = s.db.QueryRow(`
		SELECT `+messageColumns+`
		FROM messages WHERE id = ?
	`, messageID).Scan(
		&targetMsg.ID,
		&targetMsg.ChatID,
		&targetMsg.From,
		&targetMsg.To,
		&targetMsg.Content,
		&targetMsg.Timestamp,
		&targetMsg.Status,
		&targetMsg.Type,
		&targetMsg.MediaURL,
		&isAuto,
		&targetMsg.SenderName,
		&targetMsg.ReplyToID,
		&fwd,
		&reactionsJSON,
		&extraJSON,
	)
	if err != nil {
		return nil, err
	}
	targetMsg.IsAutomatic = isAuto == 1
	decodeMessageMeta(&targetMsg, fwd, reactionsJSON, extraJSON)

	// Get messages after
	after, err := s.GetMessages(chatID, half, 0, targetTimestamp)
	if err != nil {
		return nil, err
	}

	result := append(before, targetMsg)
	result = append(result, after...)

	return result, nil
}

func (s *MessageStore) GetChats() ([]Chat, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	query := `
                WITH normalized_chats AS (
                    SELECT 
                        CASE 
                            WHEN c.id LIKE '%@lid' THEN COALESCE((SELECT pn_jid FROM lid_mapping WHERE lid = c.id), c.id)
                            WHEN c.id LIKE '%@s.whatsapp.net' THEN c.id
                            ELSE c.id
                        END as target_id,
                        c.name,
                        c.avatar,
                        c.last_msg,
                        c.last_time,
                        c.unread,
                        c.is_active,
                        c.is_group,
			c.archived,
			c.pinned_at,
			c.mute_mode,
			c.muted_until,
                        c.id as original_id
                    FROM chats c
                    WHERE c.deleted_at IS NULL
                ),
                grouped_chats AS (
                    SELECT 
                        target_id, 
						COALESCE(MAX(CASE WHEN name != target_id AND name != '' AND name NOT LIKE '%@%' THEN name END), MAX(name), target_id) as name,
                        COALESCE(MAX(CASE WHEN avatar != '' THEN avatar END), '') as avatar,
                        MAX(last_time) as last_time,
                        SUM(unread) as unread,
                        MAX(is_active) as is_active,
			MAX(is_group) as is_group,
			MAX(archived) as archived,
			MAX(pinned_at) as pinned_at,
			COALESCE(MAX(CASE WHEN mute_mode != 'off' THEN mute_mode END), 'off') as mute_mode,
			MAX(muted_until) as muted_until
                    FROM normalized_chats
                    GROUP BY target_id
                )
                SELECT 
                    g.target_id, 
                    g.name, 
                    g.avatar, 
                    COALESCE((SELECT last_msg FROM normalized_chats nc2 WHERE nc2.target_id = g.target_id ORDER BY last_time DESC LIMIT 1), '') as last_msg,
                    g.last_time,
                    g.unread,
                    g.is_active,
			g.is_group,
			g.archived,
			g.pinned_at,
			g.mute_mode,
			g.muted_until
                FROM grouped_chats g
                ORDER BY g.last_time DESC
        `

	rows, err := s.db.Query(query)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var chats []Chat
	for rows.Next() {
		var chat Chat
		var isActive, isGroup, archived int
		var pinnedAt, mutedUntil sql.NullInt64
		err := rows.Scan(
			&chat.ID,
			&chat.Name,
			&chat.Avatar,
			&chat.LastMsg,
			&chat.LastTime,
			&chat.Unread,
			&isActive,
			&isGroup,
			&archived,
			&pinnedAt,
			&chat.MuteMode,
			&mutedUntil,
		)
		if err != nil {
			return nil, err
		}
		chat.IsActive = isActive == 1
		chat.IsGroup = isGroup == 1
		chat.Archived = archived == 1
		if pinnedAt.Valid && pinnedAt.Int64 > 0 {
			v := pinnedAt.Int64
			chat.PinnedAt = &v
		}
		if chat.MuteMode == "until" && mutedUntil.Valid && mutedUntil.Int64 > time.Now().UnixMilli() {
			v := mutedUntil.Int64
			chat.MutedUntil = &v
		} else if chat.MuteMode == "until" {
			chat.MuteMode = "off"
		}
		chats = append(chats, chat)
	}

	return chats, nil
}

func (s *MessageStore) GetContacts() ([]Contact, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	query := `
                SELECT id, name, jid, ifnull(avatar, '') as avatar, ifnull(about, '') as about
                FROM contacts
                ORDER BY name ASC
        `

	rows, err := s.db.Query(query)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var contacts []Contact
	for rows.Next() {
		var contact Contact
		err := rows.Scan(
			&contact.ID,
			&contact.Name,
			&contact.JID,
			&contact.Avatar,
			&contact.About,
		)
		if err != nil {
			return nil, err
		}
		contacts = append(contacts, contact)
	}

	return contacts, nil
}

func (s *MessageStore) SaveContact(contact *Contact) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	_, err := s.db.Exec(`
                INSERT INTO contacts (id, name, jid, avatar, updated_at)
                VALUES (?, ?, ?, ?, ?)
                ON CONFLICT(id) DO UPDATE SET name = excluded.name, jid = excluded.jid, avatar = excluded.avatar, updated_at = excluded.updated_at
        `, contact.ID, contact.Name, contact.JID, contact.Avatar, time.Now().Unix())

	return err
}

// UpdateContactName refreshes a contact's local display name without erasing a
// previously cached profile-picture URL.
func (s *MessageStore) UpdateContactName(jid, name string) error {
	jid = strings.TrimSpace(jid)
	name = strings.TrimSpace(name)
	if jid == "" || name == "" {
		return nil
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	result, err := s.db.Exec(`UPDATE contacts SET name = ?, updated_at = ? WHERE jid = ?`, name, time.Now().Unix(), jid)
	if err != nil {
		return err
	}
	updated, err := result.RowsAffected()
	if err != nil || updated > 0 {
		return err
	}
	_, err = s.db.Exec(`INSERT OR IGNORE INTO contacts (id, name, jid, avatar, updated_at) VALUES (?, ?, ?, '', ?)`, jid, name, jid, time.Now().Unix())
	return err
}

func (s *MessageStore) UpdateChatLastMessage(chatID, content string, timestamp int64) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	_, err := s.db.Exec(`
                UPDATE chats
                SET last_msg = ?, last_time = ?, updated_at = ?
                WHERE id = ?
        `, content, timestamp, time.Now().Unix(), chatID)

	return err
}

func (s *MessageStore) MarkAsRead(chatID string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	// Update all linked chat IDs (handle @lid and @s.whatsapp.net duality)
	_, err := s.db.Exec(`
		UPDATE chats SET unread = 0, updated_at = ?
		WHERE id = ?
		   OR id IN (SELECT lid FROM lid_mapping WHERE pn_jid = ?)
		   OR id IN (SELECT pn_jid FROM lid_mapping WHERE lid = ?)
	`, time.Now().Unix(), chatID, chatID, chatID)

	return err
}

// MarkChatDeleted soft-deletes a chat that was deleted on another device: the
// chat is hidden and its messages up to tsMs are flagged, but nothing is
// removed. It returns false when the chat has newer activity than the delete
// (e.g. a replayed old event), in which case nothing changes.
func (s *MessageStore) MarkChatDeleted(chatID string, tsMs int64) (bool, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	var last sql.NullInt64
	if err := s.db.QueryRow(`
		SELECT MAX(last_time) FROM chats
		WHERE id = ?
		   OR id IN (SELECT lid FROM lid_mapping WHERE pn_jid = ?)
		   OR id IN (SELECT pn_jid FROM lid_mapping WHERE lid = ?)
	`, chatID, chatID, chatID).Scan(&last); err != nil {
		return false, err
	}
	if !last.Valid || last.Int64 > tsMs {
		return false, nil
	}
	if err := s.flagChatMessagesLocked(chatID, tsMs); err != nil {
		return false, err
	}
	_, err := s.db.Exec(`
		UPDATE chats SET deleted_at = ?, unread = 0, last_msg = '', updated_at = ?
		WHERE id = ?
		   OR id IN (SELECT lid FROM lid_mapping WHERE pn_jid = ?)
		   OR id IN (SELECT pn_jid FROM lid_mapping WHERE lid = ?)
	`, tsMs, time.Now().Unix(), chatID, chatID, chatID)
	return err == nil, err
}

// MarkChatCleared soft-deletes every message up to tsMs in a chat that was
// cleared on another device; the chat itself stays in the list, empty.
func (s *MessageStore) MarkChatCleared(chatID string, tsMs int64) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	if err := s.flagChatMessagesLocked(chatID, tsMs); err != nil {
		return err
	}
	_, err := s.db.Exec(`
		UPDATE chats SET unread = 0, updated_at = ?,
			last_msg = COALESCE((SELECT content FROM messages WHERE chat_id = chats.id AND deleted_at IS NULL ORDER BY timestamp DESC LIMIT 1), '')
		WHERE id = ?
		   OR id IN (SELECT lid FROM lid_mapping WHERE pn_jid = ?)
		   OR id IN (SELECT pn_jid FROM lid_mapping WHERE lid = ?)
	`, time.Now().Unix(), chatID, chatID, chatID)
	return err
}

func (s *MessageStore) flagChatMessagesLocked(chatID string, tsMs int64) error {
	_, err := s.db.Exec(`
		UPDATE messages SET deleted_at = ?
		WHERE deleted_at IS NULL AND timestamp <= ?
		  AND (chat_id = ?
		    OR chat_id IN (SELECT lid FROM lid_mapping WHERE pn_jid = ?)
		    OR chat_id IN (SELECT pn_jid FROM lid_mapping WHERE lid = ?))
	`, tsMs, tsMs, chatID, chatID, chatID)
	return err
}

// MarkMessageDeleted soft-deletes one message ("delete for me" on another
// device) and refreshes its chat's preview. It returns the message's chat ID
// and the chat's new preview, or "" when the message is unknown or already
// flagged.
func (s *MessageStore) MarkMessageDeleted(msgID string, tsMs int64) (chatID, lastMsg string, err error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	if err := s.db.QueryRow(`SELECT chat_id FROM messages WHERE id = ? AND deleted_at IS NULL`, msgID).Scan(&chatID); err != nil {
		if err == sql.ErrNoRows {
			return "", "", nil
		}
		return "", "", err
	}
	if _, err := s.db.Exec(`UPDATE messages SET deleted_at = ? WHERE id = ?`, tsMs, msgID); err != nil {
		return "", "", err
	}
	err = s.db.QueryRow(`SELECT COALESCE((SELECT content FROM messages WHERE chat_id = ? AND deleted_at IS NULL ORDER BY timestamp DESC LIMIT 1), '')`, chatID).Scan(&lastMsg)
	if err != nil {
		return "", "", err
	}
	_, err = s.db.Exec(`UPDATE chats SET last_msg = ? WHERE id = ?`, lastMsg, chatID)
	return chatID, lastMsg, err
}

// ApplyPushName propagates a contact's new push name (display name chosen by
// the contact) to the stored contact and 1:1 chat name. A name the user set
// themselves (address book) is left alone: only a name that is empty, the raw
// JID or the previous push name is replaced. It returns true if the chat name
// changed.
func (s *MessageStore) ApplyPushName(jid, oldName, newName string) (bool, error) {
	jid = strings.TrimSpace(jid)
	newName = strings.TrimSpace(newName)
	if jid == "" || newName == "" {
		return false, nil
	}
	s.mu.Lock()
	defer s.mu.Unlock()

	now := time.Now().Unix()
	result, err := s.db.Exec(`UPDATE contacts SET name = ?, updated_at = ? WHERE jid = ? AND (name = '' OR name = jid OR name = ?)`, newName, now, jid, oldName)
	if err != nil {
		return false, err
	}
	if n, _ := result.RowsAffected(); n == 0 {
		if _, err := s.db.Exec(`INSERT OR IGNORE INTO contacts (id, name, jid, avatar, updated_at) VALUES (?, ?, ?, '', ?)`, jid, newName, jid, now); err != nil {
			return false, err
		}
	}
	result, err = s.db.Exec(`UPDATE chats SET name = ?, updated_at = ? WHERE id = ? AND is_group = 0 AND (name = '' OR name = id OR name = ?)`, newName, now, jid, oldName)
	if err != nil {
		return false, err
	}
	n, _ := result.RowsAffected()
	return n > 0, nil
}

// SaveContactAbout stores a contact's "about" text.
func (s *MessageStore) SaveContactAbout(jid, about string) error {
	jid = strings.TrimSpace(jid)
	if jid == "" {
		return nil
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	result, err := s.db.Exec(`UPDATE contacts SET about = ? WHERE jid = ?`, about, jid)
	if err != nil {
		return err
	}
	if n, err := result.RowsAffected(); err != nil || n > 0 {
		return err
	}
	_, err = s.db.Exec(`INSERT OR IGNORE INTO contacts (id, name, jid, avatar, about, updated_at) VALUES (?, '', ?, '', ?, ?)`, jid, jid, about, time.Now().Unix())
	return err
}

// MarkChatUnread flags a chat as unread (marked unread on another device). A
// chat that already has unread messages is left as is; it returns whether
// anything changed.
func (s *MessageStore) MarkChatUnread(chatID string) (bool, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	result, err := s.db.Exec(`
		UPDATE chats SET unread = 1, updated_at = ?
		WHERE ifnull(unread, 0) = 0 AND (id = ?
		   OR id IN (SELECT lid FROM lid_mapping WHERE pn_jid = ?)
		   OR id IN (SELECT pn_jid FROM lid_mapping WHERE lid = ?))
	`, time.Now().Unix(), chatID, chatID, chatID)
	if err != nil {
		return false, err
	}
	n, _ := result.RowsAffected()
	return n > 0, nil
}

func (s *MessageStore) UpdateChatAvatar(chatID, avatarURL string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	_, err := s.db.Exec(`
                UPDATE chats SET avatar = ?, updated_at = ? WHERE id = ?
        `, avatarURL, time.Now().Unix(), chatID)

	return err
}

func (s *MessageStore) UpdateChatName(chatID, name string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	_, err := s.db.Exec(`
                UPDATE chats SET name = ?, updated_at = ? WHERE id = ?
        `, name, time.Now().Unix(), chatID)

	return err
}

func (s *MessageStore) SaveFavoriteSticker(id, mediaURL string, isAnimated bool) error {
	return s.saveSticker(id, mediaURL, isAnimated, "custom")
}

// SaveStarredSticker stores a sticker the user starred in WhatsApp itself.
func (s *MessageStore) SaveStarredSticker(id, mediaURL string, isAnimated bool) error {
	return s.saveSticker(id, mediaURL, isAnimated, "wa")
}

func (s *MessageStore) saveSticker(id, mediaURL string, isAnimated bool, source string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	animated := 0
	if isAnimated {
		animated = 1
	}
	_, err := s.db.Exec(`
                INSERT OR REPLACE INTO favorite_stickers (id, media_url, is_animated, source)
                VALUES (?, ?, ?, ?)
        `, id, mediaURL, animated, source)
	return err
}

func (s *MessageStore) GetFavoriteStickers() ([]map[string]interface{}, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	rows, err := s.db.Query("SELECT id, media_url, is_animated, COALESCE(source, 'custom') FROM favorite_stickers ORDER BY created_at DESC")
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var stickers []map[string]interface{}
	for rows.Next() {
		var id, url, source string
		var isAnim int
		if err := rows.Scan(&id, &url, &isAnim, &source); err != nil {
			return nil, err
		}
		stickers = append(stickers, map[string]interface{}{
			"id":         id,
			"mediaUrl":   url,
			"isAnimated": isAnim == 1,
			"source":     source,
		})
	}
	return stickers, nil
}

// GetRecentStickers returns the stickers the user sent most recently, newest
// first, one entry per sticker file.
func (s *MessageStore) GetRecentStickers(limit int) ([]map[string]interface{}, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	rows, err := s.db.Query(`
		SELECT media_url FROM messages
		WHERE msg_type = 'sticker' AND sender_id = 'me' AND COALESCE(media_url, '') != '' AND deleted_at IS NULL
		GROUP BY media_url
		ORDER BY MAX(timestamp) DESC
		LIMIT ?`, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var stickers []map[string]interface{}
	for rows.Next() {
		var url string
		if err := rows.Scan(&url); err != nil {
			return nil, err
		}
		stickers = append(stickers, map[string]interface{}{
			"id":         url,
			"mediaUrl":   url,
			"isAnimated": false,
			"source":     "recent",
		})
	}
	return stickers, nil
}

func (s *MessageStore) DeleteFavoriteSticker(id string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	_, err := s.db.Exec("DELETE FROM favorite_stickers WHERE id = ?", id)
	return err
}

func (s *MessageStore) UpdateMessageStatus(msgID, status string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	_, err := s.db.Exec(`
                UPDATE messages SET status = ? WHERE id = ?
        `, status, msgID)

	return err
}

func (s *MessageStore) UpdateMessageContent(msgID, content string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	_, err := s.db.Exec(`
                UPDATE messages SET content = ? WHERE id = ?
        `, content, msgID)

	return err
}

// ApplyEdit makes newContent the message's content and keeps what it replaced
// in extra.Edits, so the edit history can be shown. It returns the message's
// extra afterwards, or an error when the message is unknown. Repeating an edit
// that is already the current content changes nothing.
func (s *MessageStore) ApplyEdit(msgID, newContent string, editedAt int64) (*MessageExtra, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	var content string
	var timestamp int64
	var extraJSON sql.NullString
	if err := s.db.QueryRow(`SELECT content, timestamp, extra_meta FROM messages WHERE id = ?`, msgID).
		Scan(&content, &timestamp, &extraJSON); err != nil {
		return nil, err
	}
	var extra MessageExtra
	if extraJSON.Valid && extraJSON.String != "" {
		_ = json.Unmarshal([]byte(extraJSON.String), &extra)
	}
	if content == newContent {
		return &extra, nil
	}

	versionAt := timestamp
	if extra.EditedAt > 0 {
		versionAt = extra.EditedAt
	}
	extra.Edits = append(extra.Edits, MessageEdit{Content: content, Timestamp: versionAt})
	if editedAt <= 0 {
		editedAt = time.Now().UnixMilli()
	}
	extra.EditedAt = editedAt

	encoded, err := json.Marshal(&extra)
	if err != nil {
		return nil, err
	}
	if _, err := s.db.Exec(`UPDATE messages SET content = ?, extra_meta = ? WHERE id = ?`, newContent, string(encoded), msgID); err != nil {
		return nil, err
	}
	return &extra, nil
}

// SyncChatPreview puts an edited message's text into its chat's preview line
// when it is the chat's newest message (an older message's edit leaves the
// preview alone). It returns the chat id and whether the preview changed.
func (s *MessageStore) SyncChatPreview(msgID, content string) (string, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()

	var chatID string
	var timestamp int64
	if err := s.db.QueryRow(`SELECT chat_id, timestamp FROM messages WHERE id = ?`, msgID).Scan(&chatID, &timestamp); err != nil {
		return "", false
	}
	res, err := s.db.Exec(`UPDATE chats SET last_msg = ?, updated_at = ?
		WHERE last_time = ? AND (id = ? OR id IN (SELECT lid FROM lid_mapping WHERE pn_jid = ?)
		OR id IN (SELECT pn_jid FROM lid_mapping WHERE lid = ?))`,
		content, time.Now().Unix(), timestamp, chatID, chatID, chatID)
	if err != nil {
		return chatID, false
	}
	n, _ := res.RowsAffected()
	return chatID, n > 0
}

func (s *MessageStore) DeleteMessage(msgID string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	_, err := s.db.Exec(`
                DELETE FROM messages WHERE id = ?
        `, msgID)

	return err
}

// ApplyReaction returns the new reaction list after sender sets (or removes,
// when emoji is empty) their reaction.
func ApplyReaction(existing []ReactionEntry, sender, emoji string) []ReactionEntry {
	out := make([]ReactionEntry, 0, len(existing)+1)
	for _, r := range existing {
		kept := make([]string, 0, len(r.Senders))
		for _, s := range r.Senders {
			if s != sender {
				kept = append(kept, s)
			}
		}
		if r.Emoji == emoji {
			if emoji != "" {
				kept = append(kept, sender)
			}
		}
		if len(kept) > 0 {
			out = append(out, ReactionEntry{Emoji: r.Emoji, Senders: kept})
		}
	}
	if emoji != "" {
		for i := range out {
			if out[i].Emoji == emoji {
				return out
			}
		}
		out = append(out, ReactionEntry{Emoji: emoji, Senders: []string{sender}})
	}
	return out
}

// GetMessageByID fetches a single message row (with reactions/extra decoded).
func (s *MessageStore) GetMessageByID(msgID string) (*Message, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	var msg Message
	var isAuto, fwd int
	var reactionsJSON, extraJSON string
	err := s.db.QueryRow(`
		SELECT `+messageColumns+`
		FROM messages WHERE id = ?
	`, msgID).Scan(
		&msg.ID, &msg.ChatID, &msg.From, &msg.To, &msg.Content, &msg.Timestamp,
		&msg.Status, &msg.Type, &msg.MediaURL, &isAuto, &msg.SenderName,
		&msg.ReplyToID, &fwd, &reactionsJSON, &extraJSON,
	)
	if err != nil {
		return nil, err
	}
	msg.IsAutomatic = isAuto == 1
	decodeMessageMeta(&msg, fwd, reactionsJSON, extraJSON)
	return &msg, nil
}

// UpdateMessageReactions replaces the stored reaction list of a message.
func (s *MessageStore) UpdateMessageReactions(msgID string, reactions []ReactionEntry) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	var reactionsJSON any
	if reactions != nil {
		b, err := json.Marshal(reactions)
		if err != nil {
			return err
		}
		reactionsJSON = string(b)
	}
	_, err := s.db.Exec(`UPDATE messages SET reactions = ? WHERE id = ?`, reactionsJSON, msgID)
	return err
}

// UpdateMessageExtra replaces the stored extra metadata of a message.
func (s *MessageStore) UpdateMessageExtra(msgID string, extra *MessageExtra) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	var extraJSON any
	if extra != nil {
		b, err := json.Marshal(extra)
		if err != nil {
			return err
		}
		extraJSON = string(b)
	}
	_, err := s.db.Exec(`UPDATE messages SET extra_meta = ? WHERE id = ?`, extraJSON, msgID)
	return err
}

// IncomingRef pairs a received message's ID with its sender.
type IncomingRef struct {
	ID     string
	Sender string
}

// GetRecentIncoming returns the most recent incoming (not own) messages of a
// chat, newest first, for read-receipt purposes.
func (s *MessageStore) GetRecentIncoming(chatID string, limit int) ([]IncomingRef, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	if limit <= 0 {
		limit = 100
	}
	rows, err := s.db.Query(`
		WITH linked_chats AS (
			SELECT ? as id
			UNION
			SELECT lid FROM lid_mapping WHERE pn_jid = ?
			UNION
			SELECT pn_jid FROM lid_mapping WHERE lid = ?
		)
		SELECT id, sender_id FROM messages
		WHERE chat_id IN (SELECT id FROM linked_chats WHERE id IS NOT NULL)
		AND deleted_at IS NULL
		AND sender_id != 'me'
		AND sender_id != ''
		ORDER BY timestamp DESC LIMIT ?
	`, chatID, chatID, chatID, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	out := make([]IncomingRef, 0, limit)
	for rows.Next() {
		var ref IncomingRef
		if err := rows.Scan(&ref.ID, &ref.Sender); err != nil {
			continue
		}
		out = append(out, ref)
	}
	return out, rows.Err()
}

// GetMessageRawProto returns the serialized waE2E.Message of a message (used
// for forwarding). Live-received messages keep it in fwd_proto; messages
// imported from history sync only have a WebMessageInfo blob in raw_message,
// so the inner message is extracted from that as a fallback.
func (s *MessageStore) GetMessageRawProto(msgID string) ([]byte, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	var fwd []byte
	err := s.db.QueryRow(`SELECT fwd_proto FROM messages WHERE id = ?`, msgID).Scan(&fwd)
	if err == nil && len(fwd) > 0 {
		return fwd, nil
	}
	if err != nil && err != sql.ErrNoRows {
		return nil, err
	}

	var web []byte
	err = s.db.QueryRow(`SELECT raw_message FROM messages WHERE id = ?`, msgID).Scan(&web)
	if err != nil {
		return nil, err
	}
	var webMsg waWeb.WebMessageInfo
	if proto.Unmarshal(web, &webMsg) != nil || webMsg.GetMessage() == nil {
		return nil, fmt.Errorf("no raw proto available for message %s", msgID)
	}
	return proto.Marshal(webMsg.GetMessage())
}

// GetMessageSender returns the stored sender_id for a message without
// loading the full row.
func (s *MessageStore) GetMessageSender(msgID string) (string, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	var sender string
	err := s.db.QueryRow(`SELECT sender_id FROM messages WHERE id = ?`, msgID).Scan(&sender)
	return sender, err
}

// SaveReadReceipt stores a receipt only for an outgoing group message in the
// stated chat. WhatsApp may replay receipts, so the first timestamp per
// participant is retained.
func (s *MessageStore) SaveReadReceipt(chatID, msgID, participantJID string, readAt int64) (bool, error) {
	participantJID = strings.TrimSpace(participantJID)
	if participantJID == "" || !strings.HasSuffix(chatID, "@g.us") {
		return false, nil
	}
	if readAt <= 0 {
		readAt = time.Now().UnixMilli()
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	var isOutgoingGroup bool
	err := s.db.QueryRow(`SELECT EXISTS(
		SELECT 1 FROM messages m JOIN chats c ON c.id = m.chat_id
		WHERE m.id = ? AND m.chat_id = ? AND m.sender_id = 'me' AND c.is_group = 1
	)`, msgID, chatID).Scan(&isOutgoingGroup)
	if err != nil || !isOutgoingGroup {
		return false, err
	}
	result, err := s.db.Exec(`INSERT OR IGNORE INTO message_read_receipts
		(message_id, chat_id, participant_jid, read_at) VALUES (?, ?, ?, ?)`, msgID, chatID, participantJID, readAt)
	if err != nil {
		return false, err
	}
	count, err := result.RowsAffected()
	return count > 0, err
}

// GetMessageReceipts returns only receipts WhatsApp actually reported for an
// outgoing message in this group.
func (s *MessageStore) GetMessageReceipts(chatID, msgID string) ([]MessageReceipt, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	rows, err := s.db.Query(`SELECT r.message_id, r.chat_id, r.participant_jid, r.read_at
		FROM message_read_receipts r
		JOIN messages m ON m.id = r.message_id AND m.chat_id = r.chat_id
		JOIN chats c ON c.id = r.chat_id
		WHERE r.chat_id = ? AND r.message_id = ? AND m.sender_id = 'me' AND c.is_group = 1
		ORDER BY r.read_at ASC`, chatID, msgID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	receipts := make([]MessageReceipt, 0)
	for rows.Next() {
		var receipt MessageReceipt
		if err := rows.Scan(&receipt.MessageID, &receipt.ChatID, &receipt.JID, &receipt.ReadAt); err != nil {
			return nil, err
		}
		receipts = append(receipts, receipt)
	}
	return receipts, rows.Err()
}

func (s *MessageStore) Close() error {
	return s.db.Close()
}

func (s *MessageStore) GetContactName(jid string) (string, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	var name string
	err := s.db.QueryRow(`SELECT name FROM contacts
		WHERE (jid = ? OR jid IN (SELECT lid FROM lid_mapping WHERE pn_jid = ?)
		OR jid IN (SELECT pn_jid FROM lid_mapping WHERE lid = ?))
		AND trim(COALESCE(name, '')) <> '' AND name <> jid
		ORDER BY CASE WHEN jid = ? THEN 0 ELSE 1 END LIMIT 1`, jid, jid, jid, jid).Scan(&name)
	if err == nil && name != "" {
		return name, nil
	}
	// History-synced messages preserve WhatsApp push names even when the
	// corresponding contact was not added to the local contacts table.
	err = s.db.QueryRow(`
		SELECT sender_name FROM messages
		WHERE (sender_id = ?
		   OR sender_id = COALESCE((SELECT lid FROM lid_mapping WHERE pn_jid = ?), '')
		   OR sender_id = COALESCE((SELECT pn_jid FROM lid_mapping WHERE lid = ?), ''))
		  AND trim(ifnull(sender_name, '')) <> ''
		ORDER BY timestamp DESC LIMIT 1
	`, jid, jid, jid).Scan(&name)
	return name, err
}

func (s *MessageStore) GetChatMedia(chatID string, limit int, before int64) ([]Message, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	var query string
	var args []interface{}

	baseQuery := `
		WITH linked_chats AS (
			SELECT ? as id
			UNION
			SELECT lid FROM lid_mapping WHERE pn_jid = ?
			UNION
			SELECT pn_jid FROM lid_mapping WHERE lid = ?
		)
		SELECT ` + messageColumns + `
		FROM messages
		WHERE chat_id IN (SELECT id FROM linked_chats WHERE id IS NOT NULL)
		AND deleted_at IS NULL
		AND msg_type IN ('image', 'video', 'gif')
		AND media_url IS NOT NULL
		AND media_url != ''`

	if before > 0 {
		query = baseQuery + ` AND timestamp < ? ORDER BY timestamp DESC, rowid DESC LIMIT ?`
		args = []interface{}{chatID, chatID, chatID, before, limit}
	} else {
		query = baseQuery + ` ORDER BY timestamp DESC, rowid DESC LIMIT ?`
		args = []interface{}{chatID, chatID, chatID, limit}
	}

	rows, err := s.db.Query(query, args...)
	if err != nil {
		return nil, err
	}
	return scanMessages(rows)
}

func (s *MessageStore) GetChatDocs(chatID string, limit int, before int64) ([]Message, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	var query string
	var args []interface{}

	baseQuery := `
		WITH linked_chats AS (
			SELECT ? as id
			UNION
			SELECT lid FROM lid_mapping WHERE pn_jid = ?
			UNION
			SELECT pn_jid FROM lid_mapping WHERE lid = ?
		)
		SELECT ` + messageColumns + `
		FROM messages
		WHERE chat_id IN (SELECT id FROM linked_chats WHERE id IS NOT NULL)
		AND deleted_at IS NULL
		AND msg_type = 'document'
		AND media_url IS NOT NULL
		AND media_url != ''`

	if before > 0 {
		query = baseQuery + ` AND timestamp < ? ORDER BY timestamp DESC, rowid DESC LIMIT ?`
		args = []interface{}{chatID, chatID, chatID, before, limit}
	} else {
		query = baseQuery + ` ORDER BY timestamp DESC, rowid DESC LIMIT ?`
		args = []interface{}{chatID, chatID, chatID, limit}
	}

	rows, err := s.db.Query(query, args...)
	if err != nil {
		return nil, err
	}
	return scanMessages(rows)
}

func (s *MessageStore) GetChatLinks(chatID string, limit int, before int64) ([]Message, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	var query string
	var args []interface{}

	baseQuery := `
		WITH linked_chats AS (
			SELECT ? as id
			UNION
			SELECT lid FROM lid_mapping WHERE pn_jid = ?
			UNION
			SELECT pn_jid FROM lid_mapping WHERE lid = ?
		)
		SELECT ` + messageColumns + `
		FROM messages
		WHERE chat_id IN (SELECT id FROM linked_chats WHERE id IS NOT NULL)
		AND deleted_at IS NULL
		AND msg_type = 'text'
		AND content LIKE '%http%://%'
	`
	if before > 0 {
		query = baseQuery + ` AND timestamp < ? ORDER BY timestamp DESC, rowid DESC LIMIT ?`
		args = []interface{}{chatID, chatID, chatID, before, limit}
	} else {
		query = baseQuery + ` ORDER BY timestamp DESC, rowid DESC LIMIT ?`
		args = []interface{}{chatID, chatID, chatID, limit}
	}

	rows, err := s.db.Query(query, args...)
	if err != nil {
		return nil, err
	}
	return scanMessages(rows)
}

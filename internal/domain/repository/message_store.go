package repository

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"strings"
	"sync"
	"time"

	_ "github.com/mattn/go-sqlite3"
	waWeb "go.mau.fi/whatsmeow/proto/waWeb"
	"google.golang.org/protobuf/proto"
)

type MessageStore struct {
	db *sql.DB
	mu sync.RWMutex
}

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
type MessageExtra struct {
	Poll        *PollMeta        `json:"poll,omitempty"`
	Location    *LocationMeta    `json:"location,omitempty"`
	Contact     *ContactMeta     `json:"contact,omitempty"`
	ViewOnce    *ViewOnceMeta    `json:"viewOnce,omitempty"`
	GIF         bool             `json:"gif,omitempty"`
	LinkPreview *LinkPreviewMeta `json:"linkPreview,omitempty"`
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
	ID         string `json:"id"`
	Name       string `json:"name"`
	Avatar     string `json:"avatar"`
	LastMsg    string `json:"lastMsg"`
	LastTime   int64  `json:"lastTime"`
	Unread     int    `json:"unread"`
	IsActive   bool   `json:"isActive"`
	IsGroup    bool   `json:"isGroup"`
	Archived   bool   `json:"archived"`
	PinnedAt   *int64 `json:"pinnedAt"`
	MuteMode   string `json:"muteMode"`
	MutedUntil *int64 `json:"mutedUntil"`
}

type Contact struct {
	ID     string `json:"id"`
	Name   string `json:"name"`
	JID    string `json:"jid"`
	Avatar string `json:"avatar"`
}

func NewMessageStore(dbPath string) (*MessageStore, error) {
	db, err := sql.Open("sqlite3", dbPath)
	if err != nil {
		return nil, fmt.Errorf("failed to open database: %w", err)
	}

	store := &MessageStore{db: db}

	if err := store.init(); err != nil {
		return nil, fmt.Errorf("failed to initialize database: %w", err)
	}

	return store, nil
}

func (s *MessageStore) init() error {
	s.mu.Lock()
	defer s.mu.Unlock()

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
		if json.Unmarshal([]byte(extraJSON), &extra) == nil && extra != (MessageExtra{}) {
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
		WHERE chat_id IN (SELECT id FROM linked_chats WHERE id IS NOT NULL)`

	if before > 0 {
		query = baseQuery + ` AND timestamp < ? ORDER BY timestamp DESC LIMIT ?`
		args = []interface{}{chatID, chatID, chatID, before, limit}
	} else if after > 0 {
		query = baseQuery + ` AND timestamp > ? ORDER BY timestamp ASC LIMIT ?`
		args = []interface{}{chatID, chatID, chatID, after, limit}
	} else {
		query = baseQuery + ` ORDER BY timestamp DESC LIMIT ?`
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
                SELECT id, name, jid, ifnull(avatar, '') as avatar
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
                INSERT OR REPLACE INTO contacts (id, name, jid, avatar, updated_at)
                VALUES (?, ?, ?, ?, ?)
        `, contact.ID, contact.Name, contact.JID, contact.Avatar, time.Now().Unix())

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
	s.mu.Lock()
	defer s.mu.Unlock()

	_, err := s.db.Exec(`
                INSERT OR REPLACE INTO favorite_stickers (id, media_url, is_animated)
                VALUES (?, ?, ?)
        `, id, mediaURL, func() int {
		if isAnimated {
			return 1
		}
		return 0
	}())

	return err
}

func (s *MessageStore) GetFavoriteStickers() ([]map[string]interface{}, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	rows, err := s.db.Query("SELECT id, media_url, is_animated FROM favorite_stickers ORDER BY created_at DESC")
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var stickers []map[string]interface{}
	for rows.Next() {
		var id, url string
		var isAnim int
		if err := rows.Scan(&id, &url, &isAnim); err != nil {
			return nil, err
		}
		stickers = append(stickers, map[string]interface{}{
			"id":         id,
			"mediaUrl":   url,
			"isAnimated": isAnim == 1,
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

func (s *MessageStore) Close() error {
	return s.db.Close()
}

func (s *MessageStore) GetContactName(jid string) (string, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	var name string
	err := s.db.QueryRow("SELECT name FROM contacts WHERE jid = ?", jid).Scan(&name)
	if err != nil {
		return "", err
	}
	return name, nil
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
		AND msg_type IN ('image', 'video', 'gif')
		AND media_url IS NOT NULL
		AND media_url != ''`

	if before > 0 {
		query = baseQuery + ` AND timestamp < ? ORDER BY timestamp DESC LIMIT ?`
		args = []interface{}{chatID, chatID, chatID, before, limit}
	} else {
		query = baseQuery + ` ORDER BY timestamp DESC LIMIT ?`
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
		AND msg_type = 'document'
		AND media_url IS NOT NULL
		AND media_url != ''`

	if before > 0 {
		query = baseQuery + ` AND timestamp < ? ORDER BY timestamp DESC LIMIT ?`
		args = []interface{}{chatID, chatID, chatID, before, limit}
	} else {
		query = baseQuery + ` ORDER BY timestamp DESC LIMIT ?`
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
		AND msg_type = 'text'
		AND content LIKE '%http%://%'
	`
	if before > 0 {
		query = baseQuery + ` AND timestamp < ? ORDER BY timestamp DESC LIMIT ?`
		args = []interface{}{chatID, chatID, chatID, before, limit}
	} else {
		query = baseQuery + ` ORDER BY timestamp DESC LIMIT ?`
		args = []interface{}{chatID, chatID, chatID, limit}
	}

	rows, err := s.db.Query(query, args...)
	if err != nil {
		return nil, err
	}
	return scanMessages(rows)
}

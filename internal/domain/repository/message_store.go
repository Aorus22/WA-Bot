package repository

import (
	"database/sql"
	"fmt"
	"strings"
	"sync"
	"time"

	_ "github.com/mattn/go-sqlite3"
)
type MessageStore struct {
	db *sql.DB
	mu sync.RWMutex
}

type Message struct {
	ID          string `json:"id"`
	ChatID      string `json:"chatId"`
	From        string `json:"from"`
	To          string `json:"to"`
	Content     string `json:"content"`
	Timestamp   int64  `json:"timestamp"`
	Status      string `json:"status"`
	Type        string `json:"type"`
	MediaURL    string `json:"mediaUrl,omitempty"`
	IsAutomatic bool   `json:"isAutomatic"`
	SenderName  string `json:"senderName,omitempty"`
	ChatName    string `json:"chatName,omitempty"`
	ReplyToID   string `json:"replyToId,omitempty"`
}

type Chat struct {
	ID       string `json:"id"`
	Name     string `json:"name"`
	Avatar   string `json:"avatar"`
	LastMsg  string `json:"lastMsg"`
	LastTime int64  `json:"lastTime"`
	Unread   int    `json:"unread"`
	IsActive bool   `json:"isActive"`
	IsGroup  bool   `json:"isGroup"`
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
		`CREATE INDEX IF NOT EXISTS idx_messages_chat_id ON messages(chat_id)`,
		`CREATE INDEX IF NOT EXISTS idx_messages_timestamp ON messages(timestamp DESC)`,
		`CREATE TRIGGER IF NOT EXISTS update_chat_timestamp
                        AFTER INSERT ON messages
                        BEGIN
                                UPDATE chats SET last_msg = NEW.content, last_time = NEW.timestamp, updated_at = strftime('%s', 'now')
                                WHERE id = NEW.chat_id;
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
	_, _ = s.db.Exec("UPDATE chats SET unread = 0 WHERE unread IS NULL")

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
	_, err = tx.Exec(`
                INSERT INTO messages (id, chat_id, sender_id, receiver_id, content, timestamp, status, msg_type, media_url, is_automatic, sender_name, metadata)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        `, msg.ID, msg.ChatID, msg.From, msg.To, msg.Content, msg.Timestamp, msg.Status, msg.Type, msg.MediaURL, func() int {
		if msg.IsAutomatic {
			return 1
		}
		return 0
	}(), msg.SenderName, msg.ReplyToID)
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
		SELECT id, chat_id, sender_id, receiver_id, content, timestamp, status, msg_type, ifnull(media_url, '') as media_url, is_automatic, ifnull(sender_name, '') as sender_name, ifnull(metadata, '') as reply_to_id
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
	defer rows.Close()

	var messages []Message
	for rows.Next() {
		var msg Message
		var isAuto int
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
		)
		if err != nil {
			return nil, err
		}
		msg.IsAutomatic = isAuto == 1
		messages = append(messages, msg)
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
		SELECT id, chat_id, sender_id, receiver_id, content, timestamp, status, msg_type, ifnull(media_url, '') as media_url, is_automatic, ifnull(sender_name, '') as sender_name, ifnull(metadata, '') as reply_to_id
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
	defer rows.Close()

	var messages []Message
	for rows.Next() {
		var msg Message
		var isAuto int
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
		)
		if err != nil {
			return nil, err
		}
		msg.IsAutomatic = isAuto == 1
		messages = append(messages, msg)
	}

	return messages, nil
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
	var isAuto int
	err = s.db.QueryRow(`
		SELECT id, chat_id, sender_id, receiver_id, content, timestamp, status, msg_type, ifnull(media_url, '') as media_url, is_automatic, ifnull(sender_name, '') as sender_name, ifnull(metadata, '') as reply_to_id
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
	)
	if err != nil {
		return nil, err
	}
	targetMsg.IsAutomatic = isAuto == 1

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
                        c.id as original_id
                    FROM chats c
                ),
                grouped_chats AS (
                    SELECT 
                        target_id, 
                        COALESCE(MAX(CASE WHEN name != target_id AND name != '' AND name NOT LIKE '%@%' THEN name END), MAX(name)) as name, 
                        COALESCE(MAX(CASE WHEN avatar != '' THEN avatar END), '') as avatar,
                        MAX(last_time) as last_time,
                        SUM(unread) as unread,
                        MAX(is_active) as is_active,
                        MAX(is_group) as is_group
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
                    g.is_group
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
		var isActive, isGroup int
		err := rows.Scan(
			&chat.ID,
			&chat.Name,
			&chat.Avatar,
			&chat.LastMsg,
			&chat.LastTime,
			&chat.Unread,
			&isActive,
			&isGroup,
		)
		if err != nil {
			return nil, err
		}
		chat.IsActive = isActive == 1
		chat.IsGroup = isGroup == 1
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

	_, err := s.db.Exec(`
                UPDATE chats SET unread = 0, updated_at = ? WHERE id = ?
        `, time.Now().Unix(), chatID)

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
		SELECT id, chat_id, sender_id, receiver_id, content, timestamp, status, msg_type, ifnull(media_url, '') as media_url, is_automatic, ifnull(sender_name, '') as sender_name, ifnull(metadata, '') as reply_to_id
		FROM messages
		WHERE chat_id IN (SELECT id FROM linked_chats WHERE id IS NOT NULL)
		AND msg_type IN ('image', 'video')
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
	defer rows.Close()

	var messages []Message
	for rows.Next() {
		var msg Message
		var isAuto int
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
		)
		if err != nil {
			return nil, err
		}
		msg.IsAutomatic = isAuto == 1
		messages = append(messages, msg)
	}

	return messages, nil
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
		SELECT id, chat_id, sender_id, receiver_id, content, timestamp, status, msg_type, ifnull(media_url, '') as media_url, is_automatic, ifnull(sender_name, '') as sender_name, ifnull(metadata, '') as reply_to_id
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
	defer rows.Close()

	var messages []Message
	for rows.Next() {
		var msg Message
		var isAuto int
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
		)
		if err != nil {
			return nil, err
		}
		msg.IsAutomatic = isAuto == 1
		messages = append(messages, msg)
	}

	return messages, nil
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
		SELECT id, chat_id, sender_id, receiver_id, content, timestamp, status, msg_type, ifnull(media_url, '') as media_url, is_automatic, ifnull(sender_name, '') as sender_name, ifnull(metadata, '') as reply_to_id
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
	defer rows.Close()

	var messages []Message
	for rows.Next() {
		var msg Message
		var isAuto int
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
		)
		if err != nil {
			return nil, err
		}
		msg.IsAutomatic = isAuto == 1
		messages = append(messages, msg)
	}

	return messages, nil
}

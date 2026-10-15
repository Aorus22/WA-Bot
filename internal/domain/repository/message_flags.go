package repository

import (
	"time"
)

// Default and allowed lifetimes of an in-chat pinned message, in seconds.
const (
	PinDuration24h = 24 * 60 * 60
	PinDuration7d  = 7 * 24 * 60 * 60
	PinDuration30d = 30 * 24 * 60 * 60
)

// NormalizePinDuration maps anything WhatsApp does not offer to the 7 day default.
func NormalizePinDuration(secs int64) int64 {
	switch secs {
	case PinDuration24h, PinDuration7d, PinDuration30d:
		return secs
	}
	return PinDuration7d
}

// StarredMessage is a starred message plus when it was starred (Unix ms).
type StarredMessage struct {
	Message
	StarredAt int64 `json:"starredAt"`
}

// ChatPin is an active pinned message of a chat. Message is nil when the
// pinned message is not stored locally.
type ChatPin struct {
	MessageID string   `json:"messageId"`
	ChatID    string   `json:"chatId"`
	PinnedBy  string   `json:"pinnedBy"`
	PinnedAt  int64    `json:"pinnedAt"`
	ExpiresAt int64    `json:"expiresAt"`
	Message   *Message `json:"message,omitempty"`
}

// linkedChatsCTE expands a chat id to every alias (LID / phone number) it is stored under.
const linkedChatsCTE = `WITH linked_chats AS (
			SELECT ? as id
			UNION
			SELECT lid FROM lid_mapping WHERE pn_jid = ?
			UNION
			SELECT pn_jid FROM lid_mapping WHERE lid = ?
		)`

// SetStarred flags or unflags a message. The row is kept either way so the
// star history is never lost.
func (s *MessageStore) SetStarred(chatID, msgID string, starred bool, atMs int64) error {
	if atMs <= 0 {
		atMs = time.Now().UnixMilli()
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	_, err := s.db.Exec(`INSERT INTO message_stars (message_id, chat_id, starred, starred_at) VALUES (?, ?, ?, ?)
		ON CONFLICT(message_id) DO UPDATE SET starred = excluded.starred, starred_at = excluded.starred_at`,
		msgID, chatID, boolInt(starred), atMs)
	return err
}

// GetStarredMessages lists starred messages, most recently starred first.
// chatID may be empty for all chats; before pages by starred_at.
func (s *MessageStore) GetStarredMessages(chatID string, limit int, before int64) ([]StarredMessage, error) {
	if limit <= 0 || limit > 500 {
		limit = 100
	}
	s.mu.RLock()
	defer s.mu.RUnlock()

	query := `SELECT ` + messageColumns + `, (SELECT starred_at FROM message_stars WHERE message_id = messages.id) AS starred_at
		FROM messages
		WHERE deleted_at IS NULL
		AND id IN (SELECT message_id FROM message_stars WHERE starred = 1`
	var args []any
	if before > 0 {
		query += ` AND starred_at < ?`
		args = append(args, before)
	}
	query += `) `
	if chatID != "" {
		query = linkedChatsCTE + query + ` AND chat_id IN (SELECT id FROM linked_chats WHERE id IS NOT NULL)`
		args = append([]any{chatID, chatID, chatID}, args...)
	}
	query += ` ORDER BY starred_at DESC LIMIT ?`
	args = append(args, limit)

	rows, err := s.db.Query(query, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	out := []StarredMessage{}
	for rows.Next() {
		var at int64
		msg, err := scanMessageRow(rows, &at)
		if err != nil {
			return nil, err
		}
		out = append(out, StarredMessage{Message: msg, StarredAt: at})
	}
	return out, rows.Err()
}

// PinMessage records an in-chat pin. A re-pin of the same message refreshes it.
func (s *MessageStore) PinMessage(chatID, msgID, pinnedBy string, pinnedAtMs, expiresAtMs int64) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	_, err := s.db.Exec(`INSERT INTO chat_pins (message_id, chat_id, pinned_by, pinned_at, expires_at, unpinned_at)
		VALUES (?, ?, ?, ?, ?, NULL)
		ON CONFLICT(message_id) DO UPDATE SET chat_id = excluded.chat_id, pinned_by = excluded.pinned_by,
			pinned_at = excluded.pinned_at, expires_at = excluded.expires_at, unpinned_at = NULL`,
		msgID, chatID, pinnedBy, pinnedAtMs, expiresAtMs)
	return err
}

// UnpinMessage flags a pin as removed; the row is kept.
func (s *MessageStore) UnpinMessage(msgID string, atMs int64) error {
	if atMs <= 0 {
		atMs = time.Now().UnixMilli()
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	_, err := s.db.Exec(`UPDATE chat_pins SET unpinned_at = ? WHERE message_id = ? AND unpinned_at IS NULL`, atMs, msgID)
	return err
}

// GetActivePins returns the chat's pins that are neither removed nor expired,
// newest first, with their messages attached.
func (s *MessageStore) GetActivePins(chatID string) ([]ChatPin, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	rows, err := s.db.Query(linkedChatsCTE+`
		SELECT message_id, chat_id, ifnull(pinned_by, ''), pinned_at, expires_at
		FROM chat_pins
		WHERE chat_id IN (SELECT id FROM linked_chats WHERE id IS NOT NULL)
		AND unpinned_at IS NULL AND (expires_at = 0 OR expires_at > ?)
		ORDER BY pinned_at DESC`, chatID, chatID, chatID, time.Now().UnixMilli())
	if err != nil {
		return nil, err
	}
	pins := []ChatPin{}
	for rows.Next() {
		var p ChatPin
		if err := rows.Scan(&p.MessageID, &p.ChatID, &p.PinnedBy, &p.PinnedAt, &p.ExpiresAt); err != nil {
			rows.Close()
			return nil, err
		}
		pins = append(pins, p)
	}
	rows.Close()
	if err := rows.Err(); err != nil {
		return nil, err
	}

	for i := range pins {
		msg, err := scanMessageRow(s.db.QueryRow(`SELECT `+messageColumns+` FROM messages WHERE id = ? AND deleted_at IS NULL`, pins[i].MessageID))
		if err == nil {
			pins[i].Message = &msg
		}
	}
	return pins, nil
}

// SetChatDisappearing stores the chat's disappearing timer (0 = off) on every
// alias of the chat. It reports whether the stored value changed.
func (s *MessageStore) SetChatDisappearing(chatID string, secs uint32) (bool, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	res, err := s.db.Exec(`UPDATE chats SET ephemeral_secs = ?
		WHERE (id = ? OR id IN (SELECT lid FROM lid_mapping WHERE pn_jid = ?)
		OR id IN (SELECT pn_jid FROM lid_mapping WHERE lid = ?))
		AND COALESCE(ephemeral_secs, 0) != ?`, secs, chatID, chatID, chatID, secs)
	if err != nil {
		return false, err
	}
	n, _ := res.RowsAffected()
	return n > 0, nil
}

// GetChatDisappearing returns the chat's disappearing timer in seconds (0 = off).
func (s *MessageStore) GetChatDisappearing(chatID string) uint32 {
	s.mu.RLock()
	defer s.mu.RUnlock()
	var secs uint32
	_ = s.db.QueryRow(`SELECT COALESCE(MAX(ephemeral_secs), 0) FROM chats
		WHERE id = ? OR id IN (SELECT lid FROM lid_mapping WHERE pn_jid = ?)
		OR id IN (SELECT pn_jid FROM lid_mapping WHERE lid = ?)`, chatID, chatID, chatID).Scan(&secs)
	return secs
}

// NewStatusReplyMeta snapshots a status for the quote card of a reply to it.
func NewStatusReplyMeta(e *StatusEntry) *StatusReplyMeta {
	return &StatusReplyMeta{
		StatusID: e.ID,
		Sender:   e.Sender,
		Type:     e.Type,
		Content:  e.Content,
		MediaURL: e.MediaURL,
	}
}

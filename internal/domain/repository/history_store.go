package repository

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"strings"
	"time"
)

// HistoryConversation is metadata kept outside the visible chat table until a
// user explicitly starts a history sync.
type HistoryConversation struct {
	ChatID      string
	Name        string
	UnreadCount int
	Archived    bool
	PinnedAt    int64
	MuteEnd     int64
}

// StagedHistoryMessage is a raw WhatsApp web message waiting to be imported.
type StagedHistoryMessage struct {
	ChatID    string
	MessageID string
	Timestamp int64
	Raw       []byte
}

// HistoryImportMessage is the additive projection written into messages.
type HistoryImportMessage struct {
	Message Message
	Raw     []byte
}

type HistoryTarget struct {
	ChatID  string
	Pending int
	// Exhausted: an earlier sync already reached the start of this chat's
	// history (the phone had nothing older to give).
	Exhausted bool
}

type MessageAnchor struct {
	ChatID    string
	MessageID string
	Timestamp int64
	IsFromMe  bool
	SenderID  string
}

type HistorySyncError struct {
	ChatID  string `json:"chatId,omitempty"`
	Message string `json:"message"`
}

// HistorySyncOptions says how far back a sync goes.
type HistorySyncOptions struct {
	// PerChat caps the older messages added per chat (0 = no cap).
	PerChat int `json:"perChat"`
	// SinceMs stops going back once messages are older than this unix-ms time (0 = no limit).
	SinceMs int64 `json:"sinceMs"`
	// IncludeExhausted also retries chats an earlier sync found to be at the
	// start of their history.
	IncludeExhausted bool `json:"includeExhausted"`
	// All is "everything the phone will give": no cap, no time limit.
	All bool `json:"all"`
}

// HistorySyncCurrent is a chat being synced right now.
type HistorySyncCurrent struct {
	ChatID string `json:"chatId"`
	Name   string `json:"name"`
	Added  int    `json:"added"`
	// Phase is "importing" or "asking-phone".
	Phase string `json:"phase"`
}

type HistorySyncStatus struct {
	State           string             `json:"state"`
	PendingChats    int                `json:"pendingChats"`
	PendingMessages int                `json:"pendingMessages"`
	ChatsTotal      int                `json:"chatsTotal"`
	ChatsProcessed  int                `json:"chatsProcessed"`
	MessagesAdded   int                `json:"messagesAdded"`
	Errors          []HistorySyncError `json:"errors"`
	StartedAt       *int64             `json:"startedAt"`
	FinishedAt      *int64             `json:"finishedAt"`
	LastRunAt       *int64             `json:"lastRunAt"`

	Options        HistorySyncOptions `json:"options"`
	ChatsWithNew   int                `json:"chatsWithNew"`
	ChatsExhausted int                `json:"chatsExhausted"`
	ChatsSkipped   int                `json:"chatsSkipped"`
	Timeouts       int                `json:"timeouts"`

	// Live detail of the running sync; not persisted.
	Current []HistorySyncCurrent `json:"current"`

	// Resumable: the job stopped (stopped by the user, interrupted by a restart
	// or a lost connection) with chats still to do, and "resume" continues it.
	Resumable bool `json:"resumable"`
	// ElapsedMs is the time the job actually ran, not counting the time it was
	// stopped. EtaMs is a rough time left, nil until there is enough to go on.
	ElapsedMs int64  `json:"elapsedMs"`
	EtaMs     *int64 `json:"etaMs"`

	// ChatsFinished is how many chats the next sync skips unless asked to retry
	// them (include-finished option).
	ChatsFinished int `json:"chatsFinished"`

	// ActiveMs is ElapsedMs as of the last save.
	ActiveMs int64 `json:"-"`
}

// HistoryJobTotals is the progress of the current job, summed from its chats.
type HistoryJobTotals struct {
	Chats     int
	Done      int
	Added     int
	WithNew   int
	Exhausted int
	Timeouts  int
}

type ChatState struct {
	ChatID     string `json:"chatId"`
	Archived   bool   `json:"archived"`
	PinnedAt   *int64 `json:"pinnedAt"`
	MuteMode   string `json:"muteMode"`
	MutedUntil *int64 `json:"mutedUntil"`
}

func (s *MessageStore) HasHistoryNotification(id string) bool {
	s.mu.RLock()
	defer s.mu.RUnlock()
	var found int
	_ = s.db.QueryRow("SELECT 1 FROM history_sync_notifications WHERE id = ?", id).Scan(&found)
	return found == 1
}

func (s *MessageStore) SaveHistoryNotification(id string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	_, err := s.db.Exec("INSERT OR IGNORE INTO history_sync_notifications (id, processed_at) VALUES (?, ?)", id, time.Now().UnixMilli())
	return err
}

func (s *MessageStore) StageHistoryConversation(meta HistoryConversation, messages []StagedHistoryMessage) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	tx, err := s.db.Begin()
	if err != nil {
		return err
	}
	defer tx.Rollback()

	_, err = tx.Exec(`INSERT INTO history_staged_conversations
		(chat_id, name, unread_count, archived, pinned_at, mute_end, updated_at)
		VALUES (?, ?, ?, ?, NULLIF(?, 0), ?, ?)
		ON CONFLICT(chat_id) DO UPDATE SET
			name = CASE WHEN excluded.name != '' THEN excluded.name ELSE history_staged_conversations.name END,
			unread_count = MAX(history_staged_conversations.unread_count, excluded.unread_count),
			archived = excluded.archived,
			pinned_at = excluded.pinned_at,
			mute_end = excluded.mute_end,
			updated_at = excluded.updated_at`,
		meta.ChatID, meta.Name, meta.UnreadCount, boolInt(meta.Archived), meta.PinnedAt, meta.MuteEnd, time.Now().UnixMilli())
	if err != nil {
		return err
	}

	for _, item := range messages {
		if item.MessageID == "" || len(item.Raw) == 0 {
			continue
		}
		_, err = tx.Exec(`INSERT OR IGNORE INTO history_staged_messages
			(chat_id, message_id, timestamp, raw_message) VALUES (?, ?, ?, ?)`,
			meta.ChatID, item.MessageID, item.Timestamp, item.Raw)
		if err != nil {
			return err
		}
	}

	return tx.Commit()
}

func (s *MessageStore) HistoryTargets() ([]HistoryTarget, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	rows, err := s.db.Query(`WITH targets AS (
		SELECT id AS chat_id FROM chats
		UNION
		SELECT DISTINCT chat_id FROM history_staged_messages WHERE imported = 0
	)
	SELECT t.chat_id, COALESCE((SELECT COUNT(*) FROM history_staged_messages h
		WHERE h.chat_id = t.chat_id AND h.imported = 0), 0) AS pending,
		EXISTS(SELECT 1 FROM history_chat_progress p WHERE p.chat_id = t.chat_id AND p.exhausted_at IS NOT NULL) AS exhausted
	FROM targets t
	WHERE t.chat_id != 'status@broadcast'
	ORDER BY pending DESC, chat_id`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []HistoryTarget
	for rows.Next() {
		var target HistoryTarget
		var exhausted int
		if err := rows.Scan(&target.ChatID, &target.Pending, &exhausted); err != nil {
			return nil, err
		}
		target.Exhausted = exhausted == 1
		out = append(out, target)
	}
	return out, rows.Err()
}

// FinishedChatCount is how many chats a sync skips unless asked to retry them:
// the ones already at the start of their history (or that the phone never
// answered for) with nothing waiting to be imported.
func (s *MessageStore) FinishedChatCount() int {
	s.mu.RLock()
	defer s.mu.RUnlock()
	var n int
	_ = s.db.QueryRow(`SELECT COUNT(*) FROM history_chat_progress p
		WHERE p.exhausted_at IS NOT NULL AND p.chat_id != 'status@broadcast'
		AND (p.chat_id IN (SELECT id FROM chats)
			OR p.chat_id IN (SELECT DISTINCT chat_id FROM history_staged_messages))
		AND NOT EXISTS (SELECT 1 FROM history_staged_messages h WHERE h.chat_id = p.chat_id AND h.imported = 0)`).Scan(&n)
	return n
}

// MarkHistoryExhausted records that the phone had nothing older for chatID.
func (s *MessageStore) MarkHistoryExhausted(chatID string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	_, err := s.db.Exec(`INSERT INTO history_chat_progress (chat_id, exhausted_at) VALUES (?, ?)
		ON CONFLICT(chat_id) DO UPDATE SET exhausted_at = excluded.exhausted_at`, chatID, time.Now().UnixMilli())
	return err
}

// HistoryChatName is the display name of a chat for progress messages.
func (s *MessageStore) HistoryChatName(chatID string) string {
	s.mu.RLock()
	defer s.mu.RUnlock()
	var name sql.NullString
	_ = s.db.QueryRow("SELECT name FROM chats WHERE id = ?", chatID).Scan(&name)
	if name.Valid && strings.TrimSpace(name.String) != "" {
		return name.String
	}
	var staged sql.NullString
	_ = s.db.QueryRow("SELECT name FROM history_staged_conversations WHERE chat_id = ?", chatID).Scan(&staged)
	if staged.Valid && strings.TrimSpace(staged.String) != "" {
		return staged.String
	}
	return chatID
}

// ChatMessageStats is how many messages a chat has and the timestamp of its
// oldest one (0 when it has none).
func (s *MessageStore) ChatMessageStats(chatID string) (count int, oldest int64) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	var min sql.NullInt64
	_ = s.db.QueryRow("SELECT COUNT(*), MIN(timestamp) FROM messages WHERE chat_id = ?", chatID).Scan(&count, &min)
	return count, min.Int64
}

func (s *MessageStore) PendingHistory(chatID string, limit int) ([]StagedHistoryMessage, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	rows, err := s.db.Query(`SELECT chat_id, message_id, timestamp, raw_message
		FROM history_staged_messages WHERE chat_id = ? AND imported = 0
		ORDER BY timestamp DESC LIMIT ?`, chatID, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []StagedHistoryMessage
	for rows.Next() {
		var item StagedHistoryMessage
		if err := rows.Scan(&item.ChatID, &item.MessageID, &item.Timestamp, &item.Raw); err != nil {
			return nil, err
		}
		out = append(out, item)
	}
	return out, rows.Err()
}

func (s *MessageStore) HistoryConversationUpdatedAt(chatID string) int64 {
	s.mu.RLock()
	defer s.mu.RUnlock()
	var updatedAt int64
	_ = s.db.QueryRow("SELECT updated_at FROM history_staged_conversations WHERE chat_id = ?", chatID).Scan(&updatedAt)
	return updatedAt
}

// ImportHistoryBatch only inserts missing messages and marks every staged row
// as consumed. Existing message and chat content is never overwritten.
func (s *MessageStore) ImportHistoryBatch(chatID string, staged []StagedHistoryMessage, imports []HistoryImportMessage) (int, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	tx, err := s.db.Begin()
	if err != nil {
		return 0, err
	}
	defer tx.Rollback()

	var meta HistoryConversation
	meta.ChatID = chatID
	var archived int
	var pinned sql.NullInt64
	err = tx.QueryRow(`SELECT name, unread_count, archived, pinned_at, mute_end
		FROM history_staged_conversations WHERE chat_id = ?`, chatID).
		Scan(&meta.Name, &meta.UnreadCount, &archived, &pinned, &meta.MuteEnd)
	if err != nil && err != sql.ErrNoRows {
		return 0, err
	}
	meta.Archived = archived == 1
	if pinned.Valid {
		meta.PinnedAt = pinned.Int64
	}

	var exists bool
	if err := tx.QueryRow("SELECT COUNT(*) > 0 FROM chats WHERE id = ?", chatID).Scan(&exists); err != nil {
		return 0, err
	}
	if !exists {
		name := strings.TrimSpace(meta.Name)
		if name == "" {
			name = chatID
		}
		muteMode := "off"
		var mutedUntil any
		if meta.MuteEnd < 0 {
			muteMode = "forever"
		} else if meta.MuteEnd > time.Now().UnixMilli() {
			muteMode = "until"
			mutedUntil = meta.MuteEnd
		}
		_, err = tx.Exec(`INSERT INTO chats
			(id, name, last_msg, last_time, unread, is_active, is_group, archived, pinned_at, mute_mode, muted_until)
			VALUES (?, ?, '', 0, ?, 1, ?, ?, NULLIF(?, 0), ?, ?)`,
			chatID, name, meta.UnreadCount, boolInt(strings.HasSuffix(chatID, "@g.us")),
			boolInt(meta.Archived), meta.PinnedAt, muteMode, mutedUntil)
		if err != nil {
			return 0, err
		}
	}

	added := 0
	for _, item := range imports {
		msg := item.Message
		result, err := tx.Exec(`INSERT OR IGNORE INTO messages
			(id, chat_id, sender_id, receiver_id, content, timestamp, status, msg_type,
			 media_url, is_automatic, sender_name, metadata, raw_message)
			VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
			msg.ID, msg.ChatID, msg.From, msg.To, msg.Content, msg.Timestamp, msg.Status,
			msg.Type, msg.MediaURL, boolInt(msg.IsAutomatic), msg.SenderName, msg.ReplyToID, item.Raw)
		if err != nil {
			return 0, err
		}
		if n, _ := result.RowsAffected(); n > 0 {
			added += int(n)
		}
	}
	for _, item := range staged {
		if _, err := tx.Exec(`UPDATE history_staged_messages SET imported = 1
			WHERE chat_id = ? AND message_id = ?`, chatID, item.MessageID); err != nil {
			return 0, err
		}
	}
	if err := tx.Commit(); err != nil {
		return 0, err
	}
	return added, nil
}

func (s *MessageStore) OldestMessageAnchor(chatID string) (*MessageAnchor, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	var anchor MessageAnchor
	var from string
	err := s.db.QueryRow(`SELECT chat_id, id, timestamp, sender_id FROM messages
		WHERE chat_id = ? ORDER BY timestamp ASC LIMIT 1`, chatID).
		Scan(&anchor.ChatID, &anchor.MessageID, &anchor.Timestamp, &from)
	if err != nil {
		return nil, err
	}
	anchor.IsFromMe = from == "me"
	anchor.SenderID = from
	return &anchor, nil
}

func (s *MessageStore) LatestMessageAnchor(chatID string) (*MessageAnchor, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	var anchor MessageAnchor
	var from string
	err := s.db.QueryRow(`SELECT chat_id, id, timestamp, sender_id FROM messages
		WHERE chat_id = ? ORDER BY timestamp DESC LIMIT 1`, chatID).
		Scan(&anchor.ChatID, &anchor.MessageID, &anchor.Timestamp, &from)
	if err != nil {
		return nil, err
	}
	anchor.IsFromMe = from == "me"
	anchor.SenderID = from
	return &anchor, nil
}

// ChatAliases returns every id the chat is stored under, LID form first: the
// phone addresses a migrated one-to-one chat by its LID even though the app
// lists it under the phone number, and app-state changes only land on the form
// the phone knows.
func (s *MessageStore) ChatAliases(chatID string) []string {
	s.mu.RLock()
	defer s.mu.RUnlock()
	rows, err := s.db.Query(`SELECT id FROM chats WHERE deleted_at IS NULL AND (id = ?
		OR id IN (SELECT lid FROM lid_mapping WHERE pn_jid = ?)
		OR id IN (SELECT pn_jid FROM lid_mapping WHERE lid = ?))
		ORDER BY (id LIKE '%@lid') DESC, id`, chatID, chatID, chatID)
	if err != nil {
		return nil
	}
	defer rows.Close()
	var ids []string
	for rows.Next() {
		var id string
		if rows.Scan(&id) == nil {
			ids = append(ids, id)
		}
	}
	return ids
}

// LatestMessageAnchorAmong is the newest message across all of a chat's ids.
func (s *MessageStore) LatestMessageAnchorAmong(chatIDs []string) (*MessageAnchor, error) {
	var newest *MessageAnchor
	var lastErr error = sql.ErrNoRows
	for _, id := range chatIDs {
		anchor, err := s.LatestMessageAnchor(id)
		if err != nil {
			lastErr = err
			continue
		}
		if newest == nil || anchor.Timestamp > newest.Timestamp {
			newest = anchor
		}
	}
	if newest == nil {
		return nil, lastErr
	}
	return newest, nil
}

func (s *MessageStore) PendingHistoryCounts() (int, int, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	var chats, messages int
	err := s.db.QueryRow(`SELECT COUNT(DISTINCT chat_id), COUNT(*)
		FROM history_staged_messages WHERE imported = 0`).Scan(&chats, &messages)
	return chats, messages, err
}

func (s *MessageStore) SetHistorySyncStatus(status HistorySyncStatus) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	errorsJSON, _ := json.Marshal(status.Errors)
	optionsJSON, _ := json.Marshal(status.Options)
	_, err := s.db.Exec(`UPDATE history_sync_runs SET state = ?, chats_total = ?, chats_processed = ?,
		messages_added = ?, errors_json = ?, started_at = ?, finished_at = ?, last_run_at = ?,
		options_json = ?, chats_skipped = ?, active_ms = ? WHERE id = 1`,
		status.State, status.ChatsTotal, status.ChatsProcessed, status.MessagesAdded, string(errorsJSON),
		status.StartedAt, status.FinishedAt, status.LastRunAt,
		string(optionsJSON), status.ChatsSkipped, status.ActiveMs)
	return err
}

func (s *MessageStore) GetHistorySyncStatus() (HistorySyncStatus, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	var status HistorySyncStatus
	var errorsJSON string
	var optionsJSON sql.NullString
	var skipped, active sql.NullInt64
	var started, finished, lastRun sql.NullInt64
	err := s.db.QueryRow(`SELECT state, chats_total, chats_processed, messages_added,
		errors_json, started_at, finished_at, last_run_at, options_json, chats_skipped, active_ms
		FROM history_sync_runs WHERE id = 1`).
		Scan(&status.State, &status.ChatsTotal, &status.ChatsProcessed, &status.MessagesAdded,
			&errorsJSON, &started, &finished, &lastRun, &optionsJSON, &skipped, &active)
	if err != nil {
		return status, err
	}
	_ = json.Unmarshal([]byte(errorsJSON), &status.Errors)
	if status.Errors == nil {
		status.Errors = []HistorySyncError{}
	}
	if optionsJSON.Valid && optionsJSON.String != "" {
		_ = json.Unmarshal([]byte(optionsJSON.String), &status.Options)
	}
	status.ChatsSkipped = int(skipped.Int64)
	status.ActiveMs = active.Int64
	if started.Valid {
		v := started.Int64
		status.StartedAt = &v
	}
	if finished.Valid {
		v := finished.Int64
		status.FinishedAt = &v
	}
	if lastRun.Valid {
		v := lastRun.Int64
		status.LastRunAt = &v
	}
	return status, nil
}

// BeginHistoryJob replaces the job's chat list with chatIDs, all still to do.
func (s *MessageStore) BeginHistoryJob(chatIDs []string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	tx, err := s.db.Begin()
	if err != nil {
		return err
	}
	defer tx.Rollback()
	if _, err := tx.Exec("DELETE FROM history_sync_job_chats"); err != nil {
		return err
	}
	for i, id := range chatIDs {
		if _, err := tx.Exec("INSERT OR IGNORE INTO history_sync_job_chats (chat_id, position) VALUES (?, ?)", id, i); err != nil {
			return err
		}
	}
	return tx.Commit()
}

// PendingHistoryJobChats are the chats of the job not finished yet, in the order
// the job visits them.
func (s *MessageStore) PendingHistoryJobChats() ([]string, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	rows, err := s.db.Query("SELECT chat_id FROM history_sync_job_chats WHERE done = 0 ORDER BY position")
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var ids []string
	for rows.Next() {
		var id string
		if err := rows.Scan(&id); err != nil {
			return nil, err
		}
		ids = append(ids, id)
	}
	return ids, rows.Err()
}

// RecordHistoryJobChat adds the messages imported for chatID to the job. A chat
// that is done is not visited again when the job resumes; outcome says how it
// ended ("", "exhausted" or "timeout").
func (s *MessageStore) RecordHistoryJobChat(chatID string, added int, done bool, outcome string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	_, err := s.db.Exec(`UPDATE history_sync_job_chats
		SET added = added + ?, done = CASE WHEN ? THEN 1 ELSE done END,
		    outcome = CASE WHEN ? THEN ? ELSE outcome END
		WHERE chat_id = ?`, added, done, done, outcome, chatID)
	return err
}

func (s *MessageStore) HistoryJobTotals() (HistoryJobTotals, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	var t HistoryJobTotals
	err := s.db.QueryRow(`SELECT COUNT(*),
		COALESCE(SUM(done), 0), COALESCE(SUM(added), 0),
		COALESCE(SUM(added > 0), 0),
		COALESCE(SUM(outcome IN ('exhausted', 'timeout')), 0),
		COALESCE(SUM(outcome = 'timeout'), 0)
		FROM history_sync_job_chats`).
		Scan(&t.Chats, &t.Done, &t.Added, &t.WithNew, &t.Exhausted, &t.Timeouts)
	return t, err
}

func (s *MessageStore) UpdateChatState(state ChatState) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	_, err := s.db.Exec(`UPDATE chats SET archived = ?, pinned_at = ?, mute_mode = ?, muted_until = ?, updated_at = ?
		WHERE id = ? OR id IN (SELECT lid FROM lid_mapping WHERE pn_jid = ?)
		OR id IN (SELECT pn_jid FROM lid_mapping WHERE lid = ?)`,
		boolInt(state.Archived), state.PinnedAt, state.MuteMode, state.MutedUntil,
		time.Now().Unix(), state.ChatID, state.ChatID, state.ChatID)
	return err
}

// ChatNotificationsSuppressed includes both phone-number and LID rows. A
// first message under a new alias must not bypass an existing mute/archive.
func (s *MessageStore) ChatNotificationsSuppressed(chatID string) (bool, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	var suppressed bool
	err := s.db.QueryRow(`SELECT EXISTS (
		SELECT 1 FROM chats WHERE
		(id = ? OR id IN (SELECT lid FROM lid_mapping WHERE pn_jid = ?)
		OR id IN (SELECT pn_jid FROM lid_mapping WHERE lid = ?))
		AND (archived = 1 OR COALESCE(mute_mode, 'off') NOT IN ('', 'off', 'until')
		OR (mute_mode = 'until' AND (muted_until IS NULL OR muted_until > ?)))
	)`, chatID, chatID, chatID, time.Now().UnixMilli()).Scan(&suppressed)
	return suppressed, err
}

func (s *MessageStore) GetChatState(chatID string) (ChatState, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	var state ChatState
	var archived int
	var pinned, muted sql.NullInt64
	err := s.db.QueryRow(`SELECT id, archived, pinned_at, mute_mode, muted_until FROM chats
		WHERE id = ? OR id IN (SELECT lid FROM lid_mapping WHERE pn_jid = ?)
		OR id IN (SELECT pn_jid FROM lid_mapping WHERE lid = ?) LIMIT 1`, chatID, chatID, chatID).
		Scan(&state.ChatID, &archived, &pinned, &state.MuteMode, &muted)
	if err != nil {
		return state, err
	}
	state.ChatID = s.resolveChatIDUnlocked(state.ChatID)
	state.Archived = archived == 1
	if pinned.Valid && pinned.Int64 > 0 {
		v := pinned.Int64
		state.PinnedAt = &v
	}
	if state.MuteMode == "until" && muted.Valid && muted.Int64 > time.Now().UnixMilli() {
		v := muted.Int64
		state.MutedUntil = &v
	} else if state.MuteMode == "until" {
		state.MuteMode = "off"
	}
	return state, nil
}

func (s *MessageStore) GetRawMessage(messageID string) ([]byte, string, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	var raw []byte
	var mediaURL string
	err := s.db.QueryRow("SELECT raw_message, ifnull(media_url, '') FROM messages WHERE id = ?", messageID).Scan(&raw, &mediaURL)
	return raw, mediaURL, err
}

func (s *MessageStore) UpdateHistoricalMediaURL(messageID, mediaURL string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	_, err := s.db.Exec("UPDATE messages SET media_url = ? WHERE id = ?", mediaURL, messageID)
	return err
}

func (s *MessageStore) resolveChatIDUnlocked(chatID string) string {
	if strings.HasSuffix(chatID, "@lid") {
		var pn string
		if s.db.QueryRow("SELECT pn_jid FROM lid_mapping WHERE lid = ?", chatID).Scan(&pn) == nil && pn != "" {
			return pn
		}
	}
	return chatID
}

func boolInt(v bool) int {
	if v {
		return 1
	}
	return 0
}

func NormalizeHistoryTimestamp(value uint64) int64 {
	if value == 0 {
		return 0
	}
	if value < 1_000_000_000_000 {
		return int64(value) * 1000
	}
	if value > uint64(^uint64(0)>>1) {
		return 0
	}
	return int64(value)
}

func ValidateMuteMode(mode string) error {
	switch mode {
	case "off", "until", "forever":
		return nil
	default:
		return fmt.Errorf("invalid mute mode %q", mode)
	}
}

package repository

import (
	"database/sql"
	"encoding/json"
	"math"
	"strings"
)

// historyMediaPlaceholder is the media_url of an imported history message whose
// file has not been downloaded yet: the app's own URL that downloads it on demand.
const historyMediaPlaceholder = "/chats/%/messages/%/media"

// HistoryMediaGroups are the kinds of media a download can be limited to, with
// the message types each one covers.
var HistoryMediaGroups = map[string][]string{
	"image":    {"image"},
	"sticker":  {"sticker"},
	"audio":    {"audio", "ptt"},
	"video":    {"video"},
	"document": {"document"},
}

// HistoryMediaOptions says which media a download job fetches.
type HistoryMediaOptions struct {
	// Types are keys of HistoryMediaGroups.
	Types []string `json:"types"`
	// RetryUnavailable also tries media an earlier job found to be gone.
	RetryUnavailable bool `json:"retryUnavailable"`
	// AskPhone asks the phone to upload media again when WhatsApp's servers no
	// longer have it. Media found to be gone from the servers is then tried too.
	AskPhone bool `json:"askPhone"`
}

// HistoryMediaItem is one message whose media is still to be downloaded.
type HistoryMediaItem struct {
	MessageID string
	ChatID    string
	Timestamp int64
	Raw       []byte
}

// HistoryMediaStatus is the progress of the media download job.
type HistoryMediaStatus struct {
	State string `json:"state"`
	// Total is what the job has to do altogether: done + unavailable + failed + remaining.
	Total       int `json:"total"`
	Done        int `json:"done"`
	Remaining   int `json:"remaining"`
	Unavailable int `json:"unavailable"`
	Failed      int `json:"failed"`
	// Recovered is how many of Done were fetched through the phone after the
	// servers no longer had them. Missing is how many the phone does not have either.
	Recovered int   `json:"recovered"`
	Missing   int   `json:"missing"`
	Bytes     int64 `json:"bytes"`

	Errors     []HistorySyncError  `json:"errors"`
	StartedAt  *int64              `json:"startedAt"`
	FinishedAt *int64              `json:"finishedAt"`
	Options    HistoryMediaOptions `json:"options"`

	// Available counts, per group, the media a job could still fetch from
	// WhatsApp's servers. Expired counts the media the servers no longer have,
	// that only the phone can give back (AskPhone).
	Available map[string]int `json:"available"`
	Expired   map[string]int `json:"expired"`
	// Current is how many files are being downloaded right now.
	Current   int    `json:"current"`
	Resumable bool   `json:"resumable"`
	ElapsedMs int64  `json:"elapsedMs"`
	EtaMs     *int64 `json:"etaMs"`

	// ActiveMs is ElapsedMs as of the last save.
	ActiveMs int64 `json:"-"`
}

// typeFilter is "msg_type IN (?, ?)" for the groups of options, with its arguments.
func (o HistoryMediaOptions) typeFilter() (string, []any) {
	var args []any
	for _, group := range o.Types {
		for _, t := range HistoryMediaGroups[group] {
			args = append(args, t)
		}
	}
	if len(args) == 0 {
		return "0", nil
	}
	return "m.msg_type IN (?" + strings.Repeat(", ?", len(args)-1) + ")", args
}

// historyMediaWhere selects the messages a job has to download: still the history
// placeholder, of a chosen kind, and not marked as something to leave alone.
// runID is the running job: whatever failed in it is not tried again within it.
func historyMediaWhere(typeSQL string, opts HistoryMediaOptions) string {
	// Media the phone does not have is left alone unless asked to retry; media
	// WhatsApp's servers no longer have is only worth another try when the phone
	// is asked.
	skip := "s.run_id = ?"
	if !opts.RetryUnavailable {
		skip += " OR s.state = 'missing'"
		if !opts.AskPhone {
			skip += " OR s.state = 'unavailable'"
		}
	}
	return `m.media_url LIKE '` + historyMediaPlaceholder + `' AND m.raw_message IS NOT NULL
		AND m.deleted_at IS NULL AND ` + typeSQL + `
		AND NOT EXISTS (SELECT 1 FROM history_media_state s WHERE s.message_id = m.id AND (` + skip + `))`
}

// CountHistoryMedia is how many messages a job with options still has to do.
func (s *MessageStore) CountHistoryMedia(opts HistoryMediaOptions, runID int64) int {
	s.mu.RLock()
	defer s.mu.RUnlock()
	typeSQL, typeArgs := opts.typeFilter()
	args := append(typeArgs, runID)
	var n int
	_ = s.db.QueryRow(`SELECT COUNT(*) FROM messages m WHERE `+historyMediaWhere(typeSQL, opts), args...).Scan(&n)
	return n
}

// AvailableHistoryMedia counts, per group, the media not yet downloaded. available
// is what WhatsApp's servers may still have; expired is what an earlier job
// found they no longer have, which only the phone can give back. Media the
// phone said it does not have is in neither.
func (s *MessageStore) AvailableHistoryMedia() (available, expired map[string]int) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	available, expired = map[string]int{}, map[string]int{}
	for group := range HistoryMediaGroups {
		available[group], expired[group] = 0, 0
	}
	rows, err := s.db.Query(`SELECT m.msg_type, COALESCE(s.state, '') = 'unavailable', COUNT(*)
		FROM messages m LEFT JOIN history_media_state s ON s.message_id = m.id
		WHERE m.media_url LIKE '` + historyMediaPlaceholder + `' AND m.raw_message IS NOT NULL AND m.deleted_at IS NULL
		AND COALESCE(s.state, '') != 'missing'
		GROUP BY m.msg_type, COALESCE(s.state, '') = 'unavailable'`)
	if err != nil {
		return available, expired
	}
	defer rows.Close()
	for rows.Next() {
		var msgType string
		var gone bool
		var n int
		if rows.Scan(&msgType, &gone, &n) != nil {
			continue
		}
		target := available
		if gone {
			target = expired
		}
		for group, types := range HistoryMediaGroups {
			for _, t := range types {
				if t == msgType {
					target[group] += n
				}
			}
		}
	}
	return available, expired
}

// NextHistoryMedia returns up to limit messages to download, newest first (the
// newest media is the most likely to still be on the server), continuing after
// the message (afterTs, afterID); pass math.MaxInt64 and "" for the first page.
func (s *MessageStore) NextHistoryMedia(opts HistoryMediaOptions, runID, afterTs int64, afterID string, limit int) ([]HistoryMediaItem, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	typeSQL, typeArgs := opts.typeFilter()
	args := append(typeArgs, runID, afterTs, afterTs, afterID, limit)
	rows, err := s.db.Query(`SELECT m.id, m.chat_id, m.timestamp, m.raw_message FROM messages m WHERE `+
		historyMediaWhere(typeSQL, opts)+`
		AND (m.timestamp < ? OR (m.timestamp = ? AND m.id < ?))
		ORDER BY m.timestamp DESC, m.id DESC LIMIT ?`, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var items []HistoryMediaItem
	for rows.Next() {
		var item HistoryMediaItem
		if err := rows.Scan(&item.MessageID, &item.ChatID, &item.Timestamp, &item.Raw); err != nil {
			return nil, err
		}
		items = append(items, item)
	}
	return items, rows.Err()
}

// FirstHistoryMediaCursor is the cursor for the first page of NextHistoryMedia.
func FirstHistoryMediaCursor() (int64, string) { return math.MaxInt64, "" }

// MarkHistoryMedia records that the media of messageID could not be downloaded
// in job runID: "unavailable" (gone from the server) or "failed" (try again later).
func (s *MessageStore) MarkHistoryMedia(messageID, state, reason string, runID int64) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	_, err := s.db.Exec(`INSERT INTO history_media_state (message_id, state, reason, run_id) VALUES (?, ?, ?, ?)
		ON CONFLICT(message_id) DO UPDATE SET state = excluded.state, reason = excluded.reason, run_id = excluded.run_id`,
		messageID, state, reason, runID)
	return err
}

// HistoryMediaDownloaded points messageID at its downloaded file and forgets
// any earlier failure.
func (s *MessageStore) HistoryMediaDownloaded(messageID, mediaURL string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	tx, err := s.db.Begin()
	if err != nil {
		return err
	}
	defer tx.Rollback()
	if _, err := tx.Exec("UPDATE messages SET media_url = ? WHERE id = ?", mediaURL, messageID); err != nil {
		return err
	}
	if _, err := tx.Exec("DELETE FROM history_media_state WHERE message_id = ?", messageID); err != nil {
		return err
	}
	return tx.Commit()
}

// HistoryMediaFailures counts what job runID could not download: media gone from
// WhatsApp's servers, failures to try again later, and media the phone does not have.
func (s *MessageStore) HistoryMediaFailures(runID int64) (unavailable, failed, missing int) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	_ = s.db.QueryRow(`SELECT COALESCE(SUM(state = 'unavailable'), 0), COALESCE(SUM(state = 'failed'), 0),
		COALESCE(SUM(state = 'missing'), 0) FROM history_media_state WHERE run_id = ?`, runID).Scan(&unavailable, &failed, &missing)
	return unavailable, failed, missing
}

// ForgetHistoryMediaFailures drops the "failed" marks of job runID that have the
// given reason, so the media is to do again (a resumed job retries it).
func (s *MessageStore) ForgetHistoryMediaFailures(runID int64, reason string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	_, err := s.db.Exec("DELETE FROM history_media_state WHERE run_id = ? AND state = 'failed' AND reason = ?", runID, reason)
	return err
}

func (s *MessageStore) SetHistoryMediaStatus(status HistoryMediaStatus) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	errorsJSON, _ := json.Marshal(status.Errors)
	optionsJSON, _ := json.Marshal(status.Options)
	_, err := s.db.Exec(`UPDATE history_media_runs SET state = ?, options_json = ?, done = ?, recovered = ?, bytes = ?,
		errors_json = ?, started_at = ?, finished_at = ?, active_ms = ? WHERE id = 1`,
		status.State, string(optionsJSON), status.Done, status.Recovered, status.Bytes, string(errorsJSON),
		status.StartedAt, status.FinishedAt, status.ActiveMs)
	return err
}

func (s *MessageStore) GetHistoryMediaStatus() (HistoryMediaStatus, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	var status HistoryMediaStatus
	var optionsJSON, errorsJSON sql.NullString
	var started, finished sql.NullInt64
	var recovered sql.NullInt64
	err := s.db.QueryRow(`SELECT state, options_json, done, recovered, bytes, errors_json, started_at, finished_at, active_ms
		FROM history_media_runs WHERE id = 1`).
		Scan(&status.State, &optionsJSON, &status.Done, &recovered, &status.Bytes, &errorsJSON, &started, &finished, &status.ActiveMs)
	if err != nil {
		return status, err
	}
	status.Recovered = int(recovered.Int64)
	if optionsJSON.Valid && optionsJSON.String != "" {
		_ = json.Unmarshal([]byte(optionsJSON.String), &status.Options)
	}
	if errorsJSON.Valid {
		_ = json.Unmarshal([]byte(errorsJSON.String), &status.Errors)
	}
	if status.Errors == nil {
		status.Errors = []HistorySyncError{}
	}
	if started.Valid {
		v := started.Int64
		status.StartedAt = &v
	}
	if finished.Valid {
		v := finished.Int64
		status.FinishedAt = &v
	}
	return status, nil
}

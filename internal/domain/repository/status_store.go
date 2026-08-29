package repository

import (
	"database/sql"
	"time"
)

// StatusEntry is one status (story) update.
type StatusEntry struct {
	ID         string `json:"id"`
	Sender     string `json:"sender"`
	SenderName string `json:"senderName,omitempty"`
	Content    string `json:"content,omitempty"`
	MediaURL   string `json:"mediaUrl,omitempty"`
	Type       string `json:"type"`
	Timestamp  int64  `json:"timestamp"`
	ExpiresAt  int64  `json:"expiresAt"`
	Viewed     bool   `json:"viewed"`
}

// StatusGroup lists one sender's active statuses plus the resolved name.
type StatusGroup struct {
	Sender     string        `json:"sender"`
	Name       string        `json:"name,omitempty"`
	Avatar     string        `json:"avatar,omitempty"`
	AllViewed  bool          `json:"allViewed"`
	Statuses   []StatusEntry `json:"statuses"`
	LatestTime int64         `json:"latestTime"`
}

const statusTTL = 24 * time.Hour

func (s *MessageStore) initStatusTables() error {
	_, err := s.db.Exec(`
		CREATE TABLE IF NOT EXISTS statuses (
			id TEXT PRIMARY KEY,
			sender TEXT NOT NULL,
			content TEXT,
			media_url TEXT,
			msg_type TEXT DEFAULT 'text',
			timestamp INTEGER NOT NULL,
			expires_at INTEGER NOT NULL,
			viewed INTEGER DEFAULT 0,
			raw_proto BLOB
		)
	`)
	if err != nil {
		return err
	}
	_, _ = s.db.Exec("CREATE INDEX IF NOT EXISTS idx_statuses_sender ON statuses(sender, timestamp)")
	return nil
}

// SaveStatus stores a status update (24h TTL).
func (s *MessageStore) SaveStatus(e *StatusEntry, rawProto []byte) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if err := s.initStatusTables(); err != nil {
		return err
	}
	_, err := s.db.Exec(`
		INSERT OR REPLACE INTO statuses (id, sender, content, media_url, msg_type, timestamp, expires_at, viewed, raw_proto)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
	`, e.ID, e.Sender, e.Content, e.MediaURL, e.Type, e.Timestamp, e.ExpiresAt, boolInt(e.Viewed), rawProto)
	return err
}

// GetStatusGroups returns active statuses grouped per sender (own statuses
// flagged with sender "me" live in the same table under the account JID).
func (s *MessageStore) GetStatusGroups(ownJID string) []StatusGroup {
	s.mu.RLock()
	defer s.mu.RUnlock()
	if err := s.initStatusTables(); err != nil {
		return nil
	}

	rows, err := s.db.Query(`
		SELECT id, sender, ifnull(content,''), ifnull(media_url,''), msg_type, timestamp, expires_at, viewed
		FROM statuses
		WHERE expires_at > ?
		ORDER BY timestamp ASC
	`, time.Now().UnixMilli())
	if err != nil {
		return nil
	}
	defer rows.Close()

	bySender := map[string]*StatusGroup{}
	order := []string{}
	for rows.Next() {
		var e StatusEntry
		var viewed int
		if err := rows.Scan(&e.ID, &e.Sender, &e.Content, &e.MediaURL, &e.Type, &e.Timestamp, &e.ExpiresAt, &viewed); err != nil {
			continue
		}
		e.Viewed = viewed == 1
		group, ok := bySender[e.Sender]
		if !ok {
			group = &StatusGroup{Sender: e.Sender}
			bySender[e.Sender] = group
			order = append(order, e.Sender)
		}
		group.Statuses = append(group.Statuses, e)
		if !e.Viewed {
			group.AllViewed = false
		}
		if e.Timestamp > group.LatestTime {
			group.LatestTime = e.Timestamp
		}
		if viewed == 1 {
			group.AllViewed = group.AllViewed && true
		}
	}

	// Resolve display names + avatars from contacts; own statuses first.
	out := make([]StatusGroup, 0, len(order))
	if own, ok := bySender[ownJID]; ok {
		own.Name = "Status Saya"
		out = append(out, *own)
	}
	for _, sender := range order {
		if sender == ownJID {
			continue
		}
		group := bySender[sender]
		var name, avatar string
		_ = s.db.QueryRow("SELECT ifnull(name,''), ifnull(avatar,'') FROM contacts WHERE jid = ?", sender).Scan(&name, &avatar)
		group.Name = name
		group.Avatar = avatar
		group.AllViewed = len(group.Statuses) > 0
		for _, st := range group.Statuses {
			if !st.Viewed {
				group.AllViewed = false
			}
		}
		out = append(out, *group)
	}
	// Unseen senders first, then latest.
	sortStatusGroups(out)
	return out
}

func sortStatusGroups(groups []StatusGroup) {
	for i := 1; i < len(groups); i++ {
		for j := i; j > 0; j-- {
			a, b := groups[j-1], groups[j]
			aUnseen := !b.AllViewed && a.AllViewed
			swap := false
			if a.Sender == "me" || (a.Name == "Status Saya") {
				swap = false
			} else if aUnseen {
				swap = true
			} else if a.AllViewed == b.AllViewed && a.LatestTime < b.LatestTime {
				swap = true
			}
			if !swap {
				break
			}
			groups[j-1], groups[j] = groups[j], groups[j-1]
		}
	}
}

// MarkStatusViewed flags a status as viewed locally.
func (s *MessageStore) MarkStatusViewed(id string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if err := s.initStatusTables(); err != nil {
		return err
	}
	_, err := s.db.Exec("UPDATE statuses SET viewed = 1 WHERE id = ?", id)
	return err
}

// GetStatusByID loads one status row.
func (s *MessageStore) GetStatusByID(id string) (*StatusEntry, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	if err := s.initStatusTables(); err != nil {
		return nil, err
	}
	var e StatusEntry
	var viewed int
	err := s.db.QueryRow(`
		SELECT id, sender, ifnull(content,''), ifnull(media_url,''), msg_type, timestamp, expires_at, viewed
		FROM statuses WHERE id = ?
	`, id).Scan(&e.ID, &e.Sender, &e.Content, &e.MediaURL, &e.Type, &e.Timestamp, &e.ExpiresAt, &viewed)
	if err != nil {
		return nil, err
	}
	e.Viewed = viewed == 1
	return &e, nil
}

// GetStatusRawProto returns the stored serialized message of a status.
func (s *MessageStore) GetStatusRawProto(id string) ([]byte, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	if err := s.initStatusTables(); err != nil {
		return nil, err
	}
	var raw []byte
	err := s.db.QueryRow("SELECT raw_proto FROM statuses WHERE id = ?", id).Scan(&raw)
	if err == sql.ErrNoRows {
		return nil, err
	}
	return raw, err
}

// ExpireStatuses deletes statuses past their TTL; returns rows removed.
func (s *MessageStore) ExpireStatuses() int64 {
	s.mu.Lock()
	defer s.mu.Unlock()
	if err := s.initStatusTables(); err != nil {
		return 0
	}
	res, err := s.db.Exec("DELETE FROM statuses WHERE expires_at <= ?", time.Now().UnixMilli())
	if err != nil {
		return 0
	}
	n, _ := res.RowsAffected()
	return n
}

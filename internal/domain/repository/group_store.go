package repository

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"time"
)

// GroupParticipantInfo is one member entry in the cached group state.
type GroupParticipantInfo struct {
	JID          string `json:"jid"`
	Name         string `json:"name,omitempty"`
	IsAdmin      bool   `json:"isAdmin"`
	IsSuperAdmin bool   `json:"isSuperAdmin,omitempty"`
}

// GroupCache mirrors a whatsmeow GroupInfo for serving member lists and
// settings without hitting the WhatsApp servers on every render.
type GroupCache struct {
	JID              string                 `json:"jid"`
	Name             string                 `json:"name"`
	Description      string                 `json:"description,omitempty"`
	Owner            string                 `json:"owner,omitempty"`
	Locked           bool                   `json:"locked"`
	Announce         bool                   `json:"announce"`
	JoinApproval     bool                   `json:"joinApproval"`
	MemberAddMode    string                 `json:"memberAddMode,omitempty"`
	OwnRole          string                 `json:"ownRole"` // "superadmin" | "admin" | "member" | ""
	ParticipantCount int                    `json:"participantCount"`
	Participants     []GroupParticipantInfo `json:"participants"`
	UpdatedAt        int64                  `json:"updatedAt"`
}

func (s *MessageStore) initGroupTables() error {
	_, err := s.db.Exec(`
		CREATE TABLE IF NOT EXISTS group_cache (
			jid TEXT PRIMARY KEY,
			name TEXT,
			description TEXT,
			owner TEXT,
			locked INTEGER DEFAULT 0,
			announce INTEGER DEFAULT 0,
			join_approval INTEGER DEFAULT 0,
			member_add_mode TEXT,
			own_role TEXT DEFAULT '',
			participants TEXT DEFAULT '[]',
			updated_at INTEGER
		)
	`)
	return err
}

// SaveGroupCache stores the latest group snapshot, resolving participant
// display names from the contacts table.
func (s *MessageStore) SaveGroupCache(g *GroupCache) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	if err := s.initGroupTables(); err != nil {
		return err
	}

	participants := g.Participants
	if participants == nil {
		participants = []GroupParticipantInfo{}
	}
	// Best-effort name resolution from contacts.
	for i := range participants {
		if participants[i].Name == "" {
			var name string
			_ = s.db.QueryRow("SELECT name FROM contacts WHERE jid = ?", participants[i].JID).Scan(&name)
			participants[i].Name = name
		}
	}
	raw, err := json.Marshal(participants)
	if err != nil {
		return err
	}

	_, err = s.db.Exec(`
		INSERT OR REPLACE INTO group_cache (jid, name, description, owner, locked, announce, join_approval, member_add_mode, own_role, participants, updated_at)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
	`, g.JID, g.Name, g.Description, g.Owner,
		boolInt(g.Locked), boolInt(g.Announce), boolInt(g.JoinApproval),
		g.MemberAddMode, g.OwnRole, string(raw), time.Now().UnixMilli())
	if err != nil {
		return err
	}

	// Keep the chats table name in sync so the sidebar label follows renames.
	if g.Name != "" {
		_, _ = s.db.Exec("UPDATE chats SET name = ?, updated_at = ? WHERE id = ?", g.Name, time.Now().Unix(), g.JID)
	}
	return nil
}

// GetGroupCache returns the cached snapshot for a group.
func (s *MessageStore) GetGroupCache(jid string) (*GroupCache, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	if err := s.initGroupTables(); err != nil {
		return nil, err
	}

	var g GroupCache
	var locked, announce, approval int
	var raw string
	err := s.db.QueryRow(`
		SELECT jid, ifnull(name,''), ifnull(description,''), ifnull(owner,''), locked, announce, join_approval, ifnull(member_add_mode,''), ifnull(own_role,''), participants, ifnull(updated_at,0)
		FROM group_cache WHERE jid = ?
	`, jid).Scan(&g.JID, &g.Name, &g.Description, &g.Owner, &locked, &announce, &approval, &g.MemberAddMode, &g.OwnRole, &raw, &g.UpdatedAt)
	if err != nil {
		if err == sql.ErrNoRows {
			return nil, fmt.Errorf("group cache miss for %s", jid)
		}
		return nil, err
	}
	g.Locked = locked == 1
	g.Announce = announce == 1
	g.JoinApproval = approval == 1
	if err := json.Unmarshal([]byte(raw), &g.Participants); err != nil {
		g.Participants = nil
	}
	g.ParticipantCount = len(g.Participants)
	return &g, nil
}

package repository

import "encoding/json"

// UnreadMentionChats examines incoming unread messages, including mentions
// followed by newer messages that do not mention the account.
func (s *MessageStore) UnreadMentionChats(ownJIDs []string) (map[string]bool, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	result := make(map[string]bool)
	if len(ownJIDs) == 0 {
		return result, nil
	}
	own := make(map[string]bool)
	for _, jid := range ownJIDs {
		own[s.resolveChatIDUnlocked(jid)] = true
	}
	rows, err := s.db.Query(`WITH unread_incoming AS (
		SELECT m.chat_id, m.extra_meta, m.deleted_at, c.unread,
		ROW_NUMBER() OVER (PARTITION BY m.chat_id ORDER BY m.timestamp DESC, m.rowid DESC) AS unread_rank
		FROM messages m JOIN chats c ON c.id = m.chat_id
		WHERE c.unread > 0 AND c.deleted_at IS NULL
		AND m.status = 'received' AND COALESCE(m.is_automatic, 0) = 0
	) SELECT chat_id, COALESCE(extra_meta, '') FROM unread_incoming
	WHERE unread_rank <= unread AND deleted_at IS NULL AND COALESCE(extra_meta, '') <> ''`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	// Collect first: resolving aliases queries the same database connection.
	type row struct{ chatID, extra string }
	var messages []row
	for rows.Next() {
		var m row
		if err := rows.Scan(&m.chatID, &m.extra); err != nil {
			return nil, err
		}
		messages = append(messages, m)
	}
	if err := rows.Err(); err != nil {
		return nil, err
	}
	if err := rows.Close(); err != nil {
		return nil, err
	}
	for _, m := range messages {
		var extra MessageExtra
		if json.Unmarshal([]byte(m.extra), &extra) != nil {
			continue
		}
		for _, jid := range extra.Mentions {
			if own[s.resolveChatIDUnlocked(jid)] {
				result[s.resolveChatIDUnlocked(m.chatID)] = true
				break
			}
		}
	}
	return result, nil
}

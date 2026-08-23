// Package store holds the desktop app's in-memory mirror of the backend's
// chat/message state — the Go analogue of web/src/stores/chatStore.ts.
//
// All methods are safe for concurrent use. Mutations emit Change events to
// subscribers synchronously on the calling goroutine (typically the WS read
// loop); subscribers must hop to the GTK main thread before touching widgets.
package store

import (
	"crypto/rand"
	"encoding/hex"
	"fmt"
	"sort"
	"strings"
	"sync"
	"time"

	"wa-bot-desktop/internal/api"
)

// OutgoingFrom matches api.Message.From for messages sent by our own account.
const OutgoingFrom = "me"

// TempIDPrefix marks optimistic local messages not yet confirmed by the
// backend's new_message echo.
const TempIDPrefix = "temp-"

// ChangeKind describes what mutated.
type ChangeKind int

const (
	// ChatsChanged means the chat list (order, names, previews, unread)
	// changed. ChatID is empty.
	ChatsChanged ChangeKind = iota
	// MessagesReset means the whole message list of ChatID was replaced
	// (initial page load or chat switch).
	MessagesReset
	// MessagesChanged means messages within ChatID changed incrementally
	// (append/prepend/patch/delete/edit).
	MessagesChanged
)

// Change is emitted to subscribers after each mutation.
type Change struct {
	Kind   ChangeKind
	ChatID string
}

// Page is one snapshot of a chat's messages.
type Page struct {
	Items   []api.Message // ascending by Timestamp
	HasMore bool          // older history exists on the server
}

// Store is the state container.
type Store struct {
	mu       sync.RWMutex
	chats    []api.Chat            // sorted by LastTime desc
	messages map[string]*chatState // chatID -> state
	subs     map[int]func(Change)
	nextSub  int

	activeChat string // chat currently open in the conversation pane
}

type chatState struct {
	items   []api.Message // ascending by Timestamp
	hasMore bool
}

// New returns an empty Store.
func New() *Store {
	return &Store{messages: make(map[string]*chatState), subs: make(map[int]func(Change))}
}

// Subscribe registers fn for change notifications. The returned func
// unsubscribes.
func (s *Store) Subscribe(fn func(Change)) (unsubscribe func()) {
	s.mu.Lock()
	id := s.nextSub
	s.nextSub++
	s.subs[id] = fn
	s.mu.Unlock()
	return func() {
		s.mu.Lock()
		defer s.mu.Unlock()
		delete(s.subs, id)
	}
}

// SetActiveChat records which chat the conversation pane displays. Incoming
// messages for that chat do not bump its unread counter.
func (s *Store) SetActiveChat(chatID string) {
	s.mu.Lock()
	s.activeChat = chatID
	s.mu.Unlock()
}

// ActiveChat returns the currently open chat ID ("" when none).
func (s *Store) ActiveChat() string {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return s.activeChat
}

// ReplaceChats swaps in a freshly fetched chat list.
func (s *Store) ReplaceChats(chats []api.Chat) {
	s.mu.Lock()
	s.chats = sortChats(chats)
	s.mu.Unlock()
	s.emit(Change{Kind: ChatsChanged})
}

// UpsertChat inserts or updates one chat and re-sorts the list.
func (s *Store) UpsertChat(c api.Chat) {
	s.mu.Lock()
	found := false
	for i := range s.chats {
		if s.chats[i].ID == c.ID {
			c.Unread = mergeUnread(s.chats[i], c)
			s.chats[i] = c
			found = true
			break
		}
	}
	if !found {
		s.chats = append(s.chats, c)
	}
	s.chats = sortChats(s.chats)
	s.mu.Unlock()
	s.emit(Change{Kind: ChatsChanged})
}

// Chats returns a copy of the sorted chat list.
func (s *Store) Chats() []api.Chat {
	s.mu.RLock()
	defer s.mu.RUnlock()
	out := make([]api.Chat, len(s.chats))
	copy(out, s.chats)
	return out
}

// Chat returns one chat by ID.
func (s *Store) Chat(id string) (api.Chat, bool) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	for _, c := range s.chats {
		if c.ID == id {
			return c, true
		}
	}
	return api.Chat{}, false
}

// ResetMessages replaces the message page for a chat. Input is the raw API
// result: newest-first DESC slice from GetMessages.
func (s *Store) ResetMessages(chatID string, newestFirst []api.Message, hasMore bool) {
	asc := make([]api.Message, len(newestFirst))
	for i, m := range newestFirst {
		asc[len(newestFirst)-1-i] = m
	}
	s.mu.Lock()
	s.messages[chatID] = &chatState{items: asc, hasMore: hasMore}
	s.mu.Unlock()
	s.emit(Change{Kind: MessagesReset, ChatID: chatID})
}

// PrependOlder merges an older DESC page at the front.
func (s *Store) PrependOlder(chatID string, olderDesc []api.Message, hasMore bool) {
	s.mu.Lock()
	cs := s.messages[chatID]
	if cs == nil {
		cs = &chatState{}
		s.messages[chatID] = cs
	}
	asc := make([]api.Message, 0, len(olderDesc)+len(cs.items))
	for i := len(olderDesc) - 1; i >= 0; i-- {
		asc = append(asc, olderDesc[i])
	}
	cs.items = append(asc, cs.items...)
	cs.hasMore = hasMore
	s.mu.Unlock()
	s.emit(Change{Kind: MessagesChanged, ChatID: chatID})
}

// AppendNewer merges a newer ASC page at the back, skipping duplicates by ID.
func (s *Store) AppendNewer(chatID string, newerAsc []api.Message) {
	if len(newerAsc) == 0 {
		return
	}
	s.mu.Lock()
	cs := s.ensureLocked(chatID)
	have := make(map[string]struct{}, len(cs.items))
	for _, m := range cs.items {
		have[m.ID] = struct{}{}
	}
	for _, m := range newerAsc {
		if _, dup := have[m.ID]; !dup {
			cs.items = append(cs.items, m)
		}
	}
	s.mu.Unlock()
	s.emit(Change{Kind: MessagesChanged, ChatID: chatID})
}

// ApplyIncoming handles a new_message event: replaces a matching pending
// temp row (own echo), otherwise appends; bumps the chat preview/unread.
func (s *Store) ApplyIncoming(m api.Message) {
	isOwn := m.From == OutgoingFrom

	s.mu.Lock()
	if cs := s.ensureLocked(m.ChatID); cs != nil {
		replaced := false
		if isOwn {
			for i := range cs.items {
				t := cs.items[i]
				if isTemp(t) && t.Type == m.Type && t.Content == m.Content {
					cs.items[i] = m // confirmed echo replaces the optimistic row
					replaced = true
					break
				}
			}
		}
		if !replaced {
			dup := false
			for i := range cs.items {
				if cs.items[i].ID == m.ID {
					cs.items[i] = m
					dup = true
					break
				}
			}
			if !dup {
				cs.items = appendSortedMessage(cs.items, m)
			}
		}
	}

	bumped := false
	for i := range s.chats {
		if s.chats[i].ID == m.ChatID {
			s.chats[i].LastMsg = previewOf(m)
			if m.Timestamp > s.chats[i].LastTime {
				s.chats[i].LastTime = m.Timestamp
			}
			if !isOwn && m.ChatID != s.activeChat && !m.IsAutomatic {
				s.chats[i].Unread++
			}
			bumped = true
			break
		}
	}
	if !bumped {
		s.chats = append(s.chats, api.Chat{
			ID:       m.ChatID,
			Name:     firstNonEmpty(m.ChatName, m.SenderName, m.From),
			LastMsg:  previewOf(m),
			LastTime: m.Timestamp,
			IsGroup:  contains("@g.us", m.ChatID),
		})
		s.chats = sortChats(s.chats)
	} else {
		s.chats = sortChats(s.chats)
	}
	s.mu.Unlock()

	s.emit(Change{Kind: MessagesChanged, ChatID: m.ChatID})
	s.emit(Change{Kind: ChatsChanged})
}

// AddOutgoingTemp inserts an optimistic outgoing message and returns its
// temp ID. Call ConfirmSend or FailSend afterwards.
func (s *Store) AddOutgoingTemp(chatID, content, msgType string) api.Message {
	temp := api.Message{
		ID:        TempIDPrefix + randID(),
		ChatID:    chatID,
		From:      OutgoingFrom,
		To:        chatID,
		Content:   content,
		Timestamp: nowMilli(),
		Status:    "pending",
		Type:      msgType,
	}
	s.mu.Lock()
	if cs := s.ensureLocked(chatID); cs != nil {
		cs.items = appendSortedMessage(cs.items, temp)
	}
	for i := range s.chats {
		if s.chats[i].ID == chatID {
			s.chats[i].LastMsg = previewOf(temp)
			s.chats[i].LastTime = temp.Timestamp
			break
		}
	}
	s.chats = sortChats(s.chats)
	s.mu.Unlock()

	s.emit(Change{Kind: MessagesChanged, ChatID: chatID})
	s.emit(Change{Kind: ChatsChanged})
	return temp
}

// PatchTempStatus sets Status on a specific temp row ("failed" on error).
func (s *Store) PatchTempStatus(chatID, tempID, status string) {
	s.mu.Lock()
	if cs := s.messages[chatID]; cs != nil {
		for i := range cs.items {
			if cs.items[i].ID == tempID {
				cs.items[i].Status = status
				break
			}
		}
	}
	s.mu.Unlock()
	s.emit(Change{Kind: MessagesChanged, ChatID: chatID})
}

// PatchStatus applies a message_status event to a non-temp row.
func (s *Store) PatchStatus(chatID, msgID, status string) {
	s.mu.Lock()
	if cs := s.messages[chatID]; cs != nil {
		for i := range cs.items {
			if cs.items[i].ID == msgID {
				cs.items[i].Status = status
				break
			}
		}
	}
	s.mu.Unlock()
	s.emit(Change{Kind: MessagesChanged, ChatID: chatID})
}

// DeleteMessage applies a message_deleted event.
func (s *Store) DeleteMessage(chatID, msgID string) {
	s.mu.Lock()
	if cs := s.messages[chatID]; cs != nil {
		out := cs.items[:0]
		for _, m := range cs.items {
			if m.ID != msgID {
				out = append(out, m)
			}
		}
		cs.items = out
	}
	s.mu.Unlock()
	s.emit(Change{Kind: MessagesChanged, ChatID: chatID})
}

// EditMessage applies a message_edited event.
func (s *Store) EditMessage(chatID, msgID, content string) {
	s.mu.Lock()
	if cs := s.messages[chatID]; cs != nil {
		for i := range cs.items {
			if cs.items[i].ID == msgID {
				cs.items[i].Content = content
				break
			}
		}
	}
	s.mu.Unlock()
	s.emit(Change{Kind: MessagesChanged, ChatID: chatID})
}

// RenameChat applies a chat_name_update event.
func (s *Store) RenameChat(chatID, name, avatar string) {
	s.mu.Lock()
	for i := range s.chats {
		if s.chats[i].ID == chatID {
			if name != "" {
				s.chats[i].Name = name
			}
			if avatar != "" {
				s.chats[i].Avatar = avatar
			}
			break
		}
	}
	s.mu.Unlock()
	s.emit(Change{Kind: ChatsChanged})
}

// MarkRead zeroes the unread counter locally.
func (s *Store) MarkRead(chatID string) {
	s.mu.Lock()
	for i := range s.chats {
		if s.chats[i].ID == chatID {
			s.chats[i].Unread = 0
			break
		}
	}
	s.mu.Unlock()
	s.emit(Change{Kind: ChatsChanged})
}

// Messages snapshots a chat's page.
func (s *Store) Messages(chatID string) (Page, bool) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	cs := s.messages[chatID]
	if cs == nil {
		return Page{}, false
	}
	items := make([]api.Message, len(cs.items))
	copy(items, cs.items)
	return Page{Items: items, HasMore: cs.hasMore}, true
}

// PatchStatusByID applies a message_status event without knowing the chat ID
// (the WS payload carries only the message id).
func (s *Store) PatchStatusByID(msgID, status string) {
	s.mu.Lock()
	for _, cs := range s.messages {
		for i := range cs.items {
			if cs.items[i].ID == msgID {
				cs.items[i].Status = status
			}
		}
	}
	s.mu.Unlock()
}

// Reset wipes all cached state (logout).
func (s *Store) Reset() {
	s.mu.Lock()
	s.chats = nil
	s.messages = make(map[string]*chatState)
	s.activeChat = ""
	s.mu.Unlock()
	s.emit(Change{Kind: ChatsChanged})
}

// Message finds one message inside a loaded chat (for reply previews).
func (s *Store) Message(chatID, msgID string) (api.Message, bool) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	for _, m := range s.messages[chatID].items {
		if m.ID == msgID {
			return m, true
		}
	}
	return api.Message{}, false
}

// --- helpers ---

func (s *Store) ensureLocked(chatID string) *chatState {
	cs := s.messages[chatID]
	if cs == nil {
		cs = &chatState{}
		s.messages[chatID] = cs
	}
	return cs
}

func (s *Store) emit(c Change) {
	s.mu.RLock()
	subs := make([]func(Change), 0, len(s.subs))
	for _, fn := range s.subs {
		subs = append(subs, fn)
	}
	s.mu.RUnlock()
	for _, fn := range subs {
		fn(c)
	}
}

func sortChats(chats []api.Chat) []api.Chat {
	out := make([]api.Chat, len(chats))
	copy(out, chats)
	sort.SliceStable(out, func(i, j int) bool {
		return out[i].LastTime > out[j].LastTime
	})
	return out
}

func appendSortedMessage(items []api.Message, m api.Message) []api.Message {
	i := len(items)
	for i > 0 && items[i-1].Timestamp > m.Timestamp {
		i--
	}
	items = append(items, api.Message{})
	copy(items[i+1:], items[i:])
	items[i] = m
	return items
}

// mergeUnread keeps the larger unread count so a REST refresh never clobbers
// live WS increments (or vice versa).
func mergeUnread(oldC, newC api.Chat) int {
	if newC.Unread >= oldC.Unread {
		return newC.Unread
	}
	return oldC.Unread
}

func isTemp(m api.Message) bool {
	return len(m.ID) > len(TempIDPrefix) && m.ID[:len(TempIDPrefix)] == TempIDPrefix
}

// OneLine flattens a preview string onto one line: newlines/tabs become
// spaces, whitespace runs collapse, and overlong text is cut at maxRunes
// with an ellipsis. Keeps chat-list rows a uniform height no matter how
// long (or multi-line) the last message was.
func OneLine(s string, maxRunes int) string {
	s = strings.Map(func(r rune) rune {
		if r == '\n' || r == '\r' || r == '\t' {
			return ' '
		}
		return r
	}, s)
	s = strings.Join(strings.Fields(s), " ")
	runes := []rune(s)
	if len(runes) > maxRunes {
		return string(runes[:maxRunes]) + "…"
	}
	return s
}

func previewOf(m api.Message) string {
	if s := strings.TrimSpace(m.Content); s != "" {
		return OneLine(s, 80)
	}
	switch m.Type {
	case "image":
		return "📷 Photo"
	case "video":
		return "🎬 Video"
	case "audio", "ptt", "voice":
		return "🎤 Voice message"
	case "sticker":
		return "Sticker"
	default:
		return "📎 Attachment"
	}
}

func contains(substr, s string) bool {
	for i := 0; i+len(substr) <= len(s); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}

func firstNonEmpty(vals ...string) string {
	for _, v := range vals {
		if v != "" {
			return v
		}
	}
	return ""
}

// nowMilli mirrors the backend's unix-millis timestamps.
func nowMilli() int64 { return time.Now().UnixMilli() }

// randID returns a short random hex id for optimistic temp messages.
func randID() string {
	var b [6]byte
	if _, err := rand.Read(b[:]); err != nil {
		return fmt.Sprintf("%d", nowMilli())
	}
	return hex.EncodeToString(b[:])
}

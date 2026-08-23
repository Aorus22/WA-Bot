package whatsapp

import (
	"context"
	"fmt"
	"net/url"
	"strings"
	"sync"
	"time"

	"go.mau.fi/whatsmeow"
	waE2E "go.mau.fi/whatsmeow/proto/waE2E"
	waHistorySync "go.mau.fi/whatsmeow/proto/waHistorySync"
	waWeb "go.mau.fi/whatsmeow/proto/waWeb"
	waTypes "go.mau.fi/whatsmeow/types"
	"go.mau.fi/whatsmeow/types/events"
	"google.golang.org/protobuf/proto"

	"wa-bot/internal/domain/repository"
	whatsappInfra "wa-bot/internal/infrastructure/whatsapp"
)

const historyPageSize = 50

// HistorySyncService owns durable staging and the single manual import job.
// Downloading pairing packets is automatic, but visible tables are only
// touched from run(), after POST /api/history-sync.
type HistorySyncService struct {
	client      *whatsappInfra.WhatsAppClient
	store       *repository.MessageStore
	broadcaster HTTPServer

	mu      sync.Mutex
	stageMu sync.Mutex
	staging map[string]bool
	running bool
	status  repository.HistorySyncStatus
}

func NewHistorySyncService(client *whatsappInfra.WhatsAppClient, store *repository.MessageStore) *HistorySyncService {
	s := &HistorySyncService{client: client, store: store, staging: make(map[string]bool)}
	status, err := store.GetHistorySyncStatus()
	if err != nil {
		status = repository.HistorySyncStatus{State: "idle", Errors: []repository.HistorySyncError{}}
	}
	if status.State == "running" {
		now := time.Now().UnixMilli()
		status.State = "failed"
		status.FinishedAt = &now
		status.Errors = append(status.Errors, repository.HistorySyncError{Message: "history sync was interrupted by a backend restart"})
		_ = store.SetHistorySyncStatus(status)
	}
	s.status = status
	return s
}

func (s *HistorySyncService) SetBroadcaster(server HTTPServer) { s.broadcaster = server }

func (s *HistorySyncService) StageNotification(messageID string, notif *waE2E.HistorySyncNotification) {
	if notif == nil || s.store.HasHistoryNotification(messageID) {
		return
	}
	go func() {
		s.stageMu.Lock()
		if s.staging[messageID] || s.store.HasHistoryNotification(messageID) {
			s.stageMu.Unlock()
			return
		}
		s.staging[messageID] = true
		s.stageMu.Unlock()
		defer func() {
			s.stageMu.Lock()
			delete(s.staging, messageID)
			s.stageMu.Unlock()
		}()
		ctx, cancel := context.WithTimeout(context.Background(), 2*time.Minute)
		defer cancel()
		data, err := s.client.CoreClient().DownloadHistorySync(ctx, notif, true)
		if err != nil {
			fmt.Printf("[HISTORY] Failed to stage notification %s: %v\n", messageID, err)
			return
		}
		if err := s.StageData(data); err != nil {
			fmt.Printf("[HISTORY] Failed to persist notification %s: %v\n", messageID, err)
			return
		}
		if err := s.store.SaveHistoryNotification(messageID); err != nil {
			fmt.Printf("[HISTORY] Failed to mark notification %s: %v\n", messageID, err)
			return
		}
		if err := s.client.CoreClient().DeleteMedia(ctx, whatsmeow.MediaHistory, notif.GetDirectPath(), notif.GetFileEncSHA256(), notif.GetEncHandle()); err != nil {
			fmt.Printf("[HISTORY] Failed to clean up staged history media %s: %v\n", messageID, err)
		}
	}()
}

func (s *HistorySyncService) StageData(data *waHistorySync.HistorySync) error {
	if data == nil {
		return nil
	}
	for _, conv := range data.GetConversations() {
		chatID := conv.GetID()
		if chatID == "" || chatID == "status@broadcast" {
			continue
		}
		pinnedAt := repository.NormalizeHistoryTimestamp(uint64(conv.GetPinned()))
		muteEnd := repository.NormalizeHistoryTimestamp(conv.GetMuteEndTime())
		meta := repository.HistoryConversation{
			ChatID:      chatID,
			Name:        firstNonBlank(conv.GetDisplayName(), conv.GetName()),
			UnreadCount: int(conv.GetUnreadCount()),
			Archived:    conv.GetArchived(),
			PinnedAt:    pinnedAt,
			MuteEnd:     muteEnd,
		}
		if conv.GetMuteEndTime() == ^uint64(0) {
			meta.MuteEnd = -1
		}
		items := make([]repository.StagedHistoryMessage, 0, len(conv.GetMessages()))
		for _, historyMsg := range conv.GetMessages() {
			webMsg := historyMsg.GetMessage()
			if webMsg == nil || webMsg.GetKey().GetID() == "" {
				continue
			}
			raw, err := proto.Marshal(webMsg)
			if err != nil {
				continue
			}
			msgChatID := chatID
			if remote := webMsg.GetKey().GetRemoteJID(); remote != "" {
				msgChatID = remote
			}
			items = append(items, repository.StagedHistoryMessage{
				ChatID:    msgChatID,
				MessageID: webMsg.GetKey().GetID(),
				Timestamp: int64(webMsg.GetMessageTimestamp()) * 1000,
				Raw:       raw,
			})
		}
		if err := s.store.StageHistoryConversation(meta, items); err != nil {
			return err
		}
	}
	return nil
}

func (s *HistorySyncService) Start() (repository.HistorySyncStatus, error) {
	if !s.client.IsConnected() {
		return s.Status(), fmt.Errorf("WhatsApp is not connected")
	}
	s.mu.Lock()
	if s.running {
		status := s.statusWithPendingLocked()
		s.mu.Unlock()
		return status, fmt.Errorf("history sync is already running")
	}
	now := time.Now().UnixMilli()
	s.running = true
	s.status = repository.HistorySyncStatus{
		State:     "running",
		Errors:    []repository.HistorySyncError{},
		StartedAt: &now,
	}
	_ = s.store.SetHistorySyncStatus(s.status)
	status := s.statusWithPendingLocked()
	s.mu.Unlock()
	go s.run()
	return status, nil
}

func (s *HistorySyncService) Status() repository.HistorySyncStatus {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.statusWithPendingLocked()
}

func (s *HistorySyncService) statusWithPendingLocked() repository.HistorySyncStatus {
	status := s.status
	status.Errors = append([]repository.HistorySyncError(nil), status.Errors...)
	status.PendingChats, status.PendingMessages, _ = s.store.PendingHistoryCounts()
	return status
}

func (s *HistorySyncService) run() {
	targets, err := s.store.HistoryTargets()
	if err != nil {
		s.finish("failed", []repository.HistorySyncError{{Message: err.Error()}})
		return
	}
	s.updateProgress(func(status *repository.HistorySyncStatus) { status.ChatsTotal = len(targets) })

	for _, target := range targets {
		if !s.client.IsConnected() {
			s.addError(target.ChatID, "WhatsApp disconnected during history sync")
			break
		}
		pending := target.Pending
		if pending == 0 {
			anchor, anchorErr := s.store.OldestMessageAnchor(target.ChatID)
			if anchorErr == nil {
				stagedAt := s.store.HistoryConversationUpdatedAt(target.ChatID)
				if requestErr := s.requestOlder(anchor); requestErr != nil {
					s.addError(target.ChatID, requestErr.Error())
				} else if !s.waitForHistoryResponse(target.ChatID, stagedAt, 30*time.Second) {
					s.addError(target.ChatID, "primary device did not return history within 30 seconds")
				}
			}
		}
		added, importErr := s.importPending(target.ChatID)
		if importErr != nil {
			s.addError(target.ChatID, importErr.Error())
		}
		s.updateProgress(func(status *repository.HistorySyncStatus) {
			status.ChatsProcessed++
			status.MessagesAdded += added
		})
	}

	s.mu.Lock()
	hasErrors := len(s.status.Errors) > 0
	processed := s.status.ChatsProcessed
	total := s.status.ChatsTotal
	s.mu.Unlock()
	state := "completed"
	if hasErrors {
		state = "partial"
		if processed == 0 && total > 0 {
			state = "failed"
		}
	}
	s.finish(state, nil)
}

func (s *HistorySyncService) requestOlder(anchor *repository.MessageAnchor) error {
	jid, err := waTypes.ParseJID(anchor.ChatID)
	if err != nil {
		return fmt.Errorf("invalid chat JID: %w", err)
	}
	info := &waTypes.MessageInfo{
		MessageSource: waTypes.MessageSource{Chat: jid, IsFromMe: anchor.IsFromMe},
		ID:            waTypes.MessageID(anchor.MessageID),
		Timestamp:     time.UnixMilli(anchor.Timestamp),
	}
	ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
	defer cancel()
	_, err = s.client.CoreClient().SendPeerMessage(ctx, s.client.CoreClient().BuildHistorySyncRequest(info, historyPageSize))
	if err != nil {
		return fmt.Errorf("request older history: %w", err)
	}
	return nil
}

func (s *HistorySyncService) waitForHistoryResponse(chatID string, previousUpdate int64, timeout time.Duration) bool {
	deadline := time.Now().Add(timeout)
	for time.Now().Before(deadline) {
		if s.store.HistoryConversationUpdatedAt(chatID) > previousUpdate {
			return true
		}
		time.Sleep(400 * time.Millisecond)
	}
	return false
}

func (s *HistorySyncService) importPending(chatID string) (int, error) {
	staged, err := s.store.PendingHistory(chatID, historyPageSize)
	if err != nil || len(staged) == 0 {
		return 0, err
	}
	imports := make([]repository.HistoryImportMessage, 0, len(staged))
	for _, item := range staged {
		msg, projectErr := s.project(item)
		if projectErr != nil {
			s.addError(chatID, fmt.Sprintf("message %s: %v", item.MessageID, projectErr))
			continue
		}
		if msg != nil {
			if msg.Timestamp <= 0 {
				msg.Timestamp = item.Timestamp
			}
			imports = append(imports, repository.HistoryImportMessage{Message: *msg, Raw: item.Raw})
		}
	}
	return s.store.ImportHistoryBatch(chatID, staged, imports)
}

func (s *HistorySyncService) project(item repository.StagedHistoryMessage) (*repository.Message, error) {
	var webMsg waWeb.WebMessageInfo
	if err := proto.Unmarshal(item.Raw, &webMsg); err != nil {
		return nil, fmt.Errorf("decode web message: %w", err)
	}
	jid, err := waTypes.ParseJID(item.ChatID)
	if err != nil {
		return nil, err
	}
	evt, err := s.client.CoreClient().ParseWebMessage(jid, &webMsg)
	if err != nil {
		return nil, fmt.Errorf("parse web message: %w", err)
	}
	return projectHistoricalEvent(evt, item.ChatID), nil
}

func projectHistoricalEvent(evt *events.Message, chatID string) *repository.Message {
	if evt == nil || evt.Message == nil || evt.Info.ID == "" {
		return nil
	}
	content, msgType, media := historicalContent(evt.Message)
	if content == "" && !media {
		return nil
	}
	from, to, status := evt.Info.Sender.ToNonAD().String(), "me", "received"
	if evt.Info.IsFromMe {
		from, to, status = "me", chatID, "sent"
	}
	msg := &repository.Message{
		ID:         evt.Info.ID,
		ChatID:     chatID,
		From:       from,
		To:         to,
		Content:    content,
		Timestamp:  evt.Info.Timestamp.UnixMilli(),
		Status:     status,
		Type:       msgType,
		SenderName: evt.Info.PushName,
	}
	if media {
		msg.MediaURL = fmt.Sprintf("/chats/%s/messages/%s/media", url.PathEscape(chatID), url.PathEscape(evt.Info.ID))
	}
	if contextInfo := getContextInfo(evt.Message); contextInfo != nil {
		msg.ReplyToID = contextInfo.GetStanzaID()
	}
	return msg
}

func historicalContent(msg *waE2E.Message) (string, string, bool) {
	if text := msg.GetExtendedTextMessage(); text != nil {
		return text.GetText(), "text", false
	}
	if text := msg.GetConversation(); text != "" {
		return text, "text", false
	}
	if image := msg.GetImageMessage(); image != nil {
		return fallback(image.GetCaption(), "[Image]"), "image", true
	}
	if video := msg.GetVideoMessage(); video != nil {
		return fallback(video.GetCaption(), "[Video]"), "video", true
	}
	if document := msg.GetDocumentMessage(); document != nil {
		return fallback(document.GetFileName(), fallback(document.GetTitle(), "[Document]")), "document", true
	}
	if audio := msg.GetAudioMessage(); audio != nil {
		if audio.GetPTT() {
			return "[Voice Message]", "ptt", true
		}
		return "[Audio]", "audio", true
	}
	if msg.GetStickerMessage() != nil {
		return "[Sticker]", "sticker", true
	}
	return "", "", false
}

func (s *HistorySyncService) addError(chatID, message string) {
	s.updateProgress(func(status *repository.HistorySyncStatus) {
		status.Errors = append(status.Errors, repository.HistorySyncError{ChatID: chatID, Message: message})
	})
}

func (s *HistorySyncService) updateProgress(update func(*repository.HistorySyncStatus)) {
	s.mu.Lock()
	update(&s.status)
	_ = s.store.SetHistorySyncStatus(s.status)
	s.mu.Unlock()
}

func (s *HistorySyncService) finish(state string, extra []repository.HistorySyncError) {
	now := time.Now().UnixMilli()
	s.mu.Lock()
	s.status.State = state
	s.status.Errors = append(s.status.Errors, extra...)
	s.status.FinishedAt = &now
	s.status.LastRunAt = &now
	s.running = false
	_ = s.store.SetHistorySyncStatus(s.status)
	status := s.statusWithPendingLocked()
	s.mu.Unlock()
	if s.broadcaster != nil {
		s.broadcaster.BroadcastMessage("chats_changed", map[string]interface{}{
			"reason":        "history_sync",
			"messagesAdded": status.MessagesAdded,
			"state":         status.State,
		})
	}
}

func firstNonBlank(values ...string) string {
	for _, value := range values {
		if strings.TrimSpace(value) != "" {
			return value
		}
	}
	return ""
}

func fallback(value, fallbackValue string) string {
	if strings.TrimSpace(value) == "" {
		return fallbackValue
	}
	return value
}

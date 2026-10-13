package whatsapp

import (
	"context"
	"fmt"
	"net/url"
	"sort"
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

const (
	// historyPageSize is how many messages one request to the phone asks for.
	historyPageSize = 50
	// historyWorkers chats are synced at the same time.
	historyWorkers = 3
	// historyResponseTimeout is how long to wait for the phone to answer one request.
	historyResponseTimeout = 25 * time.Second
	// historyMaxRounds bounds one chat (rounds x page size messages).
	historyMaxRounds = 4000
)

// historyPhone is what a sync needs from the WhatsApp connection; the real one
// wraps the client, tests supply a scripted phone.
type historyPhone interface {
	IsConnected() bool
	// RequestOlder asks the phone for the page of messages before anchor.
	RequestOlder(anchor *repository.MessageAnchor) error
	// Parse turns a staged web message into an event.
	Parse(chat waTypes.JID, msg *waWeb.WebMessageInfo) (*events.Message, error)
}

type clientPhone struct{ client *whatsappInfra.WhatsAppClient }

func (p clientPhone) IsConnected() bool { return p.client.IsConnected() }

func (p clientPhone) RequestOlder(anchor *repository.MessageAnchor) error {
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
	_, err = p.client.CoreClient().SendPeerMessage(ctx, p.client.CoreClient().BuildHistorySyncRequest(info, historyPageSize))
	if err != nil {
		return fmt.Errorf("request older history: %w", err)
	}
	return nil
}

func (p clientPhone) Parse(chat waTypes.JID, msg *waWeb.WebMessageInfo) (*events.Message, error) {
	return p.client.CoreClient().ParseWebMessage(chat, msg)
}

// HistorySyncService owns durable staging and the single manual import job.
// Downloading pairing packets is automatic, but visible tables are only
// touched from run(), after POST /api/history-sync.
type HistorySyncService struct {
	client      *whatsappInfra.WhatsAppClient
	phone       historyPhone
	store       *repository.MessageStore
	broadcaster HTTPServer
	// responseTimeout is how long to wait for the phone to answer one request.
	responseTimeout time.Duration

	mu      sync.Mutex
	stageMu sync.Mutex
	staging map[string]bool
	running bool
	cancel  context.CancelFunc
	current map[string]*repository.HistorySyncCurrent
	status  repository.HistorySyncStatus

	// A job can run in several segments (stopped, interrupted, resumed). The
	// time of the earlier ones is activeBase; segmentStart and segmentDone are
	// the running one's, and are what the time estimate is based on.
	activeBase   int64
	segmentStart time.Time
	segmentDone  int
}

func NewHistorySyncService(client *whatsappInfra.WhatsAppClient, store *repository.MessageStore) *HistorySyncService {
	s := &HistorySyncService{client: client, phone: clientPhone{client}, store: store, responseTimeout: historyResponseTimeout, staging: make(map[string]bool), current: map[string]*repository.HistorySyncCurrent{}}
	s.status = s.loadStatus()
	return s
}

// loadStatus reads the persisted status. A sync recorded as "running" cannot be
// running in a process that only just started, so it is closed as interrupted
// (the job keeps its place and can be resumed); otherwise the UI would show it
// as syncing forever.
func (s *HistorySyncService) loadStatus() repository.HistorySyncStatus {
	status, err := s.store.GetHistorySyncStatus()
	if err != nil {
		return repository.HistorySyncStatus{State: "idle", Errors: []repository.HistorySyncError{}, Current: []repository.HistorySyncCurrent{}}
	}
	applyJobTotals(&status, s.store)
	if status.State == "running" || status.State == "cancelling" {
		now := time.Now().UnixMilli()
		status.State = "interrupted"
		status.FinishedAt = &now
		_ = s.store.SetHistorySyncStatus(status)
	}
	return status
}

// applyJobTotals sets the counters of status from the chats of the job.
func applyJobTotals(status *repository.HistorySyncStatus, store *repository.MessageStore) {
	totals, err := store.HistoryJobTotals()
	if err != nil || (totals.Chats == 0 && status.ChatsSkipped == 0) {
		return // no job on record (an older sync): keep what was stored
	}
	status.ChatsTotal = status.ChatsSkipped + totals.Chats
	status.ChatsProcessed = status.ChatsSkipped + totals.Done
	status.MessagesAdded = totals.Added
	status.ChatsWithNew = totals.WithNew
	status.ChatsExhausted = totals.Exhausted
	status.Timeouts = totals.Timeouts
}

// Reload re-reads the persisted status after the message database was switched
// to another account.
func (s *HistorySyncService) Reload() {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.running {
		return // a running sync belongs to the previous account's job; it keeps its own status
	}
	s.status = s.loadStatus()
	s.current = map[string]*repository.HistorySyncCurrent{}
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

// Start begins a sync with the given depth. The zero value means "50 older
// messages per chat", which is what the first version of this feature did.
func (s *HistorySyncService) Start(opts repository.HistorySyncOptions) (repository.HistorySyncStatus, error) {
	if opts.PerChat < 0 || opts.SinceMs < 0 {
		return s.Status(), fmt.Errorf("invalid history sync options")
	}
	if opts.PerChat == 0 && opts.SinceMs == 0 && !opts.All {
		opts.PerChat = historyPageSize
	}
	if !s.phone.IsConnected() {
		return s.Status(), fmt.Errorf("WhatsApp is not connected")
	}
	s.mu.Lock()
	if s.running {
		status := s.statusWithPendingLocked()
		s.mu.Unlock()
		return status, fmt.Errorf("history sync is already running")
	}
	now := time.Now().UnixMilli()
	ctx, cancel := context.WithCancel(context.Background())
	_ = s.store.BeginHistoryJob(nil) // a new job replaces whatever was left of the last one
	s.running = true
	s.cancel = cancel
	s.current = map[string]*repository.HistorySyncCurrent{}
	s.activeBase, s.segmentStart, s.segmentDone = 0, time.Now(), 0
	s.status = repository.HistorySyncStatus{
		State:     "running",
		Errors:    []repository.HistorySyncError{},
		StartedAt: &now,
		Options:   opts,
	}
	_ = s.store.SetHistorySyncStatus(s.status)
	status := s.statusWithPendingLocked()
	s.mu.Unlock()
	go s.run(ctx, opts, false)
	return status, nil
}

// Resume continues a job that was stopped, interrupted by a restart or lost with
// the connection: chats it finished are not visited again, and its counters and
// options carry on.
func (s *HistorySyncService) Resume() (repository.HistorySyncStatus, error) {
	if !s.phone.IsConnected() {
		return s.Status(), fmt.Errorf("WhatsApp is not connected")
	}
	s.mu.Lock()
	if s.running {
		status := s.statusWithPendingLocked()
		s.mu.Unlock()
		return status, fmt.Errorf("history sync is already running")
	}
	if !s.resumableLocked() {
		status := s.statusWithPendingLocked()
		s.mu.Unlock()
		return status, fmt.Errorf("there is no history sync to resume")
	}
	ctx, cancel := context.WithCancel(context.Background())
	s.running = true
	s.cancel = cancel
	s.current = map[string]*repository.HistorySyncCurrent{}
	s.activeBase, s.segmentStart, s.segmentDone = s.status.ActiveMs, time.Now(), 0
	s.status.State = "running"
	s.status.FinishedAt = nil
	_ = s.store.SetHistorySyncStatus(s.status)
	status := s.statusWithPendingLocked()
	opts := s.status.Options
	s.mu.Unlock()
	go s.run(ctx, opts, true)
	return status, nil
}

// resumableLocked: the job stopped without finishing, and has chats left.
func (s *HistorySyncService) resumableLocked() bool {
	if s.running {
		return false
	}
	switch s.status.State {
	case "interrupted", "cancelled", "failed":
	default:
		return false
	}
	totals, err := s.store.HistoryJobTotals()
	return err == nil && totals.Chats > totals.Done
}

// elapsedLocked is how long the job has actually run, stopped time not counted.
func (s *HistorySyncService) elapsedLocked() int64 {
	if !s.running {
		return s.status.ActiveMs
	}
	return s.activeBase + time.Since(s.segmentStart).Milliseconds()
}

// etaLocked is a rough time left from the pace of the running segment, nil
// until enough chats are done to say anything.
func (s *HistorySyncService) etaLocked() *int64 {
	if !s.running {
		return nil
	}
	remaining := s.status.ChatsTotal - s.status.ChatsProcessed
	spent := time.Since(s.segmentStart).Milliseconds()
	if s.segmentDone < 3 || spent < 10_000 || remaining <= 0 {
		return nil
	}
	eta := spent * int64(remaining) / int64(s.segmentDone)
	return &eta
}

// Cancel asks the running sync to stop after the messages it is importing.
func (s *HistorySyncService) Cancel() (repository.HistorySyncStatus, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if !s.running {
		return s.statusWithPendingLocked(), fmt.Errorf("no history sync is running")
	}
	s.status.State = "cancelling"
	if s.cancel != nil {
		s.cancel()
	}
	return s.statusWithPendingLocked(), nil
}

func (s *HistorySyncService) Status() repository.HistorySyncStatus {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.statusWithPendingLocked()
}

func (s *HistorySyncService) statusWithPendingLocked() repository.HistorySyncStatus {
	status := s.status
	status.Resumable = s.resumableLocked()
	status.ElapsedMs = s.elapsedLocked()
	status.EtaMs = s.etaLocked()
	// Never nil: clients read "errors" as a list, and JSON null is not one.
	status.Errors = append([]repository.HistorySyncError{}, status.Errors...)
	status.Current = make([]repository.HistorySyncCurrent, 0, len(s.current))
	for _, c := range s.current {
		status.Current = append(status.Current, *c)
	}
	sort.Slice(status.Current, func(i, j int) bool { return status.Current[i].ChatID < status.Current[j].ChatID })
	status.PendingChats, status.PendingMessages, _ = s.store.PendingHistoryCounts()
	status.ChatsFinished = s.store.FinishedChatCount()
	return status
}

func (s *HistorySyncService) run(ctx context.Context, opts repository.HistorySyncOptions, resume bool) {
	defer func() {
		if r := recover(); r != nil {
			s.finish("failed", []repository.HistorySyncError{{Message: fmt.Sprintf("history sync crashed: %v", r)}})
		}
	}()
	if !resume {
		targets, err := s.store.HistoryTargets()
		if err != nil {
			s.finish("failed", []repository.HistorySyncError{{Message: err.Error()}})
			return
		}
		var todo []string
		for _, t := range targets {
			if t.Exhausted && t.Pending == 0 && !opts.IncludeExhausted {
				continue
			}
			todo = append(todo, t.ChatID)
		}
		if err := s.store.BeginHistoryJob(todo); err != nil {
			s.finish("failed", []repository.HistorySyncError{{Message: err.Error()}})
			return
		}
		// Chats already at the start of their history count as done.
		s.updateProgress(func(status *repository.HistorySyncStatus) {
			status.ChatsSkipped = len(targets) - len(todo)
			status.ChatsTotal = len(targets)
		})
	}
	s.refreshTotals()
	todo, err := s.store.PendingHistoryJobChats()
	if err != nil {
		s.finish("failed", []repository.HistorySyncError{{Message: err.Error()}})
		return
	}

	jobs := make(chan repository.HistoryTarget)
	var wg sync.WaitGroup
	for i := 0; i < historyWorkers; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for target := range jobs {
				s.syncChat(ctx, target, opts)
			}
		}()
	}
feed:
	for _, chatID := range todo {
		select {
		case <-ctx.Done():
			break feed
		case jobs <- repository.HistoryTarget{ChatID: chatID}:
		}
	}
	close(jobs)
	wg.Wait()

	if ctx.Err() != nil {
		s.finish("cancelled", nil)
		return
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

// recordChat saves how far chatID got in the job and refreshes the counters.
// A chat that is not done (the sync was stopped) is visited again on resume.
func (s *HistorySyncService) recordChat(chatID string, added int, done bool, outcome string) {
	if err := s.store.RecordHistoryJobChat(chatID, added, done, outcome); err != nil {
		s.addError(chatID, err.Error())
	}
	if done {
		s.mu.Lock()
		s.segmentDone++
		s.mu.Unlock()
	}
	s.refreshTotals()
}

// refreshTotals recomputes the counters from the job's chats.
func (s *HistorySyncService) refreshTotals() {
	s.updateProgress(func(status *repository.HistorySyncStatus) {
		applyJobTotals(status, s.store)
	})
}

// syncChat takes one chat back as far as opts allows: it imports what the phone
// already delivered, and keeps asking the phone for older pages until the cap or
// the time limit is reached, or the phone has nothing more.
func (s *HistorySyncService) syncChat(ctx context.Context, target repository.HistoryTarget, opts repository.HistorySyncOptions) {
	chatID := target.ChatID
	cur := &repository.HistorySyncCurrent{ChatID: chatID, Name: s.store.HistoryChatName(chatID), Phase: "importing"}
	s.mu.Lock()
	s.current[chatID] = cur
	s.mu.Unlock()
	setPhase := func(phase string, added int) {
		s.mu.Lock()
		cur.Phase, cur.Added = phase, added
		s.mu.Unlock()
	}

	added, exhausted, timedOut := 0, false, false
	lastAnchor := ""
	for round := 0; round < historyMaxRounds && ctx.Err() == nil; round++ {
		if opts.PerChat > 0 && added >= opts.PerChat {
			break
		}
		batch := historyPageSize
		if opts.PerChat > 0 && opts.PerChat-added < batch {
			batch = opts.PerChat - added
		}
		n, consumed, reachedSince, err := s.importPending(chatID, batch, opts.SinceMs)
		if err != nil {
			s.addError(chatID, err.Error())
			break
		}
		added += n
		setPhase("importing", added)
		if reachedSince {
			break
		}
		if consumed > 0 {
			continue
		}

		// Nothing staged: ask the phone for the page before our oldest message.
		anchor, anchorErr := s.store.OldestMessageAnchor(chatID)
		if anchorErr != nil {
			break // a chat without a single message cannot be anchored
		}
		if opts.SinceMs > 0 && anchor.Timestamp < opts.SinceMs {
			break
		}
		if anchor.MessageID == lastAnchor {
			exhausted = true // asked from here already and nothing new came back
			break
		}
		lastAnchor = anchor.MessageID
		if !s.phone.IsConnected() {
			s.addError(chatID, "WhatsApp disconnected during history sync")
			s.cancelRun()
			break
		}
		setPhase("asking-phone", added)
		stagedAt := s.store.HistoryConversationUpdatedAt(chatID)
		if requestErr := s.phone.RequestOlder(anchor); requestErr != nil {
			s.addError(chatID, requestErr.Error())
			break
		}
		if !s.waitForHistoryResponse(ctx, chatID, stagedAt, s.responseTimeout) {
			if ctx.Err() == nil {
				timedOut = true
			}
			break
		}
	}

	outcome := ""
	if exhausted || timedOut {
		// The phone had nothing (more) for this chat. A later sync skips it
		// unless asked to retry.
		_ = s.store.MarkHistoryExhausted(chatID)
		outcome = "exhausted"
		if timedOut {
			outcome = "timeout"
		}
	}
	s.mu.Lock()
	delete(s.current, chatID)
	s.mu.Unlock()
	// Stopped mid-chat: what was imported counts, but the chat is not done and
	// is picked up again when the job resumes.
	s.recordChat(chatID, added, ctx.Err() == nil, outcome)
}

func (s *HistorySyncService) cancelRun() {
	s.mu.Lock()
	if s.cancel != nil {
		s.cancel()
	}
	s.mu.Unlock()
}

func (s *HistorySyncService) waitForHistoryResponse(ctx context.Context, chatID string, previousUpdate int64, timeout time.Duration) bool {
	deadline := time.Now().Add(timeout)
	for time.Now().Before(deadline) {
		if s.store.HistoryConversationUpdatedAt(chatID) > previousUpdate {
			return true
		}
		select {
		case <-ctx.Done():
			return false
		case <-time.After(300 * time.Millisecond):
		}
	}
	return false
}

// importPending imports up to limit staged messages of chatID. Messages older
// than sinceMs are left staged and reachedSince reports that the time limit was
// hit. consumed is how many staged rows were used up (some may already exist).
func (s *HistorySyncService) importPending(chatID string, limit int, sinceMs int64) (added, consumed int, reachedSince bool, err error) {
	staged, err := s.store.PendingHistory(chatID, limit)
	if err != nil || len(staged) == 0 {
		return 0, 0, false, err
	}
	eligible := staged[:0:0]
	for _, item := range staged {
		if sinceMs > 0 && item.Timestamp < sinceMs {
			reachedSince = true
			continue
		}
		eligible = append(eligible, item)
	}
	if len(eligible) == 0 {
		return 0, 0, reachedSince, nil
	}
	imports := make([]repository.HistoryImportMessage, 0, len(eligible))
	for _, item := range eligible {
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
	added, err = s.store.ImportHistoryBatch(chatID, eligible, imports)
	if err != nil {
		return 0, 0, reachedSince, err
	}
	return added, len(eligible), reachedSince, nil
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
	evt, err := s.phone.Parse(jid, &webMsg)
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
	s.status.ActiveMs = s.elapsedLocked()
	_ = s.store.SetHistorySyncStatus(s.status)
	s.mu.Unlock()
}

func (s *HistorySyncService) finish(state string, extra []repository.HistorySyncError) {
	now := time.Now().UnixMilli()
	s.mu.Lock()
	s.status.ActiveMs = s.elapsedLocked()
	s.status.State = state
	s.status.Errors = append(s.status.Errors, extra...)
	s.status.FinishedAt = &now
	s.status.LastRunAt = &now
	s.running = false
	s.current = map[string]*repository.HistorySyncCurrent{}
	if s.cancel != nil {
		s.cancel()
		s.cancel = nil
	}
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

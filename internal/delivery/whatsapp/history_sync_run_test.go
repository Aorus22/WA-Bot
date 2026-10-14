package whatsapp

import (
	"encoding/json"
	"fmt"
	"strings"
	"path/filepath"
	"sync"
	"testing"
	"time"

	waCommon "go.mau.fi/whatsmeow/proto/waCommon"
	waE2E "go.mau.fi/whatsmeow/proto/waE2E"
	waWeb "go.mau.fi/whatsmeow/proto/waWeb"
	waTypes "go.mau.fi/whatsmeow/types"
	"go.mau.fi/whatsmeow/types/events"
	"google.golang.org/protobuf/proto"

	"wa-bot/internal/domain/repository"
)

const testChat = "628123@s.whatsapp.net"

// scriptedPhone holds a chat's full history (message i has timestamp i seconds,
// newest = highest) and answers "older than anchor" requests like the phone
// does: one page of historyPageSize, staged into the store.
type scriptedPhone struct {
	store     *repository.MessageStore
	total     int  // messages that exist on the phone, ts 1..total
	silent    bool // never answers (offline phone / nothing to give)
	requests  int
	mu        sync.Mutex
	connected bool
	onRequest func()
}

func (p *scriptedPhone) IsConnected() bool { return p.connected }

func (p *scriptedPhone) RequestOlder(anchor *repository.MessageAnchor) error {
	p.mu.Lock()
	p.requests++
	cb := p.onRequest
	p.mu.Unlock()
	if cb != nil {
		cb()
	}
	if p.silent {
		return nil
	}
	oldest := int(anchor.Timestamp / 1000) // anchor ts is in ms
	var page []repository.StagedHistoryMessage
	for ts := oldest - 1; ts >= 1 && len(page) < historyPageSize; ts-- {
		page = append(page, stagedMessage(ts))
	}
	// An answer with no messages still names the conversation, as a real one may.
	return p.store.StageHistoryConversation(repository.HistoryConversation{ChatID: anchor.ChatID, Name: "Contact"}, page)
}

func stagedMessage(ts int) repository.StagedHistoryMessage {
	id := fmt.Sprintf("m-%05d", ts)
	web := &waWeb.WebMessageInfo{
		Key:              &waCommon.MessageKey{ID: proto.String(id), RemoteJID: proto.String(testChat), FromMe: proto.Bool(false)},
		Message:          &waE2E.Message{Conversation: proto.String("text " + id)},
		MessageTimestamp: proto.Uint64(uint64(ts)),
	}
	raw, _ := proto.Marshal(web)
	return repository.StagedHistoryMessage{ChatID: testChat, MessageID: id, Timestamp: int64(ts) * 1000, Raw: raw}
}

func (p *scriptedPhone) Parse(chat waTypes.JID, msg *waWeb.WebMessageInfo) (*events.Message, error) {
	return &events.Message{
		Info: waTypes.MessageInfo{
			MessageSource: waTypes.MessageSource{Chat: chat, Sender: chat},
			ID:            waTypes.MessageID(msg.GetKey().GetID()),
			Timestamp:     time.Unix(int64(msg.GetMessageTimestamp()), 0),
		},
		Message: msg.GetMessage(),
	}, nil
}

func newSyncUnderTest(t *testing.T, total, haveNewest int) (*HistorySyncService, *scriptedPhone, *repository.MessageStore) {
	t.Helper()
	store, err := repository.NewMessageStore(filepath.Join(t.TempDir(), "m.db"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { store.Close() })
	// The app already has the newest `haveNewest` messages of the chat.
	for ts := total; ts > total-haveNewest; ts-- {
		if err := store.SaveMessage(&repository.Message{
			ID: fmt.Sprintf("m-%05d", ts), ChatID: testChat, From: testChat, To: "me",
			Content: "x", Timestamp: int64(ts) * 1000, Status: "received", Type: "text",
		}); err != nil {
			t.Fatal(err)
		}
	}
	phone := &scriptedPhone{store: store, total: total, connected: true}
	svc := &HistorySyncService{phone: phone, store: store, responseTimeout: 300 * time.Millisecond,
		staging: map[string]bool{}, current: map[string]*repository.HistorySyncCurrent{}}
	svc.status = repository.HistorySyncStatus{State: "idle", Errors: []repository.HistorySyncError{}}
	return svc, phone, store
}

func waitDone(t *testing.T, svc *HistorySyncService) repository.HistorySyncStatus {
	t.Helper()
	deadline := time.Now().Add(20 * time.Second)
	for time.Now().Before(deadline) {
		st := svc.Status()
		if st.State != "running" && st.State != "cancelling" {
			return st
		}
		time.Sleep(20 * time.Millisecond)
	}
	t.Fatal("sync did not finish")
	return repository.HistorySyncStatus{}
}

func messageCount(t *testing.T, store *repository.MessageStore) int {
	t.Helper()
	n, _ := store.ChatMessageStats(testChat)
	return n
}

func TestSyncPerChatLimit(t *testing.T) {
	svc, _, store := newSyncUnderTest(t, 400, 3)
	if _, err := svc.Start(repository.HistorySyncOptions{}); err != nil { // default: 50 per chat
		t.Fatal(err)
	}
	st := waitDone(t, svc)
	if st.State != "completed" || st.MessagesAdded != 50 || messageCount(t, store) != 53 {
		t.Fatalf("default sync: %+v, %d messages", st, messageCount(t, store))
	}

	svc2, _, store2 := newSyncUnderTest(t, 400, 3)
	svc2.Start(repository.HistorySyncOptions{PerChat: 120})
	st = waitDone(t, svc2)
	if st.MessagesAdded != 120 || messageCount(t, store2) != 123 || st.ChatsWithNew != 1 {
		t.Fatalf("120 per chat: %+v", st)
	}
}

func TestSyncEverythingReachesTheStartAndRemembersIt(t *testing.T) {
	svc, phone, store := newSyncUnderTest(t, 237, 3)
	svc.Start(repository.HistorySyncOptions{All: true, IncludeExhausted: true})
	st := waitDone(t, svc)
	if st.State != "completed" || messageCount(t, store) != 237 || st.MessagesAdded != 234 {
		t.Fatalf("everything: %+v, %d messages", st, messageCount(t, store))
	}
	if st.ChatsExhausted != 1 || st.Timeouts != 0 {
		t.Fatalf("expected the chat to be recorded as fully synced: %+v", st)
	}
	asked := phone.requests

	// A normal sync afterwards leaves the finished chat alone, and the status
	// says how many chats that is.
	if st := svc.Status(); st.ChatsFinished != 1 {
		t.Fatalf("finished chats not reported: %+v", st)
	}
	svc.Start(repository.HistorySyncOptions{})
	st = waitDone(t, svc)
	if st.ChatsSkipped != 1 || phone.requests != asked || st.ChatsProcessed != st.ChatsTotal {
		t.Fatalf("finished chat was not skipped: %+v (requests %d -> %d)", st, asked, phone.requests)
	}
	// "Everything" without the retry option skips it too.
	svc.Start(repository.HistorySyncOptions{All: true})
	st = waitDone(t, svc)
	if st.ChatsSkipped != 1 || phone.requests != asked {
		t.Fatalf("everything retried a finished chat without being asked: %+v", st)
	}
	// With the option it asks again.
	svc.Start(repository.HistorySyncOptions{All: true, IncludeExhausted: true})
	st = waitDone(t, svc)
	if st.ChatsSkipped != 0 || phone.requests <= asked {
		t.Fatalf("everything did not retry: %+v", st)
	}
}

func TestSyncSinceStopsAtTheTimeLimit(t *testing.T) {
	svc, _, store := newSyncUnderTest(t, 500, 3)
	// Only messages from ts >= 300 onwards (up to the newest 500).
	svc.Start(repository.HistorySyncOptions{SinceMs: 300 * 1000})
	st := waitDone(t, svc)
	if st.State != "completed" {
		t.Fatalf("state %s", st.State)
	}
	_, oldest := store.ChatMessageStats(testChat)
	if oldest < 300*1000 {
		t.Fatalf("went back past the limit: oldest %d", oldest)
	}
	if got := messageCount(t, store); got < 200 || got > 203+historyPageSize {
		t.Fatalf("expected about 200 messages, got %d", got)
	}
}

func TestSyncTimeoutIsCountedAndDoesNotHang(t *testing.T) {
	svc, phone, _ := newSyncUnderTest(t, 100, 3)
	phone.silent = true
	svc.Start(repository.HistorySyncOptions{PerChat: 50})
	st := waitDone(t, svc)
	if st.Timeouts != 1 || st.MessagesAdded != 0 || st.ChatsProcessed != 1 {
		t.Fatalf("silent phone: %+v", st)
	}
}

func TestSyncCanBeCancelled(t *testing.T) {
	svc, phone, store := newSyncUnderTest(t, 5000, 3)
	started := make(chan struct{})
	var once sync.Once
	phone.onRequest = func() { once.Do(func() { close(started) }); time.Sleep(30 * time.Millisecond) }
	svc.Start(repository.HistorySyncOptions{All: true, IncludeExhausted: true})
	<-started
	if st, err := svc.Cancel(); err != nil || st.State != "cancelling" {
		t.Fatalf("cancel: %+v %v", st, err)
	}
	st := waitDone(t, svc)
	if st.State != "cancelled" {
		t.Fatalf("state %s", st.State)
	}
	if n := messageCount(t, store); n >= 5000 {
		t.Fatal("cancel did not stop the sync")
	}
	if _, err := svc.Cancel(); err == nil {
		t.Fatal("cancelling an idle sync should fail")
	}
	// The same service can run again afterwards.
	if _, err := svc.Start(repository.HistorySyncOptions{PerChat: 10}); err != nil {
		t.Fatalf("restart after cancel: %v", err)
	}
	waitDone(t, svc)
}

func TestSyncRefusesToStartTwiceAndReportsProgress(t *testing.T) {
	svc, phone, _ := newSyncUnderTest(t, 3000, 3)
	release := make(chan struct{})
	phone.onRequest = func() { <-release }
	svc.Start(repository.HistorySyncOptions{All: true, IncludeExhausted: true})
	deadline := time.Now().Add(5 * time.Second)
	var st repository.HistorySyncStatus
	for time.Now().Before(deadline) {
		if st = svc.Status(); len(st.Current) == 1 && st.Current[0].Phase == "asking-phone" {
			break
		}
		time.Sleep(10 * time.Millisecond)
	}
	if len(st.Current) != 1 || st.Current[0].ChatID != testChat || st.Current[0].Name == "" {
		t.Fatalf("no live detail: %+v", st)
	}
	if _, err := svc.Start(repository.HistorySyncOptions{}); err == nil {
		t.Fatal("a second sync started while one was running")
	}
	close(release)
	svc.Cancel()
	waitDone(t, svc)
}

func TestInterruptedRunIsNotShownAsRunningForever(t *testing.T) {
	svc, _, store := newSyncUnderTest(t, 10, 3)
	now := time.Now().UnixMilli()
	store.SetHistorySyncStatus(repository.HistorySyncStatus{State: "running", StartedAt: &now, Errors: []repository.HistorySyncError{}})
	svc.Reload() // what happens after the account's database is bound
	st := svc.Status()
	if st.State != "interrupted" || len(st.Errors) != 0 {
		t.Fatalf("a run that died with the process shows as %q with errors %v", st.State, st.Errors)
	}
	if st.Resumable {
		t.Fatal("a run with no job on record cannot be resumed")
	}
}

func TestInterruptedRunCanBeResumedAfterARestart(t *testing.T) {
	svc, phone, store := newSyncUnderTest(t, 237, 3)
	started := make(chan struct{})
	var once sync.Once
	phone.onRequest = func() { once.Do(func() { close(started) }); time.Sleep(30 * time.Millisecond) }
	svc.Start(repository.HistorySyncOptions{All: true, IncludeExhausted: true})
	<-started
	// The process dies here: the persisted status still says "running".
	svc.cancelRun()
	waitDone(t, svc)
	persisted, _ := store.GetHistorySyncStatus()
	persisted.State = "running"
	persisted.FinishedAt = nil
	store.SetHistorySyncStatus(persisted)

	// A new process: same database, fresh service.
	phone.onRequest = nil
	fresh := &HistorySyncService{phone: phone, store: store, responseTimeout: 300 * time.Millisecond,
		staging: map[string]bool{}, current: map[string]*repository.HistorySyncCurrent{}}
	fresh.status = fresh.loadStatus()
	st := fresh.Status()
	if st.State != "interrupted" || !st.Resumable || st.ChatsTotal != 1 || st.ChatsProcessed != 0 || st.StartedAt == nil {
		t.Fatalf("after the restart: %+v", st)
	}
	addedBefore := st.MessagesAdded

	if _, err := fresh.Resume(); err != nil {
		t.Fatalf("resume: %v", err)
	}
	st = waitDone(t, fresh)
	if st.State != "completed" || messageCount(t, store) != 237 || st.ChatsProcessed != 1 {
		t.Fatalf("resumed run: %+v, %d messages", st, messageCount(t, store))
	}
	// Counters carry on: what the first part added plus what the second did, once.
	if st.MessagesAdded != 234 || st.MessagesAdded < addedBefore {
		t.Fatalf("messages added %d (before the restart %d), want 234", st.MessagesAdded, addedBefore)
	}
	if st.Resumable || st.ElapsedMs <= 0 {
		t.Fatalf("a finished job is not resumable and has run for a while: %+v", st)
	}
	if _, err := fresh.Resume(); err == nil {
		t.Fatal("resumed a job that is finished")
	}
}

func TestStoppedSyncKeepsItsPlaceAndNewStartDiscardsIt(t *testing.T) {
	svc, phone, _ := newSyncUnderTest(t, 5000, 3)
	started := make(chan struct{})
	var once sync.Once
	phone.onRequest = func() { once.Do(func() { close(started) }); time.Sleep(30 * time.Millisecond) }
	svc.Start(repository.HistorySyncOptions{All: true, IncludeExhausted: true})
	<-started
	svc.Cancel()
	st := waitDone(t, svc)
	if st.State != "cancelled" || !st.Resumable || st.ChatsProcessed != 0 {
		t.Fatalf("stopped sync: %+v", st)
	}
	if _, err := svc.Resume(); err != nil {
		t.Fatal(err)
	}
	if st := svc.Status(); st.State != "running" || st.Resumable || !st.Options.All {
		t.Fatalf("resumed sync should run with its own options: %+v", st)
	}
	if _, err := svc.Resume(); err == nil {
		t.Fatal("resumed a sync that is running")
	}
	svc.Cancel()
	waitDone(t, svc)

	if _, err := svc.Start(repository.HistorySyncOptions{PerChat: 10}); err != nil {
		t.Fatal(err)
	}
	st = waitDone(t, svc)
	if st.Resumable || st.State != "completed" {
		t.Fatalf("a fresh sync starts over: %+v", st)
	}
}

func TestResumeNeedsAConnectionAndAJob(t *testing.T) {
	svc, phone, _ := newSyncUnderTest(t, 10, 3)
	if _, err := svc.Resume(); err == nil {
		t.Fatal("resumed with nothing to resume")
	}
	phone.connected = false
	if _, err := svc.Resume(); err == nil {
		t.Fatal("resumed while WhatsApp is disconnected")
	}
}

func TestStatusJSONNeverHasNullLists(t *testing.T) {
	svc, _, _ := newSyncUnderTest(t, 10, 3)
	raw, err := json.Marshal(svc.Status())
	if err != nil {
		t.Fatal(err)
	}
	for _, field := range []string{`"errors":null`, `"current":null`} {
		if strings.Contains(string(raw), field) {
			t.Fatalf("status JSON has %s: clients cannot read a null list\n%s", field, raw)
		}
	}
	started, err := svc.Start(repository.HistorySyncOptions{PerChat: 5})
	if err != nil {
		t.Fatal(err)
	}
	if raw, _ := json.Marshal(started); strings.Contains(string(raw), `"errors":null`) {
		t.Fatalf("start response has a null errors list: %s", raw)
	}
	waitDone(t, svc)
}

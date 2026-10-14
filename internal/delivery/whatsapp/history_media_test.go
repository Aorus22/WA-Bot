package whatsapp

import (
	"context"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"

	"wa-bot/internal/domain/repository"
)

const mediaChat = "628123@s.whatsapp.net"

// scriptedMedia answers downloads by message id (the raw message is just the id).
type scriptedMedia struct {
	mu        sync.Mutex
	connected bool
	gone      map[string]bool // never downloadable
	broken    map[string]bool // fails with a network error
	attempts  map[string]int
	// What the phone does when asked to upload a file again: "" = nothing was
	// asked of it, "ok" = has the file, "missing" = does not, "silent" = no answer.
	phone      map[string]string
	phoneAsked map[string]int
	phoneNow   int // requests open right now
	phoneMax   int // the most that were open at once
	phoneHold  time.Duration
	hold       chan struct{} // when set, downloads wait for it (or the context)
	started    chan struct{}
	startOnce  sync.Once
}

func (m *scriptedMedia) IsConnected() bool { return m.connected }

func (m *scriptedMedia) Download(ctx context.Context, _ string, raw []byte, file *os.File) (string, string, error) {
	id := string(raw)
	m.mu.Lock()
	m.attempts[id]++
	hold, started := m.hold, m.started
	m.mu.Unlock()
	if started != nil {
		m.startOnce.Do(func() { close(started) })
	}
	if hold != nil {
		select {
		case <-hold:
		case <-ctx.Done():
			return "", "", ctx.Err()
		}
	}
	switch {
	case m.gone[id]:
		return "", "", fmt.Errorf("%w: 404", errMediaGone)
	case m.broken[id]:
		return "", "", fmt.Errorf("connection reset")
	}
	_, err := file.WriteString("bytes of " + id)
	return "image/jpeg", "image.jpg", err
}

func (m *scriptedMedia) RequestFromPhone(ctx context.Context, _ string, raw []byte, file *os.File) (string, string, error) {
	id := string(raw)
	m.mu.Lock()
	m.phoneAsked[id]++
	m.phoneNow++
	if m.phoneNow > m.phoneMax {
		m.phoneMax = m.phoneNow
	}
	behaviour, hold := m.phone[id], m.phoneHold
	m.mu.Unlock()
	defer func() {
		m.mu.Lock()
		m.phoneNow--
		m.mu.Unlock()
	}()
	if hold > 0 {
		select {
		case <-time.After(hold):
		case <-ctx.Done():
			return "", "", ctx.Err()
		}
	}
	switch behaviour {
	case "ok":
		_, err := file.WriteString("from the phone: " + id)
		return "image/jpeg", "image.jpg", err
	case "missing":
		return "", "", errNotOnPhone
	default:
		return "", "", errPhoneSilent
	}
}

type memoryFiles struct {
	mu    sync.Mutex
	saved map[string]string
	saves map[string]int
}

func (f *memoryFiles) Save(_ context.Context, path string, r io.Reader) (string, error) {
	data, err := io.ReadAll(r)
	f.mu.Lock()
	defer f.mu.Unlock()
	f.saved[path] = string(data)
	f.saves[path]++
	return path, err
}

// addMedia imports history messages that still have the on-demand placeholder.
func addMedia(t *testing.T, store *repository.MessageStore, msgType string, ids ...string) {
	t.Helper()
	var imports []repository.HistoryImportMessage
	for i, id := range ids {
		imports = append(imports, repository.HistoryImportMessage{
			Message: repository.Message{
				ID: id, ChatID: mediaChat, From: mediaChat, To: "me", Content: "[" + msgType + "]",
				Timestamp: int64(1000 + i), Status: "received", Type: msgType,
				MediaURL: fmt.Sprintf("/chats/%s/messages/%s/media", mediaChat, id),
			},
			Raw: []byte(id),
		})
	}
	if _, err := store.ImportHistoryBatch(mediaChat, nil, imports); err != nil {
		t.Fatal(err)
	}
}

func mediaURL(t *testing.T, store *repository.MessageStore, id string) string {
	t.Helper()
	_, url, err := store.GetRawMessage(id)
	if err != nil {
		t.Fatal(err)
	}
	return url
}

func newMediaServiceUnderTest(t *testing.T) (*HistoryMediaService, *scriptedMedia, *memoryFiles, *repository.MessageStore) {
	t.Helper()
	store, err := repository.NewMessageStore(filepath.Join(t.TempDir(), "m.db"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { store.Close() })
	src := &scriptedMedia{connected: true, gone: map[string]bool{}, broken: map[string]bool{}, attempts: map[string]int{},
		phone: map[string]string{}, phoneAsked: map[string]int{}}
	files := &memoryFiles{saved: map[string]string{}, saves: map[string]int{}}
	svc := &HistoryMediaService{source: src, files: files, store: store, timeout: 10 * time.Second}
	svc.status = svc.loadStatus()
	return svc, src, files, store
}

func waitMediaDone(t *testing.T, svc *HistoryMediaService) repository.HistoryMediaStatus {
	t.Helper()
	deadline := time.Now().Add(20 * time.Second)
	for time.Now().Before(deadline) {
		if st := svc.Status(); st.State != "running" && st.State != "cancelling" {
			return st
		}
		time.Sleep(10 * time.Millisecond)
	}
	t.Fatal("media download did not finish")
	return repository.HistoryMediaStatus{}
}

func photos(opts ...string) repository.HistoryMediaOptions {
	if len(opts) == 0 {
		opts = []string{"image"}
	}
	return repository.HistoryMediaOptions{Types: opts}
}

func TestMediaDownloadStoresFilesAndPointsMessagesAtThem(t *testing.T) {
	svc, _, files, store := newMediaServiceUnderTest(t)
	addMedia(t, store, "image", "i1", "i2", "i3")
	addMedia(t, store, "sticker", "s1")
	addMedia(t, store, "video", "v1")

	if st := svc.Status(); st.Available["image"] != 3 || st.Available["sticker"] != 1 || st.Available["video"] != 1 || st.Available["audio"] != 0 {
		t.Fatalf("available: %+v", st.Available)
	}
	if _, err := svc.Start(photos("image", "sticker")); err != nil {
		t.Fatal(err)
	}
	st := waitMediaDone(t, svc)
	if st.State != "completed" || st.Done != 4 || st.Total != 4 || st.Remaining != 0 || st.Bytes == 0 {
		t.Fatalf("status: %+v", st)
	}
	url := mediaURL(t, store, "i1")
	if !strings.HasPrefix(url, "/media/history/") || !strings.HasSuffix(url, "i1.jpg") {
		t.Fatalf("message not pointed at its file: %q", url)
	}
	if files.saved[strings.TrimPrefix(url, "/media/")] != "bytes of i1" {
		t.Fatalf("file not stored: %v", files.saved)
	}
	if got := mediaURL(t, store, "v1"); !strings.Contains(got, "/messages/") {
		t.Fatalf("a video was downloaded though only photos and stickers were chosen: %q", got)
	}
	if st.Available["image"] != 0 || st.Available["video"] != 1 {
		t.Fatalf("available after: %+v", st.Available)
	}
}

func TestMediaThatIsGoneIsRememberedUntilAskedToRetry(t *testing.T) {
	svc, src, _, store := newMediaServiceUnderTest(t)
	addMedia(t, store, "image", "ok", "old")
	src.gone["old"] = true

	svc.Start(photos())
	st := waitMediaDone(t, svc)
	if st.State != "completed" || st.Done != 1 || st.Unavailable != 1 || st.Total != 2 {
		t.Fatalf("first job: %+v", st)
	}
	if st.Available["image"] != 0 || st.Expired["image"] != 1 {
		t.Fatalf("media found to be gone is offered as expired, not as available: %+v / %+v", st.Available, st.Expired)
	}

	svc.Start(photos())
	st = waitMediaDone(t, svc)
	if src.attempts["old"] != 1 || st.Total != 0 {
		t.Fatalf("gone media was tried again: attempts %d, %+v", src.attempts["old"], st)
	}

	src.gone["old"] = false
	svc.Start(repository.HistoryMediaOptions{Types: []string{"image"}, RetryUnavailable: true})
	st = waitMediaDone(t, svc)
	if src.attempts["old"] != 2 || st.Done != 1 || st.Unavailable != 0 {
		t.Fatalf("retry: attempts %d, %+v", src.attempts["old"], st)
	}
	if !strings.HasPrefix(mediaURL(t, store, "old"), "/media/history/") {
		t.Fatal("retried media was not stored")
	}
}

func TestMediaFailuresAreNotRetriedInTheSameJobButInTheNext(t *testing.T) {
	svc, src, _, store := newMediaServiceUnderTest(t)
	addMedia(t, store, "image", "a", "b")
	src.broken["b"] = true

	svc.Start(photos())
	st := waitMediaDone(t, svc)
	if st.State != "partial" || st.Failed != 1 || st.Done != 1 || src.attempts["b"] != 1 || len(st.Errors) == 0 {
		t.Fatalf("with a network error: %+v attempts %d", st, src.attempts["b"])
	}
	if st.Available["image"] != 1 {
		t.Fatalf("a failed download should still be on offer: %+v", st.Available)
	}

	src.broken["b"] = false
	svc.Start(photos())
	st = waitMediaDone(t, svc)
	if st.State != "completed" || st.Done != 1 || st.Failed != 0 || src.attempts["a"] != 1 {
		t.Fatalf("next job: %+v (a fetched %d times)", st, src.attempts["a"])
	}
}

func TestMediaDownloadCanBeStoppedAndResumed(t *testing.T) {
	svc, src, files, store := newMediaServiceUnderTest(t)
	var ids []string
	for i := 0; i < 40; i++ {
		ids = append(ids, fmt.Sprintf("m%02d", i))
	}
	addMedia(t, store, "image", ids...)
	src.hold = make(chan struct{})
	src.started = make(chan struct{})

	svc.Start(photos())
	<-src.started
	if st, err := svc.Cancel(); err != nil || st.State != "cancelling" {
		t.Fatalf("cancel: %+v %v", st, err)
	}
	st := waitMediaDone(t, svc)
	if st.State != "cancelled" || !st.Resumable || st.Remaining != 40 || st.Done != 0 {
		t.Fatalf("stopped: %+v", st)
	}

	src.mu.Lock()
	src.hold = nil
	src.mu.Unlock()
	if _, err := svc.Resume(); err != nil {
		t.Fatal(err)
	}
	st = waitMediaDone(t, svc)
	if st.State != "completed" || st.Done != 40 || st.Resumable || st.ElapsedMs <= 0 {
		t.Fatalf("resumed: %+v", st)
	}
	for _, id := range ids {
		if n := files.saves[fmt.Sprintf("history/628123_s_whatsapp_net/%s.jpg", id)]; n != 1 {
			t.Fatalf("%s stored %d times", id, n)
		}
	}
	if _, err := svc.Resume(); err == nil {
		t.Fatal("resumed a finished job")
	}
}

func TestMediaDownloadInterruptedByARestartCanBeResumed(t *testing.T) {
	svc, src, _, store := newMediaServiceUnderTest(t)
	addMedia(t, store, "image", "a", "b", "c")
	svc.Start(photos())
	waitMediaDone(t, svc)
	addMedia(t, store, "image", "d", "e") // two more arrive; the job "died" before reaching them

	persisted, _ := store.GetHistoryMediaStatus()
	persisted.State = "running"
	persisted.FinishedAt = nil
	store.SetHistoryMediaStatus(persisted)

	fresh := &HistoryMediaService{source: src, files: svc.files, store: store, timeout: 10 * time.Second}
	fresh.status = fresh.loadStatus()
	st := fresh.Status()
	if st.State != "interrupted" || !st.Resumable || st.Done != 3 || st.Remaining != 2 || st.Total != 5 {
		t.Fatalf("after the restart: %+v", st)
	}
	if _, err := fresh.Resume(); err != nil {
		t.Fatal(err)
	}
	st = waitMediaDone(t, fresh)
	if st.State != "completed" || st.Done != 5 || src.attempts["a"] != 1 {
		t.Fatalf("resumed: %+v (a fetched %d times)", st, src.attempts["a"])
	}
}

func TestMediaDownloadRefusesNonsense(t *testing.T) {
	svc, src, _, store := newMediaServiceUnderTest(t)
	addMedia(t, store, "image", "a")
	if _, err := svc.Start(repository.HistoryMediaOptions{}); err == nil {
		t.Fatal("started with nothing chosen")
	}
	if _, err := svc.Start(photos("gif")); err == nil {
		t.Fatal("started with an unknown kind")
	}
	if _, err := svc.Resume(); err == nil {
		t.Fatal("resumed with nothing to resume")
	}
	src.connected = false
	if _, err := svc.Start(photos()); err == nil {
		t.Fatal("started while WhatsApp is disconnected")
	}
	src.connected = true
	src.hold = make(chan struct{})
	src.started = make(chan struct{})
	svc.Start(photos())
	<-src.started
	if _, err := svc.Start(photos()); err == nil {
		t.Fatal("started twice")
	}
	svc.Cancel()
	waitMediaDone(t, svc)
	if _, err := svc.Cancel(); err == nil {
		t.Fatal("cancelled a job that is not running")
	}
}

func askingPhone() repository.HistoryMediaOptions {
	return repository.HistoryMediaOptions{Types: []string{"image"}, AskPhone: true}
}

func TestPhoneIsNotAskedUnlessTheOptionIsOn(t *testing.T) {
	svc, src, _, store := newMediaServiceUnderTest(t)
	addMedia(t, store, "image", "old")
	src.gone["old"] = true
	src.phone["old"] = "ok"

	svc.Start(photos())
	st := waitMediaDone(t, svc)
	if st.Unavailable != 1 || len(src.phoneAsked) != 0 {
		t.Fatalf("the phone was asked without the option: %+v asked %v", st, src.phoneAsked)
	}
	if st.Expired["image"] != 1 {
		t.Fatalf("the expired file should be on offer for the phone: %+v", st.Expired)
	}
}

func TestPhoneGivesBackExpiredMediaAndSaysWhenItHasNone(t *testing.T) {
	svc, src, files, store := newMediaServiceUnderTest(t)
	addMedia(t, store, "image", "fine", "back", "lost")
	for _, id := range []string{"back", "lost"} {
		src.gone[id] = true
	}
	src.phone["back"], src.phone["lost"] = "ok", "missing"

	// A first job without the phone finds the two expired files.
	svc.Start(photos())
	if st := waitMediaDone(t, svc); st.Unavailable != 2 || st.Done != 1 {
		t.Fatalf("first job: %+v", st)
	}

	// A job that asks the phone takes the expired ones up again.
	if _, err := svc.Start(askingPhone()); err != nil {
		t.Fatal(err)
	}
	st := waitMediaDone(t, svc)
	if st.State != "completed" || st.Done != 1 || st.Recovered != 1 || st.Missing != 1 || st.Unavailable != 0 || st.Total != 2 {
		t.Fatalf("with the phone: %+v", st)
	}
	url := mediaURL(t, store, "back")
	if files.saved[strings.TrimPrefix(url, "/media/")] != "from the phone: back" || !strings.HasPrefix(url, "/media/history/") {
		t.Fatalf("recovered file not stored: %q %v", url, files.saved)
	}
	if got := mediaURL(t, store, "lost"); !strings.Contains(got, "/messages/") {
		t.Fatalf("a file the phone does not have was stored: %q", got)
	}
	if st.Expired["image"] != 0 || st.Available["image"] != 0 {
		t.Fatalf("what the phone does not have is not on offer any more: %+v / %+v", st.Available, st.Expired)
	}

	// Not asked again, until asked to retry what is gone.
	svc.Start(askingPhone())
	waitMediaDone(t, svc)
	if src.phoneAsked["lost"] != 1 {
		t.Fatalf("the phone was asked again for a file it does not have: %v", src.phoneAsked)
	}
	svc.Start(repository.HistoryMediaOptions{Types: []string{"image"}, AskPhone: true, RetryUnavailable: true})
	waitMediaDone(t, svc)
	if src.phoneAsked["lost"] != 2 {
		t.Fatalf("retry did not ask again: %v", src.phoneAsked)
	}
}

func TestPhoneRequestsAreLimitedToAFewAtOnce(t *testing.T) {
	svc, src, _, store := newMediaServiceUnderTest(t)
	var ids []string
	for i := 0; i < 12; i++ {
		id := fmt.Sprintf("e%02d", i)
		ids = append(ids, id)
		src.gone[id] = true
		src.phone[id] = "ok"
	}
	addMedia(t, store, "image", ids...)
	src.phoneHold = 20 * time.Millisecond
	svc.Start(photos())
	waitMediaDone(t, svc)
	svc.Start(askingPhone())
	st := waitMediaDone(t, svc)
	if st.Recovered != 12 {
		t.Fatalf("recovered: %+v", st)
	}
	if src.phoneMax > historyMediaPhoneParallel || src.phoneMax < 2 {
		t.Fatalf("up to %d requests open at once, want 2", src.phoneMax)
	}
}

func TestJobStopsWhenThePhoneStopsAnsweringAndResumes(t *testing.T) {
	svc, src, _, store := newMediaServiceUnderTest(t)
	var ids []string
	for i := 0; i < 20; i++ {
		id := fmt.Sprintf("e%02d", i)
		ids = append(ids, id)
		src.gone[id] = true
		src.phone[id] = "silent"
	}
	addMedia(t, store, "image", ids...)
	svc.Start(photos())
	waitMediaDone(t, svc)

	svc.Start(askingPhone())
	st := waitMediaDone(t, svc)
	if st.State != "cancelled" || !st.Resumable {
		t.Fatalf("a silent phone should stop the job, resumable: %+v", st)
	}
	if asked := len(src.phoneAsked); asked > historyMediaPhoneSilentLimit+historyMediaPhoneParallel+historyMediaWorkers {
		t.Fatalf("%d requests went to a phone that never answers", asked)
	}
	if !strings.Contains(strings.Join(errorTexts(st), " "), "not answering") {
		t.Fatalf("the status does not say why: %v", st.Errors)
	}
	if st.Failed != 0 || st.Remaining != 20 {
		t.Fatalf("files that failed for want of an answer must be to do again: %+v", st)
	}

	// The phone wakes up; resuming finishes the job.
	src.mu.Lock()
	for _, id := range ids {
		src.phone[id] = "ok"
	}
	src.mu.Unlock()
	if _, err := svc.Resume(); err != nil {
		t.Fatal(err)
	}
	st = waitMediaDone(t, svc)
	if st.State != "completed" || st.Recovered != 20 {
		t.Fatalf("after the phone answers again: %+v", st)
	}
}

func TestAnIsolatedSilenceIsAFailureNotAReasonToStop(t *testing.T) {
	svc, src, _, store := newMediaServiceUnderTest(t)
	addMedia(t, store, "image", "a", "b", "c", "d", "e", "f")
	for _, id := range []string{"a", "b", "c", "d", "e", "f"} {
		src.gone[id] = true
		src.phone[id] = "ok"
	}
	src.phone["c"] = "silent"
	svc.Start(photos())
	waitMediaDone(t, svc)
	svc.Start(askingPhone())
	st := waitMediaDone(t, svc)
	if st.State != "partial" || st.Recovered != 5 || st.Failed != 1 {
		t.Fatalf("one unanswered request among answered ones: %+v", st)
	}
}

func errorTexts(st repository.HistoryMediaStatus) []string {
	var out []string
	for _, e := range st.Errors {
		out = append(out, e.Message)
	}
	return out
}

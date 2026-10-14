package whatsapp

import (
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"sync"
	"time"

	"go.mau.fi/whatsmeow"
	waMmsRetry "go.mau.fi/whatsmeow/proto/waMmsRetry"
	waWeb "go.mau.fi/whatsmeow/proto/waWeb"
	waTypes "go.mau.fi/whatsmeow/types"
	"go.mau.fi/whatsmeow/types/events"
	"google.golang.org/protobuf/proto"

	"wa-bot/internal/domain/repository"
	whatsappInfra "wa-bot/internal/infrastructure/whatsapp"
)

const (
	// historyMediaWorkers files are downloaded at the same time.
	historyMediaWorkers = 3
	// historyMediaTimeout bounds the download and storing of one file.
	historyMediaTimeout = 5 * time.Minute
	// historyMediaPage is how many messages are read from the database at once.
	historyMediaPage = 100
	// historyMediaMaxErrors is how many recent problems the status keeps.
	historyMediaMaxErrors = 20
	// historyMediaSaveEvery bounds how often progress is written to the database.
	historyMediaSaveEvery = time.Second
	// historyMediaIdleCache is how long counts of an idle job are reused.
	historyMediaIdleCache = 2 * time.Second
	// historyMediaPhoneParallel is how many requests to the phone are open at once.
	historyMediaPhoneParallel = 2
	// historyMediaPhoneWait is how long the phone has to answer one request.
	historyMediaPhoneWait = 45 * time.Second
	// historyMediaPhoneSilentLimit unanswered requests in a row stop the job: the
	// phone is probably offline, and waiting out thousands more helps nobody.
	historyMediaPhoneSilentLimit = 5
	// historyMediaSilentReason marks the downloads that failed for want of an answer.
	historyMediaSilentReason = "the phone did not answer"
)

var (
	// errMediaGone: WhatsApp no longer has the file (or the message has none).
	errMediaGone = errors.New("media is no longer available")
	// errNotOnPhone: asked to upload it again, the phone does not have the file either.
	errNotOnPhone = errors.New("media is not on the phone")
	// errPhoneSilent: the phone did not answer the request in time.
	errPhoneSilent = errors.New(historyMediaSilentReason)
)

// historyMediaSource is the connection to WhatsApp as the download job needs it;
// the real one wraps the client, tests supply a scripted one.
type historyMediaSource interface {
	IsConnected() bool
	// Download fetches the media of the stored web message raw into file and
	// says what it is. errMediaGone means it can never be fetched.
	Download(ctx context.Context, chatID string, raw []byte, file *os.File) (mimeType, fileName string, err error)
	// RequestFromPhone asks the phone to upload the media again and downloads it
	// from where the phone put it. errNotOnPhone: the phone has no such file.
	// errPhoneSilent: no answer in time.
	RequestFromPhone(ctx context.Context, chatID string, raw []byte, file *os.File) (mimeType, fileName string, err error)
}

// historyMediaFiles is the storage the job writes to.
type historyMediaFiles interface {
	Save(ctx context.Context, path string, reader io.Reader) (string, error)
}

type clientMediaSource struct {
	client  *whatsappInfra.WhatsAppClient
	retries *MediaRetryHub
	// wait is how long the phone has to answer.
	wait time.Duration
}

// RequestFromPhone sends the phone a (encrypted) request to upload the media of
// the message again, waits for its answer, and downloads from the new place.
func (c clientMediaSource) RequestFromPhone(ctx context.Context, chatID string, raw []byte, file *os.File) (string, string, error) {
	if c.retries == nil {
		return "", "", fmt.Errorf("asking the phone is not set up")
	}
	var web waWeb.WebMessageInfo
	if err := proto.Unmarshal(raw, &web); err != nil {
		return "", "", fmt.Errorf("%w: %v", errMediaGone, err)
	}
	jid, err := waTypes.ParseJID(chatID)
	if err != nil {
		return "", "", fmt.Errorf("%w: %v", errMediaGone, err)
	}
	core := c.client.CoreClient()
	evt, err := core.ParseWebMessage(jid, &web)
	if err != nil || evt == nil {
		return "", "", fmt.Errorf("%w: cannot read the message", errMediaGone)
	}
	media, ok := whatsappInfra.ResolveHistoricalMedia(evt.Message)
	if !ok {
		return "", "", errMediaGone
	}
	mediaKey := media.Message.GetMediaKey()

	// Register before asking, so a quick answer is not missed.
	answer, forget := c.retries.Expect(string(evt.Info.ID))
	defer forget()
	if err := core.SendMediaRetryReceipt(ctx, &evt.Info, mediaKey); err != nil {
		return "", "", fmt.Errorf("ask the phone: %w", err)
	}
	waitCtx, stop := context.WithTimeout(ctx, c.wait)
	defer stop()
	var reply *events.MediaRetry
	select {
	case reply = <-answer:
	case <-waitCtx.Done():
		if ctx.Err() != nil {
			return "", "", ctx.Err()
		}
		return "", "", errPhoneSilent
	}

	notification, err := whatsmeow.DecryptMediaRetryNotification(reply, mediaKey)
	if err != nil {
		if errors.Is(err, whatsmeow.ErrMediaNotAvailableOnPhone) {
			return "", "", errNotOnPhone
		}
		return "", "", fmt.Errorf("the phone's answer: %w", err)
	}
	switch notification.GetResult() {
	case waMmsRetry.MediaRetryNotification_SUCCESS:
		if notification.GetDirectPath() == "" {
			return "", "", fmt.Errorf("the phone's answer has no file")
		}
	case waMmsRetry.MediaRetryNotification_NOT_FOUND:
		return "", "", errNotOnPhone
	default:
		return "", "", fmt.Errorf("the phone could not upload it again (%s)", notification.GetResult())
	}
	media.SetDirectPath(notification.GetDirectPath())
	if err := file.Truncate(0); err != nil {
		return "", "", err
	}
	if _, err := file.Seek(0, io.SeekStart); err != nil {
		return "", "", err
	}
	if err := core.DownloadToFile(ctx, media.Message, file); err != nil {
		return "", "", fmt.Errorf("download after the phone uploaded it again: %w", err)
	}
	return media.Mime, media.FileName, nil
}

func (c clientMediaSource) IsConnected() bool { return c.client.IsConnected() }

func (c clientMediaSource) Download(ctx context.Context, chatID string, raw []byte, file *os.File) (string, string, error) {
	var web waWeb.WebMessageInfo
	if err := proto.Unmarshal(raw, &web); err != nil {
		return "", "", fmt.Errorf("%w: %v", errMediaGone, err)
	}
	jid, err := waTypes.ParseJID(chatID)
	if err != nil {
		return "", "", fmt.Errorf("%w: %v", errMediaGone, err)
	}
	evt, err := c.client.CoreClient().ParseWebMessage(jid, &web)
	if err != nil || evt == nil {
		return "", "", fmt.Errorf("%w: cannot read the message", errMediaGone)
	}
	media, ok := whatsappInfra.ResolveHistoricalMedia(evt.Message)
	if !ok {
		return "", "", errMediaGone
	}
	if err := c.client.CoreClient().DownloadToFile(ctx, media.Message, file); err != nil {
		if errors.Is(err, whatsmeow.ErrMediaDownloadFailedWith403) ||
			errors.Is(err, whatsmeow.ErrMediaDownloadFailedWith404) ||
			errors.Is(err, whatsmeow.ErrMediaDownloadFailedWith410) ||
			errors.Is(err, whatsmeow.ErrNoURLPresent) ||
			errors.Is(err, whatsmeow.ErrUnknownMediaType) {
			return "", "", fmt.Errorf("%w: %v", errMediaGone, err)
		}
		return "", "", err
	}
	return media.Mime, media.FileName, nil
}

// HistoryMediaService downloads the media of messages imported from history into
// the storage. Importing history only keeps the text; this fetches the files, so
// they are in the app instead of being fetched one by one when opened.
type HistoryMediaService struct {
	source      historyMediaSource
	files       historyMediaFiles
	store       *repository.MessageStore
	broadcaster HTTPServer
	// timeout bounds the download and storing of one file.
	timeout time.Duration

	mu      sync.Mutex
	running bool
	cancel  context.CancelFunc
	status  repository.HistoryMediaStatus

	// The job can run in several segments (stopped, interrupted, resumed).
	activeBase   int64
	segmentStart time.Time
	segmentDone  int
	lastSave     time.Time
	inflight     int
	// Counts of the running job, kept here so a status poll needs no query.
	remaining   int
	unavailable int
	failed      int
	missing     int
	// silent counts requests to the phone in a row that got no answer; tripped
	// is set when that stopped the job.
	silent  int
	tripped bool
	// phoneSlots limits the requests to the phone that are open at once.
	phoneSlots     chan struct{}
	phoneSlotsOnce sync.Once

	idleAt        time.Time
	idleRemaining int
	idleAvailable map[string]int
	idleExpired   map[string]int
}

func NewHistoryMediaService(client *whatsappInfra.WhatsAppClient, store *repository.MessageStore, files historyMediaFiles, retries *MediaRetryHub) *HistoryMediaService {
	s := &HistoryMediaService{
		source: clientMediaSource{client: client, retries: retries, wait: historyMediaPhoneWait},
		files:  files, store: store, timeout: historyMediaTimeout,
	}
	s.status = s.loadStatus()
	return s
}

func (s *HistoryMediaService) SetBroadcaster(server HTTPServer) { s.broadcaster = server }

// runID identifies the job: what failed in it is not tried again within it.
func (s *HistoryMediaService) runID() int64 {
	if s.status.StartedAt == nil {
		return -2
	}
	return *s.status.StartedAt
}

// loadStatus reads the persisted status. A job recorded as "running" cannot be
// running in a process that only just started, so it is closed as interrupted
// (it can be resumed).
func (s *HistoryMediaService) loadStatus() repository.HistoryMediaStatus {
	status, err := s.store.GetHistoryMediaStatus()
	if err != nil {
		return repository.HistoryMediaStatus{State: "idle", Errors: []repository.HistorySyncError{}}
	}
	if status.State == "running" || status.State == "cancelling" {
		now := time.Now().UnixMilli()
		status.State = "interrupted"
		status.FinishedAt = &now
		_ = s.store.SetHistoryMediaStatus(status)
	}
	return status
}

// Reload re-reads the persisted status after the message database was switched
// to another account.
func (s *HistoryMediaService) Reload() {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.running {
		return
	}
	s.status = s.loadStatus()
	s.idleAt = time.Time{}
}

func validHistoryMediaOptions(opts repository.HistoryMediaOptions) error {
	if len(opts.Types) == 0 {
		return fmt.Errorf("choose at least one kind of media")
	}
	for _, kind := range opts.Types {
		if _, ok := repository.HistoryMediaGroups[kind]; !ok {
			return fmt.Errorf("unknown kind of media %q", kind)
		}
	}
	return nil
}

// Start downloads the media of opts that is still missing.
func (s *HistoryMediaService) Start(opts repository.HistoryMediaOptions) (repository.HistoryMediaStatus, error) {
	if err := validHistoryMediaOptions(opts); err != nil {
		return s.Status(), err
	}
	if !s.source.IsConnected() {
		return s.Status(), fmt.Errorf("WhatsApp is not connected")
	}
	s.mu.Lock()
	if s.running {
		status := s.statusLocked()
		s.mu.Unlock()
		return status, fmt.Errorf("media download is already running")
	}
	now := time.Now().UnixMilli()
	s.status = repository.HistoryMediaStatus{State: "running", Errors: []repository.HistorySyncError{}, StartedAt: &now, Options: opts}
	s.unavailable, s.failed, s.missing = 0, 0, 0
	s.silent, s.tripped = 0, false
	s.remaining = s.store.CountHistoryMedia(opts, now)
	s.beginSegmentLocked(0)
	ctx := s.startLocked()
	status := s.statusLocked()
	s.mu.Unlock()
	go s.run(ctx, opts)
	return status, nil
}

// Resume continues a job that was stopped or interrupted. What it already
// downloaded is not fetched again, and its counters carry on.
func (s *HistoryMediaService) Resume() (repository.HistoryMediaStatus, error) {
	if !s.source.IsConnected() {
		return s.Status(), fmt.Errorf("WhatsApp is not connected")
	}
	s.mu.Lock()
	if s.running {
		status := s.statusLocked()
		s.mu.Unlock()
		return status, fmt.Errorf("media download is already running")
	}
	s.idleAt = time.Time{}
	if !s.resumableLocked() {
		status := s.statusLocked()
		s.mu.Unlock()
		return status, fmt.Errorf("there is no media download to resume")
	}
	opts := s.status.Options
	s.unavailable, s.failed, s.missing = s.store.HistoryMediaFailures(s.runID())
	s.silent, s.tripped = 0, false
	s.remaining = s.store.CountHistoryMedia(opts, s.runID())
	s.status.State = "running"
	s.status.FinishedAt = nil
	s.beginSegmentLocked(s.status.ActiveMs)
	ctx := s.startLocked()
	status := s.statusLocked()
	s.mu.Unlock()
	go s.run(ctx, opts)
	return status, nil
}

func (s *HistoryMediaService) beginSegmentLocked(activeBase int64) {
	s.activeBase, s.segmentStart, s.segmentDone, s.inflight = activeBase, time.Now(), 0, 0
}

func (s *HistoryMediaService) startLocked() context.Context {
	ctx, cancel := context.WithCancel(context.Background())
	s.running = true
	s.cancel = cancel
	s.saveLocked(true)
	return ctx
}

// Cancel stops the job after the files being downloaded.
func (s *HistoryMediaService) Cancel() (repository.HistoryMediaStatus, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if !s.running {
		return s.statusLocked(), fmt.Errorf("no media download is running")
	}
	s.status.State = "cancelling"
	if s.cancel != nil {
		s.cancel()
	}
	return s.statusLocked(), nil
}

func (s *HistoryMediaService) Status() repository.HistoryMediaStatus {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.statusLocked()
}

// resumableLocked: the job stopped without finishing, and has media left.
func (s *HistoryMediaService) resumableLocked() bool {
	if s.running {
		return false
	}
	switch s.status.State {
	case "interrupted", "cancelled", "failed":
	default:
		return false
	}
	remaining, _, _ := s.idleCountsLocked()
	return remaining > 0
}

// idleCountsLocked are the counts of a job that is not running: the media it
// still has to do and, per kind, the media a new job could fetch (available from
// WhatsApp's servers, expired: only the phone can give it back). They need
// queries, so they are reused for a moment.
func (s *HistoryMediaService) idleCountsLocked() (int, map[string]int, map[string]int) {
	if s.idleAvailable == nil || time.Since(s.idleAt) > historyMediaIdleCache {
		s.idleRemaining = 0
		if len(s.status.Options.Types) > 0 {
			s.idleRemaining = s.store.CountHistoryMedia(s.status.Options, s.runID())
		}
		s.idleAvailable, s.idleExpired = s.store.AvailableHistoryMedia()
		s.idleAt = time.Now()
	}
	return s.idleRemaining, s.idleAvailable, s.idleExpired
}

func (s *HistoryMediaService) elapsedLocked() int64 {
	if !s.running {
		return s.status.ActiveMs
	}
	return s.activeBase + time.Since(s.segmentStart).Milliseconds()
}

func (s *HistoryMediaService) statusLocked() repository.HistoryMediaStatus {
	status := s.status
	status.Errors = append([]repository.HistorySyncError{}, status.Errors...)
	status.ElapsedMs = s.elapsedLocked()
	status.Current = s.inflight
	if s.running {
		status.Remaining, status.Unavailable, status.Failed, status.Missing = s.remaining, s.unavailable, s.failed, s.missing
		status.Available, status.Expired = map[string]int{}, map[string]int{}
		spent := time.Since(s.segmentStart).Milliseconds()
		if s.segmentDone >= 5 && spent >= 10_000 && s.remaining > 0 {
			eta := spent * int64(s.remaining) / int64(s.segmentDone)
			status.EtaMs = &eta
		}
	} else {
		remaining, available, expired := s.idleCountsLocked()
		status.Remaining = remaining
		status.Available, status.Expired = make(map[string]int, len(available)), make(map[string]int, len(expired))
		for k, v := range available {
			status.Available[k] = v
		}
		for k, v := range expired {
			status.Expired[k] = v
		}
		status.Unavailable, status.Failed, status.Missing = s.store.HistoryMediaFailures(s.runID())
		status.Resumable = s.resumableLocked()
	}
	status.Total = status.Done + status.Unavailable + status.Failed + status.Missing + status.Remaining
	return status
}

// saveLocked writes the progress to the database, at most once a second unless forced.
func (s *HistoryMediaService) saveLocked(force bool) {
	if !force && time.Since(s.lastSave) < historyMediaSaveEvery {
		return
	}
	s.status.ActiveMs = s.elapsedLocked()
	_ = s.store.SetHistoryMediaStatus(s.status)
	s.lastSave = time.Now()
}

func (s *HistoryMediaService) run(ctx context.Context, opts repository.HistoryMediaOptions) {
	defer func() {
		if r := recover(); r != nil {
			s.finish("failed", fmt.Sprintf("media download crashed: %v", r))
		}
	}()
	runID := s.runIDLocked()
	jobs := make(chan repository.HistoryMediaItem)
	var wg sync.WaitGroup
	for i := 0; i < historyMediaWorkers; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for item := range jobs {
				s.download(ctx, runID, opts, item)
			}
		}()
	}
	afterTs, afterID := repository.FirstHistoryMediaCursor()
feed:
	for ctx.Err() == nil {
		items, err := s.store.NextHistoryMedia(opts, runID, afterTs, afterID, historyMediaPage)
		if err != nil {
			s.addError("", err.Error())
			break
		}
		if len(items) == 0 {
			break
		}
		for _, item := range items {
			select {
			case <-ctx.Done():
				break feed
			case jobs <- item:
			}
		}
		last := items[len(items)-1]
		afterTs, afterID = last.Timestamp, last.MessageID
	}
	close(jobs)
	wg.Wait()

	if ctx.Err() != nil {
		s.finish("cancelled", "")
		return
	}
	s.mu.Lock()
	state := "completed"
	if s.failed > 0 {
		state = "partial"
		if s.status.Done == 0 {
			state = "failed"
		}
	}
	s.mu.Unlock()
	s.finish(state, "")
}

func (s *HistoryMediaService) runIDLocked() int64 {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.runID()
}

// download fetches one message's media into the storage and points the message at it.
func (s *HistoryMediaService) download(ctx context.Context, runID int64, opts repository.HistoryMediaOptions, item repository.HistoryMediaItem) {
	s.mu.Lock()
	s.inflight++
	s.mu.Unlock()
	defer func() {
		s.mu.Lock()
		s.inflight--
		s.mu.Unlock()
	}()
	if !s.source.IsConnected() {
		s.addError("", "WhatsApp disconnected during the media download")
		s.cancelRun()
		return
	}
	size, recovered, err := s.fetch(ctx, item, opts.AskPhone)
	switch {
	case err == nil:
		s.mu.Lock()
		s.status.Done++
		if recovered {
			s.status.Recovered++
			s.silent = 0 // the phone is answering
		}
		s.status.Bytes += size
		s.remaining--
		s.segmentDone++
		s.saveLocked(false)
		s.mu.Unlock()
	case ctx.Err() != nil:
		// Stopped: this one stays to do.
	case errors.Is(err, errNotOnPhone):
		_ = s.store.MarkHistoryMedia(item.MessageID, "missing", err.Error(), runID)
		s.mu.Lock()
		s.missing++
		s.remaining--
		s.silent = 0
		s.saveLocked(false)
		s.mu.Unlock()
	case errors.Is(err, errPhoneSilent):
		_ = s.store.MarkHistoryMedia(item.MessageID, "failed", historyMediaSilentReason, runID)
		s.mu.Lock()
		s.failed++
		s.remaining--
		s.silent++
		trip := s.silent >= historyMediaPhoneSilentLimit && !s.tripped
		if trip {
			s.tripped = true
		}
		s.saveLocked(false)
		s.mu.Unlock()
		if trip {
			s.addError("", "Your phone is not answering. Open WhatsApp on it, then resume.")
			s.cancelRun()
		}
	case errors.Is(err, errMediaGone):
		_ = s.store.MarkHistoryMedia(item.MessageID, "unavailable", err.Error(), runID)
		s.mu.Lock()
		s.unavailable++
		s.remaining--
		s.saveLocked(false)
		s.mu.Unlock()
	default:
		_ = s.store.MarkHistoryMedia(item.MessageID, "failed", err.Error(), runID)
		s.addError(item.ChatID, err.Error())
		s.mu.Lock()
		s.failed++
		s.remaining--
		s.saveLocked(false)
		s.mu.Unlock()
	}
}

// fetch downloads the media of item to a temporary file, stores it, and returns
// its size. When WhatsApp's servers no longer have it and askPhone is set, the
// phone is asked to upload it again; recovered says that is how it was got.
func (s *HistoryMediaService) fetch(ctx context.Context, item repository.HistoryMediaItem, askPhone bool) (size int64, recovered bool, err error) {
	ctx, cancel := context.WithTimeout(ctx, s.timeout)
	defer cancel()
	// A temporary file, so a big video is streamed to the storage instead of held in memory.
	file, err := os.CreateTemp("", "wa-history-media-*")
	if err != nil {
		return 0, false, err
	}
	defer func() {
		file.Close()
		os.Remove(file.Name())
	}()

	mimeType, fileName, err := s.source.Download(ctx, item.ChatID, item.Raw, file)
	if err != nil && askPhone && errors.Is(err, errMediaGone) {
		mimeType, fileName, err = s.askPhone(ctx, item, file)
		recovered = err == nil
	}
	if err != nil {
		return 0, false, err
	}
	info, err := file.Stat()
	if err != nil {
		return 0, false, err
	}
	if info.Size() == 0 {
		return 0, false, fmt.Errorf("%w: empty file", errMediaGone)
	}
	if _, err := file.Seek(0, io.SeekStart); err != nil {
		return 0, false, err
	}
	key := whatsappInfra.HistoricalMediaKey(item.ChatID, item.MessageID, fileName, mimeType)
	if _, err := s.files.Save(ctx, key, file); err != nil {
		return 0, false, fmt.Errorf("store %s: %w", key, err)
	}
	if err := s.store.HistoryMediaDownloaded(item.MessageID, "/media/"+key); err != nil {
		return 0, false, err
	}
	return info.Size(), recovered, nil
}

// askPhone asks the phone for the media, a couple of requests at a time.
func (s *HistoryMediaService) askPhone(ctx context.Context, item repository.HistoryMediaItem, file *os.File) (string, string, error) {
	s.phoneSlotsOnce.Do(func() { s.phoneSlots = make(chan struct{}, historyMediaPhoneParallel) })
	select {
	case s.phoneSlots <- struct{}{}:
	case <-ctx.Done():
		return "", "", ctx.Err()
	}
	defer func() { <-s.phoneSlots }()
	return s.source.RequestFromPhone(ctx, item.ChatID, item.Raw, file)
}

func (s *HistoryMediaService) cancelRun() {
	s.mu.Lock()
	if s.cancel != nil {
		s.cancel()
	}
	s.mu.Unlock()
}

func (s *HistoryMediaService) addError(chatID, message string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.status.Errors = append(s.status.Errors, repository.HistorySyncError{ChatID: chatID, Message: message})
	if n := len(s.status.Errors); n > historyMediaMaxErrors {
		s.status.Errors = append([]repository.HistorySyncError{}, s.status.Errors[n-historyMediaMaxErrors:]...)
	}
}

func (s *HistoryMediaService) finish(state, problem string) {
	now := time.Now().UnixMilli()
	s.mu.Lock()
	s.status.ActiveMs = s.elapsedLocked()
	s.status.State = state
	if problem != "" {
		s.status.Errors = append(s.status.Errors, repository.HistorySyncError{Message: problem})
	}
	s.status.FinishedAt = &now
	s.running = false
	s.inflight = 0
	s.idleAt = time.Time{}
	if s.tripped {
		// The files that failed for want of an answer are to do again when resumed.
		_ = s.store.ForgetHistoryMediaFailures(s.runID(), historyMediaSilentReason)
	}
	if s.cancel != nil {
		s.cancel()
		s.cancel = nil
	}
	s.saveLocked(true)
	status := s.statusLocked()
	s.mu.Unlock()
	if s.broadcaster != nil {
		s.broadcaster.BroadcastMessage("chats_changed", map[string]interface{}{
			"reason": "history_media",
			"done":   status.Done,
			"state":  status.State,
		})
	}
}

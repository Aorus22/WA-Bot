package handlers

import (
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"wa-bot/internal/domain/repository"
)

type fakeHistorySync struct {
	started   []repository.HistorySyncOptions
	cancelled int
	state     string

	resumed      int
	resumeStatus repository.HistorySyncStatus
	resumeErr    error
}

func (f *fakeHistorySync) Start(opts repository.HistorySyncOptions) (repository.HistorySyncStatus, error) {
	f.started = append(f.started, opts)
	return repository.HistorySyncStatus{State: "running", Options: opts}, nil
}

func (f *fakeHistorySync) Resume() (repository.HistorySyncStatus, error) {
	f.resumed++
	return f.resumeStatus, f.resumeErr
}

func (f *fakeHistorySync) Cancel() (repository.HistorySyncStatus, error) {
	f.cancelled++
	if f.state == "idle" {
		return repository.HistorySyncStatus{State: "idle"}, errNone
	}
	return repository.HistorySyncStatus{State: "cancelling"}, nil
}

func (f *fakeHistorySync) Status() repository.HistorySyncStatus {
	return repository.HistorySyncStatus{State: "running"}
}

type errString string

func (e errString) Error() string { return string(e) }

const errNone = errString("no history sync is running")

func post(h *HistorySyncHandler, body string) *httptest.ResponseRecorder {
	req := httptest.NewRequest(http.MethodPost, "/api/history-sync", strings.NewReader(body))
	rec := httptest.NewRecorder()
	h.Start(rec, req)
	return rec
}

func TestHistorySyncStartReadsTheDepth(t *testing.T) {
	fake := &fakeHistorySync{}
	h := NewHistorySyncHandler(&Handler{historySync: fake})

	// No body: the original behaviour (the service defaults to 50 per chat).
	if rec := post(h, ""); rec.Code != http.StatusAccepted {
		t.Fatalf("empty body: %d", rec.Code)
	}
	if got := fake.started[0]; got.PerChat != 0 || got.SinceMs != 0 || got.All {
		t.Fatalf("empty body options: %+v", got)
	}

	post(h, `{"perChat":200}`)
	if fake.started[1].PerChat != 200 {
		t.Fatalf("per chat: %+v", fake.started[1])
	}

	before := time.Now().Add(-30 * 24 * time.Hour).UnixMilli()
	post(h, `{"sinceDays":30}`)
	if since := fake.started[2].SinceMs; since < before-5000 || since > before+5000 {
		t.Fatalf("since: %d, want about %d", since, before)
	}

	post(h, `{"all":true}`)
	if got := fake.started[3]; !got.All || got.IncludeExhausted || got.PerChat != 0 {
		t.Fatalf("all must not retry finished chats by itself: %+v", got)
	}

	post(h, `{"all":true,"retryFinished":true}`)
	if got := fake.started[4]; !got.All || !got.IncludeExhausted {
		t.Fatalf("all + retryFinished: %+v", got)
	}
	post(h, `{"perChat":50,"retryFinished":true}`)
	if got := fake.started[5]; got.PerChat != 50 || !got.IncludeExhausted {
		t.Fatalf("retryFinished applies to every depth: %+v", got)
	}
}

func TestHistorySyncStartRejectsNonsense(t *testing.T) {
	fake := &fakeHistorySync{}
	h := NewHistorySyncHandler(&Handler{historySync: fake})
	for _, body := range []string{`{"perChat":-1}`, `{"sinceDays":-5}`, `{"perChat":99999999}`, `{"sinceDays":99999}`, `{not json`} {
		if rec := post(h, body); rec.Code != http.StatusBadRequest {
			t.Errorf("%s: %d", body, rec.Code)
		}
	}
	if len(fake.started) != 0 {
		t.Fatal("an invalid request started a sync")
	}
}

func TestHistorySyncCancel(t *testing.T) {
	fake := &fakeHistorySync{}
	h := NewHistorySyncHandler(&Handler{historySync: fake})
	rec := httptest.NewRecorder()
	h.Cancel(rec, httptest.NewRequest(http.MethodPost, "/api/history-sync/cancel", nil))
	if rec.Code != http.StatusAccepted || fake.cancelled != 1 {
		t.Fatalf("cancel: %d", rec.Code)
	}
	fake.state = "idle"
	rec = httptest.NewRecorder()
	h.Cancel(rec, httptest.NewRequest(http.MethodPost, "/api/history-sync/cancel", nil))
	if rec.Code != http.StatusConflict {
		t.Fatalf("cancel with nothing running: %d", rec.Code)
	}
}

func TestHistorySyncResume(t *testing.T) {
	fake := &fakeHistorySync{resumeStatus: repository.HistorySyncStatus{State: "running"}}
	h := NewHistorySyncHandler(&Handler{historySync: fake})
	if rec := post(h, `{"resume":true}`); rec.Code != http.StatusAccepted || fake.resumed != 1 || len(fake.started) != 0 {
		t.Fatalf("resume: %d, resumed %d, started %d", rec.Code, fake.resumed, len(fake.started))
	}
	// Nonsense next to resume does not matter: it is not read.
	if rec := post(h, `{"resume":true,"perChat":-4}`); rec.Code != http.StatusAccepted {
		t.Fatalf("resume with other fields: %d", rec.Code)
	}

	cases := []struct {
		name   string
		status repository.HistorySyncStatus
		want   int
	}{
		{"already running", repository.HistorySyncStatus{State: "running"}, http.StatusConflict},
		{"cannot run now", repository.HistorySyncStatus{State: "interrupted", Resumable: true}, http.StatusServiceUnavailable},
		{"nothing to resume", repository.HistorySyncStatus{State: "completed"}, http.StatusBadRequest},
	}
	for _, c := range cases {
		fake.resumeStatus, fake.resumeErr = c.status, errString("nope")
		if rec := post(h, `{"resume":true}`); rec.Code != c.want {
			t.Errorf("%s: %d, want %d", c.name, rec.Code, c.want)
		}
	}
}

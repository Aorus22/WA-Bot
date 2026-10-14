package handlers

import (
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"wa-bot/internal/domain/repository"
)

type fakeHistoryMedia struct {
	started []repository.HistoryMediaOptions
	resumed int
	status  repository.HistoryMediaStatus
	err     error
}

func (f *fakeHistoryMedia) Start(opts repository.HistoryMediaOptions) (repository.HistoryMediaStatus, error) {
	f.started = append(f.started, opts)
	return f.status, f.err
}
func (f *fakeHistoryMedia) Resume() (repository.HistoryMediaStatus, error) {
	f.resumed++
	return f.status, f.err
}
func (f *fakeHistoryMedia) Cancel() (repository.HistoryMediaStatus, error) { return f.status, f.err }
func (f *fakeHistoryMedia) Status() repository.HistoryMediaStatus          { return f.status }

func postMedia(h *HistoryMediaHandler, body string) *httptest.ResponseRecorder {
	rec := httptest.NewRecorder()
	h.Start(rec, httptest.NewRequest(http.MethodPost, "/api/history-media", strings.NewReader(body)))
	return rec
}

func TestHistoryMediaStartReadsTheOptions(t *testing.T) {
	fake := &fakeHistoryMedia{status: repository.HistoryMediaStatus{State: "running"}}
	h := NewHistoryMediaHandler(&Handler{historyMedia: fake})
	if rec := postMedia(h, `{"types":["image","audio"],"retryUnavailable":true}`); rec.Code != http.StatusAccepted {
		t.Fatalf("start: %d", rec.Code)
	}
	got := fake.started[0]
	if len(got.Types) != 2 || got.Types[0] != "image" || !got.RetryUnavailable {
		t.Fatalf("options: %+v", got)
	}
	if rec := postMedia(h, `{"resume":true}`); rec.Code != http.StatusAccepted || fake.resumed != 1 || len(fake.started) != 1 {
		t.Fatalf("resume: %d, resumed %d, started %d", rec.Code, fake.resumed, len(fake.started))
	}
	if rec := postMedia(h, `{not json`); rec.Code != http.StatusBadRequest {
		t.Fatalf("bad json: %d", rec.Code)
	}
}

func TestHistoryMediaStartMapsFailuresToCodes(t *testing.T) {
	fake := &fakeHistoryMedia{err: errString("nope")}
	h := NewHistoryMediaHandler(&Handler{historyMedia: fake})
	cases := []struct {
		name   string
		body   string
		status repository.HistoryMediaStatus
		want   int
	}{
		{"already running", `{"types":["image"]}`, repository.HistoryMediaStatus{State: "running"}, http.StatusConflict},
		{"no kind chosen", `{}`, repository.HistoryMediaStatus{State: "idle"}, http.StatusBadRequest},
		{"unknown kind", `{"types":["gif"]}`, repository.HistoryMediaStatus{State: "idle"}, http.StatusBadRequest},
		{"not connected", `{"types":["image"]}`, repository.HistoryMediaStatus{State: "idle"}, http.StatusServiceUnavailable},
		{"nothing to resume", `{"resume":true}`, repository.HistoryMediaStatus{State: "completed"}, http.StatusBadRequest},
		{"resume while running", `{"resume":true}`, repository.HistoryMediaStatus{State: "running"}, http.StatusConflict},
		{"resume, not connected", `{"resume":true}`, repository.HistoryMediaStatus{State: "interrupted", Resumable: true}, http.StatusServiceUnavailable},
	}
	for _, c := range cases {
		fake.status = c.status
		if rec := postMedia(h, c.body); rec.Code != c.want {
			t.Errorf("%s: %d, want %d", c.name, rec.Code, c.want)
		}
	}
}

func TestHistoryMediaCancelAndStatus(t *testing.T) {
	fake := &fakeHistoryMedia{status: repository.HistoryMediaStatus{State: "cancelling"}}
	h := NewHistoryMediaHandler(&Handler{historyMedia: fake})
	rec := httptest.NewRecorder()
	h.Cancel(rec, httptest.NewRequest(http.MethodPost, "/api/history-media/cancel", nil))
	if rec.Code != http.StatusAccepted {
		t.Fatalf("cancel: %d", rec.Code)
	}
	fake.err = errString("no media download is running")
	rec = httptest.NewRecorder()
	h.Cancel(rec, httptest.NewRequest(http.MethodPost, "/api/history-media/cancel", nil))
	if rec.Code != http.StatusConflict {
		t.Fatalf("cancel idle: %d", rec.Code)
	}
	rec = httptest.NewRecorder()
	h.Status(rec, httptest.NewRequest(http.MethodGet, "/api/history-media/status", nil))
	if rec.Code != http.StatusOK || !strings.Contains(rec.Body.String(), `"state":"cancelling"`) {
		t.Fatalf("status: %d %s", rec.Code, rec.Body.String())
	}
}

func TestHistoryMediaWithoutAServiceSaysSo(t *testing.T) {
	h := NewHistoryMediaHandler(&Handler{})
	if rec := postMedia(h, `{}`); rec.Code != http.StatusServiceUnavailable {
		t.Fatalf("no service: %d", rec.Code)
	}
}

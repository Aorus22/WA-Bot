package handlers

import (
	"context"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"strings"
	"testing"

	"wa-bot/internal/domain/repository"
)

// sizedStorage is a storage that only knows how big it is.
type sizedStorage struct {
	repository.StorageRepository
	size  int64
	known bool
}

func (s sizedStorage) MediaSize(context.Context) (int64, bool, error) { return s.size, s.known, nil }

func getUsage(h *StorageUsageHandler) *httptest.ResponseRecorder {
	rec := httptest.NewRecorder()
	h.Get(rec, httptest.NewRequest(http.MethodGet, "/api/storage/usage", nil))
	return rec
}

func TestStorageUsageReportsBothDatabases(t *testing.T) {
	store, err := repository.NewMessageStore(filepath.Join(t.TempDir(), "m.db"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { store.Close() })
	h := NewStorageUsageHandler(&Handler{msgRepo: store, storage: sizedStorage{size: 123456, known: true}})
	rec := getUsage(h)
	body := rec.Body.String()
	if rec.Code != http.StatusOK || !strings.Contains(body, `"mediaBytes":123456`) || strings.Contains(body, `"chatBytes":null`) || !strings.Contains(body, `"chatBytes":`) {
		t.Fatalf("usage: %d %s", rec.Code, body)
	}
}

func TestStorageUsageIsNullWhenNotKnown(t *testing.T) {
	// Media kept somewhere that is not a SQLite file (S3), and no message database yet.
	h := NewStorageUsageHandler(&Handler{storage: sizedStorage{known: false}})
	body := getUsage(h).Body.String()
	if !strings.Contains(body, `"chatBytes":null`) || !strings.Contains(body, `"mediaBytes":null`) {
		t.Fatalf("unknown sizes should be null, got %s", body)
	}
	// A storage that cannot say at all.
	h = NewStorageUsageHandler(&Handler{})
	if body := getUsage(h).Body.String(); !strings.Contains(body, `"mediaBytes":null`) {
		t.Fatalf("no storage: %s", body)
	}
}

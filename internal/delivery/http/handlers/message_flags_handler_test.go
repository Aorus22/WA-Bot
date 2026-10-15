package handlers

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"testing"
	"time"

	"github.com/gorilla/mux"

	"wa-bot/internal/domain/repository"
)

func flagsHandlerFixture(t *testing.T) (*MessageFlagsHandler, *repository.MessageStore, *mux.Router) {
	t.Helper()
	store, err := repository.NewMessageStore(filepath.Join(t.TempDir(), "messages.db"))
	if err != nil {
		t.Fatal(err)
	}
	h := NewHandler(nil, nil, nil)
	h.msgRepo = store
	fh := NewMessageFlagsHandler(h)

	chat := "628111@s.whatsapp.net"
	for i, id := range []string{"m1", "m2"} {
		if err := store.SaveMessage(&repository.Message{ID: id, ChatID: chat, ChatName: "Budi", From: chat, To: "me",
			Content: id, Timestamp: int64(i + 1), Status: "received", Type: "text"}); err != nil {
			t.Fatal(err)
		}
	}
	r := mux.NewRouter()
	r.HandleFunc("/api/starred", fh.Starred).Methods("GET")
	r.HandleFunc("/api/chats/{id}/pins", fh.Pins).Methods("GET")
	r.HandleFunc("/api/link-preview", fh.LinkPreview).Methods("GET")
	return fh, store, r
}

func getJSON(t *testing.T, r http.Handler, url string, out any) *httptest.ResponseRecorder {
	t.Helper()
	rec := httptest.NewRecorder()
	r.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, url, nil))
	if out != nil && rec.Code == http.StatusOK {
		if err := json.Unmarshal(rec.Body.Bytes(), out); err != nil {
			t.Fatalf("%s: %v\n%s", url, err, rec.Body.String())
		}
	}
	return rec
}

func TestStarredEndpointListsNewestFirstWithChatName(t *testing.T) {
	_, store, r := flagsHandlerFixture(t)
	chat := "628111@s.whatsapp.net"
	_ = store.SetStarred(chat, "m1", true, 100)
	_ = store.SetStarred(chat, "m2", true, 200)

	var got []map[string]any
	getJSON(t, r, "/api/starred", &got)
	if len(got) != 2 || got[0]["id"] != "m2" || got[0]["starred"] != true || got[0]["starredAt"] != float64(200) || got[0]["chatId"] != chat {
		t.Fatalf("starred = %v", got)
	}

	got = nil
	getJSON(t, r, "/api/starred?chatId="+chat+"&before=200", &got)
	if len(got) != 1 || got[0]["id"] != "m1" {
		t.Fatalf("paged starred = %v", got)
	}

	rec := getJSON(t, r, "/api/starred?chatId=nobody@s.whatsapp.net", nil)
	if rec.Body.String() != "[]\n" {
		t.Fatalf("empty list must be [], got %q", rec.Body.String())
	}
}

func TestPinsEndpointReturnsActivePinsWithMessages(t *testing.T) {
	_, store, r := flagsHandlerFixture(t)
	chat := "628111@s.whatsapp.net"
	now := time.Now().UnixMilli()
	_ = store.PinMessage(chat, "m1", "me", now, now+60_000)
	_ = store.PinMessage(chat, "m2", "me", now-1, now-1) // expired

	var got []map[string]any
	getJSON(t, r, "/api/chats/"+chat+"/pins", &got)
	if len(got) != 1 || got[0]["messageId"] != "m1" {
		t.Fatalf("pins = %v", got)
	}
	if msg, _ := got[0]["message"].(map[string]any); msg["content"] != "m1" {
		t.Fatalf("pin message = %v", got[0]["message"])
	}

	rec := getJSON(t, r, "/api/chats/nobody@s.whatsapp.net/pins", nil)
	if rec.Body.String() != "[]\n" {
		t.Fatalf("empty pins must be [], got %q", rec.Body.String())
	}
}

func TestLinkPreviewRefusesLocalAddresses(t *testing.T) {
	_, _, r := flagsHandlerFixture(t)
	for _, url := range []string{"/api/link-preview", "/api/link-preview?url=ftp://example.com", "/api/link-preview?url=http://127.0.0.1:8080/", "/api/link-preview?url=http://localhost/x", "/api/link-preview?url=http://192.168.1.5/"} {
		if rec := getJSON(t, r, url, nil); rec.Code != http.StatusBadRequest {
			t.Fatalf("%s = %d", url, rec.Code)
		}
	}
}

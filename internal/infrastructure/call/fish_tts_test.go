package call

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"os"
	"strings"
	"testing"
)

type fakeDoer struct {
	status int
	body   []byte
	header http.Header
	err    error
	req    *http.Request
}

func (f *fakeDoer) Do(req *http.Request) (*http.Response, error) {
	f.req = req
	if f.err != nil {
		return nil, f.err
	}
	return &http.Response{
		StatusCode: f.status,
		Header:     f.header,
		Body:       io.NopCloser(bytes.NewReader(f.body)),
	}, nil
}

// newFakeFishProvider returns a provider whose HTTP client is the given fake.
func newFakeFishProvider(cfg FishAudioConfig, fake *fakeDoer) *FishAudioTTSProvider {
	p := NewFishAudioTTSProvider(cfg)
	p.httpClient = fake
	return p
}

// readRequestBody decodes the JSON body captured on the fake request.
func readRequestBody(t *testing.T, body io.Reader) map[string]interface{} {
	t.Helper()
	raw, err := io.ReadAll(body)
	if err != nil {
		t.Fatalf("read request body: %v", err)
	}
	var m map[string]interface{}
	if err := json.Unmarshal(raw, &m); err != nil {
		t.Fatalf("decode request body %q: %v", raw, err)
	}
	return m
}

func TestFishAudioSynthesizeSuccess(t *testing.T) {
	mp3Bytes := []byte("ID3\x04\x00\x00\x00\x00\x00\x00\x10fake-mp3")
	fake := &fakeDoer{
		status: http.StatusOK,
		body:   mp3Bytes,
	}

	p := newFakeFishProvider(FishAudioConfig{
		APIKey:         "k-test",
		DefaultModel:   "s2.1-pro",
		DefaultVoiceID: "voice-default",
	}, fake)

	res, err := p.Synthesize(context.Background(), TTSRequest{
		Text:        "hello",
		ReferenceID: "voice-x",
	})
	if err != nil {
		t.Fatalf("Synthesize: %v", err)
	}
	defer res.Cleanup()

	if res.Format != "mp3" {
		t.Errorf("Format = %q, want %q", res.Format, "mp3")
	}
	got, err := os.ReadFile(res.Path)
	if err != nil {
		t.Fatalf("read temp file: %v", err)
	}
	if !bytes.Equal(got, mp3Bytes) {
		t.Errorf("temp file bytes = %q, want %q", got, mp3Bytes)
	}

	if fake.req == nil {
		t.Fatal("fake Do was never called")
	}
	if fake.req.Method != http.MethodPost {
		t.Errorf("Method = %q, want %q", fake.req.Method, http.MethodPost)
	}
	if fake.req.URL.String() != FishAudioAPIBase {
		t.Errorf("URL = %q, want %q", fake.req.URL.String(), FishAudioAPIBase)
	}
	if got := fake.req.Header.Get("Authorization"); got != "Bearer k-test" {
		t.Errorf("Authorization = %q, want %q", got, "Bearer k-test")
	}
	if got := fake.req.Header.Get("model"); got != "s2.1-pro" {
		t.Errorf("model header = %q, want %q", got, "s2.1-pro")
	}

	body := readRequestBody(t, fake.req.Body)
	if body["reference_id"] != "voice-x" {
		t.Errorf("reference_id = %v, want %q", body["reference_id"], "voice-x")
	}
	if body["format"] != "mp3" {
		t.Errorf("format = %v, want %q", body["format"], "mp3")
	}
	if body["text"] != "hello" {
		t.Errorf("text = %v, want %q", body["text"], "hello")
	}
}

func TestFishAudioSynthesizeReferenceIDEmptyFallsBackToDefault(t *testing.T) {
	fake := &fakeDoer{status: http.StatusOK, body: []byte("ID3fake")}
	p := newFakeFishProvider(FishAudioConfig{
		APIKey:         "k-test",
		DefaultVoiceID: "voice-default",
	}, fake)

	res, err := p.Synthesize(context.Background(), TTSRequest{Text: "hello"})
	if err != nil {
		t.Fatalf("Synthesize: %v", err)
	}
	defer res.Cleanup()

	body := readRequestBody(t, fake.req.Body)
	if body["reference_id"] != "voice-default" {
		t.Errorf("reference_id = %v, want %q", body["reference_id"], "voice-default")
	}
}

func TestFishAudioSynthesizeReferenceIDAndDefaultEmptyOmitsKey(t *testing.T) {
	fake := &fakeDoer{status: http.StatusOK, body: []byte("ID3fake")}
	p := newFakeFishProvider(FishAudioConfig{APIKey: "k-test"}, fake)

	res, err := p.Synthesize(context.Background(), TTSRequest{Text: "hello"})
	if err != nil {
		t.Fatalf("Synthesize: %v", err)
	}
	defer res.Cleanup()

	body := readRequestBody(t, fake.req.Body)
	if _, ok := body["reference_id"]; ok {
		t.Errorf("reference_id present in body = %v, want key omitted", body["reference_id"])
	}
}

func TestFishAudioSynthesizeHTTPError(t *testing.T) {
	fake := &fakeDoer{
		status: http.StatusPaymentRequired,
		body:   []byte(`{"status":402,"message":"No payment","reason":"quota exhausted"}`),
	}
	p := newFakeFishProvider(FishAudioConfig{
		APIKey:       "k-test",
		DefaultModel: "s2.1-pro",
	}, fake)

	res, err := p.Synthesize(context.Background(), TTSRequest{Text: "hello"})
	if res != nil {
		t.Errorf("res = %+v, want nil", res)
	}
	if err == nil {
		t.Fatal("expected error, got nil")
	}
	if !errors.Is(err, ErrTTSFailed) {
		t.Errorf("errors.Is(err, ErrTTSFailed) = false, err = %v", err)
	}
	if !strings.Contains(err.Error(), "No payment") {
		t.Errorf("error %q does not surface API message", err.Error())
	}
	if !strings.Contains(err.Error(), "quota exhausted") {
		t.Errorf("error %q does not surface API reason", err.Error())
	}
}

func TestFishAudioSynthesizeMissingAPIKey(t *testing.T) {
	p := newFakeFishProvider(FishAudioConfig{}, &fakeDoer{})

	res, err := p.Synthesize(context.Background(), TTSRequest{Text: "hello"})
	if res != nil {
		t.Errorf("res = %+v, want nil", res)
	}
	if err == nil {
		t.Fatal("expected error, got nil")
	}
	if !errors.Is(err, ErrTTSFailed) {
		t.Errorf("errors.Is(err, ErrTTSFailed) = false, err = %v", err)
	}
	if !strings.Contains(err.Error(), "api key is not configured") {
		t.Errorf("error %q does not mention missing api key", err.Error())
	}
}

func TestFishAudioDefaultModelFallback(t *testing.T) {
	p := NewFishAudioTTSProvider(FishAudioConfig{APIKey: "k-test"})
	if p.defaultModel != "s2.1-pro-free" {
		t.Errorf("defaultModel = %q, want %q", p.defaultModel, "s2.1-pro-free")
	}
}

func TestFishAudioSynthesizeTransportError(t *testing.T) {
	fake := &fakeDoer{err: errors.New("connection refused")}
	p := newFakeFishProvider(FishAudioConfig{APIKey: "k-test"}, fake)

	res, err := p.Synthesize(context.Background(), TTSRequest{Text: "hello"})
	if res != nil {
		t.Errorf("res = %+v, want nil", res)
	}
	if err == nil {
		t.Fatal("expected error, got nil")
	}
	if !errors.Is(err, ErrTTSFailed) {
		t.Errorf("errors.Is(err, ErrTTSFailed) = false, err = %v", err)
	}
	if !strings.Contains(err.Error(), "connection refused") {
		t.Errorf("error %q does not surface transport error", err.Error())
	}
}

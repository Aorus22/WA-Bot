package call_test

import (
	"bytes"
	"encoding/json"
	"net/http/httptest"
	"os/exec"
	"strings"
	"testing"
	"time"

	"github.com/gorilla/mux"
	"github.com/gorilla/websocket"
	"github.com/purpshell/meowcaller"

	httpdelivery "wa-bot/internal/delivery/http"
	"wa-bot/internal/infrastructure/call"
)

func startMediaServer(t *testing.T) (*httptest.Server, *call.CallService, *call.CallMedia) {
	t.Helper()
	svc := call.NewCallService(nil, nil, nil, nil)
	media := svc.SetActiveForTest("call_1")
	r := mux.NewRouter()
	r.HandleFunc("/ws/calls/{id}/media", httpdelivery.NewCallMediaHandler(svc).ServeWS)
	srv := httptest.NewServer(r)
	t.Cleanup(srv.Close)
	return srv, svc, media
}

func dial(t *testing.T, srv *httptest.Server, svc *call.CallService, query string) *websocket.Conn {
	t.Helper()
	sess, err := svc.CreateMediaSession("call_1")
	if err != nil {
		t.Fatal(err)
	}
	url := "ws" + strings.TrimPrefix(srv.URL, "http") + "/ws/calls/call_1/media?token=" + sess.Token + query
	c, _, err := websocket.DefaultDialer.Dial(url, nil)
	if err != nil {
		t.Fatalf("dial: %v", err)
	}
	t.Cleanup(func() { c.Close() })
	return c
}

func pcm(v int16, samples int) []byte {
	b := make([]byte, samples*2)
	for i := 0; i < samples; i++ {
		b[2*i] = byte(v)
		b[2*i+1] = byte(v >> 8)
	}
	return b
}

func TestMediaWebSocketRejectsBadToken(t *testing.T) {
	srv, _, _ := startMediaServer(t)
	url := "ws" + strings.TrimPrefix(srv.URL, "http") + "/ws/calls/call_1/media?token=nope"
	_, resp, err := websocket.DefaultDialer.Dial(url, nil)
	if err == nil {
		t.Fatal("expected the handshake to fail")
	}
	if resp == nil || resp.StatusCode != 403 {
		t.Fatalf("status %v, want 403", resp)
	}
}

func TestMediaWebSocketAudioBothWays(t *testing.T) {
	srv, svc, media := startMediaServer(t)
	c := dial(t, srv, svc, "&video=jpeg")

	// client -> call
	frame, _ := call.EncodeFrame(call.FrameKindOutgoingAudio, nil, pcm(1000, meowcaller.FrameSamples))
	if err := c.WriteMessage(websocket.BinaryMessage, frame); err != nil {
		t.Fatal(err)
	}
	ok := call.WaitForForTest(2*time.Second, func() bool {
		f := media.NextOutgoingAudioForTest()
		return len(f) == meowcaller.FrameSamples && f[0] > 0.02
	})
	if !ok {
		t.Fatal("outgoing audio never reached the call")
	}

	// call -> client
	in := make([]float32, meowcaller.FrameSamples)
	for i := range in {
		in[i] = 0.5
	}
	media.FeedIncomingAudioForTest(in)
	_ = c.SetReadDeadline(time.Now().Add(2 * time.Second))
	_, data, err := c.ReadMessage()
	if err != nil {
		t.Fatal(err)
	}
	kind, _, payload, err := call.ParseFrame(data)
	if err != nil || kind != call.FrameKindIncomingAudio || len(payload) != meowcaller.FrameSamples*2 {
		t.Fatalf("kind %d len %d err %v", kind, len(payload), err)
	}
	if v := int16(payload[0]) | int16(payload[1])<<8; v < 16000 || v > 16500 {
		t.Fatalf("sample %d, want ~16383", v)
	}
}

// TestMediaWebSocketIncomingVideoAsJPEG pushes real H.264 (made by the project's ffmpeg)
// through the handler and expects JPEG pictures and orientation on the socket.
func TestMediaWebSocketIncomingVideoAsJPEG(t *testing.T) {
	if _, err := exec.LookPath("ffmpeg"); err != nil {
		t.Skip("ffmpeg not installed")
	}
	srv, svc, media := startMediaServer(t)
	media.SetVideoOrientation(1)
	c := dial(t, srv, svc, "&video=jpeg")

	// Encode a few test pictures to H.264 with ffmpeg directly.
	var out []byte
	var err error
	for _, codec := range []string{"libx264", "libopenh264"} {
		out, err = exec.Command("ffmpeg", "-hide_banner", "-loglevel", "error", "-f", "lavfi",
			"-i", "testsrc=size=320x240:rate=15", "-frames:v", "12", "-pix_fmt", "yuv420p", "-c:v", codec,
			"-bsf:v", "h264_metadata=aud=insert", "-f", "h264", "pipe:1").Output()
		if err == nil && len(out) > 0 {
			break
		}
	}
	if err != nil || len(out) == 0 {
		t.Skipf("cannot produce test h264: %v", err)
	}
	// split into access units at each AUD
	var units [][]byte
	starts := []int{}
	for i := 0; i+4 < len(out); i++ {
		if out[i] == 0 && out[i+1] == 0 && out[i+2] == 1 && out[i+3]&0x1f == 9 {
			s := i
			if i > 0 && out[i-1] == 0 {
				s = i - 1
			}
			starts = append(starts, s)
		}
	}
	for i, s := range starts {
		e := len(out)
		if i+1 < len(starts) {
			e = starts[i+1]
		}
		units = append(units, out[s:e])
	}
	if len(units) < 5 {
		t.Fatalf("only %d access units", len(units))
	}
	go func() {
		for _, u := range units {
			media.FeedIncomingVideoForTest(u)
			time.Sleep(66 * time.Millisecond)
		}
	}()

	_ = c.SetReadDeadline(time.Now().Add(10 * time.Second))
	for {
		_, data, err := c.ReadMessage()
		if err != nil {
			t.Fatalf("no picture received: %v", err)
		}
		kind, meta, payload, err := call.ParseFrame(data)
		if err != nil || kind != call.FrameKindIncomingVideo {
			continue
		}
		if !bytes.HasPrefix(payload, []byte{0xff, 0xd8}) || !bytes.HasSuffix(payload, []byte{0xff, 0xd9}) {
			t.Fatal("payload is not a JPEG")
		}
		var m struct {
			Orientation int    `json:"orientation"`
			Format      string `json:"format"`
		}
		if err := json.Unmarshal(meta, &m); err != nil || m.Orientation != 1 || m.Format != "jpeg" {
			t.Fatalf("metadata %s (%v)", meta, err)
		}
		return
	}
}

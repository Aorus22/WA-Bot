package http

import (
	"encoding/json"
	"log"
	"net/http"
	"sync"
	"time"

	"github.com/gorilla/mux"
	"github.com/gorilla/websocket"

	"wa-bot/internal/infrastructure/call"
)

// mediaControlMessage is the JSON body carried by the keyframe control frame.
type mediaControlMessage struct {
	Type string `json:"type"`
}

// CallMediaHandler serves the dedicated binary media WebSocket for one call.
// It is separate from the JSON /ws hub and only bridges media; it never hangs
// up the WhatsApp call when the browser disconnects.
type CallMediaHandler struct {
	callSvc *call.CallService
}

// NewCallMediaHandler builds the media WebSocket handler.
func NewCallMediaHandler(callSvc *call.CallService) *CallMediaHandler {
	return &CallMediaHandler{callSvc: callSvc}
}

// ServeWS upgrades the connection and runs the media read/write pumps. The
// caller must have supplied a valid ?token= for the call id in {id}.
func (h *CallMediaHandler) ServeWS(w http.ResponseWriter, r *http.Request) {
	callID := mux.Vars(r)["id"]
	token := r.URL.Query().Get("token")

	if err := h.callSvc.ValidateMediaSession(callID, token); err != nil {
		switch err {
		case call.ErrMediaSessionExpired:
			writeMediaError(w, http.StatusUnauthorized, "media session expired")
		default:
			writeMediaError(w, http.StatusForbidden, "invalid media session")
		}
		return
	}

	// Single lock acquisition: MediaForCall verifies the call is active and
	// returns its live media in one call (no separate GetActiveCall round-trip).
	media := h.callSvc.MediaForCall(callID)
	if media == nil {
		writeMediaError(w, http.StatusConflict, "call not active")
		return
	}

	conn, err := upgrader.Upgrade(w, r, nil)
	if err != nil {
		log.Printf("media ws upgrade error: %v", err)
		return
	}
	defer conn.Close()

	var writeMu sync.Mutex
	write := func(msgType int, data []byte) error {
		writeMu.Lock()
		defer writeMu.Unlock()
		conn.SetWriteDeadline(nowPlus(10))
		return conn.WriteMessage(msgType, data)
	}

	go h.readPump(conn, media)
	h.writePump(conn, media, write)
}

// readPump reads binary frames from the browser and feeds them into the call.
// A 1 MiB read limit bounds a single frame; a pong handler keeps the read
// deadline alive so a quiet-but-connected browser is not spuriously dropped.
func (h *CallMediaHandler) readPump(conn *websocket.Conn, media *call.CallMedia) {
	defer conn.Close()
	conn.SetReadLimit(1 << 20)
	conn.SetPongHandler(func(string) error {
		conn.SetReadDeadline(nowPlus(60))
		return nil
	})
	for {
		conn.SetReadDeadline(nowPlus(60))
		_, data, err := conn.ReadMessage()
		if err != nil {
			if websocket.IsUnexpectedCloseError(err, websocket.CloseGoingAway, websocket.CloseAbnormalClosure) {
				log.Printf("media ws read error: %v", err)
			}
			return
		}
		kind, _, payload, err := call.ParseFrame(data)
		if err != nil {
			continue
		}
		switch kind {
		case call.FrameKindOutgoingAudio:
			frame, err := call.PCMS16ToFloat32(payload, 2)
			if err != nil {
				continue
			}
			media.WriteOutgoingAudio(frame)
		case call.FrameKindOutgoingVideo:
			_ = media.WriteOutgoingVideo(payload)
		}
	}
}

// writePump drains the call's inbound media and control channels and writes
// them to the browser. It returns when the connection is gone or the call ends
// (media.Ended fires), sending a close frame so the peer sees a clean shutdown.
func (h *CallMediaHandler) writePump(conn *websocket.Conn, media *call.CallMedia, write func(int, []byte) error) {
	audioCh := media.IncomingAudio()
	videoCh := media.IncomingVideo()
	keyframeCh := media.KeyframeRequests()
	endedCh := media.Ended()

	pingTicker := time.NewTicker(54 * time.Second)
	defer pingTicker.Stop()

	for {
		select {
		case frame, ok := <-audioCh:
			if !ok {
				return
			}
			data, err := call.EncodeFrame(call.FrameKindIncomingAudio, nil, call.PCMFloat32ToS16(frame, 2))
			if err != nil {
				log.Printf("media ws encode audio frame: %v", err)
				continue
			}
			if err := write(websocket.BinaryMessage, data); err != nil {
				return
			}
		case vf, ok := <-videoCh:
			if !ok {
				return
			}
			meta, _ := json.Marshal(map[string]string{"participant_jid": vf.ParticipantJID})
			data, err := call.EncodeFrame(call.FrameKindIncomingVideo, meta, vf.AccessUnit)
			if err != nil {
				log.Printf("media ws encode video frame: %v", err)
				continue
			}
			if err := write(websocket.BinaryMessage, data); err != nil {
				return
			}
		case _, ok := <-keyframeCh:
			if !ok {
				return
			}
			body, _ := json.Marshal(mediaControlMessage{Type: "keyframe_request"})
			data, err := call.EncodeFrame(call.FrameKindKeyframe, nil, body)
			if err != nil {
				log.Printf("media ws encode keyframe frame: %v", err)
				continue
			}
			if err := write(websocket.BinaryMessage, data); err != nil {
				return
			}
		case <-pingTicker.C:
			// Keepalive: pings also refresh the peer's/read pump's liveness so a
			// quiet-but-alive browser is not spuriously dropped by the 60s read
			// deadline.
			if err := write(websocket.PingMessage, nil); err != nil {
				return
			}
		case <-endedCh:
			_ = write(websocket.CloseMessage, []byte{})
			return
		}
	}
}

// writeMediaError writes a JSON error response before any upgrade attempt.
func writeMediaError(w http.ResponseWriter, status int, msg string) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(map[string]string{"error": msg})
}

func nowPlus(seconds int64) time.Time {
	return time.Now().Add(time.Duration(seconds) * time.Second)
}

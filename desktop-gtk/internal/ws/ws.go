// Package ws maintains the WebSocket connection to the backend's /ws
// endpoint and fans out typed events to subscribers.
//
// The backend protocol (see wa-bot internal/delivery/http/websocket.go):
//   - every message is a JSON envelope {"type": "...", "payload": ...}
//   - clients may send {"type":"ping"} (server replies "pong") and
//     {"type":"authenticate","payload":{"userId":"..."}} for targeted events
//   - the server sends protocol-level pings every ~54s and closes idle
//     connections after 60s; gorilla replies to pings automatically
//
// Handlers run on the read-loop goroutine. UI code must hop to the GTK main
// thread (glib.IdleAdd / ui.Dispatcher) before touching widgets.
package ws

import (
	"context"
	"encoding/json"
	"log"
	"sync"
	"sync/atomic"
	"time"

	"github.com/gorilla/websocket"
)

// Event names broadcast by the backend hub.
const (
	EventPong           = "pong"
	EventAuthSuccess    = "auth_success"
	EventQRCode         = "qr_code"
	EventNewMessage     = "new_message"
	EventMessageStatus  = "message_status"
	EventMessageDeleted = "message_deleted"
	EventMessageEdited  = "message_edited"
	EventChatNameUpdate = "chat_name_update"
	EventChatState      = "chat_state"
	EventChatsChanged   = "chats_changed"

	EventCallIncoming = "call.incoming"
	EventCallState    = "call.state"
	EventCallEnded    = "call.ended"
)

// Envelope is the wire format of every message on /ws.
type Envelope struct {
	Type    string          `json:"type"`
	Payload json.RawMessage `json:"payload"`
}

// QRCode is the payload of "qr_code" events (and GET /api/qr-code).
type QRCode struct {
	Code string `json:"code"`
}

// MessageStatus is the payload of "message_status" events.
type MessageStatus struct {
	ID     string `json:"id"`
	Status string `json:"status"`
}

// MessageRef identifies a message inside a chat ("message_deleted").
type MessageRef struct {
	ChatID string `json:"chatId"`
	ID     string `json:"id"`
}

// MessageEdited is the payload of "message_edited" events.
type MessageEdited struct {
	ChatID  string `json:"chatId"`
	ID      string `json:"id"`
	Content string `json:"content"`
}

// ChatNameUpdate is the payload of "chat_name_update" events.
type ChatNameUpdate struct {
	ChatID string `json:"chatId"`
	Name   string `json:"name"`
	Avatar string `json:"avatar"`
}

// Handler receives the raw JSON payload of an event. Called synchronously
// from the read-loop goroutine; keep it fast and hop threads for UI work.
type Handler func(payload json.RawMessage)

// Client is a reconnecting WebSocket client for the backend /ws endpoint.
type Client struct {
	url string

	mu       sync.RWMutex
	handlers map[string][]Handler
	conn     *websocket.Conn

	writeMu sync.Mutex // serializes writes on conn

	connected atomic.Bool
	onState   atomic.Value // func(connected bool), optional

	ctx    context.Context
	cancel context.CancelFunc
}

// New constructs a Client for ws://127.0.0.1:<port>/ws and starts the
// connect loop immediately.
func New(port int) *Client {
	ctx, cancel := context.WithCancel(context.Background())
	c := &Client{
		url:      "ws://127.0.0.1:" + itoa(port) + "/ws",
		handlers: make(map[string][]Handler),
		ctx:      ctx,
		cancel:   cancel,
	}
	go c.loop()
	return c
}

// Close stops the client and closes the current connection, if any.
func (c *Client) Close() {
	c.cancel()
	c.mu.Lock()
	conn := c.conn
	c.mu.Unlock()
	if conn != nil {
		conn.Close()
	}
}

// Connected reports whether the socket is currently open.
func (c *Client) Connected() bool { return c.connected.Load() }

// OnStateChange registers an optional callback invoked from the connect loop
// goroutine whenever the connection state flips.
func (c *Client) OnStateChange(fn func(connected bool)) {
	c.onState.Store(fn)
}

// On subscribes fn to the given event type.
func (c *Client) On(event string, fn Handler) {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.handlers[event] = append(c.handlers[event], fn)
}

// Send writes an app-level envelope to the server. Safe for concurrent use;
// silently dropped while disconnected.
func (c *Client) Send(msgType string, payload any) {
	c.mu.RLock()
	conn := c.conn
	c.mu.RUnlock()
	if conn == nil {
		return
	}
	data, err := json.Marshal(Envelope{Type: msgType, Payload: mustRaw(payload)})
	if err != nil {
		log.Printf("ws: marshal %s: %v", msgType, err)
		return
	}
	c.writeMu.Lock()
	defer c.writeMu.Unlock()
	_ = conn.SetWriteDeadline(time.Now().Add(5 * time.Second))
	if err := conn.WriteMessage(websocket.TextMessage, data); err != nil {
		log.Printf("ws: write %s: %v", msgType, err)
	}
}

// Authenticate identifies this client to the hub (enables targeted events).
func (c *Client) Authenticate(userID string) {
	c.Send("authenticate", map[string]string{"userId": userID})
}

// Ping sends the app-level keepalive the web client also uses.
func (c *Client) Ping() { c.Send("ping", nil) }

// loop dials, reads, and reconnects with capped exponential backoff until
// the client is closed.
func (c *Client) loop() {
	const (
		minBackoff = 1 * time.Second
		maxBackoff = 15 * time.Second
	)
	backoff := minBackoff

	for c.ctx.Err() == nil {
		conn, _, err := websocket.DefaultDialer.DialContext(c.ctx, c.url, nil)
		if err != nil {
			if c.ctx.Err() != nil {
				return
			}
			log.Printf("ws: dial %s failed: %v (retry in %s)", c.url, err, backoff)
			select {
			case <-c.ctx.Done():
				return
			case <-time.After(backoff):
			}
			backoff = min(backoff*2, maxBackoff)
			continue
		}
		backoff = minBackoff

		c.mu.Lock()
		c.conn = conn
		c.mu.Unlock()
		c.connected.Store(true)
		if fn, ok := c.onState.Load().(func(bool)); ok && fn != nil {
			fn(true)
		}
		log.Printf("ws: connected to %s", c.url)

		c.Authenticate("user-1")
		pingCtx, stopPing := context.WithCancel(c.ctx)
		go c.pingLoop(pingCtx)

		err = c.readLoop(conn)

		stopPing()
		c.connected.Store(false)
		c.mu.Lock()
		c.conn = nil
		c.mu.Unlock()
		if fn, ok := c.onState.Load().(func(bool)); ok && fn != nil {
			fn(false)
		}
		conn.Close()

		if c.ctx.Err() != nil {
			return
		}
		log.Printf("ws: disconnected: %v (retry in %s)", err, backoff)
		select {
		case <-c.ctx.Done():
			return
		case <-time.After(backoff):
		}
	}
}

// readLoop pumps inbound envelopes into the registered handlers until the
// connection breaks.
func (c *Client) readLoop(conn *websocket.Conn) error {
	for {
		_, data, err := conn.ReadMessage()
		if err != nil {
			return err
		}
		var env Envelope
		if err := json.Unmarshal(data, &env); err != nil {
			log.Printf("ws: bad envelope: %v", err)
			continue
		}
		c.dispatch(env)
	}
}

// pingLoop sends the app-level ping every 25s so NATs and the server's
// 60s read deadline stay happy.
func (c *Client) pingLoop(ctx context.Context) {
	ticker := time.NewTicker(25 * time.Second)
	defer ticker.Stop()
	for {
		select {
		case <-ctx.Done():
			return
		case <-ticker.C:
			c.Ping()
		}
	}
}

// dispatch invokes all handlers registered for the envelope's type.
func (c *Client) dispatch(env Envelope) {
	c.mu.RLock()
	handlers := make([]Handler, len(c.handlers[env.Type]))
	copy(handlers, c.handlers[env.Type])
	c.mu.RUnlock()

	for _, fn := range handlers {
		func() {
			defer func() {
				if r := recover(); r != nil {
					log.Printf("ws: handler for %s panicked: %v", env.Type, r)
				}
			}()
			fn(env.Payload)
		}()
	}
}

// Decode unmarshals a raw payload into out. "auth_success" and "pong" carry
// a null payload; Decode tolerates that when out is nil.
func Decode(payload json.RawMessage, out any) error {
	if len(payload) == 0 || string(payload) == "null" {
		return nil
	}
	return json.Unmarshal(payload, out)
}

func mustRaw(v any) json.RawMessage {
	if v == nil {
		return json.RawMessage("null")
	}
	data, err := json.Marshal(v)
	if err != nil {
		return json.RawMessage("null")
	}
	return data
}

func itoa(n int) string {
	if n == 0 {
		return "0"
	}
	var buf [20]byte
	i := len(buf)
	for n > 0 {
		i--
		buf[i] = byte('0' + n%10)
		n /= 10
	}
	return string(buf[i:])
}

package http

import (
	"encoding/json"
	"log"
	"net/http"
	"sync"
	"time"

	"github.com/gorilla/websocket"
)

var upgrader = websocket.Upgrader{
	ReadBufferSize:  1024,
	WriteBufferSize: 1024,
	CheckOrigin: func(r *http.Request) bool {
		return true
	},
}

type WSMessage struct {
	Type    string      `json:"type"`
	Payload interface{} `json:"payload"`
}

type WSClient struct {
	ID     string
	Conn   *websocket.Conn
	Send   chan WSMessage
	Hub    *WSHub
	UserID string
}

type WSHub struct {
	clients    map[*WSClient]bool
	broadcast  chan WSMessage
	register   chan *WSClient
	unregister chan *WSClient
	mu         sync.RWMutex
	byUser     map[string]*WSClient
}

func NewWSHub() *WSHub {
	return &WSHub{
		clients:    make(map[*WSClient]bool),
		broadcast:  make(chan WSMessage),
		register:   make(chan *WSClient),
		unregister: make(chan *WSClient),
		byUser:     make(map[string]*WSClient),
	}
}

func (h *WSHub) Run() {
	for {
		select {
		case client := <-h.register:
			h.mu.Lock()
			h.clients[client] = true
			if client.UserID != "" {
				h.byUser[client.UserID] = client
			}
			h.mu.Unlock()
			log.Printf("Client connected: %s", client.ID)

		case client := <-h.unregister:
			h.mu.Lock()
			if _, ok := h.clients[client]; ok {
				delete(h.clients, client)
				if client.UserID != "" {
					delete(h.byUser, client.UserID)
				}
				close(client.Send)
			}
			h.mu.Unlock()
			log.Printf("Client disconnected: %s", client.ID)

		case message := <-h.broadcast:
			h.mu.RLock()
			for client := range h.clients {
				select {
				case client.Send <- message:
				default:
					close(client.Send)
					delete(h.clients, client)
				}
			}
			h.mu.RUnlock()
		}
	}
}

func (h *WSHub) Broadcast(message WSMessage) {
	h.broadcast <- message
}

func (h *WSHub) BroadcastMessage(msgType string, payload interface{}) {
	h.broadcast <- WSMessage{Type: msgType, Payload: payload}
}

func (h *WSHub) SendToUser(userID string, message WSMessage) {
	h.mu.RLock()
	defer h.mu.RUnlock()
	if client, ok := h.byUser[userID]; ok {
		select {
		case client.Send <- message:
		default:
			log.Printf("Failed to send message to user %s", userID)
		}
	}
}

func (h *WSHub) SendMessageToUser(userID string, msgType string, payload interface{}) {
	h.mu.RLock()
	defer h.mu.RUnlock()
	if client, ok := h.byUser[userID]; ok {
		select {
		case client.Send <- WSMessage{Type: msgType, Payload: payload}:
		default:
			log.Printf("Failed to send message to user %s", userID)
		}
	}
}

func (c *WSClient) ReadPump() {
	defer func() {
		c.Hub.unregister <- c
		c.Conn.Close()
	}()

	c.Conn.SetReadDeadline(time.Now().Add(60 * time.Second))
	c.Conn.SetPongHandler(func(string) error {
		c.Conn.SetReadDeadline(time.Now().Add(60 * time.Second))
		return nil
	})

	for {
		_, message, err := c.Conn.ReadMessage()
		if err != nil {
			if websocket.IsUnexpectedCloseError(err, websocket.CloseGoingAway, websocket.CloseAbnormalClosure) {
				log.Printf("WebSocket error: %v", err)
			}
			break
		}

		var msg WSMessage
		if err := json.Unmarshal(message, &msg); err != nil {
			log.Printf("JSON error: %v", err)
			continue
		}

		// Handle incoming messages from client
		switch msg.Type {
		case "ping":
			c.Send <- WSMessage{Type: "pong", Payload: nil}
		case "authenticate":
			if payload, ok := msg.Payload.(map[string]interface{}); ok {
				if userID, ok := payload["userId"].(string); ok {
					c.Hub.mu.Lock()
					c.UserID = userID
					c.Hub.byUser[userID] = c
					c.Hub.mu.Unlock()
				}
			}
		}
	}
}

func (c *WSClient) WritePump() {
	ticker := time.NewTicker(54 * time.Second)
	defer func() {
		ticker.Stop()
		c.Conn.Close()
	}()

	for {
		select {
		case message, ok := <-c.Send:
			c.Conn.SetWriteDeadline(time.Now().Add(10 * time.Second))
			if !ok {
				c.Conn.WriteMessage(websocket.CloseMessage, []byte{})
				return
			}

			data, err := json.Marshal(message)
			if err != nil {
				log.Printf("Marshal error: %v", err)
				continue
			}

			if err := c.Conn.WriteMessage(websocket.TextMessage, data); err != nil {
				return
			}

		case <-ticker.C:
			c.Conn.SetWriteDeadline(time.Now().Add(10 * time.Second))
			if err := c.Conn.WriteMessage(websocket.PingMessage, nil); err != nil {
				return
			}
		}
	}
}

func (s *HTTPServer) handleWebSocket(hub *WSHub) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		conn, err := upgrader.Upgrade(w, r, nil)
		if err != nil {
			log.Printf("WebSocket upgrade error: %v", err)
			return
		}

		client := &WSClient{
			ID:   generateClientID(),
			Conn: conn,
			Send: make(chan WSMessage, 256),
			Hub:  hub,
		}

		hub.register <- client

		go client.WritePump()
		go client.ReadPump()
	}
}

func generateClientID() string {
	return time.Now().Format("20060102150405") + "-" + randomString(6)
}

func randomString(n int) string {
	const letters = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
	b := make([]byte, n)
	for i := range b {
		b[i] = letters[time.Now().UnixNano()%int64(len(letters))]
		time.Sleep(time.Nanosecond)
	}
	return string(b)
}

// Package ui wires the backend subprocess and HTTP client to the GTK UI.
// All widget access must occur on the main thread; see Dispatcher.
package ui

import (
	"context"
	"encoding/json"
	"log"
	"os"
	"time"

	"github.com/diamondburned/gotk4-adwaita/pkg/adw"
	"github.com/diamondburned/gotk4/pkg/gio/v2"
	"github.com/diamondburned/gotk4/pkg/glib/v2"
	qrcode "github.com/skip2/go-qrcode"

	"wa-bot-desktop/internal/api"
	"wa-bot-desktop/internal/backend"
	"wa-bot-desktop/internal/media"
	"wa-bot-desktop/internal/store"
	"wa-bot-desktop/internal/views"
	"wa-bot-desktop/internal/ws"
)

// AppConfig holds construction parameters for App.
type AppConfig struct {
	ID          string
	Name        string
	Version     string
	UserDataDir string
	Manager     *backend.Manager // nil when running with -no-backend
	PortChannel <-chan int
}

// App is the top-level GTK application. It owns the lifecycle: Adwaita shell,
// backend port discovery, API/WS clients, shared store, and all views.
type App struct {
	cfg    AppConfig
	adwApp *adw.Application
	window *Window

	client  *api.Client
	store   *store.Store
	cache   *media.Cache
	wsSock  *ws.Client
	login   *views.Login
	pane    *views.ChatsPane
	dash    *views.Dashboard
	setting *views.Settings

	sessionUp bool // WhatsApp session believed connected
}

// NewApp constructs the App but does not run the GTK main loop.
func NewApp(cfg AppConfig) (*App, error) {
	a := &App{cfg: cfg}
	a.adwApp = adw.NewApplication(cfg.ID, gio.ApplicationFlagsNone)
	a.adwApp.Application.ConnectActivate(func() { a.onActivate() })
	a.adwApp.Application.ConnectShutdown(func() { a.onShutdown() })
	return a, nil
}

// Run starts the GTK main loop. Blocks until the app exits.
func (a *App) Run(ctx context.Context) int {
	go func() {
		<-ctx.Done()
		log.Printf("context cancelled, requesting app quit")
		glib.IdleAdd(func() bool {
			a.adwApp.Quit()
			return false
		})
	}()
	return a.adwApp.Run([]string{os.Args[0]})
}

func (a *App) onActivate() {
	log.Printf("Adw app activated; constructing window")
	loadCSS()

	w, err := NewWindow(a.adwApp, a.cfg.Name, a.cfg.Version)
	if err != nil {
		log.Fatalf("create window: %v", err)
	}
	a.window = w
	views.MainWindow = w.Win

	a.store = store.New()
	a.pane = views.NewChatsPane()
	a.dash = views.NewDashboard()
	a.setting = views.NewSettings(a.cfg.UserDataDir, a.cfg.Manager)

	w.AddDashboard(a.dash)
	w.AddChats(a.pane)
	w.AddSettings(a.setting)

	a.login = views.NewLogin()
	w.SetLogin(a.login)
	a.login.SetLogoutCallback(a.onLogoutClicked)
	a.login.SetRetryCallback(a.onRetryQR)

	w.Show()
	a.registerShortcuts()

	go a.awaitBackend()

	w.Win.ConnectCloseRequest(func() bool {
		log.Printf("window close requested")
		glib.IdleAdd(func() bool {
			a.adwApp.Quit()
			return false
		})
		return false
	})
}

func (a *App) onShutdown() {
	log.Printf("Adw app shutting down; stopping backend")
	if a.wsSock != nil {
		a.wsSock.Close()
	}
	if a.cfg.Manager != nil {
		_ = a.cfg.Manager.Stop(context.Background())
	}
}

// awaitBackend waits for the discovered port, then builds clients and views.
func (a *App) awaitBackend() {
	port, ok := <-a.cfg.PortChannel
	if !ok || port == 0 {
		log.Printf("backend port channel closed without a value")
		return
	}
	log.Printf("backend port discovered: %d", port)
	a.client = api.NewClient(port)

	glib.IdleAdd(func() bool {
		go func() {
			ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
			defer cancel()
			if err := a.client.GetHealth(ctx); err != nil {
				log.Printf("/api/health FAILED: %v", err)
				return
			}
			log.Printf("/api/health OK")
			glib.IdleAdd(func() bool {
				a.bootstrapViews(port)
				return false
			})
		}()
		return false
	})
}

// bootstrapViews wires every collaborator once the API is reachable.
func (a *App) bootstrapViews(port int) {
	a.cache = media.NewCache(a.client, a.cfg.UserDataDir)
	a.pane.SetDeps(a.client, a.store, a.cache)
	a.dash.SetClient(a.client)
	a.setting.SetBackendPort(port)

	a.startWebSocket(port)

	// Initial chat snapshot + auth gate.
	go func() {
		a.fetchChats()
		// Periodic safety-net refresh: keeps order/unread fresh even if a
		// WS event was missed (backend list is a cheap local SQLite read).
		ticker := time.NewTicker(30 * time.Second)
		defer ticker.Stop()
		for {
			select {
			case <-ticker.C:
				if a.sessionUp {
					a.fetchChats()
				}
			}
		}
	}()
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 8*time.Second)
		defer cancel()
		st, err := a.client.GetStatus(ctx)
		glib.IdleAdd(func() bool {
			if err == nil && st != nil && (st.Connected || st.LoggedIn) {
				a.markSessionUp(st.Phone)
			} else {
				a.showLoginScreen(false, "Menunggu kode QR dari backend…")
				go a.pollInitialQR()
			}
			return false
		})
	}()
}

// startWebSocket opens /ws and registers event handling.
func (a *App) startWebSocket(port int) {
	a.wsSock = ws.New(port)

	a.wsSock.On(ws.EventNewMessage, func(payload json.RawMessage) {
		var m api.Message
		if err := ws.Decode(payload, &m); err != nil || m.ID == "" {
			return
		}
		if m.From != store.OutgoingFrom && m.ChatID == a.store.ActiveChat() {
			go func() {
				ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
				defer cancel()
				_ = a.client.MarkRead(ctx, m.ChatID)
			}()
		}
		a.store.ApplyIncoming(m)
	})

	a.wsSock.On(ws.EventMessageStatus, func(payload json.RawMessage) {
		var s ws.MessageStatus
		if err := ws.Decode(payload, &s); err != nil || s.ID == "" {
			return
		}
		a.store.PatchStatusByID(s.ID, s.Status)
	})

	a.wsSock.On(ws.EventMessageDeleted, func(payload json.RawMessage) {
		var ref ws.MessageRef
		if err := ws.Decode(payload, &ref); err != nil {
			return
		}
		a.store.DeleteMessage(ref.ChatID, ref.ID)
	})

	a.wsSock.On(ws.EventMessageEdited, func(payload json.RawMessage) {
		var e ws.MessageEdited
		if err := ws.Decode(payload, &e); err != nil {
			return
		}
		a.store.EditMessage(e.ChatID, e.ID, e.Content)
	})

	a.wsSock.On(ws.EventChatNameUpdate, func(payload json.RawMessage) {
		var n ws.ChatNameUpdate
		if err := ws.Decode(payload, &n); err != nil {
			return
		}
		a.store.RenameChat(n.ChatID, n.Name, n.Avatar)
	})

	a.wsSock.On(ws.EventAuthSuccess, func(_ json.RawMessage) {
		glib.IdleAdd(func() bool {
			log.Printf("ws: auth_success — session established")
			a.markSessionUp("")
			go a.fetchChats()
			return false
		})
	})

	a.wsSock.On(ws.EventQRCode, func(payload json.RawMessage) {
		var qr ws.QRCode
		if err := ws.Decode(payload, &qr); err != nil || qr.Code == "" {
			return
		}
		// The backend serves its cached QR unconditionally, even while a
		// session is live — re-verify before painting it.
		go func() {
			ctx, cancel := context.WithTimeout(context.Background(), 4*time.Second)
			defer cancel()
			st, err := a.client.GetStatus(ctx)
			glib.IdleAdd(func() bool {
				if err == nil && st != nil && st.LoggedIn {
					a.markSessionUp(st.Phone)
					return false
				}
				if !a.sessionUp {
					a.window.ShowLogin(true)
					a.presentQR(qr.Code)
				}
				return false
			})
		}()
	})

	a.wsSock.OnStateChange(func(connected bool) {
		glib.IdleAdd(func() bool {
			if !connected {
				log.Printf("ws: connection lost; will retry in background")
			}
			return false
		})
	})
}

// fetchChats refreshes the chat list snapshot into the store.
func (a *App) fetchChats() {
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	chats, err := a.client.ListChats(ctx, 500, "")
	if err != nil {
		log.Printf("chats: fetch failed: %v", err)
		return
	}
	a.store.ReplaceChats(chats)
}

// pollInitialQR grabs whatever QR is already cached at startup (the WS also
// pushes fresh codes; this covers the "QR already waiting" case). Every
// iteration re-checks the session so a stale cached QR is never painted over
// a live login.
func (a *App) pollInitialQR() {
	for i := 0; i < 30; i++ {
		if a.sessionUp {
			return
		}
		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		if st, err := a.client.GetStatus(ctx); err == nil && st != nil && st.LoggedIn {
			cancel()
			glib.IdleAdd(func() bool {
				a.markSessionUp(st.Phone)
				return false
			})
			return
		}
		code, err := a.client.GetQRCode(ctx)
		cancel()
		if err == nil && code != "" {
			glib.IdleAdd(func() bool {
				if !a.sessionUp {
					a.presentQR(code)
				}
				return false
			})
			return
		}
		time.Sleep(2 * time.Second)
	}
}

// presentQR renders the QR code into the login screen.
func (a *App) presentQR(code string) {
	png, err := qrcode.Encode(code, qrcode.Medium, 512)
	if err != nil {
		log.Printf("qr encode: %v", err)
		return
	}
	a.login.SetQR(png, "Pindai kode ini dengan WhatsApp di ponselmu.")
}

// markSessionUp hides the login screen and switches to the chats page.
func (a *App) markSessionUp(phone string) {
	a.sessionUp = true
	a.login.SetConnected(phone)
	a.window.ShowLogin(false)
	a.window.SwitchTo("chats")
}

// showLoginScreen reveals the pairing screen.
func (a *App) showLoginScreen(pollQR bool, statusText string) {
	a.sessionUp = false
	a.window.SwitchTo("chats")
	a.login.SetWaiting(statusText)
	a.window.ShowLogin(true)
	if pollQR {
		go a.pollInitialQR()
	}
}

// onRetryQR refetches the latest QR from the backend and restarts polling.
// Fresh codes also arrive over the WS; this covers a missed/expired screen.
func (a *App) onRetryQR() {
	if a.sessionUp || a.client == nil {
		return
	}
	a.login.SetWaiting("Mengambil QR terbaru…")
	go a.pollInitialQR()
}

// onLogoutClicked disconnects the WhatsApp session and returns to pairing.
func (a *App) onLogoutClicked() {
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
		defer cancel()
		if err := a.client.Logout(ctx); err != nil {
			log.Printf("logout failed: %v", err)
		}
		glib.IdleAdd(func() bool {
			a.store.Reset()
			a.cache.Clear()
			a.pane.Clear()
			a.showLoginScreen(true, "Sesi diputus. Menunggu QR baru…")
			return false
		})
	}()
}

// registerShortcuts wires Ctrl+Q (quit) and Ctrl+1..3 page switching.
func (a *App) registerShortcuts() {
	quitAction := gio.NewSimpleAction("quit", nil)
	quitAction.ConnectActivate(func(p *glib.Variant) { a.adwApp.Quit() })
	a.adwApp.AddAction(quitAction)
	a.adwApp.SetAccelsForAction("app.quit", []string{"<Primary>Q"})

	switches := []struct {
		name  string
		page  string
		accel string
	}{
		{"dashboard", "dashboard", "<Primary>1"},
		{"chats", "chats", "<Primary>2"},
		{"settings", "settings", "<Primary>comma"},
	}
	for _, sw := range switches {
		action := gio.NewSimpleAction(sw.name, nil)
		page := sw.page
		action.ConnectActivate(func(p *glib.Variant) { a.window.SwitchTo(page) })
		a.adwApp.AddAction(action)
		a.adwApp.SetAccelsForAction("app."+sw.name, []string{sw.accel})
	}
}

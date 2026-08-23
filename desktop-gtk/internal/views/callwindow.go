package views

import (
	"context"
	"fmt"
	"log"
	"strings"
	"time"

	"github.com/diamondburned/gotk4-adwaita/pkg/adw"
	"github.com/diamondburned/gotk4/pkg/glib/v2"
	"github.com/diamondburned/gotk4/pkg/gtk/v4"

	"wa-bot-desktop/internal/api"
	"wa-bot-desktop/internal/store"
)

// CallState mirrors the backend's CallStateResponse JSON broadcast on the
// call.incoming / call.state WS events.
type CallState struct {
	ID         string `json:"id"`
	Status     string `json:"status"`
	Type       string `json:"type"`
	Direction  string `json:"direction"`
	Source     string `json:"source"`
	MediaMode  string `json:"media_mode"`
	Target     string `json:"target"`
	GroupJID   string `json:"group_jid"`
	StartedAt  int64  `json:"started_at"`
	AnsweredAt *int64 `json:"answered_at"`
}

var callTerminalStatuses = map[string]bool{
	"ended": true, "rejected": true, "missed": true,
	"busy": true, "failed": true, "interrupted": true,
}

// CallWindow is the separate "call mode" window mirroring the web's
// incoming-call card: status pill, big avatar, caller name, live status, and
// one round action button. GTK has no local call audio, so an incoming ring
// offers Decline only (answer stays on phone/web); an outgoing or ongoing
// call offers Hang up.
//
// All exported methods must run on the GTK main thread.
type CallWindow struct {
	client *api.Client
	store  *store.Store

	win    *gtk.Window
	pill   *gtk.Label
	avatar *adw.Avatar
	name   *gtk.Label
	sub    *gtk.Label
	status *gtk.Label
	action *gtk.Button
	actionImg *gtk.Image
	actionLbl *gtk.Label

	state      CallState
	actionMode string // "reject" | "hangup"
	timerID    glib.SourceHandle
}

// NewCallWindow constructs the hidden call-mode window.
func NewCallWindow(client *api.Client, st *store.Store) *CallWindow {
	w := &CallWindow{client: client, store: st}

	w.win = gtk.NewWindow()
	w.win.SetTitle("WA Bot — Panggilan")
	w.win.SetDefaultSize(340, 500)
	w.win.SetResizable(false)
	w.win.SetModal(true)
	if MainWindow != nil {
		w.win.SetTransientFor(MainWindow)
	}
	w.win.SetHideOnClose(true)

	content := gtk.NewBox(gtk.OrientationVertical, 10)
	content.SetMarginTop(28)
	content.SetMarginBottom(24)
	content.SetMarginStart(24)
	content.SetMarginEnd(24)
	content.SetVAlign(gtk.AlignCenter)
	content.SetVExpand(true)

	w.pill = gtk.NewLabel("")
	w.pill.AddCSSClass("call-pill")
	content.Append(w.pill)

	w.avatar = adw.NewAvatar(96, "?", false)
	w.avatar.SetSizeRequest(96, 96)
	avWrap := gtk.NewBox(gtk.OrientationVertical, 0)
	avWrap.SetHAlign(gtk.AlignCenter)
	avWrap.Append(w.avatar)
	content.Append(avWrap)

	w.name = gtk.NewLabel("")
	w.name.AddCSSClass("title-2")
	content.Append(w.name)

	w.sub = gtk.NewLabel("")
	w.sub.AddCSSClass("caption")
	w.sub.AddCSSClass("dim-label")
	w.sub.SetEllipsize(3) // PANGO_ELLIPSIZE_END
	content.Append(w.sub)

	spacer := gtk.NewBox(gtk.OrientationVertical, 0)
	spacer.SetVExpand(true)
	content.Append(spacer)

	w.status = gtk.NewLabel("")
	w.status.AddCSSClass("call-status")
	content.Append(w.status)

	actionCol := gtk.NewBox(gtk.OrientationVertical, 8)
	actionCol.SetHAlign(gtk.AlignCenter)
	actionCol.SetMarginTop(18)

	w.action = gtk.NewButton()
	w.actionImg = gtk.NewImageFromIconName("call-end-symbolic")
	w.actionImg.SetPixelSize(26)
	w.action.SetChild(w.actionImg)
	w.action.AddCSSClass("call-round")
	w.action.ConnectClicked(func() { w.onAction() })
	actionCol.Append(w.action)

	w.actionLbl = gtk.NewLabel("")
	w.actionLbl.AddCSSClass("caption")
	w.actionLbl.AddCSSClass("dim-label")
	actionCol.Append(w.actionLbl)
	content.Append(actionCol)

	w.win.SetChild(content)
	return w
}

// Widget returns the underlying window.
func (w *CallWindow) Window() *gtk.Window { return w.win }

// Update applies a call lifecycle state, showing/hiding the window as needed.
func (w *CallWindow) Update(s CallState) {
	if s.ID == "" || callTerminalStatuses[s.Status] {
		w.Hide()
		return
	}
	w.state = s

	isVideo := s.Type == "video" || s.Type == "group_video"
	incomingRinging := s.Direction == "incoming" && s.Status == "ringing"

	kind := "voice"
	if isVideo {
		kind = "video"
	}
	if incomingRinging {
		w.pill.SetText(strings.ToUpper("Incoming " + kind + " call"))
	} else {
		w.pill.SetText(strings.ToUpper(kind + " call"))
	}

	name := callPrettyJID(s.Target)
	if w.store != nil {
		if chat, ok := w.store.Chat(s.Target); ok {
			name = displayName(chat)
		}
	}
	w.avatar.SetText(initialsOf(name))
	w.name.SetText(name)
	w.sub.SetText(s.Target)
	w.status.SetText(callStatusText(s))

	if incomingRinging {
		w.setAction("call-end-symbolic", "Decline", "reject")
	} else {
		w.setAction("call-end-symbolic", "Hang up", "hangup")
	}

	w.stopTimer()
	if s.Status == "connected" {
		from := s.StartedAt
		if s.AnsweredAt != nil {
			from = *s.AnsweredAt
		}
		w.startTimer(from)
	}

	if !w.win.Visible() {
		w.win.Show()
	}
	w.win.Present()
}

// Ended hides the window once a call reaches a terminal state.
func (w *CallWindow) Ended(id string) {
	if id == "" || id == w.state.ID {
		w.Hide()
	}
}

// Hide closes the window and stops the duration ticker.
func (w *CallWindow) Hide() {
	w.stopTimer()
	w.win.Hide()
}

func (w *CallWindow) setAction(icon, label, mode string) {
	w.actionImg.SetFromIconName(icon)
	w.actionLbl.SetText(label)
	w.actionMode = mode
}

func (w *CallWindow) onAction() {
	id := w.state.ID
	mode := w.actionMode
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		var err error
		if mode == "reject" {
			err = w.client.RejectCall(ctx, id)
		} else {
			err = w.client.HangupCall(ctx, id)
		}
		if err != nil {
			log.Printf("callwindow: %s %s: %v", mode, id, err)
			glib.IdleAdd(func() bool {
				// The backend also broadcasts call.ended on success; only
				// force-hide when the request itself failed outright.
				w.stopTimer()
				return false
			})
		}
	}()
}

func (w *CallWindow) startTimer(fromMS int64) {
	start := time.UnixMilli(fromMS)
	tick := func() bool {
		d := time.Since(start)
		w.status.SetText(fmt.Sprintf("Connected %d:%02d", int(d.Minutes()), int(d.Seconds())%60))
		return true // keep ticking
	}
	tick()
	w.timerID = glib.TimeoutSecondsAdd(1, tick)
}

func (w *CallWindow) stopTimer() {
	if w.timerID != 0 {
		glib.SourceRemove(w.timerID)
		w.timerID = 0
	}
}

func callStatusText(s CallState) string {
	switch s.Status {
	case "preparing":
		return "Preparing…"
	case "initiating":
		return "Calling…"
	case "ringing":
		return "Ringing…"
	case "connecting":
		return "Connecting…"
	case "connected":
		return "Connected"
	case "ending":
		return "Ending…"
	default:
		return callStatusLabel(s.Status)
	}
}

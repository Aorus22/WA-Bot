// Package views contains the top-level view widgets (Dashboard, Chats, Settings).
// Each view exposes a Widget() method returning the root gtk.Widgetter.
package views

import (
	"context"
	"fmt"
	"log"
	"time"

	"github.com/diamondburned/gotk4-adwaita/pkg/adw"
	"github.com/diamondburned/gotk4/pkg/glib/v2"
	"github.com/diamondburned/gotk4/pkg/gtk/v4"

	"wa-bot-desktop/internal/api"
)

// Dashboard is the top-level dashboard view. It shows the connection state,
// session info, and quick stats from GET /api/status. Auto-refreshes every 10s.
type Dashboard struct {
	root   *adw.StatusPage
	clamp  *adw.Clamp
	inner  *gtk.Box
	client *api.Client

	// Labels (kept as fields so we can update them on refresh)
	titleLabel    *gtk.Label
	statusLabel   *gtk.Label
	phoneLabel    *gtk.Label
	deviceLabel   *gtk.Label
	chatsLabel    *gtk.Label
	unreadLabel   *gtk.Label
	lastUpdateLbl *gtk.Label
}

// NewDashboard constructs the dashboard view.
func NewDashboard() *Dashboard {
	d := &Dashboard{}

	// Outer: Adw.StatusPage (libadwaita empty-state-style layout)
	d.root = adw.NewStatusPage()
	d.root.SetTitle("WA Bot")
	d.root.SetDescription("Loading status…")
	d.root.SetIconName("phone-symbolic")

	// Inner content: a centered clamp with a vertical box of stat rows.
	d.clamp = adw.NewClamp()
	d.clamp.SetMaximumSize(560)
	d.clamp.SetTighteningThreshold(800)

	d.inner = gtk.NewBox(gtk.OrientationVertical, 12)
	d.inner.SetMarginTop(24)
	d.inner.SetMarginBottom(24)
	d.inner.SetMarginStart(24)
	d.inner.SetMarginEnd(24)

	// Title row
	d.titleLabel = gtk.NewLabel("")
	d.titleLabel.AddCSSClass("title-1")
	d.titleLabel.SetHAlign(gtk.AlignStart)
	d.inner.Append(d.titleLabel)

	// Status
	d.statusLabel = gtk.NewLabel("")
	d.statusLabel.AddCSSClass("dim-label")
	d.statusLabel.SetHAlign(gtk.AlignStart)
	d.inner.Append(d.statusLabel)

	// Spacer
	spacer := gtk.NewBox(gtk.OrientationVertical, 0)
	spacer.SetSizeRequest(-1, 12)
	d.inner.Append(spacer)

	// Session info group
	grp := gtk.NewBox(gtk.OrientationVertical, 6)
	d.phoneLabel = statRow(grp, "Phone:")
	d.deviceLabel = statRow(grp, "Device:")
	d.inner.Append(grp)

	// Spacer
	spacer2 := gtk.NewBox(gtk.OrientationVertical, 0)
	spacer2.SetSizeRequest(-1, 12)
	d.inner.Append(spacer2)

	// Quick stats group
	statsGrp := gtk.NewBox(gtk.OrientationVertical, 6)
	d.chatsLabel = statRow(statsGrp, "Chats:")
	d.unreadLabel = statRow(statsGrp, "Unread:")
	d.inner.Append(statsGrp)

	// Last update timestamp
	d.lastUpdateLbl = gtk.NewLabel("Last update: never")
	d.lastUpdateLbl.AddCSSClass("dim-label")
	d.lastUpdateLbl.AddCSSClass("caption")
	d.lastUpdateLbl.SetHAlign(gtk.AlignEnd)
	d.lastUpdateLbl.SetMarginTop(24)
	d.inner.Append(d.lastUpdateLbl)

	d.clamp.SetChild(d.inner)
	d.root.SetChild(d.clamp)

	return d
}

// statRow is a small helper: appends a "Label: value" row to a Box and
// returns the value label so callers can update it.
func statRow(parent *gtk.Box, caption string) *gtk.Label {
	row := gtk.NewBox(gtk.OrientationHorizontal, 8)
	cap := gtk.NewLabel(caption)
	cap.AddCSSClass("dim-label")
	cap.SetSizeRequest(120, -1)
	cap.SetHAlign(gtk.AlignStart)
	val := gtk.NewLabel("—")
	val.SetHAlign(gtk.AlignStart)
	val.SetHExpand(true)
	row.Append(cap)
	row.Append(val)
	parent.Append(row)
	return val
}

// Widget returns the root widget for embedding in a ViewStack.
func (d *Dashboard) Widget() gtk.Widgetter { return d.root }

// SetClient wires the API client and starts the 10s auto-refresh loop.
// Subsequent calls replace the client (idempotent).
func (d *Dashboard) SetClient(c *api.Client) {
	d.client = c
	d.refresh()
	// Auto-refresh every 10s while the dashboard is visible. We use a goroutine
	// that schedules an IdleAdd on the main thread, so widget updates are safe.
	go d.refreshLoop()
}

func (d *Dashboard) refreshLoop() {
	ticker := time.NewTicker(10 * time.Second)
	defer ticker.Stop()
	for range ticker.C {
		if d.client == nil {
			return
		}
		d.refresh()
	}
}

func (d *Dashboard) refresh() {
	if d.client == nil {
		return
	}
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
		defer cancel()
		s, err := d.client.GetStatus(ctx)
		glib.IdleAdd(func() bool {
			if err != nil {
				d.statusLabel.SetText("Disconnected")
				d.statusLabel.RemoveCSSClass("dim-label")
				d.statusLabel.AddCSSClass("error")
				d.titleLabel.SetText("Cannot reach backend")
				d.lastUpdateLbl.SetText(fmt.Sprintf("Last update failed: %s", err.Error()))
				return false
			}
			d.applyStatus(s)
			return false
		})
	}()
}

func (d *Dashboard) applyStatus(s *api.Status) {
	d.statusLabel.RemoveCSSClass("error")
	d.statusLabel.AddCSSClass("dim-label")

	now := time.Now().Format("15:04:05")
	d.lastUpdateLbl.SetText("Last update: " + now)

	if s.LoggedIn {
		d.titleLabel.SetText("Connected")
		d.statusLabel.SetText("WhatsApp session active")
	} else if s.Connected {
		d.titleLabel.SetText("Connecting")
		d.statusLabel.SetText("Waiting for QR scan…")
	} else {
		d.titleLabel.SetText("Not paired")
		d.statusLabel.SetText("Open WhatsApp on your phone to pair this device")
	}

	d.phoneLabel.SetText(orDash(s.Phone))
	d.deviceLabel.SetText(orDash(s.Device))
	d.chatsLabel.SetText(fmt.Sprintf("%d", s.ChatCount))
	d.unreadLabel.SetText(fmt.Sprintf("%d", s.UnreadCount))
	log.Printf("dashboard refreshed: logged_in=%v chats=%d unread=%d", s.LoggedIn, s.ChatCount, s.UnreadCount)
}

func orDash(s string) string {
	if s == "" {
		return "—"
	}
	return s
}

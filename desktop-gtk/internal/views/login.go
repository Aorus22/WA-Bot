package views

import (
	"github.com/diamondburned/gotk4/pkg/gtk/v4"

	"wa-bot-desktop/internal/media"
)

// Login is the full-window pairing screen shown while the backend has no
// WhatsApp session. It renders the live QR (from ws "qr_code" events or the
// GET /api/qr-code poll) plus a status area and logout button.
type Login struct {
	root      *gtk.Box
	statusDot *gtk.Image
	status    *gtk.Label
	spinner   *gtk.Spinner
	qr        *gtk.Picture
	logoutBtn *gtk.Button
	retryBtn  *gtk.Button

	waitPage *gtk.Spinner // stack page while waiting for a QR
	qrFrame  *gtk.Frame   // stack page showing the QR picture
	pages    *gtk.Stack
}

// NewLogin constructs the login view.
func NewLogin() *Login {
	l := &Login{}

	l.root = gtk.NewBox(gtk.OrientationVertical, 18)
	l.root.SetHExpand(true)
	l.root.SetVExpand(true)
	l.root.SetHAlign(gtk.AlignCenter)
	l.root.SetVAlign(gtk.AlignCenter)
	l.root.SetMarginTop(32)
	l.root.SetMarginBottom(32)

	title := gtk.NewLabel("Hubungkan WhatsApp")
	title.AddCSSClass("title-1")

	sub := gtk.NewLabel("Buka WhatsApp di ponsel → Perangkat tertaut → Tautkan perangkat,\nlalu pindai kode di bawah ini.")
	sub.AddCSSClass("dim-label")
	sub.SetJustify(gtk.JustifyCenter)

	// QR display: fixed square; content swapped at runtime between the
	// spinner (no QR yet) and the QR frame.
	l.qr = gtk.NewPictureForPaintable(nil)
	l.qr.SetSizeRequest(300, 300)
	l.qrFrame = gtk.NewFrame("")
	l.qrFrame.SetChild(l.qr)

	l.spinner = gtk.NewSpinner()
	l.spinner.SetSizeRequest(300, 300)
	l.spinner.Start()
	l.waitPage = l.spinner

	l.pages = gtk.NewStack()
	l.pages.AddTitled(l.waitPage, "wait", "waiting")
	l.pages.AddTitled(l.qrFrame, "qr", "qr")
	l.pages.SetVisibleChild(l.waitPage)

	l.status = gtk.NewLabel("Menunggu kode QR dari backend…")
	l.status.AddCSSClass("dim-label")

	l.statusDot = gtk.NewImageFromIconName("network-offline-symbolic")
	l.statusDot.AddCSSClass("dim-label")
	statusRow := gtk.NewBox(gtk.OrientationHorizontal, 8)
	statusRow.SetHAlign(gtk.AlignCenter)
	statusRow.Append(l.statusDot)
	statusRow.Append(l.status)

	l.logoutBtn = gtk.NewButtonWithLabel("Keluar dari sesi")
	l.logoutBtn.AddCSSClass("destructive-action")
	l.logoutBtn.SetVisible(false)

	l.retryBtn = gtk.NewButtonFromIconName("view-refresh-symbolic")
	l.retryBtn.SetLabel("Muat ulang QR")
	l.retryBtn.SetTooltipText("Ambil kode QR terbaru dari backend")
	l.retryBtn.SetVisible(false)

	l.root.Append(title)
	l.root.Append(sub)
	l.root.Append(l.pages)
	l.root.Append(statusRow)

	btnRow := gtk.NewBox(gtk.OrientationHorizontal, 10)
	btnRow.SetHAlign(gtk.AlignCenter)
	btnRow.Append(l.retryBtn)
	btnRow.Append(l.logoutBtn)
	l.root.Append(btnRow)

	return l
}

// Widget returns the root widget.
func (l *Login) Widget() gtk.Widgetter { return l.root }

// SetLogoutCallback wires the logout button.
func (l *Login) SetLogoutCallback(cb func()) {
	l.logoutBtn.ConnectClicked(func() { cb() })
}

// SetRetryCallback wires the refresh-QR button.
func (l *Login) SetRetryCallback(cb func()) {
	l.retryBtn.ConnectClicked(func() { cb() })
}

// SetWaiting shows the spinner state (no QR available yet).
func (l *Login) SetWaiting(statusText string) {
	l.spinner.Start()
	l.pages.SetVisibleChild(l.waitPage)
	if statusText != "" {
		l.status.SetText(statusText)
	}
	l.statusDot.SetFromIconName("network-offline-symbolic")
	l.logoutBtn.SetVisible(false)
	l.retryBtn.SetVisible(true)
}

// SetQR displays an encoded QR PNG and flips to the QR page.
func (l *Login) SetQR(png []byte, statusText string) {
	tex, err := media.TextureFromBytes(png)
	if err != nil {
		l.status.SetText("Gagal merender QR: " + err.Error())
		return
	}
	l.spinner.Stop()
	l.qr.SetPaintable(tex)
	l.pages.SetVisibleChild(l.qrFrame)
	if statusText != "" {
		l.status.SetText(statusText)
	}
	l.statusDot.SetFromIconName("network-idle-symbolic")
	l.retryBtn.SetVisible(true)
}

// SetConnected flips the view into "already logged in" mode (shown briefly
// before the overlay hides).
func (l *Login) SetConnected(phone string) {
	text := "Terhubung"
	if phone != "" {
		text += " (" + phone + ")"
	}
	l.status.SetText(text)
	l.statusDot.SetFromIconName("network-wireless-signal-excellent-symbolic")
	l.logoutBtn.SetVisible(true)
	l.retryBtn.SetVisible(false)
}

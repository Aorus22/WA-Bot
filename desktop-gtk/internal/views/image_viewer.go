package views

import (
	"fmt"
	"math"

	"github.com/diamondburned/gotk4/pkg/gdk/v4"
	"github.com/diamondburned/gotk4/pkg/gtk/v4"
)

const (
	viewerMinZoom  = 0.25
	viewerMaxZoom  = 8.0
	viewerZoomStep = 1.25
)

// imageViewer holds the state of one open image viewer window.
type imageViewer struct {
	win      *gtk.Window
	pic      *gtk.Picture
	scroller *gtk.ScrolledWindow
	zoomLbl  *gtk.Label

	tex  *gdk.Texture
	natW int
	natH int

	fit  float64 // base scale that fits the initial viewport
	zoom float64 // user zoom multiplier on top of fit
}

// ShowImageViewer opens a modal in-app image viewer (zoom with mouse wheel
// or the header buttons, pan with the scrollbars). Mirrors the web UI's
// image modal instead of shelling out to an external app.
func ShowImageViewer(parent *gtk.Window, tex *gdk.Texture) {
	if tex == nil || parent == nil {
		return
	}

	v := &imageViewer{tex: tex, zoom: 1.0}
	v.natW = tex.IntrinsicWidth()
	v.natH = tex.IntrinsicHeight()
	if v.natW <= 0 {
		v.natW = 512
	}
	if v.natH <= 0 {
		v.natH = 512
	}
	// Fit within the default 900x700 window minus chrome.
	v.fit = math.Min(1.0, math.Min(860/float64(v.natW), 620/float64(v.natH)))

	v.win = gtk.NewWindow()
	v.win.SetTitle("Gambar")
	v.win.SetTransientFor(parent)
	v.win.SetModal(true)
	v.win.SetDefaultSize(900, 700)

	hb := gtk.NewHeaderBar()
	hb.SetTitleWidget(gtk.NewLabel("Gambar"))
	v.zoomLbl = gtk.NewLabel("100%")
	v.zoomLbl.AddCSSClass("dim-label")
	v.zoomLbl.SetMarginEnd(10)
	zout := gtk.NewButtonFromIconName("zoom-out-symbolic")
	zout.SetTooltipText("Perkecil (scroll bawah)")
	zin := gtk.NewButtonFromIconName("zoom-in-symbolic")
	zin.SetTooltipText("Perbesar (scroll atas)")
	zfit := gtk.NewButtonFromIconName("view-restore-symbolic")
	zfit.SetTooltipText("Reset ke ukuran pas")
	hb.PackEnd(zin)
	hb.PackEnd(zout)
	hb.PackEnd(zfit)
	hb.PackEnd(v.zoomLbl)
	zin.ConnectClicked(func() { v.setZoom(v.zoom * viewerZoomStep) })
	zout.ConnectClicked(func() { v.setZoom(v.zoom / viewerZoomStep) })
	zfit.ConnectClicked(func() { v.setZoom(1.0) })

	v.pic = gtk.NewPictureForPaintable(tex)
	v.pic.SetKeepAspectRatio(true)
	// CanShrink MUST be true: with it off, the picture's minimum size is its
	// intrinsic resolution and SetSizeRequest can never go below that —
	// zoom-out silently stops working.
	v.pic.SetCanShrink(true)
	v.pic.SetHAlign(gtk.AlignCenter)
	v.pic.SetVAlign(gtk.AlignCenter)

	v.scroller = gtk.NewScrolledWindow()
	v.scroller.SetPolicy(gtk.PolicyAutomatic, gtk.PolicyAutomatic)
	v.scroller.SetChild(v.pic)
	v.scroller.SetHExpand(true)
	v.scroller.SetVExpand(true)

	// Mouse-wheel zoom over the image.
	ctl := gtk.NewEventControllerScroll(gtk.EventControllerScrollBothAxes | gtk.EventControllerScrollDiscrete)
	v.scroller.AddController(ctl)
	ctl.ConnectScroll(func(dx, dy float64) bool {
		switch {
		case dy < 0:
			v.setZoom(v.zoom * viewerZoomStep)
		case dy > 0:
			v.setZoom(v.zoom / viewerZoomStep)
		}
		return true // consumed; don't scroll the viewport too
	})

	v.win.SetTitlebar(hb)
	v.win.SetChild(v.scroller)
	v.apply()
	v.win.Present()
}

// setZoom clamps and applies a new zoom factor.
func (v *imageViewer) setZoom(z float64) {
	z = math.Min(viewerMaxZoom, math.Max(viewerMinZoom, z))
	if z == v.zoom {
		return
	}
	v.zoom = z
	v.apply()
}

// apply resizes the GtkPicture allocation to fit*zoom so the ScrolledWindow
// gains scrollbars when the image outgrows the viewport.
func (v *imageViewer) apply() {
	w := int(float64(v.natW) * v.fit * v.zoom)
	h := int(float64(v.natH) * v.fit * v.zoom)
	if w < 1 {
		w = 1
	}
	if h < 1 {
		h = 1
	}
	v.pic.SetSizeRequest(w, h)
	v.zoomLbl.SetText(fmt.Sprintf("%d%%", int(v.zoom*100+0.5)))
}

package views

import (
	"context"
	"strings"
	"time"

	"github.com/diamondburned/gotk4/pkg/gdk/v4"
	"github.com/diamondburned/gotk4/pkg/gio/v2"
	"github.com/diamondburned/gotk4/pkg/glib/v2"
	"github.com/diamondburned/gotk4/pkg/gtk/v4"
)

const (
	imageBoxW      = 280
	imageBoxH      = 200
	stickerBoxSize = 150

	typingRepeat  = 3 * time.Second
	typingTimeout = 6
)

// buildComposer constructs the attachment menu + input + send row.
func (cv *Conversation) buildComposer() gtk.Widgetter {
	frame := gtk.NewFrame("")
	frame.AddCSSClass("composer-frame")
	frame.SetMarginStart(10)
	frame.SetMarginEnd(10)
	frame.SetMarginBottom(10)

	row := gtk.NewBox(gtk.OrientationHorizontal, 6)
	row.SetMarginTop(5)
	row.SetMarginBottom(5)
	row.SetMarginStart(5)
	row.SetMarginEnd(5)

	cv.attachBtn = gtk.NewMenuButton()
	cv.attachBtn.SetIconName("list-add-symbolic")
	cv.attachBtn.SetTooltipText("Lampirkan berkas")
	cv.attachBtn.SetAlwaysShowArrow(false)
	cv.attachBtn.SetPopover(cv.buildAttachPopover())
	row.Append(cv.attachBtn)

	cv.composerTextView = gtk.NewTextView()
	cv.composerTextView.SetWrapMode(gtk.WrapWordChar)
	cv.composerTextView.SetHExpand(true)
	inputScroll := gtk.NewScrolledWindow()
	inputScroll.SetPolicy(gtk.PolicyNever, gtk.PolicyNever)
	inputScroll.SetChild(cv.composerTextView)
	inputScroll.SetSizeRequest(-1, 40)
	inputScroll.SetHExpand(true)
	row.Append(inputScroll)

	buf := cv.composerTextView.Buffer()
	buf.ConnectChanged(func() {
		cv.onComposerChanged()
	})

	keyCtl := gtk.NewEventControllerKey()
	cv.composerTextView.AddController(keyCtl)
	keyCtl.ConnectKeyPressed(func(keyval, keycode uint, state gdk.ModifierType) bool {
		if keyval == gdk.KEY_Return || keyval == gdk.KEY_KP_Enter {
			if !state.Has(gdk.ShiftMask) {
				cv.onSend()
				return true // swallow Enter; Shift+Enter inserts newline natively
			}
		}
		return false
	})

	cv.sendBtn = gtk.NewButtonFromIconName("paper-plane-symbolic")
	cv.sendBtn.AddCSSClass("suggested-action")
	cv.sendBtn.SetTooltipText("Kirim (Enter)")
	cv.sendBtn.ConnectClicked(func() { cv.onSend() })
	row.Append(cv.sendBtn)

	frame.SetChild(row)
	return frame
}

// buildAttachPopover builds the image/video/document picker menu.
func (cv *Conversation) buildAttachPopover() gtk.Widgetter {
	pop := gtk.NewPopover()

	box := gtk.NewBox(gtk.OrientationVertical, 0)
	box.SetMarginTop(4)
	box.SetMarginBottom(4)

	addItem := func(icon, label, category string) {
		btn := gtk.NewButton()
		content := gtk.NewBox(gtk.OrientationHorizontal, 8)
		content.SetMarginTop(4)
		content.SetMarginBottom(4)
		content.SetMarginStart(8)
		content.SetMarginEnd(8)
		img := gtk.NewImageFromIconName(icon)
		content.Append(img)
		lbl := gtk.NewLabel(label)
		content.Append(lbl)
		btn.SetChild(content)
		btn.AddCSSClass("flat")
		btn.ConnectClicked(func() {
			pop.Popdown()
			cv.pickAndSend(category)
		})
		box.Append(btn)
	}

	addItem("image-x-generic-symbolic", "Foto/Gambar", "image")
	addItem("video-x-generic-symbolic", "Video", "video")
	addItem("text-x-generic-symbolic", "Dokumen", "document")

	pop.SetChild(box)
	return pop
}

// pickAndSend opens the platform file dialog and uploads the chosen file.
func (cv *Conversation) pickAndSend(category string) {
	if !cv.hasChat {
		return
	}
	fd := gtk.NewFileDialog()
	switch category {
	case "image":
		ff := gtk.NewFileFilter()
		ff.SetName("Gambar")
		ff.AddMIMEType("image/*")
		fd.SetDefaultFilter(ff)
	case "video":
		ff := gtk.NewFileFilter()
		ff.SetName("Video")
		ff.AddMIMEType("video/*")
		fd.SetDefaultFilter(ff)
	}

	fd.Open(context.Background(), MainWindow, func(res gio.AsyncResulter) {
		file, err := fd.OpenFinish(res)
		if err != nil || file == nil {
			return // cancelled or failed
		}
		path := file.Path()
		if path == "" {
			return
		}
		go cv.uploadMedia(path, category)
	})
}

// uploadMedia POSTs the file as multipart form data.
func (cv *Conversation) uploadMedia(path, category string) {
	id := cv.current.ID
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Minute)
	defer cancel()
	if err := cv.client.SendMedia(ctx, id, path, category, ""); err != nil {
		glib.IdleAdd(func() bool {
			if cv.toast != nil {
				cv.toast("Gagal mengirim media: " + err.Error())
			}
			return false
		})
	}
}

// bufferText reads the composer contents.
func (cv *Conversation) bufferText() string {
	buf := cv.composerTextView.Buffer()
	if s, ok := buf.ObjectProperty("text").(string); ok {
		return s
	}
	return ""
}

// onComposerChanged throttles outgoing typing presence.
func (cv *Conversation) onComposerChanged() {
	id := cv.chatID()
	if id == "" {
		return
	}
	now := time.Now()
	if !cv.typingActive || now.Sub(cv.lastTyping) >= typingRepeat {
		cv.typingActive = true
		cv.lastTyping = now
		go func() {
			ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
			defer cancel()
			_ = cv.client.SendTyping(ctx, id, true)
		}()
	}
	if cv.typingStop != 0 {
		glib.SourceRemove(cv.typingStop)
	}
	cv.typingStop = glib.TimeoutSecondsAdd(typingTimeout, func() bool {
		cv.typingActive = false
		cv.typingStop = 0
		chat := cv.current.ID
		go func() {
			ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
			defer cancel()
			_ = cv.client.SendTyping(ctx, chat, false)
		}()
		return false
	})
}
// onSend performs an optimistic text send.
func (cv *Conversation) onSend() {
	if !cv.hasChat {
		return
	}
	text := strings.TrimSpace(cv.bufferText())
	if text == "" {
		return
	}
	id := cv.current.ID

	temp := cv.store.AddOutgoingTemp(id, text, "text")
	cv.composerTextView.Buffer().SetText("")
	cv.stopTypingIndicator(id)
	cv.scrollToBottom()

	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
		defer cancel()
		if err := cv.client.SendText(ctx, id, text); err != nil {
			glib.IdleAdd(func() bool {
				cv.store.PatchTempStatus(id, temp.ID, "failed")
				if cv.toast != nil {
					cv.toast("Gagal mengirim pesan: " + err.Error())
				}
				return false
			})
		}
	}()
}

// stopTypingIndicator immediately ends typing presence after a send.
func (cv *Conversation) stopTypingIndicator(chatID string) {
	if cv.typingStop != 0 {
		glib.SourceRemove(cv.typingStop)
		cv.typingStop = 0
	}
	cv.typingActive = false
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		_ = cv.client.SendTyping(ctx, chatID, false)
	}()
}

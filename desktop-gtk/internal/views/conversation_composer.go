package views

import (
	"context"
	"strings"
	"time"

	"github.com/diamondburned/gotk4/pkg/gdk/v4"
	"github.com/diamondburned/gotk4/pkg/gio/v2"
	"github.com/diamondburned/gotk4/pkg/glib/v2"
	"github.com/diamondburned/gotk4/pkg/gtk/v4"
	pango "github.com/diamondburned/gotk4/pkg/pango"

	"wa-bot-desktop/internal/api"
	"wa-bot-desktop/internal/store"
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

	cv.replyBar, cv.replyWho, cv.replyTxt = cv.buildContextBar("mail-reply-sender-symbolic", cv.cancelReply)
	cv.editBar, _, cv.editTxt = cv.buildContextBar("document-edit-symbolic", cv.cancelEdit)

	wrap := gtk.NewBox(gtk.OrientationVertical, 0)
	wrap.Append(cv.replyBar)
	wrap.Append(cv.editBar)
	wrap.Append(frame)
	return wrap
}

// buildContextBar constructs the hidden quote bar shown above the input frame
// while composing a reply or an edit. Returns the bar plus its who/text labels.
func (cv *Conversation) buildContextBar(icon string, closeFn func()) (*gtk.Box, *gtk.Label, *gtk.Label) {
	bar := gtk.NewBox(gtk.OrientationHorizontal, 8)
	bar.AddCSSClass("context-bar")
	bar.SetMarginStart(10)
	bar.SetMarginEnd(10)
	bar.SetMarginBottom(4)
	bar.SetVisible(false)

	img := gtk.NewImageFromIconName(icon)
	img.SetPixelSize(16)
	img.SetVAlign(gtk.AlignStart)
	bar.Append(img)

	col := gtk.NewBox(gtk.OrientationVertical, 0)
	col.SetHExpand(true)
	who := gtk.NewLabel("")
	who.AddCSSClass("context-who")
	who.SetXAlign(0)
	col.Append(who)
	txt := gtk.NewLabel("")
	txt.AddCSSClass("context-text")
	txt.SetEllipsize(pango.EllipsizeEnd)
	txt.SetXAlign(0)
	col.Append(txt)
	bar.Append(col)

	close := gtk.NewButtonFromIconName("window-close-symbolic")
	close.AddCSSClass("flat")
	close.SetVAlign(gtk.AlignStart)
	close.SetTooltipText("Batal")
	close.ConnectClicked(closeFn)
	bar.Append(close)

	return bar, who, txt
}

// StartReply primes the composer to send a quoted reply to m.
func (cv *Conversation) StartReply(m api.Message) {
	if !cv.hasChat {
		return
	}
	cv.cancelEdit()
	cv.replyTo = &m
	cv.replyWho.SetText("Membalas " + replyTargetName(cv.current, m))
	cv.replyTxt.SetText(oneLine(msgPreview(m)))
	cv.replyBar.SetVisible(true)
	cv.composerTextView.GrabFocus()
}

// StartEdit loads own message m into the composer for editing.
func (cv *Conversation) StartEdit(m api.Message) {
	if !cv.hasChat {
		return
	}
	cv.cancelReply()
	cv.editing = &m
	cv.editTxt.SetText(oneLine(msgPreview(m)))
	cv.editBar.SetVisible(true)
	cv.composerTextView.Buffer().SetText(m.Content)
	cv.composerTextView.GrabFocus()
}

func (cv *Conversation) cancelReply() {
	cv.replyTo = nil
	cv.replyBar.SetVisible(false)
}

// cancelEdit exits edit mode and restores an empty composer. No-op when no
// edit is active so switching to reply mode never wipes a draft.
func (cv *Conversation) cancelEdit() {
	if cv.editing == nil {
		return
	}
	cv.editing = nil
	cv.editBar.SetVisible(false)
	cv.composerTextView.Buffer().SetText("")
}

// resetReplyEdit drops reply/edit state without touching draft text (chat switch).
func (cv *Conversation) resetReplyEdit() {
	cv.replyTo = nil
	cv.replyBar.SetVisible(false)
	cv.editing = nil
	cv.editBar.SetVisible(false)
}

// replyTargetName picks the display name shown in the reply quote bar.
func replyTargetName(c api.Chat, m api.Message) string {
	if m.From == store.OutgoingFrom {
		return "Anda"
	}
	if n := m.SenderName; n != "" {
		return n
	}
	if n := displayName(c); n != "" {
		return n
	}
	return shortJID(m.From)
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
// onSend sends composer text: an edit of an own message when edit mode is
// active, otherwise a plain text or quoted-reply message (optimistic).
func (cv *Conversation) onSend() {
	if !cv.hasChat {
		return
	}
	text := strings.TrimSpace(cv.bufferText())
	if text == "" {
		return
	}
	id := cv.current.ID

	if cv.editing != nil {
		msg := *cv.editing
		cv.cancelEdit() // also clears the composer
		cv.store.EditMessage(id, msg.ID, text)
		go func() {
			ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
			defer cancel()
			if err := cv.client.EditMessage(ctx, id, msg.ID, text); err != nil {
				glib.IdleAdd(func() bool {
					cv.store.EditMessage(id, msg.ID, msg.Content) // revert
					if cv.toast != nil {
						cv.toast("Gagal mengedit pesan: " + err.Error())
					}
					return false
				})
			}
		}()
		return
	}

	ref := cv.replyTo
	if ref != nil {
		cv.cancelReply()
	}

	temp := cv.store.AddOutgoingTemp(id, text, "text")
	cv.composerTextView.Buffer().SetText("")
	cv.stopTypingIndicator(id)
	cv.scrollToBottom()

	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
		defer cancel()
		var err error
		if ref != nil {
			err = cv.client.ReplyMessage(ctx, id, ref.ID, text)
		} else {
			err = cv.client.SendText(ctx, id, text)
		}
		if err != nil {
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

// startCall places an outgoing audio/video call via the backend. The GTK app
// has no local call media; the peer is rung (bot/TTS calls answer server-side).
func (cv *Conversation) startCall(callType string) {
	if !cv.hasChat || cv.current.IsGroup {
		return
	}
	target := cv.current.ID
	name := displayName(cv.current)
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
		defer cancel()
		if err := cv.client.CreateCall(ctx, target, callType); err != nil {
			glib.IdleAdd(func() bool {
				if cv.toast == nil {
					return false
				}
				switch {
				case strings.Contains(err.Error(), "call_already_active"):
					cv.toast("Masih ada panggilan aktif")
				case strings.Contains(err.Error(), "whatsapp_not_connected"):
					cv.toast("WhatsApp belum terhubung")
				default:
					cv.toast("Gagal memanggil " + name + ": " + err.Error())
				}
				return false
			})
			return
		}
		glib.IdleAdd(func() bool {
			if cv.toast != nil {
				if callType == "video" {
					cv.toast("Memanggil video " + name + "…")
				} else {
					cv.toast("Memanggil " + name + "…")
				}
			}
			return false
		})
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

package views

import (
	"context"
	"log"
	"strings"
	"time"

	"github.com/diamondburned/gotk4-adwaita/pkg/adw"
	"github.com/diamondburned/gotk4/pkg/gdk/v4"
	"github.com/diamondburned/gotk4/pkg/gio/v2"
	"github.com/diamondburned/gotk4/pkg/glib/v2"
	"github.com/diamondburned/gotk4/pkg/gtk/v4"
	pango "github.com/diamondburned/gotk4/pkg/pango"

	"wa-bot-desktop/internal/api"
	"wa-bot-desktop/internal/media"
	"wa-bot-desktop/internal/openx"
	"wa-bot-desktop/internal/store"
)

// buildRow constructs one message row. The returned label (non-nil for
// outgoing messages) is the status glyph kept in cv.ticks for live updates.
func (cv *Conversation) buildRow(m api.Message) (*gtk.ListBoxRow, *gtk.Label) {
	outgoing := m.From == store.OutgoingFrom

	row := gtk.NewListBoxRow()
	row.SetSelectable(false)
	row.SetActivatable(false)
	row.SetFocusable(false)

	align := gtk.NewBox(gtk.OrientationVertical, 0)
	align.SetHExpand(true)

	bubble := gtk.NewBox(gtk.OrientationVertical, 3)
	bubble.AddCSSClass("bubble")
	if outgoing {
		bubble.AddCSSClass("out")
		bubble.SetHAlign(gtk.AlignEnd)
		align.SetMarginStart(90)
	} else {
		bubble.AddCSSClass("in")
		bubble.SetHAlign(gtk.AlignStart)
		align.SetMarginEnd(90)
	}
	align.SetMarginTop(2)
	align.SetMarginBottom(2)
	align.Append(bubble)

	// Sender name above incoming bubbles in groups.
	if !outgoing && cv.current.IsGroup {
		name := m.SenderName
		if name == "" {
			name = shortJID(m.From)
		}
		if name != "" {
			sender := gtk.NewLabel(name)
			sender.AddCSSClass("sender-name")
			sender.SetXAlign(0)
			bubble.Append(sender)
		}
	}

	// Reply quote.
	if m.ReplyToID != "" {
		if ref, found := cv.store.Message(cv.current.ID, m.ReplyToID); found {
			q := buildQuote(ref)
			addClick(q, func() { cv.scrollToMessage(m.ReplyToID) })
			bubble.Append(q)
		}
	}

	if body := cv.bodyWidget(m); body != nil {
		bubble.Append(body)
	}

	// Meta line: timestamp (+ delivery ticks on outgoing).
	meta := gtk.NewBox(gtk.OrientationHorizontal, 5)
	meta.SetHAlign(gtk.AlignEnd)
	ts := gtk.NewLabel(timeLabel(m.Timestamp))
	ts.AddCSSClass("caption")
	ts.AddCSSClass("msg-time")
	meta.Append(ts)

	var tick *gtk.Label
	if outgoing {
		tick = gtk.NewLabel(tickGlyph(m.Status))
		tick.AddCSSClass("caption")
		tick.AddCSSClass(classForTick(m.Status))
		meta.Append(tick)
	}
	bubble.Append(meta)

	cv.addBubbleMenu(align, m)

	row.SetChild(align)
	return row, tick
}

// addBubbleMenu attaches the right-click handler that opens the shared
// context menu for this message.
func (cv *Conversation) addBubbleMenu(w gtk.Widgetter, m api.Message) {
	gc := gtk.NewGestureClick()
	gc.SetButton(gdk.BUTTON_SECONDARY)
	gtk.BaseWidget(w).AddController(gc)
	gc.ConnectPressed(func(nPress int, x, y float64) {
		cv.openBubbleMenu(w, x, y, m)
	})
}

// openBubbleMenu shows the context popover anchored at the clicked point.
// The popover is parented to cv.content — outside the scrolled message list —
// so mapping and dismissing it can never perturb list layout or trigger the
// focus-return scroll-to-first-row jump.
func (cv *Conversation) openBubbleMenu(host gtk.Widgetter, x, y float64, m api.Message) {
	if !cv.hasChat {
		return
	}

	menu := gio.NewMenu()
	menu.Append("Balas", "msg.reply")
	if strings.TrimSpace(m.Content) != "" {
		menu.Append("Salin Teks", "msg.copy")
	}
	outgoing := m.From == store.OutgoingFrom
	if outgoing && isEditableText(m) {
		menu.Append("Edit", "msg.edit")
	}
	if outgoing {
		menu.Append("Hapus untuk Semua Orang", "msg.delete")
	}

	group := gio.NewSimpleActionGroup()
	add := func(name string, fn func()) {
		act := gio.NewSimpleAction(name, nil)
		act.ConnectActivate(func(*glib.Variant) { fn() })
		group.Insert(act)
	}
	add("reply", func() { cv.StartReply(m) })
	add("copy", func() { gtk.BaseWidget(host).Clipboard().SetText(m.Content) })
	add("edit", func() { cv.StartEdit(m) })
	add("delete", func() { cv.confirmDelete(m) })
	gtk.BaseWidget(cv.content).InsertActionGroup("msg", group)

	if cv.ctxPopover == nil {
		cv.ctxPopover = gtk.NewPopoverMenuFromModel(menu)
		gtk.BaseWidget(cv.ctxPopover).SetParent(cv.content)
	} else {
		cv.ctxPopover.SetMenuModel(menu)
	}

	gx, gy, ok := gtk.BaseWidget(host).TranslateCoordinates(cv.content, x, y)
	if !ok {
		return
	}
	rect := gdk.NewRectangle(int(gx)-2, int(gy)-2, 4, 4)
	cv.ctxPopover.SetPointingTo(&rect)
	cv.ctxPopover.Popup()
}

// confirmDelete asks for confirmation, then revokes the message for everyone.
func (cv *Conversation) confirmDelete(m api.Message) {
	dialog := adw.NewMessageDialog(MainWindow, "Hapus pesan?", "Pesan ini akan dihapus untuk semua orang.")
	dialog.AddResponse("cancel", "Batal")
	dialog.AddResponse("delete", "Hapus")
	dialog.SetCloseResponse("cancel")
	dialog.SetResponseAppearance("delete", adw.ResponseDestructive)
	dialog.ConnectResponse(func(resp string) {
		if resp != "delete" {
			return
		}
		chatID := cv.chatID()
		go func() {
			ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
			defer cancel()
			if err := cv.client.DeleteMessage(ctx, chatID, m.ID); err != nil {
				log.Printf("conversation: delete message: %v", err)
				glib.IdleAdd(func() bool {
					if cv.toast != nil {
						cv.toast("Gagal menghapus pesan: " + err.Error())
					}
					return false
				})
				return
			}
			cv.store.DeleteMessage(chatID, m.ID) // WS echo is idempotent
		}()
	})
	dialog.Show()
}

// isEditableText reports whether m can be edited (own plain text message).
func isEditableText(m api.Message) bool {
	return m.Type == "" || m.Type == "text"
}

// buildQuote renders the quoted reply preview inside a bubble.
func buildQuote(ref api.Message) gtk.Widgetter {
	q := gtk.NewBox(gtk.OrientationVertical, 1)
	q.AddCSSClass("quote")

	who := firstNonEmptyStr(ref.SenderName, shortJID(ref.From))
	if who == store.OutgoingFrom {
		who = "Anda"
	}
	whoLbl := gtk.NewLabel(who)
	whoLbl.AddCSSClass("quote-who")
	whoLbl.SetXAlign(0)
	q.Append(whoLbl)

	snippet := gtk.NewLabel(oneLine(msgPreview(ref)))
	snippet.AddCSSClass("quote-text")
	snippet.SetEllipsize(pango.EllipsizeEnd)
	snippet.SetXAlign(0)
	snippet.SetMaxWidthChars(44)
	q.Append(snippet)
	return q
}

// bodyWidget builds the per-type content widget; nil means empty text.
func (cv *Conversation) bodyWidget(m api.Message) gtk.Widgetter {
	switch m.Type {
	case "image":
		return cv.pictureBody(m, imageBoxW, imageBoxH)
	case "sticker":
		return cv.pictureBody(m, stickerBoxSize, stickerBoxSize)
	case "video":
		return cv.playableBody(m, "video-x-generic", "Video")
	case "audio", "ptt", "voice":
		icon := "audio-x-generic"
		label := "Audio"
		if m.Type != "audio" {
			icon = "audio-input-microphone-symbolic"
			label = "Pesan suara"
		}
		return cv.playableBody(m, icon, label)
	default:
		text := strings.TrimSpace(m.Content)
		if text == "" {
			text = "[" + m.Type + "]"
		}
		lbl := gtk.NewLabel(text)
		lbl.SetWrap(true)
		lbl.SetSelectable(true)
		lbl.SetXAlign(0)
		return lbl
	}
}

// pictureBody renders an image/sticker bubble with async loading; clicking
// opens the in-app zoomable viewer.
func (cv *Conversation) pictureBody(m api.Message, w, h int) gtk.Widgetter {
	frame := gtk.NewFrame("")
	frame.AddCSSClass("media-frame")
	pic := gtk.NewPictureForPaintable(nil)
	pic.SetKeepAspectRatio(true)
	pic.SetCanShrink(true)
	pic.SetSizeRequest(w, h)
	frame.SetChild(pic)

	rawURL := cv.client.MediaURL(m.MediaURL)
	chat := cv.chatID()
	var curTex *gdk.Texture
	if tex := cv.cache.MemoryTexture(rawURL); tex != nil {
		curTex = tex // instant: no async reload flicker on rebuilds
		pic.SetPaintable(tex)
	} else {
		cv.cache.ImageAsync(rawURL, func(tex *gdk.Texture, err error) {
			if err != nil || tex == nil {
				log.Printf("conversation: image load: %v", err)
				return
			}
			if cv.chatID() != chat {
				return
			}
			curTex = tex
			pic.SetPaintable(tex)
		})
	}

	addClick(frame, func() {
		if curTex != nil {
			ShowImageViewer(MainWindow, curTex)
			return
		}
		// Not loaded yet (or load raced): fetch, decode, then open.
		go func() {
			ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
			defer cancel()
			localPath, err := cv.cache.Get(ctx, rawURL)
			if err != nil {
				log.Printf("conversation: image open: %v", err)
				return
			}
			glib.IdleAdd(func() bool {
				tex, err := media.TextureFromFile(localPath)
				if err == nil {
					curTex = tex
					pic.SetPaintable(tex)
					ShowImageViewer(MainWindow, tex)
				}
				return false
			})
		}()
	})
	return frame
}

// playableBody renders video/audio as a card. With a GStreamer runtime the
// play button swaps the card for an inline player; otherwise it opens the
// downloaded file in the system player.
func (cv *Conversation) playableBody(m api.Message, icon, label string) gtk.Widgetter {
	card := gtk.NewBox(gtk.OrientationHorizontal, 8)
	card.SetMarginTop(2)
	card.SetMarginBottom(2)

	img := gtk.NewImageFromIconName(icon)
	img.SetPixelSize(36)
	card.Append(img)

	col := gtk.NewBox(gtk.OrientationVertical, 1)
	name := gtk.NewLabel(label)
	name.AddCSSClass("heading")
	name.SetXAlign(0)
	col.Append(name)
	sub := gtk.NewLabel("Klik tombol untuk memutar")
	sub.AddCSSClass("caption")
	sub.AddCSSClass("dim-label")
	sub.SetXAlign(0)
	col.Append(sub)
	card.Append(col)

	play := gtk.NewButtonFromIconName("media-playback-start-symbolic")
	play.SetTooltipText("Putar")
	card.Append(play)

	rawURL := cv.client.MediaURL(m.MediaURL)
	chat := cv.chatID()
	inline := media.GStreamerAvailable()

	play.ConnectClicked(func() {
		if !inline {
			go cv.openExternal(rawURL)
			return
		}
		go func() {
			ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
			defer cancel()
			localPath, err := cv.cache.Get(ctx, rawURL)
			glib.IdleAdd(func() bool {
				if err != nil {
					log.Printf("conversation: media download: %v", err)
					if cv.chatID() == chat && cv.toast != nil {
						cv.toast("Gagal mengunduh media: " + err.Error())
					}
					return false
				}
				player := gtk.NewVideoForFile(gio.NewFileForPath(localPath))
				player.SetAutoplay(true)
				player.SetSizeRequest(imageBoxW, imageBoxH)
				card.Remove(img)
				card.Remove(col)
				card.Remove(play)
				parent := card.Parent()
				if holder, ok := parent.(*gtk.Box); ok {
					holder.Remove(card)
					holder.Append(player)
				}
				return false
			})
		}()
	})
	return card
}

// openExternal caches the URL locally then hands it to the OS default app.
func (cv *Conversation) openExternal(rawURL string) {
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	localPath, err := cv.cache.Get(ctx, rawURL)
	if err != nil {
		log.Printf("conversation: external open: %v", err)
		return
	}
	if err := openx.File(localPath); err != nil {
		log.Printf("conversation: open %s: %v", localPath, err)
	}
}

// addClick attaches a release-click handler to any widget.
func addClick(w gtk.Widgetter, fn func()) {
	gc := gtk.NewGestureClick()
	gtk.BaseWidget(w).AddController(gc)
	gc.ConnectReleased(func(nPress int, x, y float64) {
		if nPress > 0 {
			fn()
		}
	})
}

// dateSeparatorRow builds the centered "day" divider row.
func dateSeparatorRow(prevTS, ts int64) *gtk.ListBoxRow {
	row := gtk.NewListBoxRow()
	row.SetSelectable(false)
	row.SetActivatable(false)
	row.SetFocusable(false)

	lbl := gtk.NewLabel(dayLabel(prevTS, ts))
	lbl.AddCSSClass("date-separator")
	lbl.SetHAlign(gtk.AlignCenter)
	box := gtk.NewBox(gtk.OrientationVertical, 0)
	box.SetMarginTop(10)
	box.SetMarginBottom(6)
	box.Append(lbl)
	row.SetChild(box)
	return row
}

// --- tiny helpers ---

func tickGlyph(status string) string {
	switch status {
	case "pending":
		return "🕓"
	case "sent":
		return "✓"
	case "delivered":
		return "✓✓"
	case "read":
		return "✓✓"
	case "failed":
		return "⚠"
	default:
		return ""
	}
}

func classForTick(status string) string {
	switch status {
	case "read":
		return "tick-read"
	case "failed":
		return "tick-failed"
	default:
		return "dim-label"
	}
}

func resetCSS(w *gtk.Label, classes []string) {
	for _, c := range classes {
		w.RemoveCSSClass(c)
	}
}

func timeLabel(millis int64) string {
	return time.UnixMilli(millis).Format("15:04")
}

func dayOf(millis int64) int64 {
	return dayKey(time.UnixMilli(millis))
}

func dayKey(t time.Time) int64 {
	y, m, d := t.Date()
	return int64(y)*10000 + int64(m)*100 + int64(d)
}

func dayLabel(_, ts int64) string {
	t := time.UnixMilli(ts)
	today := time.Now()
	switch {
	case dayKey(t) == dayKey(today):
		return "Hari ini"
	case dayKey(t) == dayKey(today.AddDate(0, 0, -1)):
		return "Kemarin"
	default:
		return t.Format("02 January 2006")
	}
}

func shortJID(jid string) string {
	if i := strings.IndexByte(jid, '@'); i > 0 {
		return jid[:i]
	}
	if jid == "" {
		return ""
	}
	return jid
}

func oneLine(s string) string {
	s = strings.ReplaceAll(s, "\n", " ")
	return strings.TrimSpace(s)
}

func firstNonEmptyStr(vals ...string) string {
	for _, v := range vals {
		if v != "" {
			return v
		}
	}
	return ""
}

// msgPreview renders a one-line fallback text for a message (media types get
// an emoji-prefixed label).
func msgPreview(m api.Message) string {
	if strings.TrimSpace(m.Content) != "" {
		return m.Content
	}
	switch m.Type {
	case "image":
		return "Foto"
	case "video":
		return "Video"
	case "audio", "ptt", "voice":
		return "Pesan suara"
	case "sticker":
		return "Stiker"
	default:
		return "Lampiran"
	}
}

func equalStrings(a, b []string) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}

func isPrefix(prefix, full []string) bool {
	if len(prefix) > len(full) {
		return false
	}
	for i := range prefix {
		if prefix[i] != full[i] {
			return false
		}
	}
	return true
}

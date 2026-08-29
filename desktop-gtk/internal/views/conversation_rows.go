package views

import (
	"context"
	"fmt"
	"log"
	"os"
	"strconv"
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

	// Forwarded marker above the content.
	if m.Forwarded {
		fwd := gtk.NewLabel("↪ Diteruskan")
		fwd.AddCSSClass("caption")
		fwd.AddCSSClass("dim-label")
		fwd.SetXAlign(0)
		bubble.Append(fwd)
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

	// Reaction chips under the meta line.
	if len(m.Reactions) > 0 {
		chips := gtk.NewBox(gtk.OrientationHorizontal, 4)
		for _, r := range m.Reactions {
			text := r.Emoji
			if len(r.Senders) > 1 {
				text += " " + strconv.Itoa(len(r.Senders))
			}
			chip := gtk.NewLabel(text)
			chip.AddCSSClass("reaction-chip")
			chips.Append(chip)
		}
		if outgoing {
			chips.SetHAlign(gtk.AlignEnd)
		} else {
			chips.SetHAlign(gtk.AlignStart)
		}
		bubble.Append(chips)
	}

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

	// React submenu: quick emoji set, plus remove when already reacted.
	reactMenu := gio.NewMenu()
	for i, e := range quickReactions {
		reactMenu.Append(e, fmt.Sprintf("msg.react%d", i))
	}
	if hasOwnReaction(m) {
		reactMenu.Append("Hapus reaksi", "msg.reactremove")
	}
	menu.AppendSubmenu("Reaksi", reactMenu)
	menu.Append("Teruskan…", "msg.forward")
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
	for i, e := range quickReactions {
		e := e
		add(fmt.Sprintf("react%d", i), func() { cv.sendReaction(m, e) })
	}
	add("reactremove", func() { cv.sendReaction(m, "") })
	add("forward", func() { cv.openForwardDialog(m) })
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

// updateHeaderSub refreshes the header subtitle: typing/recording indicator,
// then availability (online / last seen) for 1:1 chats, then the JID.
func (cv *Conversation) updateHeaderSub() {
	if !cv.hasChat {
		return
	}
	chatID := cv.current.ID
	if media := cv.store.TypingIn(chatID); media != "" {
		if media == "audio" {
			cv.headerSub.SetText("merekam suara…")
		} else {
			cv.headerSub.SetText("mengetik…")
		}
		return
	}
	if !cv.current.IsGroup {
		if available, lastSeen := cv.store.PresenceOf(chatID); available {
			cv.headerSub.SetText("online")
			return
		} else if lastSeen > 0 {
			t := time.UnixMilli(lastSeen)
			label := "terakhir dilihat "
			switch {
			case dayKey(t) == dayKey(time.Now()):
				label += t.Format("15:04")
			case dayKey(t) == dayKey(time.Now().AddDate(0, 0, -1)):
				label += "kemarin " + t.Format("15:04")
			default:
				label += t.Format("02/01 15:04")
			}
			cv.headerSub.SetText(label)
			return
		}
	}
	jid := chatID
	name := displayName(cv.current)
	if i := strings.IndexByte(jid, '@'); i > 0 && jid != name {
		cv.headerSub.SetText(jid)
	} else {
		cv.headerSub.SetText("")
	}
}

// quickReactions is the emoji set offered in the bubble context menu.
var quickReactions = []string{"👍", "❤️", "😂", "😮", "😢", "🙏", "👎"}

// hasOwnReaction reports whether this account already reacted to m.
func hasOwnReaction(m api.Message) bool {
	for _, r := range m.Reactions {
		for _, s := range r.Senders {
			if s == store.OutgoingFrom {
				return true
			}
		}
	}
	return false
}

// ownReactionEmoji returns the emoji of our own reaction on m, or "".
func ownReactionEmoji(m api.Message) string {
	for _, r := range m.Reactions {
		for _, s := range r.Senders {
			if s == store.OutgoingFrom {
				return r.Emoji
			}
		}
	}
	return ""
}

// sendReaction reacts to (or removes the reaction on) a message and applies
// the change optimistically.
func (cv *Conversation) sendReaction(m api.Message, emoji string) {
	chatID := cv.chatID()

	reactions := applyReactionLocal(m.Reactions, store.OutgoingFrom, emoji)
	cv.store.ApplyReaction(chatID, m.ID, reactions)

	author := ""
	if m.From != store.OutgoingFrom {
		author = m.From
	}
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
		defer cancel()
		if err := cv.client.SendReaction(ctx, chatID, m.ID, emoji, author); err != nil {
			log.Printf("conversation: react: %v", err)
		}
	}()
}

// applyReactionLocal mirrors the backend's reaction list update.
func applyReactionLocal(existing []api.ReactionEntry, sender, emoji string) []api.ReactionEntry {
	out := make([]api.ReactionEntry, 0, len(existing)+1)
	for _, r := range existing {
		kept := make([]string, 0, len(r.Senders))
		for _, s := range r.Senders {
			if s != sender {
				kept = append(kept, s)
			}
		}
		if r.Emoji == emoji && emoji != "" {
			kept = append(kept, sender)
		}
		if len(kept) > 0 {
			out = append(out, api.ReactionEntry{Emoji: r.Emoji, Senders: kept})
		}
	}
	if emoji != "" {
		for i := range out {
			if out[i].Emoji == emoji {
				return out
			}
		}
		out = append(out, api.ReactionEntry{Emoji: emoji, Senders: []string{sender}})
	}
	return out
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
		if m.Extra != nil && m.Extra.ViewOnce != nil {
			return cv.viewOnceBody(m, "image")
		}
		return cv.pictureBody(m, imageBoxW, imageBoxH)
	case "sticker":
		return cv.pictureBody(m, stickerBoxSize, stickerBoxSize)
	case "video":
		if m.Extra != nil && m.Extra.ViewOnce != nil {
			return cv.viewOnceBody(m, "video")
		}
		return cv.playableBody(m, "video-x-generic", "Video")
	case "gif":
		return cv.playableBody(m, "emblem-videos-symbolic", "GIF")
	case "audio", "ptt", "voice":
		icon := "audio-x-generic"
		label := "Audio"
		if m.Type != "audio" {
			icon = "audio-input-microphone-symbolic"
			label = "Pesan suara"
		}
		return cv.playableBody(m, icon, label)
	case "poll":
		return cv.pollBody(m)
	case "location":
		return cv.locationBody(m)
	case "contact":
		return cv.contactBody(m)
	default:
		var body gtk.Widgetter
		if m.Extra != nil && m.Extra.LinkPreview != nil {
			body = cv.textWithPreviewBody(m)
		} else {
			text := strings.TrimSpace(m.Content)
			if text == "" {
				text = "[" + m.Type + "]"
			}
			lbl := gtk.NewLabel(text)
			lbl.SetWrap(true)
			lbl.SetSelectable(true)
			lbl.SetXAlign(0)
			body = lbl
		}
		return body
	}
}

// viewOnceBody renders view-once media with the "sekali lihat" strip; opening
// the media marks it viewed locally.
func (cv *Conversation) viewOnceBody(m api.Message, mediaKind string) gtk.Widgetter {
	box := gtk.NewBox(gtk.OrientationVertical, 4)

	strip := gtk.NewBox(gtk.OrientationHorizontal, 6)
	icon := "camera-photo-symbolic"
	label := "Foto sekali lihat"
	if mediaKind == "video" {
		icon = "camera-video-symbolic"
		label = "Video sekali lihat"
	}
	if m.Extra.ViewOnce.Viewed {
		label += " (sudah dilihat)"
	}
	strip.Append(gtk.NewImageFromIconName(icon))
	lbl := gtk.NewLabel(label)
	lbl.AddCSSClass("caption")
	lbl.SetXAlign(0)
	strip.Append(lbl)
	box.Append(strip)

	var body gtk.Widgetter
	if mediaKind == "video" {
		body = cv.playableBody(m, "camera-video-symbolic", label)
	} else {
		body = cv.pictureBody(m, imageBoxW, imageBoxH)
	}
	box.Append(body)

	if !m.Extra.ViewOnce.Viewed {
		addClick(box, func() {
			cv.store.SetViewOnceViewed(cv.chatID(), m.ID)
		})
	}
	return box
}

// pollBody renders a poll bubble: question, clickable options with vote
// counts, and the total voter count.
func (cv *Conversation) pollBody(m api.Message) gtk.Widgetter {
	poll := m.Extra.Poll

	box := gtk.NewBox(gtk.OrientationVertical, 6)
	q := gtk.NewLabel(poll.Question)
	q.SetWrap(true)
	q.SetXAlign(0)
	q.AddCSSClass("heading")
	box.Append(q)

	counts := poll.VoteCounts()
	myVote := map[string]bool{}
	for _, o := range poll.MyVote() {
		myVote[o] = true
	}
	total := len(poll.Votes)

	for _, opt := range poll.Options {
		name := opt.Name
		count := counts[name]

		row := gtk.NewBox(gtk.OrientationHorizontal, 8)
		marker := "○"
		if myVote[name] {
			marker = "●"
		}
		mark := gtk.NewLabel(marker)
		mark.AddCSSClass("dim-label")
		row.Append(mark)

		nameLbl := gtk.NewLabel(name)
		nameLbl.SetXAlign(0)
		nameLbl.SetHExpand(true)
		if myVote[name] {
			nameLbl.AddCSSClass("poll-voted")
		}
		row.Append(nameLbl)

		countLbl := gtk.NewLabel(strconv.Itoa(count))
		countLbl.AddCSSClass("caption")
		countLbl.AddCSSClass("dim-label")
		row.Append(countLbl)

		addClick(row, func() { cv.togglePollVote(m, name, myVote[name]) })
		box.Append(row)
	}

	voters := fmt.Sprintf("%d pemilih", total)
	if poll.MultiSelect {
		voters = fmt.Sprintf("%d pemilih · pilih beberapa", total)
	}
	foot := gtk.NewLabel(voters)
	foot.AddCSSClass("caption")
	foot.AddCSSClass("dim-label")
	foot.SetXAlign(0)
	box.Append(foot)
	return box
}

// togglePollVote casts or retracts one option, optimistically updating the
// store before the REST call confirms.
func (cv *Conversation) togglePollVote(m api.Message, option string, currentlyVoted bool) {
	if m.Extra == nil || m.Extra.Poll == nil {
		return
	}
	chatID := cv.chatID()

	next := make([]string, 0, len(m.Extra.Poll.MyVote()))
	if m.Extra.Poll.MultiSelect {
		found := false
		for _, o := range m.Extra.Poll.MyVote() {
			if o == option {
				found = true
				continue
			}
			next = append(next, o)
		}
		if !found {
			next = append(next, option)
		}
	} else {
		if currentlyVoted {
			next = nil
		} else {
			next = []string{option}
		}
	}

	// Optimistic local update.
	extra := *m.Extra
	pollCopy := *extra.Poll
	votes := make(map[string][]string, len(pollCopy.Votes))
	for k, v := range pollCopy.Votes {
		votes[k] = v
	}
	if len(next) == 0 {
		delete(votes, "me")
	} else {
		votes["me"] = next
	}
	pollCopy.Votes = votes
	extra.Poll = &pollCopy
	cv.store.ApplyExtra(chatID, m.ID, &extra)

	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
		defer cancel()
		if err := cv.client.SendPollVote(ctx, chatID, m.ID, next); err != nil {
			log.Printf("conversation: poll vote: %v", err)
		}
	}()
}

// locationBody renders a location card with an optional thumbnail and an
// "open in Maps" action.
func (cv *Conversation) locationBody(m api.Message) gtk.Widgetter {
	loc := m.Extra.Location

	box := gtk.NewBox(gtk.OrientationVertical, 6)

	if loc.ThumbnailURL != "" {
		fake := api.Message{ID: m.ID, Type: "image", MediaURL: loc.ThumbnailURL}
		if body := cv.pictureBody(fake, imageBoxW, 140); body != nil {
			box.Append(body)
		}
	}

	title := loc.Name
	if title == "" {
		title = loc.Address
	}
	if title == "" {
		title = "Lokasi"
	}
	t := gtk.NewLabel(title)
	t.SetWrap(true)
	t.SetXAlign(0)
	t.AddCSSClass("heading")
	box.Append(t)

	coords := fmt.Sprintf("%.5f, %.5f", loc.Latitude, loc.Longitude)
	cLbl := gtk.NewLabel(coords)
	cLbl.AddCSSClass("caption")
	cLbl.AddCSSClass("dim-label")
	cLbl.SetXAlign(0)
	box.Append(cLbl)

	mapsBtn := gtk.NewButtonWithLabel("Buka di Maps")
	mapsBtn.AddCSSClass("suggested-action")
	mapsBtn.SetHAlign(gtk.AlignStart)
	mapsBtn.ConnectClicked(func() {
		_ = openx.URL(fmt.Sprintf("https://www.google.com/maps?q=%.6f,%.6f", loc.Latitude, loc.Longitude))
	})
	box.Append(mapsBtn)
	return box
}

// contactBody renders a shared contact card with a save-as-vCard action.
func (cv *Conversation) contactBody(m api.Message) gtk.Widgetter {
	meta := m.Extra.Contact

	box := gtk.NewBox(gtk.OrientationVertical, 6)

	for _, c := range meta.Contacts {
		row := gtk.NewBox(gtk.OrientationHorizontal, 10)
		row.SetMarginTop(2)
		row.SetMarginBottom(2)
		avatar := adw.NewAvatar(40, c.DisplayName, true)
		row.Append(avatar)

		col := gtk.NewBox(gtk.OrientationVertical, 1)
		name := gtk.NewLabel(c.DisplayName)
		name.SetXAlign(0)
		name.AddCSSClass("heading")
		col.Append(name)
		if phone := vcardPhone(c.VCard); phone != "" {
			ph := gtk.NewLabel(phone)
			ph.AddCSSClass("caption")
			ph.AddCSSClass("dim-label")
			ph.SetXAlign(0)
			col.Append(ph)
		}
		row.Append(col)
		box.Append(row)
	}

	if len(meta.Contacts) == 1 {
		save := gtk.NewButtonWithLabel("Simpan Kontak")
		save.SetHAlign(gtk.AlignStart)
		save.ConnectClicked(func() { cv.saveContactVCard(meta.Contacts[0]) })
		box.Append(save)
	}
	return box
}

// saveContactVCard writes the vCard to a temp file and opens it with the
// system contacts handler.
func (cv *Conversation) saveContactVCard(entry api.ContactEntry) {
	vcard := entry.VCard
	if vcard == "" {
		vcard = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:" + entry.DisplayName + "\r\nEND:VCARD\r\n"
	}
	go func() {
		f, err := os.CreateTemp("", "contact-*.vcf")
		if err != nil {
			log.Printf("contact vcard: %v", err)
			return
		}
		path := f.Name()
		_, werr := f.WriteString(vcard)
		f.Close()
		if werr != nil {
			log.Printf("contact vcard write: %v", werr)
			return
		}
		if err := openx.File(path); err != nil {
			log.Printf("contact vcard open: %v", err)
		}
	}()
}

// textWithPreviewBody renders a text bubble with the sender-attached link
// preview card above the text.
func (cv *Conversation) textWithPreviewBody(m api.Message) gtk.Widgetter {
	prev := m.Extra.LinkPreview

	box := gtk.NewBox(gtk.OrientationVertical, 6)

	card := gtk.NewBox(gtk.OrientationHorizontal, 8)
	card.AddCSSClass("link-preview")

	if prev.ThumbnailURL != "" {
		fake := api.Message{ID: m.ID, Type: "image", MediaURL: prev.ThumbnailURL}
		pic := gtk.NewPictureForPaintable(nil)
		pic.SetKeepAspectRatio(true)
		pic.SetCanShrink(true)
		pic.SetSizeRequest(90, 68)
		rawURL := cv.client.MediaURL(prev.ThumbnailURL)
		if tex := cv.cache.MemoryTexture(rawURL); tex != nil {
			pic.SetPaintable(tex)
		} else {
			chat := cv.chatID()
			cv.cache.ImageAsync(rawURL, func(tex *gdk.Texture, err error) {
				if err == nil && tex != nil && cv.chatID() == chat {
					pic.SetPaintable(tex)
				}
			})
		}
		card.Append(pic)
		_ = fake
	}

	col := gtk.NewBox(gtk.OrientationVertical, 1)
	title := prev.Title
	if title == "" {
		title = prev.URL
	}
	tLbl := gtk.NewLabel(title)
	tLbl.SetEllipsize(pango.EllipsizeEnd)
	tLbl.SetXAlign(0)
	tLbl.SetMaxWidthChars(36)
	tLbl.AddCSSClass("link-preview-title")
	col.Append(tLbl)
	if prev.Description != "" {
		dLbl := gtk.NewLabel(prev.Description)
		dLbl.SetEllipsize(pango.EllipsizeEnd)
		dLbl.SetXAlign(0)
		dLbl.SetMaxWidthChars(36)
		dLbl.AddCSSClass("caption")
		dLbl.AddCSSClass("dim-label")
		col.Append(dLbl)
	}
	hostLbl := gtk.NewLabel(prev.URL)
	hostLbl.SetEllipsize(pango.EllipsizeEnd)
	hostLbl.SetXAlign(0)
	hostLbl.SetMaxWidthChars(36)
	hostLbl.AddCSSClass("caption")
	hostLbl.AddCSSClass("dim-label")
	col.Append(hostLbl)
	card.Append(col)

	addClick(card, func() { _ = openx.URL(prev.URL) })
	box.Append(card)

	text := strings.TrimSpace(m.Content)
	if text != "" {
		lbl := gtk.NewLabel(text)
		lbl.SetWrap(true)
		lbl.SetSelectable(true)
		lbl.SetXAlign(0)
		box.Append(lbl)
	}
	return box
}

// vcardPhone extracts the first TEL number from a vCard string.
func vcardPhone(vcard string) string {
	for _, line := range strings.Split(vcard, "\n") {
		line = strings.TrimRight(line, "\r")
		if strings.HasPrefix(line, "TEL") {
			if i := strings.IndexByte(line, ':'); i >= 0 {
				return line[i+1:]
			}
		}
	}
	return ""
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
	overlay := gtk.NewOverlay()
	overlay.SetChild(pic)
	unavailable := gtk.NewLabel("Media tidak lagi tersedia")
	unavailable.AddCSSClass("dim-label")
	unavailable.SetWrap(true)
	unavailable.SetVisible(false)
	overlay.AddOverlay(unavailable)
	frame.SetChild(overlay)

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
				unavailable.SetVisible(true)
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
	switch m.Type {
	case "poll":
		if m.Extra != nil && m.Extra.Poll != nil && m.Extra.Poll.Question != "" {
			return m.Extra.Poll.Question
		}
	case "location":
		if m.Extra != nil && m.Extra.Location != nil {
			if m.Extra.Location.Live {
				return "Lokasi langsung"
			}
			return "Lokasi"
		}
	case "contact":
		if m.Extra != nil && m.Extra.Contact != nil && m.Extra.Contact.DisplayName != "" {
			return m.Extra.Contact.DisplayName
		}
	case "gif":
		return "GIF"
	}
	if strings.TrimSpace(m.Content) != "" {
		return m.Content
	}
	switch m.Type {
	case "image":
		if m.Extra != nil && m.Extra.ViewOnce != nil {
			return "Foto (sekali lihat)"
		}
		return "Foto"
	case "video":
		if m.Extra != nil && m.Extra.ViewOnce != nil {
			return "Video (sekali lihat)"
		}
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

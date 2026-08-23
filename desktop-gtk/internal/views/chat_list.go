package views

import (
	"context"
	"fmt"
	"log"
	"strings"
	"sync/atomic"
	"time"

	"github.com/diamondburned/gotk4/pkg/gdk/v4"
	"github.com/diamondburned/gotk4/pkg/gio/v2"
	"github.com/diamondburned/gotk4/pkg/glib/v2"
	"github.com/diamondburned/gotk4/pkg/gtk/v4"

	"wa-bot-desktop/internal/api"
	"wa-bot-desktop/internal/media"
	"wa-bot-desktop/internal/store"
)

// ChatList is the live chats sidebar. It renders the shared store's chat
// list (sorted by recency) with avatars, unread badges and last-message
// previews, and refreshes automatically on store change events.
type ChatList struct {
	root         *gtk.Box
	search       *gtk.SearchEntry
	listBox      *gtk.ListBox
	status       *gtk.Label
	empty        *gtk.Label
	scroller     *gtk.ScrolledWindow
	modeTitle    *gtk.Label
	backBtn      *gtk.Button
	archiveBtn   *gtk.Button
	archiveCount *gtk.Label
	ctxPopover   *gtk.PopoverMenu

	client       *api.Client
	store        *store.Store
	cache        *media.Cache
	filter       string
	archivedMode bool
	gen          atomic.Uint64 // bumped on every rebuild; invalidates stale avatar loads
	queued       bool          // a rebuild is already scheduled on the main loop
	unsub        func()

	onActivate func(chatID string)
	toast      func(string)
	activeID   string // highlighted row
}

// NewChatList constructs the chat list view.
func NewChatList() *ChatList {
	cl := &ChatList{}

	cl.root = gtk.NewBox(gtk.OrientationVertical, 0)
	modeRow := gtk.NewBox(gtk.OrientationHorizontal, 6)
	modeRow.SetMarginTop(8)
	modeRow.SetMarginStart(10)
	modeRow.SetMarginEnd(10)
	cl.backBtn = gtk.NewButtonFromIconName("go-previous-symbolic")
	cl.backBtn.SetVisible(false)
	cl.backBtn.ConnectClicked(func() {
		cl.archivedMode = false
		cl.filter = ""
		cl.search.SetText("")
		cl.rebuild()
	})
	modeRow.Append(cl.backBtn)
	cl.modeTitle = gtk.NewLabel("Chat")
	cl.modeTitle.AddCSSClass("title-2")
	cl.modeTitle.SetXAlign(0)
	modeRow.Append(cl.modeTitle)
	cl.root.Append(modeRow)

	cl.search = gtk.NewSearchEntry()
	cl.search.SetPlaceholderText("Cari chat…")
	cl.search.SetMarginTop(8)
	cl.search.SetMarginBottom(8)
	cl.search.SetMarginStart(10)
	cl.search.SetMarginEnd(10)
	cl.search.ConnectSearchChanged(func() {
		cl.filter = strings.ToLower(cl.search.Text())
		cl.rebuild()
	})
	cl.root.Append(cl.search)

	cl.archiveBtn = gtk.NewButton()
	cl.archiveBtn.SetMarginStart(10)
	cl.archiveBtn.SetMarginEnd(10)
	cl.archiveBtn.SetMarginBottom(6)
	cl.archiveBtn.AddCSSClass("flat")
	archiveContent := gtk.NewBox(gtk.OrientationHorizontal, 10)
	archiveIcon := gtk.NewImageFromIconName("folder-symbolic")
	archiveContent.Append(archiveIcon)
	archiveLabel := gtk.NewLabel("Arsip")
	archiveLabel.SetXAlign(0)
	archiveLabel.SetHExpand(true)
	archiveContent.Append(archiveLabel)
	cl.archiveCount = gtk.NewLabel("")
	cl.archiveCount.AddCSSClass("dim-label")
	archiveContent.Append(cl.archiveCount)
	cl.archiveBtn.SetChild(archiveContent)
	cl.archiveBtn.SetVisible(false)
	cl.archiveBtn.ConnectClicked(func() {
		cl.archivedMode = true
		cl.filter = ""
		cl.search.SetText("")
		cl.rebuild()
	})
	cl.root.Append(cl.archiveBtn)

	cl.status = gtk.NewLabel("Memuat chat…")
	cl.status.AddCSSClass("dim-label")
	cl.status.SetMarginTop(12)
	cl.status.SetMarginBottom(12)
	cl.status.SetHAlign(gtk.AlignCenter)
	cl.root.Append(cl.status)

	cl.empty = gtk.NewLabel("Belum ada chat.")
	cl.empty.AddCSSClass("dim-label")
	cl.empty.SetVExpand(true)
	cl.empty.SetVisible(false)
	cl.root.Append(cl.empty)

	cl.listBox = gtk.NewListBox()
	cl.listBox.AddCSSClass("navigation-sidebar")
	cl.listBox.SetSelectionMode(gtk.SelectionSingle)
	cl.listBox.SetShowSeparators(false)
	cl.listBox.SetHExpand(true)
	cl.listBox.SetVExpand(true)

	cl.scroller = gtk.NewScrolledWindow()
	cl.scroller.SetPolicy(gtk.PolicyNever, gtk.PolicyAutomatic)
	cl.scroller.SetChild(cl.listBox)
	cl.root.Append(cl.scroller)

	cl.listBox.ConnectRowActivated(func(row *gtk.ListBoxRow) {
		if cl.onActivate != nil {
			if id := row.Name(); id != "" {
				cl.onActivate(id)
			}
		}
	})

	return cl
}

// Widget returns the root widget for embedding.
func (cl *ChatList) Widget() gtk.Widgetter { return cl.root }

// SetDeps wires collaborators and starts listening for store changes.
func (cl *ChatList) SetDeps(client *api.Client, st *store.Store, cache *media.Cache, toast func(string)) {
	cl.client = client
	cl.store = st
	cl.cache = cache
	cl.toast = toast
	st.Subscribe(func(c store.Change) {
		if c.Kind != store.ChatsChanged {
			return
		}
		cl.queueRebuild()
	})
	cl.rebuild()
}

// queueRebuild schedules one rebuild on the GTK main thread, collapsing
// bursts of change events (e.g. mark-read + list refresh on every chat
// switch) into a single pass instead of freezing the UI with back-to-back
// full rebuilds.
func (cl *ChatList) queueRebuild() {
	if cl.queued {
		return
	}
	cl.queued = true
	glib.IdleAdd(func() bool {
		cl.queued = false
		cl.rebuild()
		return false
	})
}

// SetActivateCallback registers the row-activation handler.
func (cl *ChatList) SetActivateCallback(cb func(chatID string)) {
	cl.onActivate = cb
}

// Highlight selects the row of the given chat without activating it.
func (cl *ChatList) Highlight(chatID string) {
	cl.activeID = chatID
	cl.listBox.SelectRow(nil)
	for i := 0; ; i++ {
		row := cl.listBox.RowAtIndex(i)
		if row == nil {
			break
		}
		if row.Name() == chatID {
			cl.listBox.SelectRow(row)
			break
		}
	}
}

// rebuild re-renders all rows from the current store snapshot.
func (cl *ChatList) rebuild() {
	gen := cl.gen.Add(1)

	for {
		row := cl.listBox.RowAtIndex(0)
		if row == nil {
			break
		}
		cl.listBox.Remove(row)
	}

	all := cl.allChats()
	cl.modeTitle.SetText(map[bool]string{true: "Diarsipkan", false: "Chat"}[cl.archivedMode])
	cl.backBtn.SetVisible(cl.archivedMode)
	archivedCount, archivedUnread := 0, 0
	for _, c := range all {
		if c.Archived {
			archivedCount++
			archivedUnread += c.Unread
		}
	}
	cl.archiveBtn.SetVisible(!cl.archivedMode && cl.filter == "" && archivedCount > 0)
	if archivedUnread > 0 {
		cl.archiveCount.SetText(fmt.Sprintf("%d belum dibaca", archivedUnread))
	} else {
		cl.archiveCount.SetText(fmt.Sprintf("%d", archivedCount))
	}
	if len(all) == 0 {
		cl.status.SetVisible(true)
		cl.status.SetText("Belum ada chat tersinkron.")
		cl.scroller.SetVisible(false)
		cl.empty.SetVisible(false)
		return
	}
	cl.status.SetVisible(false)
	cl.empty.SetVisible(false)
	cl.scroller.SetVisible(true)

	chats := cl.filtered(all)
	for _, c := range chats {
		row := cl.buildRow(gen, c)
		cl.listBox.Append(row)
		if c.ID == cl.activeID {
			cl.listBox.SelectRow(row)
		}
	}

	if len(chats) == 0 {
		cl.status.SetVisible(true)
		if cl.filter != "" {
			cl.status.SetText("Tidak ada yang cocok dengan pencarian.")
		} else if cl.archivedMode {
			cl.status.SetText("Belum ada chat yang diarsipkan.")
		} else {
			cl.status.SetText("Belum ada chat aktif.")
		}
	}
}

func (cl *ChatList) allChats() []api.Chat {
	if cl.store == nil {
		return nil
	}
	return cl.store.Chats()
}

// filtered applies the search filter over the snapshot.
func (cl *ChatList) filtered(all []api.Chat) []api.Chat {
	var out []api.Chat
	for _, c := range all {
		if c.Archived != cl.archivedMode {
			continue
		}
		if cl.filter == "" || strings.Contains(strings.ToLower(c.Name), cl.filter) ||
			strings.Contains(strings.ToLower(c.LastMsg), cl.filter) ||
			strings.Contains(strings.ToLower(c.ID), cl.filter) {
			out = append(out, c)
		}
	}
	return out
}

// buildRow constructs one row widget for a chat.
func (cl *ChatList) buildRow(gen uint64, c api.Chat) *gtk.ListBoxRow {
	row := gtk.NewListBoxRow()
	row.SetActivatable(true)
	row.SetSelectable(true)
	row.SetName(c.ID)
	// Uniform row height: avatar (44px) + vertical margins dominate, so a
	// multi-line last message can never stretch the row.
	row.SetSizeRequest(-1, 64)

	box := gtk.NewBox(gtk.OrientationHorizontal, 10)
	box.SetMarginTop(6)
	box.SetMarginBottom(6)
	box.SetMarginStart(8)
	box.SetMarginEnd(8)
	box.SetVAlign(gtk.AlignCenter)

	box.Append(newChatAvatar(cl.client, cl.cache, func() bool { return cl.gen.Load() != gen }, c))

	mid := gtk.NewBox(gtk.OrientationVertical, 2)
	mid.SetHExpand(true)
	mid.SetVAlign(gtk.AlignCenter)

	name := gtk.NewLabel(store.OneLine(displayName(c), 40))
	name.AddCSSClass("heading")
	name.SetEllipsize(3) // PANGO_ELLIPSIZE_END
	name.SetXAlign(0)
	name.SetMaxWidthChars(22)
	nameRow := gtk.NewBox(gtk.OrientationHorizontal, 4)
	nameRow.Append(name)
	if c.PinnedAt != nil {
		pin := gtk.NewLabel("📌")
		pin.SetTooltipText("Disematkan")
		nameRow.Append(pin)
	}
	mid.Append(nameRow)

	preview := gtk.NewLabel(store.OneLine(c.LastMsg, 80))
	preview.AddCSSClass("dim-label")
	preview.SetEllipsize(3)
	preview.SetSingleLineMode(true)
	preview.SetXAlign(0)
	preview.SetMaxWidthChars(26)
	previewRow := gtk.NewBox(gtk.OrientationHorizontal, 4)
	if c.MuteMode != "" && c.MuteMode != "off" {
		muted := gtk.NewLabel("🔕")
		muted.SetTooltipText("Notifikasi dibisukan")
		previewRow.Append(muted)
	}
	previewRow.Append(preview)
	mid.Append(previewRow)
	box.Append(mid)

	right := gtk.NewBox(gtk.OrientationVertical, 4)
	right.SetVAlign(gtk.AlignCenter)

	if c.LastTime > 0 {
		ts := gtk.NewLabel(formatTimestamp(c.LastTime))
		ts.AddCSSClass("caption")
		ts.AddCSSClass("dim-label")
		right.Append(ts)
	}
	if c.Unread > 0 {
		badge := gtk.NewLabel(unreadLabel(c.Unread))
		badge.AddCSSClass("unread-badge")
		badge.SetHAlign(gtk.AlignEnd)
		right.Append(badge)
	}
	box.Append(right)

	row.SetChild(box)
	gc := gtk.NewGestureClick()
	gc.SetButton(gdk.BUTTON_SECONDARY)
	row.AddController(gc)
	gc.ConnectPressed(func(_ int, x, y float64) { cl.openChatMenu(row, x, y, c) })
	return row
}

func (cl *ChatList) openChatMenu(host gtk.Widgetter, x, y float64, chat api.Chat) {
	menu := gio.NewMenu()
	if chat.PinnedAt != nil {
		menu.Append("Lepas sematan", "chat.unpin")
	} else {
		menu.Append("Sematkan", "chat.pin")
	}
	if chat.Archived {
		menu.Append("Keluarkan dari arsip", "chat.unarchive")
	} else {
		menu.Append("Arsipkan", "chat.archive")
	}
	if chat.MuteMode != "" && chat.MuteMode != "off" {
		menu.Append("Nyalakan notifikasi", "chat.unmute")
	} else {
		menu.Append("Bisukan 8 jam", "chat.mute8h")
		menu.Append("Bisukan 1 minggu", "chat.mute1w")
		menu.Append("Bisukan selamanya", "chat.muteforever")
	}

	group := gio.NewSimpleActionGroup()
	add := func(name string, fn func()) {
		action := gio.NewSimpleAction(name, nil)
		action.ConnectActivate(func(*glib.Variant) { fn() })
		group.Insert(action)
	}
	add("pin", func() { cl.mutateChat(chat, "pin", "") })
	add("unpin", func() { cl.mutateChat(chat, "unpin", "") })
	add("archive", func() { cl.mutateChat(chat, "archive", "") })
	add("unarchive", func() { cl.mutateChat(chat, "unarchive", "") })
	add("unmute", func() { cl.mutateChat(chat, "mute", "off") })
	add("mute8h", func() { cl.mutateChat(chat, "mute", "8h") })
	add("mute1w", func() { cl.mutateChat(chat, "mute", "1w") })
	add("muteforever", func() { cl.mutateChat(chat, "mute", "forever") })
	cl.root.InsertActionGroup("chat", group)
	if cl.ctxPopover == nil {
		cl.ctxPopover = gtk.NewPopoverMenuFromModel(menu)
		gtk.BaseWidget(cl.ctxPopover).SetParent(cl.root)
	} else {
		cl.ctxPopover.SetMenuModel(menu)
	}
	gx, gy, ok := gtk.BaseWidget(host).TranslateCoordinates(cl.root, x, y)
	if !ok {
		return
	}
	rect := gdk.NewRectangle(int(gx)-2, int(gy)-2, 4, 4)
	cl.ctxPopover.SetPointingTo(&rect)
	cl.ctxPopover.Popup()
}

func (cl *ChatList) mutateChat(chat api.Chat, action, mode string) {
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
		defer cancel()
		var state *api.ChatState
		var err error
		switch action {
		case "pin":
			state, err = cl.client.PinChat(ctx, chat.ID, true)
		case "unpin":
			state, err = cl.client.PinChat(ctx, chat.ID, false)
		case "archive":
			state, err = cl.client.ArchiveChat(ctx, chat.ID, true)
		case "unarchive":
			state, err = cl.client.ArchiveChat(ctx, chat.ID, false)
		case "mute":
			state, err = cl.client.MuteChat(ctx, chat.ID, mode)
		}
		if err != nil {
			log.Printf("chat state: %v", err)
			glib.IdleAdd(func() bool {
				if cl.toast != nil {
					cl.toast("Gagal memperbarui chat: " + err.Error())
				}
				return false
			})
			return
		}
		if state != nil {
			cl.store.PatchChatState(*state)
		}
	}()
}

// unreadLabel collapses large counters ("99+").
func unreadLabel(n int) string {
	if n > 99 {
		return "99+"
	}
	return fmt.Sprintf("%d", n)
}

// displayName resolves a human-friendly name with JID fallback.
func displayName(c api.Chat) string {
	if c.Name != "" && c.Name != "unknown" {
		return c.Name
	}
	jid := c.ID
	if i := strings.IndexByte(jid, '@'); i > 0 {
		jid = jid[:i]
	}
	return jid
}

// formatTimestamp renders chat-list timestamps: HH:MM today, else short date.
func formatTimestamp(millis int64) string {
	t := time.UnixMilli(millis)
	now := time.Now()
	if t.Format("2006-01-02") == now.Format("2006-01-02") {
		return t.Format("15:04")
	}
	if t.Year() == now.Year() {
		return t.Format("02/01")
	}
	return t.Format("02/01/06")
}

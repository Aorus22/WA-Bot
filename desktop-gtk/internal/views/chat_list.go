package views

import (
	"fmt"
	"strings"
	"sync/atomic"
	"time"

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
	root     *gtk.Box
	search   *gtk.SearchEntry
	listBox  *gtk.ListBox
	status   *gtk.Label
	empty    *gtk.Label
	scroller *gtk.ScrolledWindow

	client *api.Client
	store  *store.Store
	cache  *media.Cache
	filter string
	gen    atomic.Uint64 // bumped on every rebuild; invalidates stale avatar loads
	unsub  func()

	onActivate func(chatID string)
	activeID   string // highlighted row
}

// NewChatList constructs the chat list view.
func NewChatList() *ChatList {
	cl := &ChatList{}

	cl.root = gtk.NewBox(gtk.OrientationVertical, 0)

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
func (cl *ChatList) SetDeps(client *api.Client, st *store.Store, cache *media.Cache) {
	cl.client = client
	cl.store = st
	cl.cache = cache
	st.Subscribe(func(c store.Change) {
		if c.Kind != store.ChatsChanged {
			return
		}
		glib.IdleAdd(func() bool {
			cl.rebuild()
			return false
		})
	})
	cl.rebuild()
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
		cl.status.SetText("Tidak ada yang cocok dengan pencarian.")
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
	if cl.filter == "" {
		return all
	}
	var out []api.Chat
	for _, c := range all {
		if strings.Contains(strings.ToLower(c.Name), cl.filter) ||
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
	mid.Append(name)

	preview := gtk.NewLabel(store.OneLine(c.LastMsg, 80))
	preview.AddCSSClass("dim-label")
	preview.SetEllipsize(3)
	preview.SetSingleLineMode(true)
	preview.SetXAlign(0)
	preview.SetMaxWidthChars(26)
	mid.Append(preview)
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
	return row
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

package views

import (
	"context"
	"log"
	"sort"
	"strings"
	"time"

	"github.com/diamondburned/gotk4-adwaita/pkg/adw"
	"github.com/diamondburned/gotk4/pkg/glib/v2"
	"github.com/diamondburned/gotk4/pkg/gtk/v4"

	"wa-bot-desktop/internal/api"
	"wa-bot-desktop/internal/media"
	"wa-bot-desktop/internal/store"
)

const (
	pageSize     = 30
	loadOlderAt  = 140 // px from top edge that triggers older-page fetch
	nearBottomPx = 220
)

// Conversation is the right-hand chat pane: header, scrolling bubble history,
// and composer. It renders exclusively from the shared store and refreshes on
// store events for the open chat.
type Conversation struct {
	client *api.Client
	store  *store.Store
	cache  *media.Cache
	toast  func(string)

	root    *gtk.Box
	content *gtk.Box
	empty   *gtk.Label

	headerAvatar *adw.Avatar
	headerName   *gtk.Label
	headerSub    *gtk.Label

	listBox    *gtk.ListBox
	scroller   *gtk.ScrolledWindow
	msgOverlay *gtk.Overlay
	loadSpinW  *gtk.Box
	loadSpin   *gtk.Spinner
	loading    int // in-flight initial loads
	adj        *gtk.Adjustment

	composerTextView *gtk.TextView
	attachBtn        *gtk.MenuButton
	sendBtn          *gtk.Button

	current           api.Chat
	hasChat           bool
	fetchedOnce       map[string]bool
	loadingOld        bool
	loadingNext       bool // a newer-page fetch is in flight
	pendingKeepAnchor int  // next append must preserve the viewport anchor

	renderedIDs []string
	ticks       map[string]*gtk.Label      // msgID -> status glyph label (outgoing)
	rows        map[string]*gtk.ListBoxRow // msgID -> rendered row (teleport/highlight)

	onSidebar     func()
	onHeaderClick func()
	onSearchClick func()

	// typing presence state
	typingActive bool
	lastTyping   time.Time
	typingStop   glib.SourceHandle // glib source id of the stop timer
}

// NewConversation constructs the conversation pane widgets.
func NewConversation() *Conversation {
	cv := &Conversation{
		fetchedOnce: make(map[string]bool),
		ticks:       make(map[string]*gtk.Label),
		rows:        make(map[string]*gtk.ListBoxRow),
	}

	cv.root = gtk.NewBox(gtk.OrientationVertical, 0)

	cv.empty = gtk.NewLabel("Pilih sebuah chat untuk mulai berbicara.")
	cv.empty.AddCSSClass("dim-label")
	cv.empty.AddCSSClass("title-4")
	cv.empty.SetVExpand(true)
	cv.empty.SetHExpand(true)
	cv.root.Append(cv.empty)

	// ---- Header ----
	header := gtk.NewBox(gtk.OrientationHorizontal, 10)
	header.SetMarginTop(8)
	header.SetMarginBottom(8)
	header.SetMarginStart(10)
	header.SetMarginEnd(10)

	sidebarBtn := gtk.NewButtonFromIconName("sidebar-show-symbolic")
	sidebarBtn.SetTooltipText("Tampilkan/sembunyikan daftar chat")
	sidebarBtn.ConnectClicked(func() {
		if cv.onSidebar != nil {
			cv.onSidebar()
		}
	})
	header.Append(sidebarBtn)

	cv.headerAvatar = adw.NewAvatar(36, "?", false)

	hcol := gtk.NewBox(gtk.OrientationVertical, 0)
	cv.headerName = gtk.NewLabel("")
	cv.headerName.AddCSSClass("title-3")
	cv.headerName.SetXAlign(0)
	hcol.Append(cv.headerName)
	cv.headerSub = gtk.NewLabel("")
	cv.headerSub.AddCSSClass("caption")
	cv.headerSub.AddCSSClass("dim-label")
	cv.headerSub.SetXAlign(0)
	hcol.Append(cv.headerSub)

	// Identity block opens the right-side chat info panel (web parity:
	// clicking anywhere on the header shows the sheet). It expands over the
	// remaining header width; the sidebar-toggle button stays outside so
	// button clicks never double-trigger the panel.
	ident := gtk.NewBox(gtk.OrientationHorizontal, 10)
	ident.Append(cv.headerAvatar)
	ident.Append(hcol)
	addClick(ident, func() {
		if cv.onHeaderClick != nil {
			cv.onHeaderClick()
		}
	})
	ident.SetHExpand(true)
	header.Append(ident)

	searchBtn := gtk.NewButtonFromIconName("system-search-symbolic")
	searchBtn.SetTooltipText("Cari pesan")
	searchBtn.ConnectClicked(func() {
		if cv.onSearchClick != nil {
			cv.onSearchClick()
		}
	})
	header.Append(searchBtn)

	// ---- Message list ----
	cv.listBox = gtk.NewListBox()
	cv.listBox.SetSelectionMode(gtk.SelectionNone)
	cv.listBox.SetShowSeparators(false)
	cv.listBox.SetHExpand(true)
	cv.listBox.SetVExpand(true)

	cv.scroller = gtk.NewScrolledWindow()
	cv.scroller.SetPolicy(gtk.PolicyNever, gtk.PolicyAutomatic)
	cv.scroller.SetChild(cv.listBox)
	cv.scroller.SetVExpand(true)
	cv.scroller.SetHExpand(true)

	// Loading overlay above the message list: switching chats is instant,
	// history fetches show this spinner instead of freezing/stale content.
	cv.msgOverlay = gtk.NewOverlay()
	cv.msgOverlay.SetChild(cv.scroller)
	cv.msgOverlay.SetVExpand(true)
	cv.msgOverlay.SetHExpand(true)

	spinWrap := gtk.NewBox(gtk.OrientationVertical, 10)
	spinWrap.SetHAlign(gtk.AlignCenter)
	spinWrap.SetVAlign(gtk.AlignCenter)
	spinWrap.SetVisible(false)
	cv.loadSpin = gtk.NewSpinner()
	cv.loadSpin.SetSizeRequest(28, 28)
	cv.loadSpin.Start()
	spinLbl := gtk.NewLabel("Memuat pesan…")
	spinLbl.AddCSSClass("dim-label")
	spinWrap.Append(cv.loadSpin)
	spinWrap.Append(spinLbl)
	cv.loadSpinW = spinWrap
	cv.msgOverlay.AddOverlay(spinWrap)

	// ---- Composer ----
	composer := cv.buildComposer()

	cv.content = gtk.NewBox(gtk.OrientationVertical, 0)
	cv.content.Append(header)
	cv.content.Append(cv.msgOverlay)
	cv.content.Append(composer)
	cv.content.SetVisible(false)
	cv.root.Append(cv.content)

	return cv
}

// Widget returns the root widget.
func (cv *Conversation) Widget() gtk.Widgetter { return cv.root }

// SetDeps wires collaborators and subscribes to store changes for the open
// chat only.
func (cv *Conversation) SetDeps(client *api.Client, st *store.Store, cache *media.Cache, toast func(string)) {
	cv.client = client
	cv.store = st
	cv.cache = cache
	cv.toast = toast

	cv.adj = cv.scroller.VAdjustment()
	cv.adj.ConnectValueChanged(func() { cv.onScroll() })

	st.Subscribe(func(c store.Change) {
		if c.ChatID == "" || c.ChatID != cv.chatID() {
			return
		}
		glib.IdleAdd(func() bool {
			switch c.Kind {
			case store.MessagesReset:
				cv.fullRebuild()
			case store.MessagesChanged:
				cv.syncMessages()
			}
			return false
		})
	})
}

// SetSidebarCallback wires the toggle-sidebar button.
func (cv *Conversation) SetSidebarCallback(cb func()) { cv.onSidebar = cb }

// SetHeaderCallback wires the identity-block click (opens the info panel).
func (cv *Conversation) SetHeaderCallback(cb func()) { cv.onHeaderClick = cb }

// SetSearchCallback wires the header search button (opens the search panel).
func (cv *Conversation) SetSearchCallback(cb func()) { cv.onSearchClick = cb }

func (cv *Conversation) chatID() string {
	if !cv.hasChat {
		return ""
	}
	return cv.current.ID
}

// OpenChat switches to the given chat instantly: the previous chat's rows
// are dropped immediately and, if history still has to be fetched, a
// spinner is shown instead of stale content.
func (cv *Conversation) OpenChat(c api.Chat) {
	cv.hasChat = true
	cv.current = c
	cv.store.SetActiveChat(c.ID)
	cv.store.MarkRead(c.ID)
	go cv.client.MarkRead(context.Background(), c.ID)

	name := displayName(c)
	cv.headerAvatar.SetText(initialsOf(name))
	cv.headerName.SetText(name)
	jid := c.ID
	if i := strings.IndexByte(jid, '@'); i > 0 && jid != name {
		cv.headerSub.SetText(jid)
	} else {
		cv.headerSub.SetText("")
	}

	cv.empty.SetVisible(false)
	cv.content.SetVisible(true)

	// Instant switch: never leave the old chat's bubbles on screen.
	cv.clearRows()
	cv.renderedIDs = nil

	page, _ := cv.store.Messages(c.ID)
	if len(page.Items) > 0 {
		cv.fullRebuild()
		cv.scrollToBottom()
		return
	}
	if !cv.fetchedOnce[c.ID] {
		cv.fetchedOnce[c.ID] = true
		cv.setLoading(true)
		cv.loadInitial(c.ID)
	}
}

// setLoading tracks in-flight initial loads; the spinner overlay is visible
// while any of them runs.
func (cv *Conversation) setLoading(on bool) {
	if on {
		cv.loading++
	} else if cv.loading > 0 {
		cv.loading--
	}
	visible := cv.loading > 0
	cv.loadSpinW.SetVisible(visible)
	if visible {
		cv.loadSpin.Start()
	} else {
		cv.loadSpin.Stop()
	}
}

// Clear returns to the empty state (used on logout).
func (cv *Conversation) Clear() {
	cv.hasChat = false
	cv.current = api.Chat{}
	cv.renderedIDs = nil
	cv.clearRows()
	cv.content.SetVisible(false)
	cv.empty.SetVisible(true)
}

// loadInitial fetches the newest page for chatID off-thread.
func (cv *Conversation) loadInitial(chatID string) {
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		msgs, err := cv.client.GetMessages(ctx, chatID, pageSize, 0)
		glib.IdleAdd(func() bool {
			cv.setLoading(false) // balance even if the user switched away
			if err != nil {
				log.Printf("conversation: initial load %s: %v", chatID, err)
				if cv.toast != nil {
					cv.toast("Gagal memuat pesan: " + err.Error())
				}
				return false
			}
			if cv.chatID() != chatID {
				return false
			}
			cv.store.ResetMessages(chatID, msgs, len(msgs) == pageSize)
			cv.scrollToBottom()
			return false
		})
	}()
}

// onScroll reacts to scrollbar movement: near-top triggers older-page loads,
// near-bottom triggers newer-page loads (after a context teleport).
func (cv *Conversation) onScroll() {
	if !cv.hasChat {
		return
	}
	id := cv.chatID()

	// --- older pages (top edge) ---
	if !cv.loadingOld {
		page, ok := cv.store.Messages(id)
		if ok && page.HasMore && len(page.Items) > 0 && cv.adj.Value() < loadOlderAt {
			cv.loadingOld = true
			before := page.Items[0].Timestamp
			go func() {
				ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
				defer cancel()
				msgs, err := cv.client.GetMessages(ctx, id, pageSize, before)
				glib.IdleAdd(func() bool {
					if err != nil {
						log.Printf("conversation: older page %s: %v", id, err)
					} else {
						cv.store.PrependOlder(id, msgs, len(msgs) == pageSize)
					}
					cv.loadingOld = false
					return false
				})
			}()
		}
	}

	// --- newer pages (bottom edge) ---
	if !cv.loadingNext && cv.pendingKeepAnchor == 0 {
		if cv.store.HasNext(id) && cv.nearBottom() {
			cv.loadingNext = true
			cv.pendingKeepAnchor++ // appended page must NOT pin the viewport
			go func() {
				ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
				defer cancel()
				msgs, err := cv.client.GetMessagesAfter(ctx, id, pageSize, cv.nextCursor())
				glib.IdleAdd(func() bool {
					if err != nil {
						log.Printf("conversation: newer page %s: %v", id, err)
					} else {
						cv.store.AppendNewer(id, msgs)
						if len(msgs) < pageSize {
							cv.store.SetHasNext(id, false)
						}
					}
					cv.loadingNext = false
					return false
				})
			}()
		}
	}
}

// nextCursor returns the timestamp of the newest loaded message.
func (cv *Conversation) nextCursor() int64 {
	page, _ := cv.store.Messages(cv.chatID())
	if n := len(page.Items); n > 0 {
		return page.Items[n-1].Timestamp
	}
	return 0
}

// clearRows removes every rendered row and tick ref.
func (cv *Conversation) clearRows() {
	for {
		row := cv.listBox.RowAtIndex(0)
		if row == nil {
			break
		}
		cv.listBox.Remove(row)
	}
	cv.ticks = make(map[string]*gtk.Label)
	cv.rows = make(map[string]*gtk.ListBoxRow)
}

// syncMessages applies incremental changes: fast-path appends, in-place tick
// updates, or a full rebuild for anything else.
func (cv *Conversation) syncMessages() {
	if !cv.hasChat {
		return
	}
	page, ok := cv.store.Messages(cv.chatID())
	if !ok {
		return
	}
	newIDs := make([]string, len(page.Items))
	for i, m := range page.Items {
		newIDs[i] = m.ID
	}

	// A newer-page REST fetch sets this: the appended rows must not yank the
	// viewport to the bottom while the user reads mid-history.
	keepAnchor := false
	if cv.pendingKeepAnchor > 0 {
		cv.pendingKeepAnchor--
		keepAnchor = true
	}

	if equalStrings(cv.renderedIDs, newIDs) {
		cv.updateTicks(page)
		return
	}

	if isPrefix(cv.renderedIDs, newIDs) && len(newIDs) > len(cv.renderedIDs) {
		atBottom := cv.nearBottom() && !keepAnchor

		var prevTS int64
		if n := len(cv.renderedIDs); n > 0 {
			prevTS = page.Items[n-1].Timestamp
		}
		start := len(cv.renderedIDs)
		if start == 0 {
			cv.fullRebuild()
			return
		}
		gapBefore := float64(0)
		if !atBottom {
			gapBefore = cv.bottomGap()
		}
		for i := start; i < len(newIDs); i++ {
			m := page.Items[i]
			if dayOf(prevTS) != dayOf(m.Timestamp) {
				cv.listBox.Append(dateSeparatorRow(prevTS, m.Timestamp))
			}
			row, tick := cv.buildRow(m)
			cv.listBox.Append(row)
			cv.rows[m.ID] = row
			if tick != nil {
				cv.ticks[m.ID] = tick
			}
			prevTS = m.Timestamp
		}
		cv.renderedIDs = newIDs
		if atBottom {
			cv.scrollToBottom()
		} else if gapBefore > 0 {
			glib.IdleAdd(func() bool {
				want := cv.adj.Upper() - gapBefore
				if want >= 0 {
					cv.adj.SetValue(want)
				}
				return false
			})
		}
		cv.updateTicks(page)
		return
	}

	cv.fullRebuild()
}

// fullRebuild re-renders the whole list, pinning to bottom when the user was
// already at/near it and preserving the distance-from-bottom otherwise.
func (cv *Conversation) fullRebuild() {
	page, ok := cv.store.Messages(cv.chatID())
	if !ok {
		return
	}
	newIDs := make([]string, len(page.Items))
	for i, m := range page.Items {
		newIDs[i] = m.ID
	}

	pinBottom := cv.nearBottom() || len(cv.renderedIDs) == 0
	gapBefore := cv.bottomGap()

	cv.clearRows()

	for i, m := range page.Items {
		var prevTS int64
		if i > 0 {
			prevTS = page.Items[i-1].Timestamp
		}
		if i == 0 || dayOf(prevTS) != dayOf(m.Timestamp) {
			cv.listBox.Append(dateSeparatorRow(prevTS, m.Timestamp))
		}
		row, tick := cv.buildRow(m)
		cv.listBox.Append(row)
		cv.rows[m.ID] = row
		if tick != nil {
			cv.ticks[m.ID] = tick
		}
	}
	cv.renderedIDs = newIDs

	if pinBottom {
		cv.scrollToBottom()
	} else if gapBefore > 0 {
		glib.IdleAdd(func() bool {
			// Re-apply the old gap between viewport bottom edge and content end.
			want := cv.adj.Upper() - gapBefore
			if want >= 0 {
				cv.adj.SetValue(want)
			}
			return false
		})
	}
	cv.updateTicks(page)
}

func (cv *Conversation) bottomGap() float64 {
	gap := cv.adj.Upper() - (cv.adj.Value() + cv.adj.PageSize())
	if gap < 0 {
		return 0
	}
	return gap
}

// nearBottom reports whether the viewport sits within nearBottomPx of the end.
func (cv *Conversation) nearBottom() bool {
	return cv.adj.Upper() <= cv.adj.PageSize()+1 ||
		cv.adj.Value()+cv.adj.PageSize() >= cv.adj.Upper()-nearBottomPx
}

// scrollToBottom jumps to the newest message after layout settles.
func (cv *Conversation) scrollToBottom() {
	glib.IdleAdd(func() bool {
		cv.adj.SetValue(cv.adj.Upper())
		// A second pass after allocations settle fully.
		glib.IdleAdd(func() bool {
			cv.adj.SetValue(cv.adj.Upper())
			return false
		})
		return false
	})
}

// updateTicks refreshes outgoing status glyphs without touching rows.
func (cv *Conversation) updateTicks(page store.Page) {
	for _, m := range page.Items {
		lbl, ok := cv.ticks[m.ID]
		if !ok {
			continue
		}
		lbl.SetText(tickGlyph(m.Status))
		resetCSS(lbl, []string{"dim-label", "tick-read", "tick-failed"})
		lbl.AddCSSClass(classForTick(m.Status))
	}
}

// TeleportTo loads a context window around msgID (search hit), replaces the
// rendered page, then scrolls to and briefly highlights the target message.
func (cv *Conversation) TeleportTo(msgID string) {
	if !cv.hasChat {
		return
	}
	id := cv.current.ID
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
		defer cancel()
		msgs, err := cv.client.GetMessageContext(ctx, id, msgID, pageSize)
		glib.IdleAdd(func() bool {
			if err != nil {
				if cv.toast != nil {
					cv.toast("Gagal memuat konteks pesan: " + err.Error())
				}
				return false
			}
			if cv.chatID() != id {
				return false
			}
			// Backend returns [older DESC] + target + [newer ASC]; normalize.
			sort.SliceStable(msgs, func(a, b int) bool {
				return msgs[a].Timestamp < msgs[b].Timestamp
			})
			cv.fetchedOnce[id] = true
			// A saturated window (limit+1 incl. target) means both sides
			// likely continue beyond what we received.
			sidesFull := len(msgs) >= pageSize
			cv.store.ResetContext(id, msgs, sidesFull, sidesFull)
			cv.scrollToMessage(msgID)
			return false
		})
	}()
}

// scrollToMessage pins the target row near the top of the viewport and
// flashes it. Must run after the rebuild has been laid out.
func (cv *Conversation) scrollToMessage(msgID string) {
	glib.IdleAdd(func() bool {
		row := cv.rows[msgID]
		if row == nil {
			return false
		}
		offset := float64(0)
		for i := 0; ; i++ {
			r := cv.listBox.RowAtIndex(i)
			if r == nil {
				break
			}
			if r == row {
				break
			}
			offset += float64(r.Height())
		}
		cv.adj.SetValue(offset - 12)

		row.AddCSSClass("msg-highlight")
		glib.TimeoutSecondsAdd(2, func() bool {
			row.RemoveCSSClass("msg-highlight")
			return false
		})
		return false
	})
}

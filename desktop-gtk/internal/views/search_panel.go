package views

import (
	"context"
	"strings"
	"time"

	"github.com/diamondburned/gotk4/pkg/glib/v2"
	"github.com/diamondburned/gotk4/pkg/gtk/v4"
	pango "github.com/diamondburned/gotk4/pkg/pango"

	"wa-bot-desktop/internal/api"
)

const (
	searchPageSize    = 50
	searchDebounceSec = 1 // glib whole-second debounce
)

// SearchPanel is the right-side message search sheet for the open chat:
// debounced live query + result list; picking a result teleports the
// conversation to that message (web ChatSearchSheet parity).
type SearchPanel struct {
	client *api.Client
	toast  func(string)

	root  *gtk.Box
	entry *gtk.SearchEntry
	list  *gtk.ListBox

	hint     *gtk.Label // pre-query / no-results text
	spinWrap *gtk.Box
	results  []api.Message
	chatID   string
	query    string
	loading  bool
	debounce glib.SourceHandle

	onClose func()
	onPick  func(msgID string)
}

// NewSearchPanel constructs the search panel widgets.
func NewSearchPanel() *SearchPanel {
	p := &SearchPanel{}

	p.root = gtk.NewBox(gtk.OrientationVertical, 0)

	header := gtk.NewBox(gtk.OrientationHorizontal, 8)
	header.SetMarginTop(10)
	header.SetMarginBottom(6)
	header.SetMarginStart(12)
	header.SetMarginEnd(10)
	title := gtk.NewLabel("Cari Pesan")
	title.AddCSSClass("title-3")
	title.SetHExpand(true)
	title.SetXAlign(0)
	header.Append(title)
	closeBtn := gtk.NewButtonFromIconName("window-close-symbolic")
	closeBtn.SetTooltipText("Tutup")
	closeBtn.ConnectClicked(func() {
		if p.onClose != nil {
			p.onClose()
		}
	})
	header.Append(closeBtn)
	p.root.Append(header)

	searchRow := gtk.NewBox(gtk.OrientationHorizontal, 8)
	searchRow.SetMarginTop(4)
	searchRow.SetMarginBottom(8)
	searchRow.SetMarginStart(12)
	searchRow.SetMarginEnd(12)
	p.entry = gtk.NewSearchEntry()
	p.entry.SetPlaceholderText("Cari dalam chat ini…")
	p.entry.SetHExpand(true)
	searchRow.Append(p.entry)
	p.root.Append(searchRow)

	p.hint = gtk.NewLabel("Ketik kata kunci untuk mencari pesan.")
	p.hint.AddCSSClass("dim-label")
	p.hint.SetHAlign(gtk.AlignCenter)
	p.hint.SetVAlign(gtk.AlignCenter)
	p.hint.SetVExpand(true)
	p.root.Append(p.hint)

	p.spinWrap = gtk.NewBox(gtk.OrientationVertical, 8)
	p.spinWrap.SetHAlign(gtk.AlignCenter)
	p.spinWrap.SetMarginTop(24)
	spin := gtk.NewSpinner()
	spin.SetSizeRequest(26, 26)
	spin.Start()
	lbl := gtk.NewLabel("Mencari…")
	lbl.AddCSSClass("dim-label")
	p.spinWrap.Append(spin)
	p.spinWrap.Append(lbl)
	p.spinWrap.SetVisible(false)
	p.root.Append(p.spinWrap)

	p.list = gtk.NewListBox()
	p.list.SetSelectionMode(gtk.SelectionNone)
	p.list.SetShowSeparators(true)
	p.list.SetMarginStart(10)
	p.list.SetMarginEnd(10)
	p.list.SetMarginBottom(10)
	p.list.SetHExpand(true)

	scroller := gtk.NewScrolledWindow()
	scroller.SetPolicy(gtk.PolicyNever, gtk.PolicyAutomatic)
	scroller.SetChild(p.list)
	scroller.SetVExpand(true)
	scroller.SetHExpand(true)
	p.root.Append(scroller)

	var debouncing bool
	p.entry.ConnectSearchChanged(func() {
		if debouncing {
			return
		}
		debouncing = true
		p.debounce = glib.TimeoutSecondsAdd(0, func() bool { // coalesce per second
			debouncing = false
			p.runSearch(strings.TrimSpace(p.entry.Text()))
			return false
		})
	})
	p.entry.ConnectActivate(func() {
		p.runSearch(strings.TrimSpace(p.entry.Text()))
	})

	return p
}

// Widget returns the root widget.
func (sp *SearchPanel) Widget() gtk.Widgetter { return sp.root }

// SetDeps wires collaborators.
func (sp *SearchPanel) SetDeps(client *api.Client, toast func(string)) {
	sp.client = client
	sp.toast = toast
}

// SetCloseCallback wires the X button.
func (sp *SearchPanel) SetCloseCallback(cb func()) { sp.onClose = cb }

// SetPickCallback fires with the selected message ID.
func (sp *SearchPanel) SetPickCallback(cb func(msgID string)) { sp.onPick = cb }

// OpenFor resets the panel for a chat and focuses the entry.
func (sp *SearchPanel) OpenFor(chatID string) {
	if sp.chatID != chatID {
		sp.chatID = chatID
		sp.clearResults()
		sp.hint.SetText("Ketik kata kunci untuk mencari pesan.")
		sp.hint.SetVisible(true)
	}
	sp.entry.GrabFocus()
}

func (sp *SearchPanel) clearResults() {
	for {
		row := sp.list.RowAtIndex(0)
		if row == nil {
			break
		}
		sp.list.Remove(row)
	}
	sp.results = nil
}

// runSearch executes the (debounced) query against the backend.
func (sp *SearchPanel) runSearch(query string) {
	if sp.client == nil || sp.chatID == "" {
		return
	}
	if sp.debounce != 0 {
		glib.SourceRemove(sp.debounce)
		sp.debounce = 0
	}
	if query == "" {
		sp.clearResults()
		sp.loading = false
		sp.spinWrap.SetVisible(false)
		sp.list.SetVisible(false)
		sp.hint.SetText("Ketik kata kunci untuk mencari pesan.")
		sp.hint.SetVisible(true)
		return
	}

	sp.query = query
	sp.loading = true
	sp.spinWrap.SetVisible(true)
	sp.hint.SetVisible(false)
	sp.list.SetVisible(false)

	chatID := sp.chatID
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
		defer cancel()
		msgs, err := sp.client.SearchMessages(ctx, chatID, query, searchPageSize)
		glib.IdleAdd(func() bool {
			// Drop stale responses (user typed again / switched chat).
			if !sp.loading || sp.chatID != chatID || strings.TrimSpace(sp.entry.Text()) != query {
				return false
			}
			sp.loading = false
			sp.spinWrap.SetVisible(false)
			sp.clearResults()

			if err != nil {
				if sp.toast != nil {
					sp.toast("Pencarian gagal: " + err.Error())
				}
				sp.hint.SetText("Pencarian gagal.")
				sp.hint.SetVisible(true)
				return false
			}

			sp.results = msgs
			if len(msgs) == 0 {
				sp.hint.SetText("Tidak ada hasil untuk \"" + query + "\".")
				sp.hint.SetVisible(true)
				return false
			}
			for _, m := range msgs {
				sp.appendResult(m)
			}
			sp.list.SetVisible(true)
			return false
		})
	}()
}

// appendResult renders one hit row: sender, snippet, date.
func (sp *SearchPanel) appendResult(m api.Message) {
	row := gtk.NewListBoxRow()
	row.SetActivatable(true)
	row.SetName(m.ID)

	box := gtk.NewBox(gtk.OrientationVertical, 2)
	box.SetMarginTop(6)
	box.SetMarginBottom(6)

	sender := firstNonEmptyStr(m.SenderName, shortJID(m.From))
	if sender == "me" {
		sender = "Anda"
	}
	top := gtk.NewBox(gtk.OrientationHorizontal, 6)
	who := gtk.NewLabel(sender)
	who.AddCSSClass("heading")
	who.SetXAlign(0)
	who.SetHExpand(true)
	who.SetEllipsize(pango.EllipsizeEnd)
	top.Append(who)
	ts := gtk.NewLabel(dayLabel(0, m.Timestamp))
	ts.AddCSSClass("caption")
	ts.AddCSSClass("dim-label")
	top.Append(ts)
	box.Append(top)

	snippet := gtk.NewLabel(oneLine(m.Content))
	if strings.TrimSpace(snippet.Text()) == "" {
		snippet.SetText("[" + m.Type + "]")
	}
	snippet.AddCSSClass("dim-label")
	snippet.SetEllipsize(pango.EllipsizeEnd)
	snippet.SetXAlign(0)
	snippet.SetMaxWidthChars(38)
	box.Append(snippet)

	row.SetChild(box)
	row.SetTooltipText("Klik untuk lompat ke pesan")
	sp.list.Append(row)
}

// wireActivation must be called once pane callbacks are known.
func (sp *SearchPanel) wireActivation(onPick func(msgID string)) {
	sp.list.ConnectRowActivated(func(row *gtk.ListBoxRow) {
		if id := row.Name(); id != "" && onPick != nil {
			onPick(id)
		}
	})
}

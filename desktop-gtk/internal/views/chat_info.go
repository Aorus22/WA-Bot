package views

import (
	"context"
	"regexp"
	"strings"
	"time"

	"github.com/diamondburned/gotk4/pkg/gdk/v4"
	"github.com/diamondburned/gotk4/pkg/glib/v2"
	"github.com/diamondburned/gotk4/pkg/gtk/v4"
	pango "github.com/diamondburned/gotk4/pkg/pango"

	"wa-bot-desktop/internal/api"
	"wa-bot-desktop/internal/media"
	"wa-bot-desktop/internal/openx"

	"github.com/diamondburned/gotk4-adwaita/pkg/adw"
)

const (
	infoPageSize     = 30
	infoTileSize     = 92 // square media tile edge (3 columns fit the min pane width)
	infoBottomMargin = 90 // px above scroll end that triggers the next page
)

var linkPattern = regexp.MustCompile(`https?://[^\s]+`)

// ChatInfo is the right-side panel opened from the conversation header:
// identity block plus Media/Docs/Links tabs, mirroring the web UI's sheet.
type ChatInfo struct {
	client *api.Client
	cache  *media.Cache
	toast  func(string)

	root    *gtk.Box
	avatar  *adw.Avatar
	nameLbl *gtk.Label
	jidLbl  *gtk.Label

	stack    *gtk.Stack
	switcher *gtk.StackSwitcher

	mediaTab *infoMediaTab
	docsTab  *infoListTab
	linksTab *infoListTab

	current api.Chat
	hasChat bool
	onClose func()
}

// NewChatInfo constructs the info panel widgets.
func NewChatInfo(client *api.Client, cache *media.Cache, toast func(string)) *ChatInfo {
	ci := &ChatInfo{client: client, cache: cache, toast: toast}

	ci.root = gtk.NewBox(gtk.OrientationVertical, 0)

	// Header row: title + close button.
	header := gtk.NewBox(gtk.OrientationHorizontal, 8)
	header.SetMarginTop(10)
	header.SetMarginBottom(6)
	header.SetMarginStart(12)
	header.SetMarginEnd(10)
	title := gtk.NewLabel("Chat Info")
	title.AddCSSClass("title-3")
	title.SetHExpand(true)
	title.SetXAlign(0)
	header.Append(title)
	closeBtn := gtk.NewButtonFromIconName("window-close-symbolic")
	closeBtn.SetTooltipText("Tutup")
	closeBtn.ConnectClicked(func() {
		if ci.onClose != nil {
			ci.onClose()
		}
	})
	header.Append(closeBtn)
	ci.root.Append(header)

	// Identity block: avatar / name / JID.
	ident := gtk.NewBox(gtk.OrientationVertical, 4)
	ident.SetHAlign(gtk.AlignCenter)
	ident.SetMarginTop(8)
	ident.SetMarginBottom(12)
	ci.avatar = adw.NewAvatar(56, "?", false)
	ci.avatar.SetSizeRequest(56, 56)
	ident.Append(ci.avatar)
	ci.nameLbl = gtk.NewLabel("")
	ci.nameLbl.AddCSSClass("heading")
	ci.nameLbl.SetMaxWidthChars(28)
	ci.nameLbl.SetEllipsize(pango.EllipsizeEnd)
	ident.Append(ci.nameLbl)
	ci.jidLbl = gtk.NewLabel("")
	ci.jidLbl.AddCSSClass("caption")
	ci.jidLbl.AddCSSClass("dim-label")
	ci.jidLbl.SetMaxWidthChars(30)
	ci.jidLbl.SetEllipsize(pango.EllipsizeEnd)
	ident.Append(ci.jidLbl)
	ci.root.Append(ident)

	// Tabs.
	ci.mediaTab = newInfoMediaTab(ci)
	ci.docsTab = newInfoListTab(ci, "docs")
	ci.linksTab = newInfoListTab(ci, "links")

	ci.switcher = gtk.NewStackSwitcher()
	ci.switcher.SetHAlign(gtk.AlignCenter)
	ci.switcher.SetMarginTop(4)
	ci.switcher.SetMarginBottom(6)
	ci.root.Append(ci.switcher)

	ci.stack = gtk.NewStack()
	ci.stack.SetVExpand(true)
	ci.stack.SetHExpand(true)
	ci.stack.AddTitled(ci.mediaTab.wrap(), "media", "Media")
	ci.stack.AddTitled(ci.docsTab.wrap(), "docs", "Docs")
	ci.stack.AddTitled(ci.linksTab.wrap(), "links", "Links")
	// Without this link the switcher renders completely empty.
	ci.switcher.SetStack(ci.stack)
	ci.root.Append(ci.stack)

	return ci
}

// Widget returns the root widget.
func (ci *ChatInfo) Widget() gtk.Widgetter { return ci.root }

// SetDeps wires collaborators (called after construction by ChatsPane).
func (ci *ChatInfo) SetDeps(client *api.Client, cache *media.Cache, toast func(string)) {
	ci.client = client
	ci.cache = cache
	ci.toast = toast
}

// SetCloseCallback wires the X button.
func (ci *ChatInfo) SetCloseCallback(cb func()) { ci.onClose = cb }

// SetChat updates the identity block; when fetch is true all three sections
// are reset and refetched (web parity fetches everything at open time).
func (ci *ChatInfo) SetChat(c api.Chat, fetch bool) {
	ci.current = c
	ci.hasChat = true

	name := displayName(c)
	ci.avatar.SetText(initialsOf(name))
	ci.nameLbl.SetText(name)
	ci.jidLbl.SetText(c.ID)
	if tex := ci.cache.MemoryTexture(ci.client.AvatarURL(c.ID)); tex != nil {
		ci.avatar.SetCustomImage(tex)
	} else {
		url := ci.client.AvatarURL(c.ID)
		ci.cache.ImageAsync(url, func(tex *gdk.Texture, err error) {
			if err == nil && tex != nil && ci.hasChat && ci.current.ID == c.ID {
				ci.avatar.SetCustomImage(tex)
			}
		})
	}

	if !fetch {
		return
	}
	ci.mediaTab.reset()
	ci.docsTab.reset()
	ci.linksTab.reset()
	ci.loadMore(ci.mediaTab, "media")
	ci.loadMore(ci.docsTab, "docs")
	ci.loadMore(ci.linksTab, "links")
}

// Reset clears everything (chat closed / logout).
func (ci *ChatInfo) Reset() {
	ci.hasChat = false
	ci.current = api.Chat{}
	ci.mediaTab.reset()
	ci.docsTab.reset()
	ci.linksTab.reset()
}

func (ci *ChatInfo) reportError(msg string) {
	if ci.toast != nil {
		ci.toast(msg)
	}
}

// loadMore fetches the next page of a section and appends it on the main
// thread once it arrives.
func (ci *ChatInfo) loadMore(tab interface {
	bussy() bool
	setBussy(bool)
	lastTimestamp() int64
	appendPage([]api.Message, bool)
}, section string,
) {
	if tab.bussy() || !ci.hasChat {
		return
	}
	tab.setBussy(true)

	var before int64
	if ts := tab.lastTimestamp(); ts > 0 {
		before = ts
	}
	chatID := ci.current.ID
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
		defer cancel()
		var (
			msgs []api.Message
			err  error
		)
		switch section {
		case "media":
			msgs, err = ci.client.GetChatMedia(ctx, chatID, infoPageSize, before)
		case "docs":
			msgs, err = ci.client.GetChatDocs(ctx, chatID, infoPageSize, before)
		default:
			msgs, err = ci.client.GetChatLinks(ctx, chatID, infoPageSize, before)
		}
		glib.IdleAdd(func() bool {
			tab.setBussy(false)
			if err != nil {
				ci.reportError("Gagal memuat " + section + ": " + err.Error())
				return false
			}
			if ci.hasChat && ci.current.ID == chatID {
				tab.appendPage(msgs, len(msgs) == infoPageSize)
			}
			return false
		})
	}()
}

// --- shared tab plumbing ---

type infoTabBase struct {
	ci       *ChatInfo
	scroller *gtk.ScrolledWindow
	adj      *gtk.Adjustment
	overlay  *gtk.Overlay

	spinnerWrap *gtk.Box // full-area spinner ("Loading…")
	footSpin    *gtk.Spinner
	empty       *gtk.Label
	emptyIcon   string
	emptyText   string

	items   []api.Message
	hasMore bool
	loading bool
	started bool
}

// buildOverlay assembles scroller + spinner/empty overlays and hooks the
// bottom-edge infinite-scroll trigger through loadMoreFn.
func (t *infoTabBase) buildOverlay(emptyIcon, emptyText string, loadMoreFn func()) {
	t.emptyIcon = emptyIcon
	t.emptyText = emptyText

	t.spinnerWrap = gtk.NewBox(gtk.OrientationVertical, 8)
	t.spinnerWrap.SetHAlign(gtk.AlignCenter)
	t.spinnerWrap.SetVAlign(gtk.AlignCenter)
	spin := gtk.NewSpinner()
	spin.SetSizeRequest(26, 26)
	spin.Start()
	lbl := gtk.NewLabel("Memuat…")
	lbl.AddCSSClass("dim-label")
	t.spinnerWrap.Append(spin)
	t.spinnerWrap.Append(lbl)
	t.spinnerWrap.SetVisible(false)

	t.footSpin = gtk.NewSpinner()
	t.footSpin.SetSizeRequest(20, 20)
	t.footSpin.SetMarginTop(6)
	t.footSpin.SetMarginBottom(10)
	t.footSpin.SetHAlign(gtk.AlignCenter)
	t.footSpin.SetVisible(false)

	t.empty = gtk.NewLabel(emptyText)
	t.empty.AddCSSClass("dim-label")
	t.empty.SetHAlign(gtk.AlignCenter)
	t.empty.SetVAlign(gtk.AlignCenter)

	t.overlay = gtk.NewOverlay()
	t.overlay.SetChild(t.scroller)
	t.overlay.AddOverlay(t.spinnerWrap)
	t.overlay.AddOverlay(t.empty)

	t.adj = t.scroller.VAdjustment()
	t.adj.ConnectValueChanged(func() {
		if !t.hasMore || t.loading {
			return
		}
		if t.adj.Value()+t.adj.PageSize() >= t.adj.Upper()-infoBottomMargin {
			loadMoreFn()
		}
	})
}

// syncOverlays refreshes spinner/empty visibility from current state.
func (t *infoTabBase) syncOverlays() {
	firstLoad := t.loading && len(t.items) == 0
	paging := t.loading && len(t.items) > 0
	t.spinnerWrap.SetVisible(firstLoad)
	t.footSpin.SetVisible(paging)
	t.empty.SetVisible(!t.loading && t.started && len(t.items) == 0)
}

// --- media tab ---

type infoMediaTab struct {
	infoTabBase
	flow     *gtk.FlowBox
	textures map[string]*gdk.Texture // message id -> loaded tile texture
}

func newInfoMediaTab(ci *ChatInfo) *infoMediaTab {
	t := &infoMediaTab{infoTabBase: infoTabBase{ci: ci}, textures: map[string]*gdk.Texture{}}

	t.flow = gtk.NewFlowBox()
	t.flow.SetMaxChildrenPerLine(3)
	t.flow.SetMinChildrenPerLine(3)
	t.flow.SetHomogeneous(true)
	t.flow.SetSelectionMode(gtk.SelectionNone)
	t.flow.SetColumnSpacing(6)
	t.flow.SetRowSpacing(6)
	t.flow.SetMarginStart(10)
	t.flow.SetMarginEnd(10)
	t.flow.SetMarginTop(6)
	t.flow.SetMarginBottom(6)
	t.flow.SetHExpand(true)

	scroller := gtk.NewScrolledWindow()
	scroller.SetPolicy(gtk.PolicyNever, gtk.PolicyAutomatic)
	scroller.SetChild(t.flow)
	scroller.SetVExpand(true)
	scroller.SetHExpand(true)
	t.scroller = scroller
	t.buildOverlay("image-x-generic-symbolic", "Belum ada media.", func() {
		ci.loadMore(t, "media")
	})

	t.flow.ConnectChildActivated(func(child *gtk.FlowBoxChild) {
		idx := int(child.Index())
		if idx < 0 || idx >= len(t.items) {
			return
		}
		mItem := t.items[idx]
		rawURL := ci.client.MediaURL(mItem.MediaURL)
		if mItem.Type == "image" {
			if tex := t.textures[mItem.ID]; tex != nil {
				ShowImageViewer(MainWindow, tex)
				return
			}
		}
		go openCachedFile(ci.cache, rawURL)
	})
	return t
}

func (t *infoMediaTab) wrap() gtk.Widgetter {
	box := gtk.NewBox(gtk.OrientationVertical, 0)
	box.Append(t.overlay)
	box.Append(t.footSpin)
	return box
}

func (t *infoMediaTab) bussy() bool     { return t.loading }
func (t *infoMediaTab) setBussy(v bool) { t.loading = v; t.syncOverlays() }
func (t *infoMediaTab) lastTimestamp() int64 {
	if n := len(t.items); n > 0 {
		return t.items[n-1].Timestamp
	}
	return 0
}

func (t *infoMediaTab) reset() {
	t.items = nil
	t.hasMore = false
	t.loading = false
	t.started = false
	t.textures = map[string]*gdk.Texture{}
	for {
		child := t.flow.ChildAtIndex(0)
		if child == nil {
			break
		}
		t.flow.Remove(child)
	}
	t.syncOverlays()
}

func (t *infoMediaTab) appendPage(msgs []api.Message, hasMore bool) {
	t.items = append(t.items, msgs...)
	t.hasMore = hasMore
	for _, m := range msgs {
		tile := t.buildTile(m)
		t.flow.Append(tile)
	}
	t.syncOverlays()
}

func (t *infoMediaTab) buildTile(m api.Message) gtk.Widgetter {
	frame := gtk.NewFrame("")
	// Fixed square, centered in its FlowBox cell — never stretches into a
	// rectangle regardless of pane width.
	frame.SetSizeRequest(infoTileSize, infoTileSize)
	frame.SetHAlign(gtk.AlignCenter)
	frame.SetVAlign(gtk.AlignCenter)

	if m.Type != "image" || m.MediaURL == "" {
		box := gtk.NewBox(gtk.OrientationVertical, 4)
		box.SetHAlign(gtk.AlignCenter)
		box.SetVAlign(gtk.AlignCenter)
		icon := gtk.NewImageFromIconName("video-x-generic-symbolic")
		icon.SetPixelSize(34)
		box.Append(icon)
		frame.SetChild(box)
		return frame
	}

	pic := gtk.NewPictureForPaintable(nil)
	pic.SetKeepAspectRatio(true)
	pic.SetCanShrink(true)
	pic.SetHExpand(true)
	pic.SetVExpand(true)
	frame.SetChild(pic)

	rawURL := t.ci.client.MediaURL(m.MediaURL)
	chatID := t.ci.current.ID
	if tex := t.ci.cache.MemoryTexture(rawURL); tex != nil {
		t.textures[m.ID] = tex
		pic.SetPaintable(tex)
	} else {
		t.ci.cache.ImageAsync(rawURL, func(tex *gdk.Texture, err error) {
			if err != nil || tex == nil || t.ci.current.ID != chatID {
				return
			}
			t.textures[m.ID] = tex
			pic.SetPaintable(tex)
		})
	}
	return frame
}

// --- docs / links list tab ---

type infoListTab struct {
	infoTabBase
	list     *gtk.ListBox
	kind     string // "docs" | "links"
	linkURLs []string
}

func newInfoListTab(ci *ChatInfo, kind string) *infoListTab {
	t := &infoListTab{infoTabBase: infoTabBase{ci: ci}, kind: kind}

	t.list = gtk.NewListBox()
	t.list.SetSelectionMode(gtk.SelectionNone)
	t.list.SetShowSeparators(true)
	t.list.SetMarginStart(10)
	t.list.SetMarginEnd(10)
	t.list.SetMarginTop(6)
	t.list.SetMarginBottom(6)
	t.list.SetHExpand(true)

	scroller := gtk.NewScrolledWindow()
	scroller.SetPolicy(gtk.PolicyNever, gtk.PolicyAutomatic)
	scroller.SetChild(t.list)
	scroller.SetVExpand(true)
	scroller.SetHExpand(true)
	t.scroller = scroller

	icon, emptyText := "text-x-generic-symbolic", "Belum ada dokumen."
	if kind == "links" {
		icon, emptyText = "web-browser-symbolic", "Belum ada tautan."
	}
	t.buildOverlay(icon, emptyText, func() { ci.loadMore(t, kind) })

	t.list.ConnectRowActivated(func(row *gtk.ListBoxRow) {
		target := row.Name()
		if target == "" {
			return
		}
		if t.kind == "links" {
			go openx.URL(target)
			return
		}
		rawURL := target // docs: row name carries the media URL
		go openCachedFile(ci.cache, rawURL)
	})
	return t
}

func (t *infoListTab) wrap() gtk.Widgetter {
	box := gtk.NewBox(gtk.OrientationVertical, 0)
	box.Append(t.overlay)
	box.Append(t.footSpin)
	return box
}

func (t *infoListTab) bussy() bool     { return t.loading }
func (t *infoListTab) setBussy(v bool) { t.loading = v; t.syncOverlays() }

func (t *infoListTab) lastTimestamp() int64 {
	if n := len(t.items); n > 0 {
		return t.items[n-1].Timestamp
	}
	return 0
}

func (t *infoListTab) reset() {
	t.items = nil
	t.linkURLs = nil
	t.hasMore = false
	t.loading = false
	t.started = false
	for {
		row := t.list.RowAtIndex(0)
		if row == nil {
			break
		}
		t.list.Remove(row)
	}
	t.syncOverlays()
}

func (t *infoListTab) appendPage(msgs []api.Message, hasMore bool) {
	t.items = append(t.items, msgs...)
	t.hasMore = hasMore
	for _, m := range msgs {
		if t.kind == "links" {
			for _, u := range linkPattern.FindAllString(m.Content, -1) {
				u = strings.TrimRight(u, ").,;\"'")
				t.appendRow(u, dayLabel(0, m.Timestamp))
			}
			continue
		}
		rawURL := ""
		if m.MediaURL != "" {
			rawURL = t.ci.client.MediaURL(m.MediaURL)
		}
		t.appendRow(rawURL, dayLabel(0, m.Timestamp))
	}
	t.syncOverlays()
}

// appendRow adds one row; for docs the media URL travels in rowName, for
// links the target URL itself does.
func (t *infoListTab) appendRow(rowName, dateText string) {
	row := gtk.NewListBoxRow()
	row.SetActivatable(true)

	box := gtk.NewBox(gtk.OrientationHorizontal, 10)
	box.SetMarginTop(5)
	box.SetMarginBottom(5)

	iconBox := gtk.NewBox(gtk.OrientationVertical, 0)
	iconBox.SetHAlign(gtk.AlignCenter)
	iconBox.SetVAlign(gtk.AlignCenter)
	iconBox.SetSizeRequest(38, 38)
	iconBox.AddCSSClass("bubble")
	icon := gtk.NewImageFromIconName(t.emptyIcon)
	icon.SetPixelSize(20)
	iconBox.Append(icon)
	box.Append(iconBox)

	col := gtk.NewBox(gtk.OrientationVertical, 1)
	col.SetVAlign(gtk.AlignCenter)
	title := gtk.NewLabel("Document")
	subtitle := dateText
	if t.kind == "links" {
		title = gtk.NewLabel(shortDisplayURL(rowName))
		subtitle = dateText
	}
	title.SetEllipsize(3) // PANGO_ELLIPSIZE_END
	title.SetXAlign(0)
	title.SetMaxWidthChars(32)
	col.Append(title)
	sub := gtk.NewLabel(subtitle)
	sub.AddCSSClass("caption")
	sub.AddCSSClass("dim-label")
	sub.SetXAlign(0)
	col.Append(sub)
	box.Append(col)

	row.SetChild(box)
	row.SetName(rowName)
	t.list.Append(row)
}

// openCachedFile downloads URL into cache then opens it externally.
func openCachedFile(cache *media.Cache, rawURL string) {
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	path, err := cache.Get(ctx, rawURL)
	if err != nil {
		return
	}
	_ = openx.File(path)
}

// shortDisplayURL strips scheme + trailing slash so long URLs stay readable.
func shortDisplayURL(rawURL string) string {
	s := rawURL
	s = strings.TrimPrefix(s, "https://")
	s = strings.TrimPrefix(s, "http://")
	return s
}

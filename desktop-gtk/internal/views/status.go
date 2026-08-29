package views

import (
	"context"
	"fmt"
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
)

// Status is the top-level Status (stories) page: a status list on the left
// ("Status Saya" + recent updates) and an in-pane viewer on the right,
// mirroring the official desktop client layout.
type Status struct {
	root   *adw.OverlaySplitView
	list   *gtk.ListBox
	status *gtk.Label

	ownSub *gtk.Label

	// Right pane.
	right      *gtk.Box
	emptyState *gtk.Box
	viewer     *gtk.Box
	viewerSlot *gtk.Box
	progress   *gtk.Box
	viewerWho  *gtk.Label
	viewerSub  *gtk.Label
	viewerOwn  bool
	current    *api.StatusGroup
	viewerPos  int
	navPrev    *gtk.Button
	navNext    *gtk.Button
	autoAdv    glib.SourceHandle

	client *api.Client
	cache  *media.Cache
	toast  func(string)

	groups  []api.StatusGroup
	loading bool
}

// NewStatus constructs the Status page widgets.
func NewStatus() *Status {
	s := &Status{}

	// ─── Left: status list ───
	left := gtk.NewBox(gtk.OrientationVertical, 0)
	left.SetSizeRequest(300, -1)

	header := gtk.NewBox(gtk.OrientationHorizontal, 8)
	header.SetMarginTop(10)
	header.SetMarginBottom(6)
	header.SetMarginStart(12)
	header.SetMarginEnd(10)
	title := gtk.NewLabel("Status")
	title.AddCSSClass("title-2")
	title.SetXAlign(0)
	title.SetHExpand(true)
	header.Append(title)

	composeBtn := gtk.NewMenuButton()
	composeBtn.SetIconName("list-add-symbolic")
	composeBtn.SetTooltipText("Buat status")
	menu := gtk.NewPopover()
	menuBox := gtk.NewBox(gtk.OrientationVertical, 0)
	menuBox.SetMarginTop(4)
	menuBox.SetMarginBottom(4)
	addItem := func(label, icon string, fn func()) {
		btn := gtk.NewButton()
		content := gtk.NewBox(gtk.OrientationHorizontal, 8)
		content.SetMarginTop(4)
		content.SetMarginBottom(4)
		content.SetMarginStart(8)
		content.SetMarginEnd(8)
		content.Append(gtk.NewImageFromIconName(icon))
		content.Append(gtk.NewLabel(label))
		btn.SetChild(content)
		btn.AddCSSClass("flat")
		btn.ConnectClicked(func() {
			menu.Popdown()
			fn()
		})
		menuBox.Append(btn)
	}
	addItem("Status Teks", "text-x-generic-symbolic", s.composeText)
	addItem("Foto/Gambar", "image-x-generic-symbolic", func() { s.composeMedia("image") })
	addItem("Video", "video-x-generic-symbolic", func() { s.composeMedia("video") })
	menu.SetChild(menuBox)
	composeBtn.SetPopover(menu)
	header.Append(composeBtn)
	left.Append(header)

	s.status = gtk.NewLabel("Memuat status…")
	s.status.AddCSSClass("dim-label")
	s.status.SetMarginTop(12)
	s.status.SetMarginBottom(12)
	s.status.SetHAlign(gtk.AlignCenter)
	left.Append(s.status)

	s.list = gtk.NewListBox()
	s.list.SetSelectionMode(gtk.SelectionNone)
	s.list.SetShowSeparators(true)

	scroller := gtk.NewScrolledWindow()
	scroller.SetPolicy(gtk.PolicyNever, gtk.PolicyAutomatic)
	scroller.SetChild(s.list)
	scroller.SetVExpand(true)
	scroller.SetHExpand(true)
	left.Append(scroller)

	// ─── Right: empty state / viewer ───
	s.right = gtk.NewBox(gtk.OrientationVertical, 0)
	s.right.SetHExpand(true)

	s.emptyState = gtk.NewBox(gtk.OrientationVertical, 12)
	s.emptyState.SetHAlign(gtk.AlignCenter)
	s.emptyState.SetVAlign(gtk.AlignCenter)
	s.emptyState.SetVExpand(true)
	emptyIcon := gtk.NewImageFromIconName("user-available-symbolic")
	emptyIcon.SetPixelSize(56)
	emptyIcon.AddCSSClass("dim-label")
	s.emptyState.Append(emptyIcon)
	emptyTitle := gtk.NewLabel("Bagikan status")
	emptyTitle.AddCSSClass("title-1")
	s.emptyState.Append(emptyTitle)
	emptySub := gtk.NewLabel("Bagikan foto, video, dan teks yang hilang setelah 24 jam.")
	emptySub.AddCSSClass("dim-label")
	s.emptyState.Append(emptySub)
	s.right.Append(s.emptyState)

	s.viewer = gtk.NewBox(gtk.OrientationVertical, 6)
	s.viewer.AddCSSClass("status-viewer")
	s.viewer.SetVisible(false)

	s.progress = gtk.NewBox(gtk.OrientationHorizontal, 4)
	s.progress.SetMarginTop(10)
	s.progress.SetMarginStart(10)
	s.progress.SetMarginEnd(10)
	s.viewer.Append(s.progress)

	viewerHeader := gtk.NewBox(gtk.OrientationHorizontal, 10)
	viewerHeader.SetMarginStart(10)
	viewerHeader.SetMarginEnd(10)
	vAvatar := adw.NewAvatar(36, "?", true)
	viewerHeader.Append(vAvatar)
	vCol := gtk.NewBox(gtk.OrientationVertical, 0)
	vCol.SetHExpand(true)
	vCol.SetVAlign(gtk.AlignCenter)
	s.viewerWho = gtk.NewLabel("")
	s.viewerWho.AddCSSClass("heading")
	s.viewerWho.SetXAlign(0)
	vCol.Append(s.viewerWho)
	s.viewerSub = gtk.NewLabel("")
	s.viewerSub.AddCSSClass("caption")
	s.viewerSub.AddCSSClass("dim-label")
	s.viewerSub.SetXAlign(0)
	vCol.Append(s.viewerSub)
	viewerHeader.Append(vCol)
	s.viewer.Append(viewerHeader)

	s.viewerSlot = gtk.NewBox(gtk.OrientationVertical, 0)
	s.viewerSlot.SetVExpand(true)
	s.viewerSlot.SetHExpand(true)
	s.viewer.Append(s.viewerSlot)

	nav := gtk.NewBox(gtk.OrientationHorizontal, 6)
	nav.SetHAlign(gtk.AlignCenter)
	nav.SetMarginBottom(10)
	s.navPrev = gtk.NewButtonFromIconName("go-previous-symbolic")
	s.navPrev.SetTooltipText("Sebelumnya")
	s.navPrev.ConnectClicked(func() { s.navigate(-1) })
	nav.Append(s.navPrev)
	s.navNext = gtk.NewButtonFromIconName("go-next-symbolic")
	s.navNext.SetTooltipText("Berikutnya")
	s.navNext.ConnectClicked(func() { s.navigate(1) })
	nav.Append(s.navNext)
	s.viewer.Append(nav)

	s.right.Append(s.viewer)

	// Same sidebar geometry as the Chats pane (never 50:50).
	split := adw.NewOverlaySplitView()
	split.SetSidebar(left)
	split.SetContent(s.right)
	split.SetMinSidebarWidth(260)
	split.SetMaxSidebarWidth(360)
	s.root = split
	return s
}

// Widget returns the page widget.
func (s *Status) Widget() gtk.Widgetter { return s.root }

// SetDeps wires collaborators and triggers a reload.
func (s *Status) SetDeps(client *api.Client, cache *media.Cache, toast func(string)) {
	s.client = client
	s.cache = cache
	s.toast = toast
	s.Refresh()
}

// Refresh reloads the status groups.
func (s *Status) Refresh() {
	if s.client == nil || s.loading {
		return
	}
	s.loading = true
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
		defer cancel()
		groups, err := s.client.ListStatuses(ctx)
		glib.IdleAdd(func() bool {
			s.loading = false
			if err != nil {
				log.Printf("status: load: %v", err)
				s.status.SetText("Gagal memuat status")
				return false
			}
			s.groups = groups
			s.render()
			return false
		})
	}()
}

func ownGroup(groups []api.StatusGroup) *api.StatusGroup {
	for i := range groups {
		if groups[i].Name == "Status Saya" {
			return &groups[i]
		}
	}
	return nil
}

func (s *Status) render() {
	for {
		row := s.list.RowAtIndex(0)
		if row == nil {
			break
		}
		s.list.Remove(row)
	}

	own := ownGroup(s.groups)
	s.list.Append(s.ownStatusRow(own))

	recent := make([]api.StatusGroup, 0, len(s.groups))
	for _, g := range s.groups {
		if g.Name != "Status Saya" {
			recent = append(recent, g)
		}
	}
	if len(recent) == 0 {
		if own == nil {
			s.status.SetText("Belum ada status")
			s.status.SetVisible(true)
		}
		return
	}
	s.status.SetVisible(false)

	section := gtk.NewLabel("Terkini")
	section.AddCSSClass("caption")
	section.AddCSSClass("dim-label")
	section.SetXAlign(0)
	section.SetMarginStart(12)
	section.SetMarginTop(8)
	sectionRow := gtk.NewListBoxRow()
	sectionRow.SetSelectable(false)
	sectionRow.SetActivatable(false)
	sectionRow.SetChild(section)
	s.list.Append(sectionRow)

	for _, group := range recent {
		s.list.Append(s.contactRow(group))
	}
}

// statusTimeLabel formats "Hari ini pukul 19:20" style timestamps.
func statusTimeLabel(millis int64) string {
	t := time.UnixMilli(millis)
	now := time.Now()
	clock := t.Format("15:04")
	switch {
	case dayKey(t) == dayKey(now):
		return "Hari ini pukul " + clock
	case dayKey(t) == dayKey(now.AddDate(0, 0, -1)):
		return "Kemarin pukul " + clock
	default:
		return t.Format("02 January pukul ") + clock
	}
}

func (s *Status) ownStatusRow(own *api.StatusGroup) *gtk.ListBoxRow {
	row := gtk.NewListBoxRow()
	row.SetActivatable(true)

	box := gtk.NewBox(gtk.OrientationHorizontal, 10)
	box.SetMarginTop(8)
	box.SetMarginBottom(8)
	box.SetMarginStart(10)
	box.SetMarginEnd(10)

	overlay := gtk.NewOverlay()
	avatar := adw.NewAvatar(48, "Status Saya", true)
	overlay.SetChild(avatar)
	if own == nil {
		badge := gtk.NewButton()
		badge.SetHasFrame(false)
		badge.SetTooltipText("Tambah status")
		badgeContent := gtk.NewImageFromIconName("list-add-symbolic")
		badgeContent.SetPixelSize(14)
		badge.SetChild(badgeContent)
		badge.SetHAlign(gtk.AlignEnd)
		badge.SetVAlign(gtk.AlignEnd)
		badge.AddCSSClass("own-status-badge")
		overlay.AddOverlay(badge)
		badge.ConnectClicked(func() { s.composeText() })
	}
	box.Append(overlay)

	col := gtk.NewBox(gtk.OrientationVertical, 1)
	nameLbl := gtk.NewLabel("Status Saya")
	nameLbl.SetXAlign(0)
	col.Append(nameLbl)
	s.ownSub = gtk.NewLabel("Klik untuk tambah status")
	s.ownSub.AddCSSClass("caption")
	s.ownSub.AddCSSClass("dim-label")
	s.ownSub.SetXAlign(0)
	if own != nil && len(own.Statuses) > 0 {
		s.ownSub.SetText(statusTimeLabel(own.LatestTime))
	}
	col.Append(s.ownSub)
	box.Append(col)

	row.SetChild(box)
	addClick(row, func() {
		if own != nil && len(own.Statuses) > 0 {
			s.openViewer(*own, 0)
		} else {
			s.composeText()
		}
	})
	return row
}

func (s *Status) contactRow(group api.StatusGroup) *gtk.ListBoxRow {
	row := gtk.NewListBoxRow()
	row.SetActivatable(true)

	box := gtk.NewBox(gtk.OrientationHorizontal, 10)
	box.SetMarginTop(6)
	box.SetMarginBottom(6)
	box.SetMarginStart(10)
	box.SetMarginEnd(10)

	name := group.Name
	if name == "" {
		name = shortJID(group.Sender)
	}
	avatar := adw.NewAvatar(44, name, true)
	avatar.AddCSSClass("status-ring")
	if !group.AllViewed {
		avatar.AddCSSClass("status-ring-unseen")
	}
	box.Append(avatar)

	col := gtk.NewBox(gtk.OrientationVertical, 1)
	nameLbl := gtk.NewLabel(name)
	nameLbl.SetXAlign(0)
	nameLbl.SetEllipsize(pango.EllipsizeEnd)
	col.Append(nameLbl)
	sub := gtk.NewLabel(statusTimeLabel(group.LatestTime))
	sub.AddCSSClass("caption")
	sub.AddCSSClass("dim-label")
	sub.SetXAlign(0)
	col.Append(sub)
	box.Append(col)

	row.SetChild(box)
	addClick(row, func() { s.openViewer(group, 0) })
	return row
}

// openViewer shows a story in the right pane starting at entry start.
func (s *Status) openViewer(group api.StatusGroup, start int) {
	if len(group.Statuses) == 0 {
		return
	}
	s.stopAutoAdvance()
	s.current = &group
	s.viewerPos = start
	s.viewerOwn = group.Name == "Status Saya"

	s.emptyState.SetVisible(false)
	s.viewer.SetVisible(true)
	s.renderEntry()
}

// renderEntry paints the current status entry into the right pane.
func (s *Status) renderEntry() {
	if s.current == nil || s.viewerPos < 0 || s.viewerPos >= len(s.current.Statuses) {
		s.closeViewer()
		return
	}
	entry := s.current.Statuses[s.viewerPos]
	name := s.current.Name
	if name == "" {
		name = shortJID(s.current.Sender)
	}

	s.viewerWho.SetText(name)
	s.viewerSub.SetText(statusTimeLabel(entry.Timestamp))
	s.renderProgress()
	s.navPrev.SetSensitive(s.viewerPos > 0)
	s.navNext.SetSensitive(s.viewerPos < len(s.current.Statuses)-1)

	// Viewed receipts for other people's stories.
	if !entry.Viewed && !s.viewerOwn {
		go func(id string) {
			ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
			defer cancel()
			_ = s.client.MarkStatusViewed(ctx, id)
		}(entry.ID)
		s.current.Statuses[s.viewerPos].Viewed = true
	}

	removeAllChildren(s.viewerSlot)
	s.viewerSlot.Append(s.statusBody(*s.current, entry))

	// Auto-advance still statuses after 5s, like the official clients.
	s.stopAutoAdvance()
	if entry.Type != "video" && len(s.current.Statuses) > 1 {
		s.autoAdv = glib.TimeoutSecondsAdd(5, func() bool {
			s.autoAdv = 0
			s.navigate(1)
			return false
		})
	}
}

func (s *Status) renderProgress() {
	removeAllChildren(s.progress)
	for i := range s.current.Statuses {
		seg := gtk.NewBox(gtk.OrientationHorizontal, 0)
		seg.SetHExpand(true)
		seg.SetSizeRequest(-1, 3)
		if i <= s.viewerPos {
			seg.AddCSSClass("status-seg-done")
		} else {
			seg.AddCSSClass("status-seg")
		}
		s.progress.Append(seg)
	}
}

func (s *Status) navigate(delta int) {
	if s.viewer == nil {
		return
	}
	next := s.viewerPos + delta
	if next < 0 || next >= len(s.current.Statuses) {
		s.closeViewer()
		s.Refresh()
		return
	}
	s.stopAutoAdvance()
	s.viewerPos = next
	s.renderEntry()
}

func (s *Status) closeViewer() {
	s.stopAutoAdvance()
	s.current = nil
	s.viewer.SetVisible(false)
	s.emptyState.SetVisible(true)
}

func (s *Status) stopAutoAdvance() {
	if s.autoAdv != 0 {
		glib.SourceRemove(s.autoAdv)
		s.autoAdv = 0
	}
}

// statusBody builds the widget for one status update inside the viewer.
func (s *Status) statusBody(group api.StatusGroup, entry api.StatusEntry) gtk.Widgetter {
	switch entry.Type {
	case "image", "video":
		url := entry.MediaURL
		if url == "" {
			url = s.client.StatusMediaURL(entry.ID)
		} else {
			url = s.client.MediaURL(url)
		}
		if entry.Type == "image" {
			pic := gtk.NewPictureForPaintable(nil)
			pic.SetKeepAspectRatio(true)
			pic.SetCanShrink(true)
			pic.SetHExpand(true)
			pic.SetVExpand(true)
			if tex := s.cache.MemoryTexture(url); tex != nil {
				pic.SetPaintable(tex)
			} else {
				s.cache.ImageAsync(url, func(tex *gdk.Texture, err error) {
					if err == nil && tex != nil {
						pic.SetPaintable(tex)
					}
				})
			}
			return pic
		}
		play := gtk.NewVideoForFile(nil)
		play.SetHExpand(true)
		play.SetVExpand(true)
		go func() {
			ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
			defer cancel()
			path, err := s.cache.Get(ctx, url)
			if err != nil {
				log.Printf("status video: %v", err)
				return
			}
			glib.IdleAdd(func() bool {
				play.SetFilename(path)
				play.SetAutoplay(true)
				return false
			})
		}()
		return play
	default:
		wrap := gtk.NewBox(gtk.OrientationVertical, 0)
		wrap.SetHExpand(true)
		wrap.SetVExpand(true)
		wrap.AddCSSClass("status-text-bg")
		lbl := gtk.NewLabel(entry.Content)
		lbl.SetWrap(true)
		lbl.SetJustify(gtk.JustifyCenter)
		lbl.SetHAlign(gtk.AlignCenter)
		lbl.SetVAlign(gtk.AlignCenter)
		lbl.SetMarginStart(40)
		lbl.SetMarginEnd(40)
		lbl.AddCSSClass("status-text")
		wrap.Append(lbl)
		return wrap
	}
}

// composeText opens the text status composer.
func (s *Status) composeText() {
	dialog := adw.NewDialog()
	dialog.SetTitle("Status teks")
	dialog.SetContentWidth(420)

	box := gtk.NewBox(gtk.OrientationVertical, 8)
	box.SetMarginTop(12)
	box.SetMarginBottom(12)
	box.SetMarginStart(12)
	box.SetMarginEnd(12)

	tv := gtk.NewTextView()
	tv.SetWrapMode(gtk.WrapWordChar)
	tv.SetSizeRequest(-1, 120)
	buf := tv.Buffer()
	box.Append(tv)

	bg := api.StatusBackgrounds[0]
	colors := gtk.NewBox(gtk.OrientationHorizontal, 6)
	colors.SetHAlign(gtk.AlignCenter)
	for i, c := range api.StatusBackgrounds {
		c := c
		btn := gtk.NewButton()
		btn.SetSizeRequest(28, 28)
		btn.AddCSSClass("color-button")
		btn.ConnectClicked(func() { bg = c })
		btn.SetTooltipText(fmt.Sprintf("Warna %d", i+1))
		colors.Append(btn)
	}
	box.Append(colors)

	send := gtk.NewButtonWithLabel("Kirim")
	send.AddCSSClass("suggested-action")
	send.ConnectClicked(func() {
		text := strings.TrimSpace(bufText(buf))
		if text == "" {
			return
		}
		dialog.Close()
		go func() {
			ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
			defer cancel()
			if err := s.client.PostStatusText(ctx, text, bg); err != nil {
				log.Printf("status text: %v", err)
				glib.IdleAdd(func() bool {
					if s.toast != nil {
						s.toast("Gagal mengirim status: " + err.Error())
					}
					return false
				})
				return
			}
			glib.IdleAdd(func() bool {
				s.Refresh()
				if s.toast != nil {
					s.toast("Status terkirim")
				}
				return false
			})
		}()
	})
	box.Append(send)

	dialog.SetChild(box)
	dialog.Present(MainWindow)
}

// composeMedia opens a file picker and posts the chosen image/video.
func (s *Status) composeMedia(kind string) {
	fd := gtk.NewFileDialog()
	ff := gtk.NewFileFilter()
	if kind == "video" {
		ff.SetName("Video")
		ff.AddMIMEType("video/*")
	} else {
		ff.SetName("Gambar")
		ff.AddMIMEType("image/*")
	}
	fd.SetDefaultFilter(ff)

	fd.Open(context.Background(), MainWindow, func(res gio.AsyncResulter) {
		file, err := fd.OpenFinish(res)
		if err != nil || file == nil {
			return
		}
		path := file.Path()
		if path == "" {
			return
		}
		go func() {
			ctx, cancel := context.WithTimeout(context.Background(), 3*time.Minute)
			defer cancel()
			if err := s.client.PostStatusMedia(ctx, kind, path, ""); err != nil {
				log.Printf("status media: %v", err)
				glib.IdleAdd(func() bool {
					if s.toast != nil {
						s.toast("Gagal mengirim status: " + err.Error())
					}
					return false
				})
				return
			}
			glib.IdleAdd(func() bool {
				s.Refresh()
				if s.toast != nil {
					s.toast("Status terkirim")
				}
				return false
			})
		}()
	})
}

func bufText(buf *gtk.TextBuffer) string {
	if s, ok := buf.ObjectProperty("text").(string); ok {
		return s
	}
	return ""
}

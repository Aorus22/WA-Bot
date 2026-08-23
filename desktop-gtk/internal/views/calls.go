package views

import (
	"context"
	"fmt"
	"log"
	"strings"
	"sync"
	"time"

	"github.com/diamondburned/gotk4/pkg/glib/v2"
	"github.com/diamondburned/gotk4/pkg/gtk/v4"
	pango "github.com/diamondburned/gotk4/pkg/pango"

	"wa-bot-desktop/internal/api"
	"wa-bot-desktop/internal/store"
)

const (
	callPageSize        = 25
	callRefreshDebounce = 500 * time.Millisecond
)

// Calls is the top-level call history page, mirroring the web app's
// CallHistoryPage: search + pill filter chips, date-grouped rows with
// direction/missed badges, a right-side detail panel, and WS-driven refresh.
type Calls struct {
	client *api.Client
	store  *store.Store
	toast  func(string)
	// onOpenChat switches to the chats page and opens the given chat
	// ("View chat" action in the detail panel).
	onOpenChat func(chatID string)

	root     *gtk.Box
	countLbl *gtk.Label
	search   *gtk.SearchEntry
	chips    []*gtk.ToggleButton

	listBox  *gtk.ListBox
	moreBtn  *gtk.Button
	emptyBox *gtk.Box

	detailWrap    *gtk.Box
	detailRows    *gtk.Box
	detailBtnChat *gtk.Button
	detailBtnCall *gtk.Button
	detailLog     *api.CallLog

	// refs holds named widgets of the detail panel (populated at build time).
	refs map[string]gtk.Widgetter

	filterIx int
	query    string

	mu      sync.Mutex
	logs    []api.CallLog
	hasMore bool
	loading bool
	pending bool // a debounced WS refresh is already scheduled
}

// NewCalls constructs the calls view widgets.
func NewCalls() *Calls {
	c := &Calls{filterIx: 0}

	c.root = gtk.NewBox(gtk.OrientationVertical, 0)

	// ---- Header: title + count / search / filter chips ----
	header := gtk.NewBox(gtk.OrientationVertical, 14)
	header.SetMarginTop(18)
	header.SetMarginBottom(10)
	header.SetMarginStart(20)
	header.SetMarginEnd(20)

	titleRow := gtk.NewBox(gtk.OrientationHorizontal, 10)
	title := gtk.NewLabel("Calls")
	title.AddCSSClass("title-2")
	title.SetXAlign(0)
	titleRow.Append(title)
	c.countLbl = gtk.NewLabel("0")
	c.countLbl.AddCSSClass("dim-label")
	c.countLbl.SetVAlign(gtk.AlignEnd)
	titleRow.Append(c.countLbl)
	titleRow.SetVAlign(gtk.AlignCenter)
	header.Append(titleRow)

	c.search = gtk.NewSearchEntry()
	c.search.SetPlaceholderText("Search calls")
	c.search.ConnectSearchChanged(func() {
		c.query = strings.TrimSpace(c.search.Text())
		c.render()
	})
	header.Append(c.search)

	chipsRow := gtk.NewBox(gtk.OrientationHorizontal, 8)
	labels := []string{"All", "Incoming", "Outgoing", "Missed"}
	for ix, label := range labels {
		c.addChip(chipsRow, label, ix)
	}
	header.Append(chipsRow)
	c.root.Append(header)

	// ---- Body: list + detail panel ----
	body := gtk.NewBox(gtk.OrientationHorizontal, 0)
	body.SetHExpand(true)
	body.SetVExpand(true)

	c.listBox = gtk.NewListBox()
	c.listBox.SetSelectionMode(gtk.SelectionSingle)
	c.listBox.SetHExpand(true)
	c.listBox.SetVExpand(true)
	c.listBox.ConnectRowActivated(func(row *gtk.ListBoxRow) {
		id := row.Name()
		for i := range c.logs {
			if c.logs[i].ID == id {
				c.selectLog(&c.logs[i])
				return
			}
		}
	})

	scroller := gtk.NewScrolledWindow()
	scroller.SetPolicy(gtk.PolicyNever, gtk.PolicyAutomatic)
	scroller.SetChild(c.listBox)
	scroller.SetHExpand(true)
	scroller.SetVExpand(true)

	// Keep the empty state in the same allocation as the scroller. Appending it
	// below the scroller makes GtkBox distribute the spare height between both
	// expanding children, which pushes the placeholder towards the bottom.
	listArea := gtk.NewOverlay()
	listArea.SetChild(scroller)
	listArea.SetHExpand(true)
	listArea.SetVExpand(true)
	c.emptyBox = c.buildEmptyState()
	listArea.AddOverlay(c.emptyBox)

	listCol := gtk.NewBox(gtk.OrientationVertical, 0)
	listCol.SetHExpand(true)
	listCol.SetVExpand(true)
	listCol.Append(listArea)

	c.moreBtn = gtk.NewButtonFromIconName("go-down-symbolic")
	c.moreBtn.SetLabel("Load more")
	c.moreBtn.SetHAlign(gtk.AlignCenter)
	c.moreBtn.SetMarginTop(6)
	c.moreBtn.SetMarginBottom(14)
	c.moreBtn.SetVisible(false)
	c.moreBtn.ConnectClicked(func() { c.reload(false) })
	listCol.Append(c.moreBtn)

	body.Append(listCol)

	// Right-side detail panel (360px, like web's desktop panel).
	c.detailWrap = c.buildDetailPanel()
	c.detailWrap.SetVisible(false)
	body.Append(c.detailWrap)

	c.root.Append(body)

	return c
}

// Widget returns the root widget for embedding in a ViewStack.
func (c *Calls) Widget() gtk.Widgetter { return c.root }

// SetDeps wires collaborators and performs the first load. Call once, on the
// GTK main thread.
func (c *Calls) SetDeps(client *api.Client, st *store.Store, toast func(string)) {
	c.client = client
	c.store = st
	c.toast = toast
	c.reload(true)
}

// SetOpenChatCallback wires the "View chat" detail action.
func (c *Calls) SetOpenChatCallback(cb func(chatID string)) { c.onOpenChat = cb }

// Refresh schedules one coalesced reload after bursts of WS call events.
func (c *Calls) Refresh() {
	c.mu.Lock()
	if c.pending || c.client == nil {
		c.mu.Unlock()
		return
	}
	c.pending = true
	c.mu.Unlock()

	go func() {
		time.Sleep(callRefreshDebounce)
		glib.IdleAdd(func() bool {
			c.mu.Lock()
			c.pending = false
			c.mu.Unlock()
			c.reload(true)
			return false
		})
	}()
}

// addChip appends one pill filter chip; exactly one is active at a time.
func (c *Calls) addChip(row *gtk.Box, label string, ix int) {
	tb := gtk.NewToggleButtonWithLabel(label)
	tb.AddCSSClass("call-chip")
	if ix == 0 {
		tb.SetActive(true)
	}
	tb.ConnectToggled(func() {
		if !tb.Active() {
			return
		}
		c.filterIx = ix
		for j, other := range c.chips {
			if j != ix && other.Active() {
				other.SetActive(false)
			}
		}
		c.reload(true)
	})
	c.chips = append(c.chips, tb)
	row.Append(tb)
}

// buildEmptyState creates the centered empty/error placeholder.
func (c *Calls) buildEmptyState() *gtk.Box {
	box := gtk.NewBox(gtk.OrientationVertical, 8)
	box.SetHAlign(gtk.AlignCenter)
	box.SetVAlign(gtk.AlignCenter)
	box.SetVisible(false)

	circle := gtk.NewBox(gtk.OrientationVertical, 0)
	circle.AddCSSClass("call-empty-circle")
	circle.SetSizeRequest(56, 56)
	circle.SetHAlign(gtk.AlignCenter)
	circle.SetVAlign(gtk.AlignCenter)
	icon := gtk.NewImageFromIconName("call-missed-symbolic")
	icon.SetPixelSize(24)
	centerCallIcon(icon)
	circle.Append(icon)
	box.Append(circle)

	title := gtk.NewLabel("No calls yet")
	title.AddCSSClass("heading")
	box.Append(title)

	sub := gtk.NewLabel("Your call history will appear here.")
	sub.AddCSSClass("dim-label")
	sub.AddCSSClass("caption")
	box.Append(sub)

	return box
}

// setEmptyStateTexts swaps the placeholder texts for the search/no-result case.
func (c *Calls) setEmptyStateTexts(searching bool) {
	children := widgetChildren(c.emptyBox)
	if len(children) < 3 {
		return
	}
	title := children[1].(*gtk.Label)
	sub := children[2].(*gtk.Label)
	if searching {
		title.SetText("No results found")
		sub.SetText("Nothing matches \"" + c.query + "\"")
	} else {
		title.SetText("No calls yet")
		sub.SetText("Your call history will appear here.")
	}
}

// reload fetches a page of call logs. Runs on the main thread; the HTTP round
// trip happens off-thread and results are applied back via IdleAdd.
func (c *Calls) reload(reset bool) {
	if c == nil || c.client == nil {
		return
	}
	c.mu.Lock()
	if c.loading {
		c.mu.Unlock()
		return
	}
	c.loading = true
	before := int64(0)
	if !reset && len(c.logs) > 0 {
		before = c.logs[len(c.logs)-1].StartedAt
	}
	c.mu.Unlock()

	direction, status := c.filterParams()
	c.moreBtn.SetSensitive(false)

	client := c.client
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		logs, err := client.GetCallHistory(ctx, callPageSize, before, direction, status)
		glib.IdleAdd(func() bool {
			c.mu.Lock()
			c.loading = false
			if err != nil {
				c.mu.Unlock()
				log.Printf("calls: history: %v", err)
				if c.toast != nil {
					c.toast("Failed to load call history: " + err.Error())
				}
				return false
			}
			if reset {
				c.logs = logs
			} else {
				c.logs = append(c.logs, logs...)
			}
			c.hasMore = len(logs) >= callPageSize
			c.mu.Unlock()

			c.render()
			return false
		})
	}()
}

// filterParams maps the active chip to API query values (web parity: the
// Missed chip queries status=missed; rejected/failed are filtered client-side).
func (c *Calls) filterParams() (direction, status string) {
	switch c.filterIx {
	case 1:
		return "incoming", ""
	case 2:
		return "outgoing", ""
	case 3:
		return "", "missed"
	default:
		return "", ""
	}
}

// render rebuilds the log rows from current data, applying the client-side
// search and missed-filter exactly like the web page does.
func (c *Calls) render() {
	c.mu.Lock()
	logs := make([]api.CallLog, len(c.logs))
	copy(logs, c.logs)
	c.mu.Unlock()

	// Client-side filtering (web parity).
	visible := logs[:0:0]
	for _, l := range logs {
		if c.filterIx == 3 && !callIsMissed(l) {
			continue
		}
		if c.query != "" {
			q := strings.ToLower(c.query)
			hay := strings.ToLower(callDisplayName(l)) + "\n" + strings.ToLower(l.Target) + "\n" + strings.ToLower(l.GroupJID)
			if !strings.Contains(hay, q) {
				continue
			}
		}
		visible = append(visible, l)
	}

	for {
		row := c.listBox.RowAtIndex(0)
		if row == nil {
			break
		}
		c.listBox.Remove(row)
	}

	c.countLbl.SetText(fmt.Sprintf("%d", len(logs)))

	// Date-grouped rows (Today / Yesterday / "Aug 12, 2025").
	var prevKey string
	for _, l := range visible {
		key := callGroupLabel(l.StartedAt)
		if key != prevKey {
			c.listBox.Append(callGroupHeaderRow(key))
			prevKey = key
		}
		c.listBox.Append(c.buildRow(l))
	}

	hasContent := len(visible) > 0
	c.moreBtn.SetVisible(c.hasMore)
	c.moreBtn.SetSensitive(!c.loading)
	c.emptyBox.SetVisible(!hasContent)
	c.setEmptyStateTexts(c.query != "")

	if sel := c.detailLog; sel != nil {
		found := false
		for i := range visible {
			if visible[i].ID == sel.ID {
				found = true
				break
			}
		}
		if !found {
			c.clearDetail()
		}
	}
}

// buildRow renders one call log entry following the web CallRow design:
// circular direction badge, bold name, meta line with duration, timestamp
// column with a red dot for missed calls.
func (c *Calls) buildRow(l api.CallLog) *gtk.ListBoxRow {
	row := gtk.NewListBoxRow()
	row.SetName(l.ID)
	row.SetActivatable(true)

	box := gtk.NewBox(gtk.OrientationHorizontal, 16)
	box.SetMarginTop(10)
	box.SetMarginBottom(10)
	box.SetMarginStart(14)
	box.SetMarginEnd(14)
	box.SetVAlign(gtk.AlignCenter)

	missed := callIsMissed(l)
	isVideo := l.CallType == "video" || l.CallType == "group_video"
	isGroup := l.CallType == "group_audio" || l.CallType == "group_video"

	// Direction badge (circle) with optional video dot overlay.
	badge := gtk.NewOverlay()
	badge.AddCSSClass("call-badge")
	if missed {
		badge.AddCSSClass("call-badge-missed")
	}
	badge.SetSizeRequest(44, 44)
	badge.SetHExpand(false)
	badge.SetVExpand(false)
	badge.SetHAlign(gtk.AlignCenter)
	badge.SetVAlign(gtk.AlignCenter)

	iconName := "call-incoming-symbolic"
	if missed {
		iconName = "call-missed-symbolic"
	} else if l.Direction == "outgoing" {
		iconName = "call-outgoing-symbolic"
	}
	icon := gtk.NewImageFromIconName(iconName)
	icon.SetPixelSize(20)
	centerCallIcon(icon)
	if missed {
		icon.AddCSSClass("call-icon-missed")
	} else {
		icon.AddCSSClass("call-icon-accent")
	}
	badge.SetChild(icon)

	if isVideo && !missed {
		dot := gtk.NewImageFromIconName("camera-video-symbolic")
		dot.SetPixelSize(11)
		dot.AddCSSClass("call-video-dot")
		dot.SetHAlign(gtk.AlignEnd)
		dot.SetVAlign(gtk.AlignEnd)
		badge.AddOverlay(dot)
	}
	box.Append(badge)

	// Name + meta line.
	mid := gtk.NewBox(gtk.OrientationVertical, 2)
	mid.SetHExpand(true)
	mid.SetVAlign(gtk.AlignCenter)

	name := gtk.NewLabel(store.OneLine(callDisplayName(l), 40))
	name.AddCSSClass("heading")
	name.SetEllipsize(3) // PANGO_ELLIPSIZE_END
	name.SetXAlign(0)
	name.SetMaxWidthChars(24)
	mid.Append(name)

	meta := gtk.NewBox(gtk.OrientationHorizontal, 5)
	meta.SetVAlign(gtk.AlignCenter)
	if missed {
		mIcon := gtk.NewImageFromIconName("call-missed-symbolic")
		mIcon.SetPixelSize(11)
		mIcon.AddCSSClass("call-icon-missed")
		meta.Append(mIcon)
		mTxt := gtk.NewLabel("Missed")
		mTxt.AddCSSClass("call-meta-missed")
		meta.Append(mTxt)
	} else {
		aIcon := gtk.NewImageFromIconName(iconName)
		aIcon.SetPixelSize(11)
		aIcon.AddCSSClass("call-icon-accent")
		meta.Append(aIcon)
		kind := "Voice"
		if isVideo {
			kind = "Video"
		}
		meta.Append(gtk.NewLabel(kind))
	}
	if !isGroup {
		meta.Append(gtk.NewLabel("·"))
		meta.Append(gtk.NewLabel(callFormatDuration(l.DurationMS)))
	} else if n := len(l.Participants); n > 0 {
		meta.Append(gtk.NewLabel("·"))
		meta.Append(gtk.NewLabel(fmt.Sprintf("%d people", n)))
	}
	mid.Append(meta)
	box.Append(mid)

	// Timestamp column + missed dot.
	right := gtk.NewBox(gtk.OrientationVertical, 6)
	right.SetVAlign(gtk.AlignCenter)
	ts := gtk.NewLabel(callTimeLabel(l.StartedAt))
	ts.AddCSSClass("caption")
	ts.AddCSSClass("dim-label")
	right.Append(ts)
	if missed {
		dot := gtk.NewBox(gtk.OrientationVertical, 0)
		dot.AddCSSClass("call-dot")
		dot.SetSizeRequest(8, 8)
		dot.SetHAlign(gtk.AlignEnd)
		right.Append(dot)
	}
	box.Append(right)

	row.SetChild(box)
	if l.Status == "failed" && l.ErrorMessage != "" {
		row.SetTooltipText(l.ErrorMessage)
	}
	return row
}

// callGroupHeaderRow renders the uppercase date group label row.
func callGroupHeaderRow(label string) *gtk.ListBoxRow {
	row := gtk.NewListBoxRow()
	row.SetSelectable(false)
	row.SetActivatable(false)
	row.SetFocusable(false)
	row.SetSensitive(false)

	lbl := gtk.NewLabel(strings.ToUpper(label))
	lbl.AddCSSClass("call-group-label")
	lbl.SetXAlign(0)
	wrap := gtk.NewBox(gtk.OrientationVertical, 0)
	wrap.SetMarginTop(12)
	wrap.SetMarginBottom(4)
	wrap.SetMarginStart(14)
	wrap.Append(lbl)
	row.SetChild(wrap)
	return row
}

// ---- detail panel ----

func (c *Calls) buildDetailPanel() *gtk.Box {
	panel := gtk.NewBox(gtk.OrientationVertical, 0)
	panel.AddCSSClass("call-detail")
	panel.SetSizeRequest(360, -1)

	head := gtk.NewBox(gtk.OrientationHorizontal, 8)
	head.SetMarginTop(12)
	head.SetMarginStart(16)
	head.SetMarginEnd(12)
	headLbl := gtk.NewLabel("CALL DETAILS")
	headLbl.AddCSSClass("call-group-label")
	headLbl.SetHExpand(true)
	headLbl.SetXAlign(0)
	head.Append(headLbl)
	closeBtn := gtk.NewButtonFromIconName("window-close-symbolic")
	closeBtn.AddCSSClass("flat")
	closeBtn.SetTooltipText("Close")
	closeBtn.ConnectClicked(func() { c.clearDetail() })
	head.Append(closeBtn)
	panel.Append(head)

	// Hero: big circle + name + subtitle + actions.
	hero := gtk.NewBox(gtk.OrientationVertical, 8)
	hero.SetHAlign(gtk.AlignCenter)
	hero.SetMarginTop(18)
	hero.SetMarginBottom(14)

	heroCircle := gtk.NewBox(gtk.OrientationVertical, 0)
	heroCircle.AddCSSClass("call-hero")
	heroCircle.SetSizeRequest(72, 72)
	heroCircle.SetHAlign(gtk.AlignCenter)
	heroCircle.SetVAlign(gtk.AlignCenter)
	heroIcon := gtk.NewImageFromIconName("call-incoming-symbolic")
	heroIcon.SetPixelSize(30)
	centerCallIcon(heroIcon)
	heroIcon.AddCSSClass("call-icon-accent")
	heroCircle.Append(heroIcon)
	hero.Append(heroCircle)
	panelSetRef(c, "heroCircle", heroCircle)
	panelSetRef(c, "heroIcon", heroIcon)

	name := gtk.NewLabel("")
	name.AddCSSClass("title-3")
	hero.Append(name)
	panelSetRef(c, "heroName", name)

	sub := gtk.NewLabel("")
	sub.AddCSSClass("dim-label")
	hero.Append(sub)
	panelSetRef(c, "heroSub", sub)

	actions := gtk.NewBox(gtk.OrientationHorizontal, 8)
	actions.SetMarginTop(4)
	chatBtn := gtk.NewButtonFromIconName("message-symbolic")
	chatBtn.SetLabel("View chat")
	chatBtn.ConnectClicked(func() {
		if c.detailLog != nil && c.onOpenChat != nil && c.detailLog.Target != "" {
			cb := c.onOpenChat
			target := c.detailLog.Target
			c.clearDetail()
			cb(target)
		}
	})
	actions.Append(chatBtn)
	panelSetRef(c, "btnChat", chatBtn)

	callBtn := gtk.NewButtonFromIconName("call-start-symbolic")
	callBtn.SetLabel("Call back")
	callBtn.ConnectClicked(func() {
		if c.detailLog == nil {
			return
		}
		l := *c.detailLog
		c.clearDetail()
		go c.callBack(l)
	})
	actions.Append(callBtn)
	panelSetRef(c, "btnCall", callBtn)
	hero.Append(actions)
	panel.Append(hero)

	// Detail rows.
	rows := gtk.NewBox(gtk.OrientationVertical, 0)
	rows.SetMarginTop(6)
	rows.SetMarginStart(16)
	rows.SetMarginEnd(16)
	rows.SetMarginBottom(16)
	panel.Append(rows)
	panelSetRef(c, "detailRows", rows)

	return panel
}

// centerCallIcon expands only along a vertical GtkBox's packing axis, keeping
// the rendered symbolic icon at its natural width in the exact centre. Using
// HExpand here propagates through GtkOverlay and stretches a circular badge
// across the whole row.
func centerCallIcon(icon *gtk.Image) {
	icon.SetHExpand(false)
	icon.SetVExpand(true)
	icon.SetHAlign(gtk.AlignCenter)
	icon.SetVAlign(gtk.AlignCenter)
}

// panelSetRef stores a detail-panel widget by name on the Calls struct via a
// small registry, avoiding a dozen dedicated fields.
func panelSetRef(c *Calls, name string, w gtk.Widgetter) {
	if c.refs == nil {
		c.refs = make(map[string]gtk.Widgetter)
	}
	c.refs[name] = w
}

// widgetChildren lists the direct children of a Box in order.
func widgetChildren(box *gtk.Box) []gtk.Widgetter {
	var out []gtk.Widgetter
	for w := box.FirstChild(); w != nil; w = gtk.BaseWidget(w).NextSibling() {
		out = append(out, w)
	}
	return out
}

func panelRef[T gtk.Widgetter](c *Calls, name string) T {
	var zero T
	w, ok := c.refs[name]
	if !ok {
		return zero
	}
	if t, ok2 := w.(T); ok2 {
		return t
	}
	return zero
}

func (c *Calls) selectLog(l *api.CallLog) {
	cp := *l
	c.detailLog = &cp
	c.populateDetail(cp)
	c.detailWrap.SetVisible(true)
}

func (c *Calls) clearDetail() {
	c.detailLog = nil
	c.detailWrap.SetVisible(false)
}

func (c *Calls) populateDetail(l api.CallLog) {
	missed := callIsMissed(l)
	isVideo := l.CallType == "video" || l.CallType == "group_video"
	isGroup := l.CallType == "group_audio" || l.CallType == "group_video"

	if heroCircle := panelRef[*gtk.Box](c, "heroCircle"); heroCircle != nil {
		if missed {
			heroCircle.AddCSSClass("call-hero-missed")
		} else {
			heroCircle.RemoveCSSClass("call-hero-missed")
		}
	}
	if heroIcon := panelRef[*gtk.Image](c, "heroIcon"); heroIcon != nil {
		name := "call-incoming-symbolic"
		if l.Direction == "outgoing" {
			name = "call-outgoing-symbolic"
		}
		if missed {
			name = "call-missed-symbolic"
		}
		heroIcon.SetFromIconName(name)
		if missed {
			heroIcon.AddCSSClass("call-icon-missed")
			heroIcon.RemoveCSSClass("call-icon-accent")
		} else {
			heroIcon.RemoveCSSClass("call-icon-missed")
			heroIcon.AddCSSClass("call-icon-accent")
		}
	}
	if name := panelRef[*gtk.Label](c, "heroName"); name != nil {
		name.SetText(callDisplayName(l))
	}
	if sub := panelRef[*gtk.Label](c, "heroSub"); sub != nil {
		dir := "Outgoing call"
		if l.Direction == "incoming" {
			dir = "Incoming call"
		}
		if missed {
			dir = "Missed call"
		}
		kind := "Voice"
		if isVideo {
			kind = "Video"
		}
		sub.SetText(dir + " · " + kind)
	}
	if chatBtn := panelRef[*gtk.Button](c, "btnChat"); chatBtn != nil {
		chatBtn.SetSensitive(l.Target != "")
	}
	if callBtn := panelRef[*gtk.Button](c, "btnCall"); callBtn != nil {
		callBtn.SetSensitive(l.GroupJID == "" && !isGroup && l.Target != "")
	}

	if rows := panelRef[*gtk.Box](c, "detailRows"); rows != nil {
		for {
			child := rows.FirstChild()
			if child == nil {
				break
			}
			rows.Remove(child)
		}
		addRow := func(label, value string) {
			rows.Append(callDetailRow(label, value))
		}
		addRow("Status", callStatusLabel(l.Status))
		addRow("Direction", l.Direction)
		addRow("Type", l.CallType)
		addRow("Duration", callFormatDuration(l.DurationMS))
		addRow("Started", callFullTime(l.StartedAt))
		if l.EndedAt != nil {
			addRow("Ended", callFullTime(*l.EndedAt))
		}
		addRow("Target", l.Target)
		if l.GroupJID != "" {
			addRow("Group", l.GroupJID)
		}
		if l.ErrorMessage != "" {
			addRow("Error", l.ErrorMessage)
		}
		addRow("ID", l.ID)
	}
}

func callDetailRow(label, value string) gtk.Widgetter {
	row := gtk.NewBox(gtk.OrientationHorizontal, 16)
	row.SetMarginTop(8)
	row.SetMarginBottom(8)
	l := gtk.NewLabel(label)
	l.AddCSSClass("dim-label")
	l.SetXAlign(0)
	l.SetVAlign(gtk.AlignStart)
	row.Append(l)
	v := gtk.NewLabel(value)
	v.AddCSSClass("detail-value")
	v.SetWrap(true)
	v.SetWrapMode(pango.WrapWordChar)
	v.SetJustify(gtk.JustifyRight)
	v.SetHExpand(true)
	v.SetXAlign(1)
	row.Append(v)
	return row
}

// callBack re-dials a history entry (1:1 only, like the web page).
func (c *Calls) callBack(l api.CallLog) {
	if l.Target == "" || l.GroupJID != "" {
		return
	}
	callType := "audio"
	if l.CallType == "video" || l.CallType == "group_video" {
		callType = "video"
	}
	ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
	defer cancel()
	if err := c.client.CreateCall(ctx, l.Target, callType); err != nil {
		glib.IdleAdd(func() bool {
			if c.toast != nil {
				if strings.Contains(err.Error(), "call_already_active") {
					c.toast("Masih ada panggilan aktif")
				} else {
					c.toast("Failed to start call")
				}
			}
			return false
		})
	}
}

// --- call log helpers (web parity) ---

var callMissedStatuses = map[string]bool{"missed": true, "rejected": true, "failed": true}

func callIsMissed(l api.CallLog) bool { return callMissedStatuses[l.Status] }

// callDisplayName mirrors the web's displayName: group jid wins, else the
// part of target before "@".
func callDisplayName(l api.CallLog) string {
	if l.GroupJID != "" {
		return l.GroupJID
	}
	target := l.Target
	if target == "" {
		return "Unknown"
	}
	if i := strings.IndexByte(target, '@'); i > 0 {
		return target[:i]
	}
	return target
}

// callPrettyJID spaces phone-number-like JIDs for readability (web parity).
func callPrettyJID(jid string) string {
	s := jid
	if i := strings.IndexByte(s, '@'); i > 0 {
		s = s[:i]
	}
	digits := true
	for _, r := range s {
		if r < '0' || r > '9' {
			digits = false
			break
		}
	}
	if digits && len(s) > 7 {
		return s[:3] + " " + s[3:6] + " " + s[6:]
	}
	return s
}

func callFormatDuration(ms *int64) string {
	if ms == nil || *ms <= 0 {
		return "—"
	}
	s := int(*ms / 1000)
	return fmt.Sprintf("%d:%02d", s/60, s%60)
}

func callStatusLabel(status string) string {
	if status == "" {
		return "—"
	}
	return strings.ToUpper(status[:1]) + strings.ReplaceAll(status[1:], "_", " ")
}

func callGroupLabel(ts int64) string {
	t := time.UnixMilli(ts)
	now := time.Now()
	td := t.Format("2006-01-02")
	if td == now.Format("2006-01-02") {
		return "Today"
	}
	if td == now.AddDate(0, 0, -1).Format("2006-01-02") {
		return "Yesterday"
	}
	return t.Format("Jan 2, 2006")
}

func callTimeLabel(ts int64) string {
	t := time.UnixMilli(ts)
	if t.Format("2006-01-02") == time.Now().Format("2006-01-02") {
		return t.Format("15:04")
	}
	return t.Format("Jan 2, 2006")
}

func callFullTime(ts int64) string {
	return time.UnixMilli(ts).Format("Jan 2, 2006 15:04")
}

package views

import (
	"github.com/diamondburned/gotk4-adwaita/pkg/adw"
	"github.com/diamondburned/gotk4/pkg/gtk/v4"

	"wa-bot-desktop/internal/api"
	"wa-bot-desktop/internal/media"
	"wa-bot-desktop/internal/store"
)

// MainWindow is the application's top-level window, set by ui.App during
// startup. Dialogs (e.g. the file chooser) parent themselves to it.
var MainWindow *gtk.Window

// Right-panel modes for the inner split's sidebar slot.
const (
	rightNone   = ""
	rightInfo   = "info"
	rightSearch = "search"
)

// ChatsPane is the two-column "Chats" page: chat list sidebar + conversation,
// plus a right-side panel (Chat Info or message Search) toggled from the
// conversation header. A toast overlay at the pane root shows transient
// errors for all sides.
type ChatsPane struct {
	root       *adw.ToastOverlay
	split      *adw.OverlaySplitView // outer: chat list | content
	info       *adw.OverlaySplitView // inner: conversation | right panel
	rightStack *gtk.Stack            // swaps Chat Info ↔ Search in the sidebar slot

	list     *ChatList
	conv     *Conversation
	infoPane *ChatInfo
	search   *SearchPanel

	curChatID string
	rightMode string
}

// NewChatsPane constructs the composite chats view.
func NewChatsPane() *ChatsPane {
	p := &ChatsPane{}
	p.list = NewChatList()
	p.conv = NewConversation()
	p.infoPane = NewChatInfo(nil, nil, nil)
	p.search = NewSearchPanel()

	// Right slot hosts either panel; only one is visible at a time.
	p.rightStack = gtk.NewStack()
	p.rightStack.AddTitled(p.infoPane.root, rightInfo, "Info")
	p.rightStack.AddTitled(p.search.root, rightSearch, "Search")

	// Inner split: conversation (content) + right panel.
	p.info = adw.NewOverlaySplitView()
	p.info.SetSidebarPosition(gtk.PackEnd)
	p.info.SetSidebar(p.rightStack)
	p.info.SetContent(p.conv.root)
	p.info.SetMinSidebarWidth(320)
	p.info.SetMaxSidebarWidth(420)
	p.info.SetShowSidebar(false)

	p.conv.SetHeaderCallback(func() { p.toggleInfo() })
	p.infoPane.SetCloseCallback(func() { p.showRight(rightNone) })
	p.conv.SetSearchCallback(func() { p.toggleSearch() })
	p.search.SetCloseCallback(func() { p.showRight(rightNone) })
	p.search.SetPickCallback(func(msgID string) {
		p.showRight(rightNone)
		p.conv.TeleportTo(msgID)
	})

	// Outer split: chat list (left) + inner split.
	p.split = adw.NewOverlaySplitView()
	p.split.SetSidebar(p.list.root)
	p.split.SetContent(p.info)
	p.split.SetMinSidebarWidth(260)
	p.split.SetMaxSidebarWidth(360)

	p.conv.SetSidebarCallback(func() {
		p.split.SetShowSidebar(!p.split.ShowSidebar())
	})

	p.root = adw.NewToastOverlay()
	p.root.SetChild(p.split)
	return p
}

// Widget returns the root widget.
func (p *ChatsPane) Widget() gtk.Widgetter { return p.root }

// SetDeps wires collaborators into all halves and activates row selection.
func (p *ChatsPane) SetDeps(client *api.Client, st *store.Store, cache *media.Cache) {
	toast := func(msg string) {
		t := adw.NewToast(msg)
		t.SetTimeout(5)
		p.root.AddToast(t)
	}
	p.list.SetDeps(client, st, cache)
	p.list.SetActivateCallback(func(chatID string) {
		if c, ok := st.Chat(chatID); ok {
			p.OpenChat(c)
		}
	})
	p.conv.SetDeps(client, st, cache, toast)
	p.infoPane.SetDeps(client, cache, toast)
	p.search.SetDeps(client, toast)
	p.search.wireActivation(func(msgID string) {
		p.showRight(rightNone)
		p.conv.TeleportTo(msgID)
	})
}

// OpenChat displays a chat in the conversation pane. The search sheet closes
// on switch (web parity); a visible info panel refreshes its identity.
func (p *ChatsPane) OpenChat(c api.Chat) {
	p.list.Highlight(c.ID)
	p.curChatID = c.ID
	if p.rightMode == rightSearch {
		p.showRight(rightNone)
	}
	p.conv.OpenChat(c)
	if p.rightMode == rightInfo {
		p.infoPane.SetChat(c, true)
	} else {
		p.infoPane.SetChat(c, false)
	}
}

// toggleInfo flips the info panel (header identity click).
func (p *ChatsPane) toggleInfo() {
	if p.rightMode == rightInfo {
		p.showRight(rightNone)
		return
	}
	p.showRight(rightInfo)
}

// toggleSearch flips the search panel (header search button).
func (p *ChatsPane) toggleSearch() {
	if p.rightMode == rightSearch {
		p.showRight(rightNone)
		return
	}
	p.showRight(rightSearch)
}

// showRight switches the right panel to the requested mode ("" hides it).
func (p *ChatsPane) showRight(mode string) {
	if mode == p.rightMode {
		return
	}
	p.rightMode = mode
	switch mode {
	case rightInfo:
		p.rightStack.SetVisibleChildName(rightInfo)
		p.info.SetShowSidebar(true)
		if p.conv.hasChat {
			p.infoPane.SetChat(p.conv.current, true)
		}
	case rightSearch:
		p.rightStack.SetVisibleChildName(rightSearch)
		p.info.SetShowSidebar(true)
		if p.conv.hasChat {
			p.search.OpenFor(p.conv.current.ID)
		}
	default:
		p.info.SetShowSidebar(false)
	}
}

// Clear resets all halves (logout).
func (p *ChatsPane) Clear() {
	p.conv.Clear()
	p.curChatID = ""
	p.showRight(rightNone)
	p.infoPane.Reset()
}

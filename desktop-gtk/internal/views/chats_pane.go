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

// ChatsPane is the two-column "Chats" page: chat list sidebar + conversation,
// plus a right-side Chat Info panel toggled from the conversation header.
// A toast overlay at the pane root shows transient errors for all sides.
type ChatsPane struct {
	root  *adw.ToastOverlay
	split *adw.OverlaySplitView
	info  *adw.OverlaySplitView

	list     *ChatList
	conv     *Conversation
	infoPane *ChatInfo

	curChatID string
	infoShown bool
}

// NewChatsPane constructs the composite chats view.
func NewChatsPane() *ChatsPane {
	p := &ChatsPane{}
	p.list = NewChatList()
	p.conv = NewConversation()
	p.infoPane = NewChatInfo(nil, nil, nil)

	// Inner split: conversation (content) + info panel (right sidebar).
	p.info = adw.NewOverlaySplitView()
	p.info.SetSidebarPosition(gtk.PackEnd)
	p.info.SetSidebar(p.infoPane.root)
	p.info.SetContent(p.conv.root)
	p.info.SetMinSidebarWidth(320)
	p.info.SetMaxSidebarWidth(420)
	p.info.SetShowSidebar(false)

	p.conv.SetHeaderCallback(func() { p.ToggleInfo(false) })
	p.infoPane.SetCloseCallback(func() { p.ToggleInfo(false) })

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

// SetDeps wires collaborators into both halves and activates row selection.
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
}

// OpenChat displays a chat in the conversation pane and refreshes the info
// panel when it is visible.
func (p *ChatsPane) OpenChat(c api.Chat) {
	p.list.Highlight(c.ID)
	p.curChatID = c.ID
	p.conv.OpenChat(c)
	if p.infoShown {
		p.infoPane.SetChat(c, true)
	} else {
		p.infoPane.SetChat(c, false)
	}
}

// ToggleInfo shows (show=true), hides (show=false), or flips (show=false as
// "auto") the right-side chat info panel.
func (p *ChatsPane) ToggleInfo(show bool) {
	target := !p.infoShown
	if show {
		target = true
	}
	if target == p.infoShown {
		return
	}
	p.infoShown = target
	p.info.SetShowSidebar(target)
	if target && p.conv.hasChat {
		// Load/reload sections for the chat that is currently open.
		p.infoPane.SetChat(p.conv.current, true)
	}
}

// Clear resets all halves (logout).
func (p *ChatsPane) Clear() {
	p.conv.Clear()
	p.curChatID = ""
	p.infoShown = false
	p.info.SetShowSidebar(false)
	p.infoPane.Reset()
}

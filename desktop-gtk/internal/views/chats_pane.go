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

// ChatsPane is the two-column "Chats" page: chat list sidebar + conversation.
// A toast overlay at the pane root shows transient errors for both sides.
type ChatsPane struct {
	root  *adw.ToastOverlay
	split *adw.OverlaySplitView

	list *ChatList
	conv *Conversation
}

// NewChatsPane constructs the composite chats view.
func NewChatsPane() *ChatsPane {
	p := &ChatsPane{}
	p.list = NewChatList()
	p.conv = NewConversation()

	p.split = adw.NewOverlaySplitView()
	p.split.SetSidebar(p.list.root)
	p.split.SetContent(p.conv.root)
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
}

// OpenChat displays a chat in the conversation pane.
func (p *ChatsPane) OpenChat(c api.Chat) {
	p.list.Highlight(c.ID)
	p.conv.OpenChat(c)
}

// Clear resets both halves (logout).
func (p *ChatsPane) Clear() {
	p.conv.Clear()
}

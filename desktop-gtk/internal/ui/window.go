package ui

import (
	"fmt"
	"log"

	"github.com/diamondburned/gotk4-adwaita/pkg/adw"
	"github.com/diamondburned/gotk4/pkg/gtk/v4"

	"wa-bot-desktop/internal/views"
)

// Window wraps an Adw.ApplicationWindow with the WA Bot title, a libadwaita
// ToolbarView (header bar + content), and an AdwViewStack that a sidebar
// ListBox drives. Three pages are pre-registered:
//   - "chats"    (chat list + conversation)
//   - "calls"    (call history, added via AddCalls)
//   - "settings"
type Window struct {
	// AdwWin is the Adw.ApplicationWindow.
	AdwWin *adw.ApplicationWindow
	// Win is the embedded *gtk.Window (used with gtk.Application.AddWindow).
	Win *gtk.Window
	// stack is the Adw.ViewStack; pages are registered into it.
	stack *adw.ViewStack
	// navList is the window-level navigation sidebar ListBox.
	navList *gtk.ListBox
	// pageNames is the list of registered page names in order.
	pageNames []string
	// version is the app version string.
	version string
	// overlay hosts the login screen above the main shell.
	overlay *gtk.Overlay
	login   *views.Login
}

// NewWindow constructs the main Adwaita window.
func NewWindow(app *adw.Application, name, version string) (*Window, error) {
	adwWin := adw.NewApplicationWindow((*gtk.Application)(&app.Application))

	adwWin.SetTitle(fmt.Sprintf("%s — v%s", name, version))
	adwWin.SetDefaultSize(1200, 800)
	adwWin.SetResizable(true)

	// Layout:
	//   Adw.OverlaySplitView
	//     sidebar: a manual GtkListBox with .navigation-sidebar style
	//     content: Adw.ToolbarView
	//                top:    Adw.HeaderBar
	//                content: Adw.ViewStack
	stack := adw.NewViewStack()

	toolbarView := adw.NewToolbarView()
	headerBar := adw.NewHeaderBar()
	toolbarView.AddTopBar(headerBar)

	bin := adw.NewBin()
	bin.SetChild(stack)
	toolbarView.SetContent(bin)

	// Sidebar: ListBox styled as navigation-sidebar (the modern Adwaita
	// pattern; Adw.ViewSwitcherSidebar is in libadwaita 1.4+ but its binding
	// is not yet exposed in gotk4-adwaita, so we use this portable pattern).
	sidebar := buildSidebar()

	split := adw.NewOverlaySplitView()
	split.SetSidebar(sidebar)
	split.SetContent(toolbarView)
	split.SetMinSidebarWidth(110)
	split.SetMaxSidebarWidth(150)

	// Root overlay lets the login screen cover everything when needed.
	root := gtk.NewOverlay()
	root.SetChild(split)

	adwWin.SetContent(root)

	log.Printf("window constructed: %q (v%s) with Adwaita shell", name, version)
	w := &Window{
		AdwWin:  adwWin,
		Win:     (*gtk.Window)(&adwWin.Window),
		stack:   stack,
		navList: sidebar,
		version: version,
		overlay: root,
	}
	// Wire sidebar -> stack
	sidebar.ConnectRowActivated(func(row *gtk.ListBoxRow) {
		if idx := row.Index(); idx >= 0 && idx < len(w.pageNames) {
			w.stack.SetVisibleChildName(w.pageNames[idx])
		}
	})

	return w, nil
}

// buildSidebar constructs the navigation sidebar (GtkListBox styled as
// .navigation-sidebar). Returns the ListBox; rows are appended by AddXxx methods.
func buildSidebar() *gtk.ListBox {
	lb := gtk.NewListBox()
	lb.AddCSSClass("navigation-sidebar")
	lb.SetSelectionMode(gtk.SelectionSingle)
	return lb
}

// addSidebarRow appends a row to the sidebar ListBox and returns it.
func addSidebarRow(lb *gtk.ListBox, label, iconName string) *gtk.ListBoxRow {
	row := gtk.NewListBoxRow()
	row.SetSelectable(true)
	row.SetActivatable(true)

	box := gtk.NewBox(gtk.OrientationHorizontal, 12)
	box.SetMarginStart(12)
	box.SetMarginEnd(12)
	box.SetMarginTop(8)
	box.SetMarginBottom(8)

	icon := gtk.NewImageFromIconName(iconName)
	icon.SetIconSize(gtk.IconSizeNormal)
	box.Append(icon)

	lbl := gtk.NewLabel(label)
	lbl.SetHAlign(gtk.AlignStart)
	lbl.SetHExpand(true)
	box.Append(lbl)

	row.SetChild(box)
	lb.Append(row)
	return row
}

// AddChats registers a ChatsPane view as the "chats" page.
func (w *Window) AddChats(c *views.ChatsPane) {
	w.stack.AddTitledWithIcon(c.Widget(), "chats", "Chats", "chat-bubble-symbolic")
	w.registerPage("chats")
}

// AddCalls registers a Calls view as the "calls" page.
func (w *Window) AddCalls(c *views.Calls) {
	w.stack.AddTitledWithIcon(c.Widget(), "calls", "Calls", "call-start-symbolic")
	w.registerPage("calls")
}

// AddSettings registers a Settings view as the "settings" page.
func (w *Window) AddSettings(s *views.Settings) {
	w.stack.AddTitledWithIcon(s.Widget(), "settings", "Settings", "preferences-system-symbolic")
	w.registerPage("settings")
}

// SetLogin installs the login overlay widget (hidden by default).
func (w *Window) SetLogin(l *views.Login) {
	w.login = l
	gtk.BaseWidget(l.Widget()).SetVisible(false)
	w.overlay.AddOverlay(l.Widget())
}

// ShowLogin reveals or hides the full-window login screen.
func (w *Window) ShowLogin(show bool) {
	if w.login == nil {
		return
	}
	gtk.BaseWidget(w.login.Widget()).SetVisible(show)
}

// registerPage appends a sidebar row for the given page name.
// Called by AddXxx methods; row order matches ViewStack registration order.
func (w *Window) registerPage(name string) {
	idx := len(w.pageNames)
	w.pageNames = append(w.pageNames, name)

	var icon, label string
	switch name {
	case "chats":
		icon, label = "chat-bubble-symbolic", "Chats"
	case "calls":
		icon, label = "call-start-symbolic", "Calls"
	case "settings":
		icon, label = "preferences-system-symbolic", "Settings"
	default:
		icon, label = "applications-symbolic", name
	}
	addSidebarRow(w.navList, label, icon)

	// Auto-select the first page
	if idx == 0 {
		w.stack.SetVisibleChildName(name)
	}
}

// SwitchTo selects the page with the given name and highlights its
// navigation row.
func (w *Window) SwitchTo(name string) {
	w.stack.SetVisibleChildName(name)
	for i, n := range w.pageNames {
		if n == name {
			if row := w.navList.RowAtIndex(i); row != nil {
				w.navList.SelectRow(row)
			}
			break
		}
	}
}

// Show presents the window.
func (w *Window) Show() {
	w.AdwWin.Show()
}

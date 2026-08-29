package views

import (
	"context"
	"log"
	"time"

	"github.com/diamondburned/gotk4-adwaita/pkg/adw"
	"github.com/diamondburned/gotk4/pkg/gio/v2"
	"github.com/diamondburned/gotk4/pkg/glib/v2"
	"github.com/diamondburned/gotk4/pkg/gtk/v4"
	pango "github.com/diamondburned/gotk4/pkg/pango"

	"wa-bot-desktop/internal/api"
)

// groupTab is the "Anggota" tab of the info panel for group chats: member
// list with actions, invite link, group settings (admin gated), and
// leave-group.
type groupTab struct {
	ci *ChatInfo

	content     *gtk.Box
	scroller    *gtk.ScrolledWindow
	list        *gtk.ListBox
	status      *gtk.Label
	inviteRow   *gtk.Box
	settingsBox *gtk.Box
	leaveBtn    *gtk.Button

	group   *api.GroupCache
	loading bool
}

func newGroupTab(ci *ChatInfo) *groupTab {
	t := &groupTab{ci: ci}

	t.content = gtk.NewBox(gtk.OrientationVertical, 6)
	t.content.SetMarginTop(6)
	t.content.SetMarginBottom(6)

	t.status = gtk.NewLabel("Memuat anggota…")
	t.status.AddCSSClass("dim-label")
	t.status.SetHAlign(gtk.AlignCenter)
	t.status.SetMarginTop(12)
	t.content.Append(t.status)

	t.inviteRow = gtk.NewBox(gtk.OrientationVertical, 4)
	t.content.Append(t.inviteRow)

	t.settingsBox = gtk.NewBox(gtk.OrientationVertical, 4)
	t.content.Append(t.settingsBox)

	t.list = gtk.NewListBox()
	t.list.SetSelectionMode(gtk.SelectionNone)
	t.list.SetShowSeparators(true)
	t.list.SetMarginStart(10)
	t.list.SetMarginEnd(10)
	t.content.Append(t.list)

	t.leaveBtn = gtk.NewButtonWithLabel("Keluar dari Grup")
	t.leaveBtn.AddCSSClass("destructive-action")
	t.leaveBtn.SetMarginTop(10)
	t.leaveBtn.SetMarginStart(10)
	t.leaveBtn.SetMarginEnd(10)
	t.leaveBtn.SetMarginBottom(10)
	t.leaveBtn.ConnectClicked(func() { t.confirmLeave() })
	t.content.Append(t.leaveBtn)

	t.scroller = gtk.NewScrolledWindow()
	t.scroller.SetPolicy(gtk.PolicyNever, gtk.PolicyAutomatic)
	t.scroller.SetChild(t.content)
	t.scroller.SetVExpand(true)
	t.scroller.SetHExpand(true)
	return t
}

func (t *groupTab) wrap() gtk.Widgetter { return t.scroller }

// reset clears the tab.
func (t *groupTab) reset() {
	t.group = nil
	for {
		row := t.list.RowAtIndex(0)
		if row == nil {
			break
		}
		t.list.Remove(row)
	}
	removeAllChildren(t.inviteRow)
	removeAllChildren(t.settingsBox)
	t.status.SetText("Memuat anggota…")
	t.status.SetVisible(true)
}

// refresh fetches and renders the group snapshot.
func (t *groupTab) refresh(chatID string) {
	if t.loading {
		return
	}
	t.loading = true
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 25*time.Second)
		defer cancel()
		group, err := t.ci.client.GetGroup(ctx, chatID)
		glib.IdleAdd(func() bool {
			t.loading = false
			if err != nil {
				log.Printf("group info: %v", err)
				t.status.SetText("Gagal memuat info grup")
				return false
			}
			if t.ci.hasChat && t.ci.current.ID == chatID {
				t.render(group)
			}
			return false
		})
	}()
}

// render paints the fetched snapshot.
func (t *groupTab) render(group *api.GroupCache) {
	t.group = group
	t.reset()
	t.status.SetVisible(false)

	isAdmin := group.IsAdmin()

	// Invite link row.
	linkRow := gtk.NewBox(gtk.OrientationHorizontal, 6)
	linkRow.SetMarginStart(10)
	linkRow.SetMarginEnd(10)
	linkLbl := gtk.NewLabel("Tautan undangan")
	linkLbl.SetXAlign(0)
	linkLbl.SetHExpand(true)
	linkRow.Append(linkLbl)
	copyBtn := gtk.NewButtonFromIconName("edit-copy-symbolic")
	copyBtn.SetTooltipText("Salin tautan")
	copyBtn.ConnectClicked(func() { t.copyInviteLink() })
	linkRow.Append(copyBtn)
	if isAdmin {
		revokeBtn := gtk.NewButtonFromIconName("view-refresh-symbolic")
		revokeBtn.SetTooltipText("Setel ulang tautan")
		revokeBtn.ConnectClicked(func() { t.resetInviteLink() })
		linkRow.Append(revokeBtn)
	}
	t.inviteRow.Append(linkRow)

	// Settings switches (admin gated).
	if isAdmin {
		addSwitch := func(label string, active bool, onChange func(bool)) {
			row := gtk.NewBox(gtk.OrientationHorizontal, 6)
			row.SetMarginStart(10)
			row.SetMarginEnd(10)
			lbl := gtk.NewLabel(label)
			lbl.SetXAlign(0)
			lbl.SetHExpand(true)
			row.Append(lbl)
			sw := gtk.NewSwitch()
			sw.SetActive(active)
			sw.ConnectStateSet(func(state bool) bool {
				onChange(state)
				return false
			})
			row.Append(sw)
			t.settingsBox.Append(row)
		}
		addSwitch("Hanya admin edit info grup", group.Locked, func(v bool) {
			t.patchGroup(func(c *api.UpdateGroupChanges) { c.Locked = &v })
		})
		addSwitch("Hanya admin kirim pesan", group.Announce, func(v bool) {
			t.patchGroup(func(c *api.UpdateGroupChanges) { c.Announce = &v })
		})
		addSwitch("Persetujuan anggota baru", group.JoinApproval, func(v bool) {
			t.patchGroup(func(c *api.UpdateGroupChanges) { c.JoinApproval = &v })
		})
	}

	// Participants.
	for _, p := range group.Participants {
		t.list.Append(t.participantRow(p, group))
	}

	t.leaveBtn.SetVisible(true)
}

func (t *groupTab) participantRow(p api.GroupParticipantInfo, group *api.GroupCache) *gtk.ListBoxRow {
	row := gtk.NewListBoxRow()
	row.SetSelectable(false)
	row.SetActivatable(false)

	box := gtk.NewBox(gtk.OrientationHorizontal, 8)
	box.SetMarginTop(4)
	box.SetMarginBottom(4)
	box.SetMarginStart(8)
	box.SetMarginEnd(4)

	name := p.Name
	if name == "" {
		name = shortJID(p.JID)
	}
	avatar := adw.NewAvatar(32, name, true)
	box.Append(avatar)

	col := gtk.NewBox(gtk.OrientationVertical, 1)
	col.SetHExpand(true)
	nameLbl := gtk.NewLabel(name)
	nameLbl.SetXAlign(0)
	nameLbl.SetEllipsize(pango.EllipsizeEnd)
	nameLbl.SetMaxWidthChars(26)
	col.Append(nameLbl)
	badge := ""
	if p.IsSuperAdmin || (group.Owner != "" && p.JID == group.Owner) {
		badge = "Pemilik grup"
	} else if p.IsAdmin {
		badge = "Admin"
	}
	if badge != "" {
		badgeLbl := gtk.NewLabel(badge)
		badgeLbl.AddCSSClass("caption")
		badgeLbl.AddCSSClass("dim-label")
		badgeLbl.SetXAlign(0)
		col.Append(badgeLbl)
	}
	box.Append(col)

	// Per-member actions for admins (not on the owner or super admins).
	if group.IsAdmin() && badge != "Pemilik grup" && !p.IsSuperAdmin {
		menuBtn := gtk.NewMenuButton()
		menuBtn.SetIconName("open-menu-symbolic")
		menuBtn.AddCSSClass("flat")

		menu := gio.NewMenu()
		if !p.IsAdmin {
			menu.Append("Jadikan Admin", "member.promote")
		} else {
			menu.Append("Cabut Admin", "member.demote")
		}
		menu.Append("Keluarkan", "member.remove")
		menuBtn.SetPopover(gtk.NewPopoverMenuFromModel(menu))

		group := gio.NewSimpleActionGroup()
		addAction := func(name string, fn func()) {
			act := gio.NewSimpleAction(name, nil)
			act.ConnectActivate(func(*glib.Variant) { fn() })
			group.Insert(act)
		}
		addAction("promote", func() { t.updateParticipant("promote", p) })
		addAction("demote", func() { t.updateParticipant("demote", p) })
		addAction("remove", func() { t.updateParticipant("remove", p) })
		gtk.BaseWidget(row).InsertActionGroup("member", group)
		box.Append(menuBtn)
	}

	row.SetChild(box)
	return row
}

func (t *groupTab) updateParticipant(action string, p api.GroupParticipantInfo) {
	if t.group == nil {
		return
	}
	chatID := t.ci.current.ID
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 25*time.Second)
		defer cancel()
		if _, err := t.ci.client.UpdateGroupParticipants(ctx, chatID, action, []string{p.JID}); err != nil {
			log.Printf("group participant %s: %v", action, err)
			glib.IdleAdd(func() bool {
				t.ci.reportError("Gagal memperbarui anggota: " + err.Error())
				return false
			})
			return
		}
		glib.IdleAdd(func() bool {
			t.refresh(chatID)
			return false
		})
	}()
}

func (t *groupTab) patchGroup(mutate func(*api.UpdateGroupChanges)) {
	if t.group == nil {
		return
	}
	changes := api.UpdateGroupChanges{}
	mutate(&changes)
	chatID := t.ci.current.ID
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 25*time.Second)
		defer cancel()
		if _, err := t.ci.client.UpdateGroup(ctx, chatID, changes); err != nil {
			log.Printf("group update: %v", err)
			glib.IdleAdd(func() bool {
				t.ci.reportError("Gagal mengubah pengaturan grup: " + err.Error())
				return false
			})
		}
	}()
}

func (t *groupTab) copyInviteLink() {
	chatID := t.ci.current.ID
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 25*time.Second)
		defer cancel()
		link, err := t.ci.client.GetInviteLink(ctx, chatID, false)
		glib.IdleAdd(func() bool {
			if err != nil {
				t.ci.reportError("Gagal mengambil tautan: " + err.Error())
				return false
			}
			gtk.BaseWidget(t.content).Clipboard().SetText(link)
			t.ci.reportError("Tautan disalin")
			return false
		})
	}()
}

func (t *groupTab) resetInviteLink() {
	chatID := t.ci.current.ID
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 25*time.Second)
		defer cancel()
		if _, err := t.ci.client.GetInviteLink(ctx, chatID, true); err != nil {
			glib.IdleAdd(func() bool {
				t.ci.reportError("Gagal memperbarui tautan: " + err.Error())
				return false
			})
		} else {
			glib.IdleAdd(func() bool {
				t.ci.reportError("Tautan lama dicabut")
				return false
			})
		}
	}()
}

func (t *groupTab) confirmLeave() {
	dialog := adw.NewMessageDialog(MainWindow, "Keluar grup?", "Anda tidak akan lagi menerima pesan dari grup ini.")
	dialog.AddResponse("cancel", "Batal")
	dialog.AddResponse("leave", "Keluar")
	dialog.SetCloseResponse("cancel")
	dialog.SetResponseAppearance("leave", adw.ResponseDestructive)
	dialog.ConnectResponse(func(resp string) {
		if resp != "leave" || t.group == nil {
			return
		}
		chatID := t.ci.current.ID
		go func() {
			ctx, cancel := context.WithTimeout(context.Background(), 25*time.Second)
			defer cancel()
			if err := t.ci.client.LeaveGroup(ctx, chatID); err != nil {
				glib.IdleAdd(func() bool {
					t.ci.reportError("Gagal keluar grup: " + err.Error())
					return false
				})
			}
		}()
	})
	dialog.Show()
}

func removeAllChildren(box *gtk.Box) {
	for {
		child := box.FirstChild()
		if child == nil {
			break
		}
		box.Remove(child)
	}
}

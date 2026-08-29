package views

import (
	"context"
	"log"
	"strings"
	"time"

	"github.com/diamondburned/gotk4-adwaita/pkg/adw"
	"github.com/diamondburned/gotk4/pkg/glib/v2"
	"github.com/diamondburned/gotk4/pkg/gtk/v4"
	pango "github.com/diamondburned/gotk4/pkg/pango"

	"wa-bot-desktop/internal/api"
)

// OpenCreateGroupDialog shows a name entry plus a contact picker built from
// existing 1:1 chats, then creates the group.
func OpenCreateGroupDialog(client *api.Client, chats []api.Chat, toast func(string)) {
	dialog := adw.NewDialog()
	dialog.SetTitle("Buat grup")
	dialog.SetContentWidth(420)
	dialog.SetContentHeight(480)

	box := gtk.NewBox(gtk.OrientationVertical, 8)
	box.SetMarginTop(12)
	box.SetMarginBottom(12)
	box.SetMarginStart(12)
	box.SetMarginEnd(12)

	nameEntry := gtk.NewEntry()
	nameEntry.SetPlaceholderText("Nama grup")
	box.Append(nameEntry)

	checked := map[string]*gtk.CheckButton{}
	list := gtk.NewListBox()
	list.SetSelectionMode(gtk.SelectionNone)
	for _, chat := range chats {
		if chat.IsGroup || chat.Archived {
			continue
		}
		display := chat.Name
		if display == "" {
			display = shortJID(chat.ID)
		}
		row := gtk.NewBox(gtk.OrientationHorizontal, 8)
		row.SetMarginTop(2)
		row.SetMarginBottom(2)
		row.Append(adw.NewAvatar(30, display, true))
		lbl := gtk.NewLabel(display)
		lbl.SetHExpand(true)
		lbl.SetXAlign(0)
		lbl.SetEllipsize(pango.EllipsizeEnd)
		row.Append(lbl)
		cb := gtk.NewCheckButton()
		checked[chat.ID] = cb
		row.Append(cb)

		click := gtk.NewGestureClick()
		gtk.BaseWidget(row).AddController(click)
		click.ConnectReleased(func(nPress int, x, y float64) {
			if nPress > 0 {
				cb.SetActive(!cb.Active())
			}
		})
		list.Append(row)
	}

	scroller := gtk.NewScrolledWindow()
	scroller.SetPolicy(gtk.PolicyNever, gtk.PolicyAutomatic)
	scroller.SetVExpand(true)
	scroller.SetChild(list)
	box.Append(scroller)

	send := gtk.NewButtonWithLabel("Buat grup")
	send.AddCSSClass("suggested-action")
	send.ConnectClicked(func() {
		name := strings.TrimSpace(nameEntry.Buffer().Text())
		var participants []string
		for jid, cb := range checked {
			if cb.Active() {
				participants = append(participants, jid)
			}
		}
		if name == "" {
			if toast != nil {
				toast("Nama grup wajib diisi")
			}
			return
		}
		dialog.Close()
		go func() {
			ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
			defer cancel()
			if _, err := client.CreateGroup(ctx, name, participants); err != nil {
				log.Printf("create group: %v", err)
				glib.IdleAdd(func() bool {
					if toast != nil {
						toast("Gagal membuat grup: " + err.Error())
					}
					return false
				})
				return
			}
			glib.IdleAdd(func() bool {
				if toast != nil {
					toast("Grup dibuat")
				}
				return false
			})
		}()
	})
	box.Append(send)

	dialog.SetChild(box)
	dialog.Present(MainWindow)
}

// OpenJoinGroupDialog pastes an invite link, previews it, and joins on
// confirmation.
func OpenJoinGroupDialog(client *api.Client, toast func(string)) {
	dialog := adw.NewDialog()
	dialog.SetTitle("Gabung grup")
	dialog.SetContentWidth(400)

	box := gtk.NewBox(gtk.OrientationVertical, 8)
	box.SetMarginTop(12)
	box.SetMarginBottom(12)
	box.SetMarginStart(12)
	box.SetMarginEnd(12)

	linkEntry := gtk.NewEntry()
	linkEntry.SetPlaceholderText("https://chat.whatsapp.com/…")
	box.Append(linkEntry)

	preview := gtk.NewLabel("")
	preview.AddCSSClass("dim-label")
	preview.SetWrap(true)
	preview.SetXAlign(0)
	box.Append(preview)

	previewBtn := gtk.NewButtonWithLabel("Pratinjau")
	previewBtn.ConnectClicked(func() {
		link := strings.TrimSpace(linkEntry.Buffer().Text())
		if link == "" {
			return
		}
		go func() {
			ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
			defer cancel()
			p, err := client.PreviewGroupLink(ctx, link)
			glib.IdleAdd(func() bool {
				if err != nil {
					preview.SetText("Tautan tidak valid atau sudah dicabut")
					return false
				}
				preview.SetText(p.Name + " · " + itoa(p.ParticipantCount) + " anggota")
				return false
			})
		}()
	})
	box.Append(previewBtn)

	join := gtk.NewButtonWithLabel("Gabung")
	join.AddCSSClass("suggested-action")
	join.ConnectClicked(func() {
		link := strings.TrimSpace(linkEntry.Buffer().Text())
		if link == "" {
			return
		}
		dialog.Close()
		go func() {
			ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
			defer cancel()
			jid, err := client.JoinGroupWithLink(ctx, link)
			glib.IdleAdd(func() bool {
				if err != nil {
					if toast != nil {
						toast("Gagal bergabung: " + err.Error())
					}
					return false
				}
				if toast != nil {
					toast("Berhasil bergabung: " + shortJID(jid))
				}
				return false
			})
		}()
	})
	box.Append(join)

	dialog.SetChild(box)
	dialog.Present(MainWindow)
}

func itoa(n int) string {
	if n == 0 {
		return "0"
	}
	neg := n < 0
	if neg {
		n = -n
	}
	var buf [12]byte
	i := len(buf)
	for n > 0 {
		i--
		buf[i] = byte('0' + n%10)
		n /= 10
	}
	if neg {
		i--
		buf[i] = '-'
	}
	return string(buf[i:])
}

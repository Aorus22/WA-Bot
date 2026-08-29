package views

import (
	"context"
	"log"
	"strconv"
	"strings"
	"time"

	"github.com/diamondburned/gotk4-adwaita/pkg/adw"
	"github.com/diamondburned/gotk4/pkg/glib/v2"
	"github.com/diamondburned/gotk4/pkg/gtk/v4"
	pango "github.com/diamondburned/gotk4/pkg/pango"

	"wa-bot-desktop/internal/api"
)

// openForwardDialog shows a multi-select chat picker and forwards the message
// to every checked chat (WhatsApp allows up to 5 chats per forward).
func (cv *Conversation) openForwardDialog(m api.Message) {
	if !cv.hasChat {
		return
	}
	dialog := adw.NewDialog()
	dialog.SetTitle("Teruskan pesan")
	dialog.SetContentWidth(420)
	dialog.SetContentHeight(480)

	box := gtk.NewBox(gtk.OrientationVertical, 8)
	box.SetMarginTop(12)
	box.SetMarginBottom(12)
	box.SetMarginStart(12)
	box.SetMarginEnd(12)

	chats := cv.store.Chats()
	checked := map[string]*gtk.CheckButton{}
	for _, chat := range chats {
		if chat.ID == cv.chatID() {
			continue
		}
		row := gtk.NewBox(gtk.OrientationHorizontal, 8)
		name := chat.Name
		if name == "" {
			name = shortJID(chat.ID)
		}
		avatar := adw.NewAvatar(32, name, chat.IsGroup)
		row.Append(avatar)
		lbl := gtk.NewLabel(name)
		lbl.SetHExpand(true)
		lbl.SetXAlign(0)
		lbl.SetEllipsize(pango.EllipsizeEnd)
		row.Append(lbl)
		cb := gtk.NewCheckButton()
		checked[chat.ID] = cb
		row.Append(cb)

		// Clicking anywhere on the row toggles its checkbox.
		rowClick := gtk.NewGestureClick()
		gtk.BaseWidget(row).AddController(rowClick)
		rowClick.ConnectReleased(func(nPress int, x, y float64) {
			if nPress > 0 {
				cb.SetActive(!cb.Active())
			}
		})
		box.Append(row)
	}

	outer := gtk.NewBox(gtk.OrientationVertical, 0)

	scroller := gtk.NewScrolledWindow()
	scroller.SetPolicy(gtk.PolicyNever, gtk.PolicyAutomatic)
	scroller.SetHExpand(true)
	scroller.SetVExpand(true)
	scroller.SetChild(box)
	outer.Append(scroller)

	send := gtk.NewButtonWithLabel("Teruskan")
	send.AddCSSClass("suggested-action")
	send.SetMarginTop(8)
	send.SetMarginStart(12)
	send.SetMarginEnd(12)
	send.SetMarginBottom(12)
	send.ConnectClicked(func() {
		var targets []string
		for id, cb := range checked {
			if cb.Active() {
				targets = append(targets, id)
				if len(targets) >= 5 {
					break
				}
			}
		}
		if len(targets) == 0 {
			return
		}
		dialog.Close()
		srcChat := cv.chatID()
		go func() {
			ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
			defer cancel()
			if err := cv.client.ForwardMessage(ctx, srcChat, m.ID, targets); err != nil {
				log.Printf("conversation: forward: %v", err)
				glib.IdleAdd(func() bool {
					if cv.toast != nil {
						cv.toast("Gagal meneruskan pesan: " + err.Error())
					}
					return false
				})
				return
			}
			glib.IdleAdd(func() bool {
				if cv.toast != nil {
					cv.toast("Pesan diteruskan")
				}
				return false
			})
		}()
	})
	outer.Append(send)
	dialog.SetChild(outer)

	dialog.Present(MainWindow)
}

// openPollDialog composes and sends a poll.
func (cv *Conversation) openPollDialog() {
	if !cv.hasChat {
		return
	}
	dialog := adw.NewDialog()
	dialog.SetTitle("Buat polling")
	dialog.SetContentWidth(400)

	box := gtk.NewBox(gtk.OrientationVertical, 8)
	box.SetMarginTop(12)
	box.SetMarginBottom(12)
	box.SetMarginStart(12)
	box.SetMarginEnd(12)

	questionEntry := gtk.NewEntry()
	questionEntry.SetPlaceholderText("Pertanyaan")
	box.Append(questionEntry)

	optionsBox := gtk.NewBox(gtk.OrientationVertical, 4)
	box.Append(optionsBox)
	entries := []*gtk.Entry{}
	addOptionRow := func(prefill string) {
		row := gtk.NewBox(gtk.OrientationHorizontal, 4)
		e := gtk.NewEntry()
		e.SetPlaceholderText("Opsi " + strconv.Itoa(len(entries)+1))
		e.SetText(prefill)
		entries = append(entries, e)
		row.Append(e)
		if len(entries) > 2 {
			del := gtk.NewButtonFromIconName("user-trash-symbolic")
			del.AddCSSClass("flat")
			del.ConnectClicked(func() {
				for i, x := range entries {
					if x == e {
						entries = append(entries[:i], entries[i+1:]...)
						break
					}
				}
				optionsBox.Remove(row)
			})
			row.Append(del)
		}
		optionsBox.Append(row)
	}
	addOptionRow("")
	addOptionRow("")

	addBtn := gtk.NewButtonWithLabel("Tambah opsi")
	addBtn.AddCSSClass("flat")
	addBtn.ConnectClicked(func() {
		if len(entries) >= 12 {
			return
		}
		addOptionRow("")
	})
	box.Append(addBtn)

	multi := gtk.NewCheckButtonWithLabel("Izinkan memilih beberapa jawaban")
	box.Append(multi)

	send := gtk.NewButtonWithLabel("Kirim")
	send.AddCSSClass("suggested-action")
	send.ConnectClicked(func() {
		question := strings.TrimSpace(questionEntry.Buffer().Text())
		var options []string
		for _, e := range entries {
			if v := strings.TrimSpace(e.Buffer().Text()); v != "" {
				options = append(options, v)
			}
		}
		if question == "" || len(options) < 2 {
			if cv.toast != nil {
				cv.toast("Polling butuh pertanyaan dan minimal 2 opsi")
			}
			return
		}
		dialog.Close()
		chatID := cv.chatID()
		go func() {
			ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
			defer cancel()
			if err := cv.client.SendPoll(ctx, chatID, question, options, multi.Active()); err != nil {
				log.Printf("conversation: poll: %v", err)
				glib.IdleAdd(func() bool {
					if cv.toast != nil {
						cv.toast("Gagal mengirim polling: " + err.Error())
					}
					return false
				})
			}
		}()
	})
	box.Append(send)

	dialog.SetChild(box)
	dialog.Present(MainWindow)
}

// openLocationDialog shares a location (static or live).
func (cv *Conversation) openLocationDialog() {
	if !cv.hasChat {
		return
	}
	dialog := adw.NewDialog()
	dialog.SetTitle("Bagikan lokasi")
	dialog.SetContentWidth(400)

	box := gtk.NewBox(gtk.OrientationVertical, 8)
	box.SetMarginTop(12)
	box.SetMarginBottom(12)
	box.SetMarginStart(12)
	box.SetMarginEnd(12)

	latEntry := gtk.NewEntry()
	latEntry.SetPlaceholderText("Lintang (latitude, mis. -6.2088)")
	box.Append(latEntry)
	lngEntry := gtk.NewEntry()
	lngEntry.SetPlaceholderText("Bujur (longitude, mis. 106.8456)")
	box.Append(lngEntry)
	nameEntry := gtk.NewEntry()
	nameEntry.SetPlaceholderText("Nama lokasi (opsional)")
	box.Append(nameEntry)
	addrEntry := gtk.NewEntry()
	addrEntry.SetPlaceholderText("Alamat (opsional)")
	box.Append(addrEntry)

	live := gtk.NewCheckButtonWithLabel("Bagikan lokasi langsung (live)")
	box.Append(live)

	send := gtk.NewButtonWithLabel("Bagikan")
	send.AddCSSClass("suggested-action")
	send.ConnectClicked(func() {
		lat, err1 := strconv.ParseFloat(strings.TrimSpace(latEntry.Buffer().Text()), 64)
		lng, err2 := strconv.ParseFloat(strings.TrimSpace(lngEntry.Buffer().Text()), 64)
		if err1 != nil || err2 != nil {
			if cv.toast != nil {
				cv.toast("Koordinat tidak valid")
			}
			return
		}
		dialog.Close()
		chatID := cv.chatID()
		go func() {
			ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
			defer cancel()
			if err := cv.client.SendLocation(ctx, chatID, lat, lng,
				strings.TrimSpace(nameEntry.Buffer().Text()), strings.TrimSpace(addrEntry.Buffer().Text()),
				live.Active(), ""); err != nil {
				log.Printf("conversation: location: %v", err)
				glib.IdleAdd(func() bool {
					if cv.toast != nil {
						cv.toast("Gagal mengirim lokasi: " + err.Error())
					}
					return false
				})
			}
		}()
	})
	box.Append(send)

	dialog.SetChild(box)
	dialog.Present(MainWindow)
}

// openContactDialog shares a contact card.
func (cv *Conversation) openContactDialog() {
	if !cv.hasChat {
		return
	}
	dialog := adw.NewDialog()
	dialog.SetTitle("Bagikan kontak")
	dialog.SetContentWidth(400)

	box := gtk.NewBox(gtk.OrientationVertical, 8)
	box.SetMarginTop(12)
	box.SetMarginBottom(12)
	box.SetMarginStart(12)
	box.SetMarginEnd(12)

	nameEntry := gtk.NewEntry()
	nameEntry.SetPlaceholderText("Nama kontak")
	box.Append(nameEntry)
	phoneEntry := gtk.NewEntry()
	phoneEntry.SetPlaceholderText("Nomor telepon (mis. +62812…)")
	box.Append(phoneEntry)

	send := gtk.NewButtonWithLabel("Bagikan")
	send.AddCSSClass("suggested-action")
	send.ConnectClicked(func() {
		name := strings.TrimSpace(nameEntry.Buffer().Text())
		phone := strings.TrimSpace(phoneEntry.Buffer().Text())
		if name == "" || phone == "" {
			if cv.toast != nil {
				cv.toast("Nama dan nomor telepon wajib diisi")
			}
			return
		}
		dialog.Close()
		chatID := cv.chatID()
		go func() {
			ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
			defer cancel()
			if err := cv.client.SendContact(ctx, chatID, name, phone); err != nil {
				log.Printf("conversation: contact: %v", err)
				glib.IdleAdd(func() bool {
					if cv.toast != nil {
						cv.toast("Gagal mengirim kontak: " + err.Error())
					}
					return false
				})
			}
		}()
	})
	box.Append(send)

	dialog.SetChild(box)
	dialog.Present(MainWindow)
}

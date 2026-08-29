package views

import (
	"context"
	"fmt"
	"log"
	"strconv"
	"strings"
	"time"

	"github.com/diamondburned/gotk4-adwaita/pkg/adw"
	"github.com/diamondburned/gotk4/pkg/glib/v2"
	"github.com/diamondburned/gotk4/pkg/gtk/v4"
	pango "github.com/diamondburned/gotk4/pkg/pango"

	"wa-bot-desktop/internal/api"
	"wa-bot-desktop/internal/media"
)

// Channels is the top-level Channels page: a channel list on the left and a
// chat-like post feed on the right (mirrors the Chats pane layout).
type Channels struct {
	root   *gtk.Box
	list   *gtk.ListBox
	status *gtk.Label

	// Right pane.
	detail      *gtk.Box
	emptyState  *gtk.Box
	avatar      *adw.Avatar
	nameLbl     *gtk.Label
	subLbl      *gtk.Label
	muteBtn     *gtk.Button
	unfollowBtn *gtk.Button
	posts       *gtk.ListBox
	feedSpin    *gtk.Spinner
	moreBtn     *gtk.Button

	client *api.Client
	cache  *media.Cache
	toast  func(string)

	channels  []api.ChannelInfo
	selected  *api.ChannelInfo
	postsData []api.ChannelMessage
	hasMore   bool
	loading   bool
	loadingOp bool
}

// NewChannels constructs the Channels page widgets.
func NewChannels() *Channels {
	c := &Channels{}

	c.root = gtk.NewBox(gtk.OrientationHorizontal, 0)

	// ─── Left: channel list ───
	left := gtk.NewBox(gtk.OrientationVertical, 0)
	left.SetSizeRequest(300, -1)

	header := gtk.NewBox(gtk.OrientationHorizontal, 8)
	header.SetMarginTop(10)
	header.SetMarginBottom(6)
	header.SetMarginStart(12)
	header.SetMarginEnd(10)
	title := gtk.NewLabel("Channels")
	title.AddCSSClass("title-2")
	title.SetXAlign(0)
	title.SetHExpand(true)
	header.Append(title)

	discover := gtk.NewButtonFromIconName("list-add-symbolic")
	discover.SetTooltipText("Ikuti channel via tautan")
	discover.ConnectClicked(func() { c.openDiscover() })
	header.Append(discover)
	left.Append(header)

	c.status = gtk.NewLabel("Memuat channel…")
	c.status.AddCSSClass("dim-label")
	c.status.SetMarginTop(12)
	c.status.SetMarginBottom(12)
	c.status.SetHAlign(gtk.AlignCenter)
	left.Append(c.status)

	c.list = gtk.NewListBox()
	c.list.SetSelectionMode(gtk.SelectionSingle)
	c.list.SetShowSeparators(true)
	c.list.AddCSSClass("navigation-sidebar")
	c.list.SetVExpand(true)

	scroller := gtk.NewScrolledWindow()
	scroller.SetPolicy(gtk.PolicyNever, gtk.PolicyAutomatic)
	scroller.SetChild(c.list)
	left.Append(scroller)

	c.root.Append(left)
	c.root.Append(gtk.NewSeparator(gtk.OrientationVertical))

	// ─── Right: conversation pane ───
	right := gtk.NewBox(gtk.OrientationVertical, 0)
	right.SetHExpand(true)

	c.emptyState = gtk.NewBox(gtk.OrientationVertical, 8)
	c.emptyState.SetHAlign(gtk.AlignCenter)
	c.emptyState.SetVAlign(gtk.AlignCenter)
	c.emptyState.SetVExpand(true)
	emptyIcon := gtk.NewImageFromIconName("emblem-presentation-symbolic")
	emptyIcon.SetPixelSize(56)
	emptyIcon.AddCSSClass("dim-label")
	c.emptyState.Append(emptyIcon)
	emptyLbl := gtk.NewLabel("Pilih channel untuk melihat postingannya")
	emptyLbl.AddCSSClass("dim-label")
	c.emptyState.Append(emptyLbl)
	right.Append(c.emptyState)

	c.detail = gtk.NewBox(gtk.OrientationVertical, 0)
	c.detail.SetVisible(false)

	// Channel header bar.
	chHeader := gtk.NewBox(gtk.OrientationHorizontal, 10)
	chHeader.SetMarginTop(8)
	chHeader.SetMarginBottom(8)
	chHeader.SetMarginStart(12)
	chHeader.SetMarginEnd(10)
	c.avatar = adw.NewAvatar(40, "?", true)
	chHeader.Append(c.avatar)
	nameCol := gtk.NewBox(gtk.OrientationVertical, 1)
	nameCol.SetHExpand(true)
	nameCol.SetVAlign(gtk.AlignCenter)
	c.nameLbl = gtk.NewLabel("")
	c.nameLbl.AddCSSClass("heading")
	c.nameLbl.SetEllipsize(pango.EllipsizeEnd)
	c.nameLbl.SetXAlign(0)
	nameCol.Append(c.nameLbl)
	c.subLbl = gtk.NewLabel("")
	c.subLbl.AddCSSClass("caption")
	c.subLbl.AddCSSClass("dim-label")
	c.subLbl.SetXAlign(0)
	nameCol.Append(c.subLbl)
	chHeader.Append(nameCol)

	c.muteBtn = gtk.NewButtonFromIconName("audio-volume-high-symbolic")
	c.muteBtn.AddCSSClass("flat")
	c.muteBtn.SetTooltipText("Mute/Unmute")
	c.muteBtn.ConnectClicked(func() { c.toggleMute() })
	chHeader.Append(c.muteBtn)

	c.unfollowBtn = gtk.NewButtonFromIconName("user-trash-symbolic")
	c.unfollowBtn.AddCSSClass("flat")
	c.unfollowBtn.SetTooltipText("Berhenti mengikuti")
	c.unfollowBtn.ConnectClicked(func() { c.confirmUnfollow() })
	chHeader.Append(c.unfollowBtn)
	c.detail.Append(chHeader)
	c.detail.Append(gtk.NewSeparator(gtk.OrientationHorizontal))

	c.posts = gtk.NewListBox()
	c.posts.SetSelectionMode(gtk.SelectionNone)
	c.posts.SetShowSeparators(false)
	c.posts.SetMarginTop(6)

	feedScroll := gtk.NewScrolledWindow()
	feedScroll.SetPolicy(gtk.PolicyNever, gtk.PolicyAutomatic)
	feedScroll.SetChild(c.posts)
	feedScroll.SetVExpand(true)
	c.detail.Append(feedScroll)

	footer := gtk.NewBox(gtk.OrientationHorizontal, 8)
	footer.SetHAlign(gtk.AlignCenter)
	footer.SetMarginTop(4)
	footer.SetMarginBottom(8)
	c.moreBtn = gtk.NewButtonWithLabel("Muat postingan lama")
	c.moreBtn.AddCSSClass("flat")
	c.moreBtn.SetVisible(false)
	c.moreBtn.ConnectClicked(func() { c.loadPosts(false) })
	footer.Append(c.moreBtn)
	c.feedSpin = gtk.NewSpinner()
	c.feedSpin.SetVisible(false)
	footer.Append(c.feedSpin)
	c.detail.Append(footer)

	right.Append(c.detail)
	c.root.Append(right)

	c.list.ConnectRowSelected(func(row *gtk.ListBoxRow) {
		if row == nil {
			return
		}
		idx := int(row.Index())
		if idx < 0 || idx >= len(c.channels) {
			return
		}
		c.selectChannel(c.channels[idx])
	})
	return c
}

// Widget returns the page widget.
func (c *Channels) Widget() gtk.Widgetter { return c.root }

// SetDeps wires collaborators and triggers a reload.
func (c *Channels) SetDeps(client *api.Client, cache *media.Cache, toast func(string)) {
	c.client = client
	c.cache = cache
	c.toast = toast
	c.Refresh()
}

// Refresh reloads the followed channel list.
func (c *Channels) Refresh() {
	if c.client == nil || c.loading {
		return
	}
	c.loading = true
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
		defer cancel()
		channels, err := c.client.ListChannels(ctx)
		glib.IdleAdd(func() bool {
			c.loading = false
			if err != nil {
				log.Printf("channels: load: %v", err)
				c.status.SetText("Gagal memuat channel")
				return false
			}
			c.channels = channels
			c.renderList()
			return false
		})
	}()
}

func (c *Channels) renderList() {
	for {
		row := c.list.RowAtIndex(0)
		if row == nil {
			break
		}
		c.list.Remove(row)
	}
	if len(c.channels) == 0 {
		c.status.SetText("Belum mengikuti channel")
		c.status.SetVisible(true)
		return
	}
	c.status.SetVisible(false)
	for _, ch := range c.channels {
		c.list.Append(c.channelRow(ch))
	}
	if c.selected != nil {
		for i, ch := range c.channels {
			if ch.JID == c.selected.JID {
				if row := c.list.RowAtIndex(i); row != nil {
					c.list.SelectRow(row)
				}
				break
			}
		}
	}
}

func (c *Channels) channelRow(ch api.ChannelInfo) *gtk.ListBoxRow {
	row := gtk.NewListBoxRow()
	row.SetActivatable(true)

	box := gtk.NewBox(gtk.OrientationHorizontal, 10)
	box.SetMarginTop(6)
	box.SetMarginBottom(6)
	box.SetMarginStart(10)
	box.SetMarginEnd(10)

	avatar := adw.NewAvatar(42, ch.Name, true)
	box.Append(avatar)

	col := gtk.NewBox(gtk.OrientationVertical, 1)
	name := ch.Name
	if ch.Verified {
		name += " ✓"
	}
	nameLbl := gtk.NewLabel(name)
	nameLbl.SetXAlign(0)
	nameLbl.SetEllipsize(pango.EllipsizeEnd)
	nameLbl.SetMaxWidthChars(22)
	col.Append(nameLbl)
	sub := fmt.Sprintf("%d pengikut", ch.Subscribers)
	if ch.Muted {
		sub += " · diMute"
	}
	subLbl := gtk.NewLabel(sub)
	subLbl.AddCSSClass("caption")
	subLbl.AddCSSClass("dim-label")
	subLbl.SetXAlign(0)
	col.Append(subLbl)
	box.Append(col)

	row.SetChild(box)
	return row
}

// selectChannel shows a channel's feed on the right pane.
func (c *Channels) selectChannel(ch api.ChannelInfo) {
	c.selected = &ch
	c.postsData = nil
	c.hasMore = false

	c.emptyState.SetVisible(false)
	c.detail.SetVisible(true)
	c.avatar.SetText(ch.Name)
	c.nameLbl.SetText(ch.Name)
	c.subLbl.SetText(fmt.Sprintf("%d pengikut", ch.Subscribers))
	c.updateMuteIcon()
	c.clearPosts()
	c.feedSpin.SetVisible(true)
	c.feedSpin.Start()
	c.loadPosts(true)
}

func (c *Channels) updateMuteIcon() {
	if c.selected == nil {
		return
	}
	if c.selected.Muted {
		c.muteBtn.SetIconName("audio-volume-muted-symbolic")
	} else {
		c.muteBtn.SetIconName("audio-volume-high-symbolic")
	}
}

func (c *Channels) clearPosts() {
	for {
		row := c.posts.RowAtIndex(0)
		if row == nil {
			break
		}
		c.posts.Remove(row)
	}
}

// loadPosts fetches the first page (reset) or the next older page.
func (c *Channels) loadPosts(reset bool) {
	if c.selected == nil || c.loadingOp {
		return
	}
	c.loadingOp = true
	c.moreBtn.SetSensitive(false)

	var before int64
	if !reset && len(c.postsData) > 0 {
		before = c.postsData[len(c.postsData)-1].ServerID
	}
	channelID := c.selected.JID
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
		defer cancel()
		msgs, err := c.client.GetChannelMessages(ctx, channelID, 30, before)
		glib.IdleAdd(func() bool {
			c.loadingOp = false
			c.feedSpin.SetVisible(false)
			c.feedSpin.Stop()
			c.moreBtn.SetSensitive(true)
			if err != nil {
				log.Printf("channels: posts: %v", err)
				if c.toast != nil {
					c.toast("Gagal memuat postingan: " + err.Error())
				}
				return false
			}
			if c.selected == nil || c.selected.JID != channelID {
				return false
			}
			if reset {
				c.postsData = msgs
				c.clearPosts()
			} else {
				c.postsData = append(c.postsData, msgs...)
			}
			c.hasMore = len(msgs) == 30
			c.moreBtn.SetVisible(c.hasMore && len(c.postsData) > 0)
			for _, m := range msgs {
				c.posts.Append(c.postRow(m))
			}
			return false
		})
	}()
}

// postRow renders one channel post as an incoming chat bubble with a meta
// line (time, view count, reactions) and a react popover.
func (c *Channels) postRow(m api.ChannelMessage) *gtk.ListBoxRow {
	row := gtk.NewListBoxRow()
	row.SetSelectable(false)
	row.SetActivatable(false)

	align := gtk.NewBox(gtk.OrientationVertical, 0)
	align.SetHExpand(true)
	align.SetMarginEnd(90)
	align.SetMarginTop(2)
	align.SetMarginBottom(2)

	bubble := gtk.NewBox(gtk.OrientationVertical, 3)
	bubble.AddCSSClass("bubble")
	bubble.AddCSSClass("in")
	bubble.SetHAlign(gtk.AlignStart)
	align.Append(bubble)

	content := strings.TrimSpace(m.Content)
	if content == "" {
		content = "[" + strings.ToUpper(m.MediaType) + "]"
	}
	lbl := gtk.NewLabel(content)
	lbl.SetWrap(true)
	lbl.SetSelectable(true)
	lbl.SetXAlign(0)
	bubble.Append(lbl)

	meta := gtk.NewBox(gtk.OrientationHorizontal, 8)
	meta.SetHAlign(gtk.AlignEnd)
	ts := gtk.NewLabel(timeLabel(m.Timestamp))
	ts.AddCSSClass("caption")
	ts.AddCSSClass("dim-label")
	meta.Append(ts)
	views := gtk.NewLabel("👁 " + strconv.Itoa(m.ViewsCount))
	views.AddCSSClass("caption")
	views.AddCSSClass("dim-label")
	meta.Append(views)
	for emoji, count := range m.Reactions {
		chip := gtk.NewLabel(emoji + " " + strconv.Itoa(count))
		chip.AddCSSClass("caption")
		chip.AddCSSClass("dim-label")
		meta.Append(chip)
	}
	bubble.Append(meta)

	// React popover (same emoji set as chat reactions).
	reactBtn := gtk.NewMenuButton()
	reactBtn.SetIconName("emote-smile-symbolic")
	reactBtn.AddCSSClass("flat")
	reactBtn.SetTooltipText("Reaksi")
	react := gtk.NewPopover()
	reactBox := gtk.NewBox(gtk.OrientationHorizontal, 2)
	reactBox.SetMarginTop(4)
	reactBox.SetMarginBottom(4)
	reactBox.SetMarginStart(6)
	reactBox.SetMarginEnd(6)
	for _, e := range quickReactions {
		e := e
		btn := gtk.NewButtonWithLabel(e)
		btn.AddCSSClass("flat")
		btn.ConnectClicked(func() {
			react.Popdown()
			c.sendReaction(m, e)
		})
		reactBox.Append(btn)
	}
	react.SetChild(reactBox)
	reactBtn.SetPopover(react)
	meta.Append(reactBtn)

	row.SetChild(align)
	return row
}

func (c *Channels) sendReaction(m api.ChannelMessage, emoji string) {
	if c.selected == nil {
		return
	}
	channelID := c.selected.JID
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
		defer cancel()
		if err := c.client.ReactChannelMessage(ctx, channelID, m.ServerID, m.ID, emoji); err != nil {
			log.Printf("channels: react: %v", err)
			return
		}
		// Reaction counts are server-owned: refetch the first page.
		glib.IdleAdd(func() bool {
			c.loadPosts(true)
			return false
		})
	}()
}

func (c *Channels) toggleMute() {
	if c.selected == nil {
		return
	}
	next := !c.selected.Muted
	channelID := c.selected.JID
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
		defer cancel()
		if err := c.client.MuteChannel(ctx, channelID, next); err != nil {
			glib.IdleAdd(func() bool {
				if c.toast != nil {
					c.toast("Gagal mengubah mute: " + err.Error())
				}
				return false
			})
			return
		}
		glib.IdleAdd(func() bool {
			if c.selected != nil && c.selected.JID == channelID {
				c.selected.Muted = next
				c.updateMuteIcon()
			}
			c.Refresh()
			return false
		})
	}()
}

func (c *Channels) confirmUnfollow() {
	if c.selected == nil {
		return
	}
	dialog := adw.NewMessageDialog(MainWindow, "Berhenti mengikuti?", "Post dari channel ini tidak akan lagi muncul.")
	dialog.AddResponse("cancel", "Batal")
	dialog.AddResponse("unfollow", "Berhenti")
	dialog.SetCloseResponse("cancel")
	dialog.SetResponseAppearance("unfollow", adw.ResponseDestructive)
	dialog.ConnectResponse(func(resp string) {
		if resp != "unfollow" || c.selected == nil {
			return
		}
		channelID := c.selected.JID
		name := c.selected.Name
		go func() {
			ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
			defer cancel()
			if err := c.client.UnfollowChannel(ctx, channelID); err != nil {
				glib.IdleAdd(func() bool {
					if c.toast != nil {
						c.toast("Gagal berhenti mengikuti: " + err.Error())
					}
					return false
				})
				return
			}
			glib.IdleAdd(func() bool {
				c.selected = nil
				c.detail.SetVisible(false)
				c.emptyState.SetVisible(true)
				c.Refresh()
				if c.toast != nil {
					c.toast("Berhenti mengikuti " + name)
				}
				return false
			})
		}()
	})
	dialog.Show()
}

// openDiscover follows a channel via pasted invite link.
func (c *Channels) openDiscover() {
	dialog := adw.NewDialog()
	dialog.SetTitle("Temukan channel")
	dialog.SetContentWidth(400)

	box := gtk.NewBox(gtk.OrientationVertical, 8)
	box.SetMarginTop(12)
	box.SetMarginBottom(12)
	box.SetMarginStart(12)
	box.SetMarginEnd(12)

	entry := gtk.NewEntry()
	entry.SetPlaceholderText("https://whatsapp.com/channel/…")
	box.Append(entry)

	preview := gtk.NewLabel("")
	preview.AddCSSClass("dim-label")
	preview.SetWrap(true)
	preview.SetXAlign(0)
	box.Append(preview)

	previewBtn := gtk.NewButtonWithLabel("Pratinjau")
	previewBtn.ConnectClicked(func() {
		link := strings.TrimSpace(entry.Buffer().Text())
		if link == "" {
			return
		}
		go func() {
			ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
			defer cancel()
			info, err := c.client.PreviewChannel(ctx, link)
			glib.IdleAdd(func() bool {
				if err != nil {
					preview.SetText("Tautan tidak valid")
					return false
				}
				preview.SetText(info.Name + " · " + strconv.Itoa(info.Subscribers) + " pengikut")
				return false
			})
		}()
	})
	box.Append(previewBtn)

	follow := gtk.NewButtonWithLabel("Ikuti")
	follow.AddCSSClass("suggested-action")
	follow.ConnectClicked(func() {
		link := strings.TrimSpace(entry.Buffer().Text())
		if link == "" {
			return
		}
		dialog.Close()
		go func() {
			ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
			defer cancel()
			if _, err := c.client.FollowChannel(ctx, link); err != nil {
				log.Printf("channels: follow: %v", err)
				glib.IdleAdd(func() bool {
					if c.toast != nil {
						c.toast("Gagal mengikuti channel: " + err.Error())
					}
					return false
				})
				return
			}
			glib.IdleAdd(func() bool {
				c.Refresh()
				if c.toast != nil {
					c.toast("Channel diikuti")
				}
				return false
			})
		}()
	})
	box.Append(follow)

	dialog.SetChild(box)
	dialog.Present(MainWindow)
}

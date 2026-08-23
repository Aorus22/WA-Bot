package views

import (
	"strings"

	"github.com/diamondburned/gotk4-adwaita/pkg/adw"
	"github.com/diamondburned/gotk4/pkg/gdk/v4"
	"github.com/diamondburned/gotk4/pkg/gtk/v4"

	"wa-bot-desktop/internal/api"
	"wa-bot-desktop/internal/media"
)

// newChatAvatar builds an AdwAvatar showing initials immediately and swapping
// in the real image once the async fetch completes. `stale` lets callers
// invalidate in-flight loads after a list rebuild.
func newChatAvatar(client *api.Client, cache *media.Cache, stale func() bool, c api.Chat) *adw.Avatar {
	name := displayName(c)
	av := adw.NewAvatar(44, initialsOf(name), false)
	av.SetSizeRequest(44, 44)
	av.SetVAlign(gtk.AlignCenter)

	if cache == nil || client == nil {
		return av
	}
	url := client.AvatarURL(c.ID)
	cache.ImageAsync(url, func(tex *gdk.Texture, err error) {
		if err != nil || tex == nil || stale() {
			return
		}
		av.SetCustomImage(tex)
	})
	return av
}

// initialsOf derives up-to-two-letter initials for the avatar fallback.
func initialsOf(name string) string {
	name = strings.TrimSpace(name)
	if name == "" {
		return "?"
	}
	parts := strings.Fields(name)
	switch {
	case len(parts) >= 2:
		r1 := []rune(parts[0])
		r2 := []rune(parts[1])
		out := ""
		if len(r1) > 0 {
			out += strings.ToUpper(string(r1[0]))
		}
		if len(r2) > 0 {
			out += strings.ToUpper(string(r2[0]))
		}
		if out != "" {
			return out
		}
	case len(parts) == 1:
		r := []rune(parts[0])
		if len(r) > 0 {
			return strings.ToUpper(string(r[0]))
		}
	}
	return "?"
}

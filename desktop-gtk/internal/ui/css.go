package ui

import (
	"github.com/diamondburned/gotk4/pkg/gdk/v4"
	"github.com/diamondburned/gotk4/pkg/gtk/v4"
)

// appCSS holds the custom widget styles layered over the Adwaita theme.
const appCSS = `
.bubble {
  background: @card_bg_color;
  border-radius: 12px;
  padding: 6px 10px;
}
.bubble.in {
  border-bottom-left-radius: 4px;
}
.bubble.out {
  background: alpha(@accent_bg_color, 0.92);
  color: @accent_fg_color;
  border-bottom-right-radius: 4px;
}
.msg-time { font-size: 10px; }
.bubble.in .msg-time { color: alpha(@window_fg_color, 0.55); }
.bubble.out .msg-time { color: alpha(@accent_fg_color, 0.80); }
.sender-name {
  font-size: 12px;
  font-weight: bold;
  color: @accent_bg_color;
}
.quote {
  border-left: 3px solid @accent_bg_color;
  background: alpha(@window_fg_color, 0.07);
  border-radius: 4px;
  padding: 3px 8px;
  margin-bottom: 2px;
}
.quote-who { font-size: 11px; font-weight: bold; color: @accent_bg_color; }
.quote-text { font-size: 11px; }
.caption { font-size: 11px; }
.dim-label { opacity: 0.65; }
.tick-read { color: @accent_bg_color; font-weight: bold; }
.tick-failed { color: @error_fg_color; font-weight: bold; }
.unread-badge {
  background: @accent_bg_color;
  color: @accent_fg_color;
  border-radius: 999px;
  padding: 1px 8px;
  font-size: 12px;
  font-weight: bold;
}
.date-separator {
  font-size: 11px;
  color: alpha(@window_fg_color, 0.60);
  background: alpha(@window_fg_color, 0.07);
  border-radius: 999px;
  padding: 2px 12px;
}
.composer-frame { border-radius: 16px; }
`

// loadCSS installs the application stylesheet for the default display.
func loadCSS() {
	css := gtk.NewCSSProvider()
	css.LoadFromData(appCSS)
	gtk.StyleContextAddProviderForDisplay(
		gdk.DisplayGetDefault(), css, gtk.STYLE_PROVIDER_PRIORITY_APPLICATION,
	)
}

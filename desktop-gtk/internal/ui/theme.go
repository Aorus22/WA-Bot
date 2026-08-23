package ui

import (
	"fmt"
	"math"
	"strings"

	"github.com/diamondburned/gotk4/pkg/gdk/v4"
	"github.com/diamondburned/gotk4/pkg/gtk/v4"
	"github.com/diamondburned/gotk4-adwaita/pkg/adw"
)

// currentTheme is the active preset; currentProvider is its CSS provider so
// switching themes removes exactly one layer.
var (
	currentTheme    *ThemePreset
	currentProvider *gtk.CSSProvider
)

// FindTheme resolves a theme name with fallback to the default preset.
func FindTheme(name string) *ThemePreset {
	for i := range Themes {
		if Themes[i].Name == name {
			return &Themes[i]
		}
	}
	return &Themes[0]
}

// ActiveTheme returns the currently applied preset.
func ActiveTheme() *ThemePreset {
	if currentTheme == nil {
		return FindTheme(DefaultThemeName)
	}
	return currentTheme
}

// ApplyTheme switches the entire app to the named web preset: it overrides
// libadwaita's named colors, matches the widget color scheme to the palette's
// brightness, and re-loads the widget stylesheet. Safe to call repeatedly.
func ApplyTheme(name string) *ThemePreset {
	t := FindTheme(name)
	display := gdk.DisplayGetDefault()
	if display == nil {
		return t // called before GTK display exists; nothing to do
	}

	if currentProvider != nil {
		gtk.StyleContextRemoveProviderForDisplay(display, currentProvider)
		currentProvider = nil
	}
	provider := gtk.NewCSSProvider()
	provider.LoadFromData(themeCSS(t))
	gtk.StyleContextAddProviderForDisplay(display, provider, gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
	currentProvider = provider
	currentTheme = t

	sm := adw.StyleManagerGetDefault()
	if sm != nil {
		if luminance(t.Colors.Background) < 0.5 {
			sm.SetColorScheme(adw.ColorSchemeForceDark)
		} else {
			sm.SetColorScheme(adw.ColorSchemeForceLight)
		}
	}
	return t
}

// themeCSS builds the full stylesheet for a preset: libadwaita named-color
// overrides followed by the widget styles (which reference those colors).
func themeCSS(t *ThemePreset) string {
	c := &t.Colors
	var b strings.Builder
	define := func(name, value string) {
		fmt.Fprintf(&b, "@define-color %s %s;\n", name, value)
	}

	// Window / view / chrome.
	define("window_bg_color", c.Background)
	define("window_fg_color", c.Foreground)
	define("view_bg_color", c.Background)
	define("view_fg_color", c.Foreground)
	define("dialog_bg_color", c.Card)
	define("dialog_fg_color", c.Foreground)
	define("sidebar_bg_color", c.Background)
	define("sidebar_fg_color", c.Foreground)
	define("headerbar_bg_color", c.Background)
	define("headerbar_fg_color", c.Foreground)
	define("headerbar_backdrop_bg_color", c.Background)
	define("headerbar_backdrop_fg_color", c.Foreground)

	// Surfaces.
	define("card_bg_color", c.Card)
	define("card_fg_color", c.CardForeground)
	define("popover_bg_color", c.Card)
	define("popover_fg_color", c.CardForeground)
	define("thumbnail_bg_color", c.Secondary)

	// Accents & status colors.
	define("accent_bg_color", c.Primary)
	define("accent_fg_color", c.PrimaryForeground)
	define("destructive_bg_color", c.Destructive)
	define("destructive_fg_color", c.DestructiveForeground)
	define("error_bg_color", c.Destructive)
	define("error_fg_color", c.DestructiveForeground)

	// App-specific extras referenced from widgetCSS.
	define("wa_border_color", c.Border)
	define("wa_muted_fg_color", c.MutedForeground)

	b.WriteString("\n")
	b.WriteString(widgetCSS)
	return b.String()
}

// luminance computes WCAG relative luminance of a #RRGGBB color (0..1).
func luminance(hexColor string) float64 {
	h := strings.TrimPrefix(hexColor, "#")
	if len(h) != 6 {
		return 0
	}
	var r, g, b int
	if _, err := fmt.Sscanf(strings.ToUpper(h), "%02X%02X%02X", &r, &g, &b); err != nil {
		return 0
	}
	chans := []float64{float64(r) / 255, float64(g) / 255, float64(b) / 255}
	lin := make([]float64, 3)
	for i, c := range chans {
		if c <= 0.03928 {
			lin[i] = c / 12.92
		} else {
			lin[i] = math.Pow((c+0.055)/1.055, 2.4)
		}
	}
	return 0.2126*lin[0] + 0.7152*lin[1] + 0.0722*lin[2]
}

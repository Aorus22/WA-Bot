package ui

// widgetCSS holds the custom widget styles layered over (themed) Adwaita
// named colors. It is concatenated after the @define-color overrides built
// by themeCSS, so it always sees the active palette.
const widgetCSS = `
.bubble {
  background: @card_bg_color;
  border-radius: 12px;
  padding: 6px 10px;
}
.bubble.in {
  border-bottom-left-radius: 4px;
}
.bubble.out {
  background: @accent_bg_color;
  color: @accent_fg_color;
  border-bottom-right-radius: 4px;
}
.msg-time { font-size: 10px; }
.bubble.in .msg-time { color: @wa_muted_fg_color; }
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
.dim-label { color: @wa_muted_fg_color; }
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
  color: @wa_muted_fg_color;
  background: alpha(@window_fg_color, 0.07);
  border-radius: 999px;
  padding: 2px 12px;
}
.composer-frame { border-radius: 16px; }

/* OverlaySplitView panes: libadwaita ≥1.6 paints these from CSS variables
   with literal defaults, so @define-color alone is not enough — set the
   variables (and a plain background fallback) per pane. */
.sidebar-pane, .content-pane {
  --sidebar-bg-color: @window_bg_color;
  --sidebar-fg-color: @window_fg_color;
  --sidebar-shade-color: alpha(@window_fg_color, 0.06);
  --sidebar-border-color: @wa_border_color;
  --sidebar-backdrop-color: @window_bg_color;
  background-color: @window_bg_color;
  color: @window_fg_color;
}

/* Navigation sidebars (window nav + chat list): transparent rows over the
   themed pane, themed hover/active/selected states. */
.navigation-sidebar {
  background-color: @window_bg_color;
}
.navigation-sidebar > row.activatable:hover {
  background-color: alpha(@window_fg_color, 0.07);
}
.navigation-sidebar > row.activatable:active {
  background-color: alpha(@window_fg_color, 0.12);
}
.navigation-sidebar > row:selected {
  background-color: alpha(@accent_bg_color, 0.30);
  color: @window_fg_color;
}
`

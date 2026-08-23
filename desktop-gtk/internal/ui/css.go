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
.context-bar {
  background: @card_bg_color;
  border: 1px solid @wa_border_color;
  border-radius: 10px;
  padding: 5px 8px;
}
.context-bar .context-who { font-size: 11px; font-weight: bold; color: @accent_bg_color; }
.context-bar .context-text { font-size: 12px; }
.call-missed { color: @error_fg_color; }

/* Calls page (web CallHistoryPage parity) */
.call-chip {
  border-radius: 9999px;
  padding: 5px 14px;
  font-weight: 600;
}
.call-chip:checked {
  background: @accent_bg_color;
  color: @accent_fg_color;
}
.call-group-label {
  font-size: 11px;
  font-weight: bold;
  color: @wa_muted_fg_color;
}
.call-badge {
  background: alpha(@window_fg_color, 0.08);
  border-radius: 9999px;
}
.call-badge-missed {
  background: alpha(@error_fg_color, 0.12);
}
.call-icon-accent { color: @accent_bg_color; }
.call-icon-missed { color: @error_fg_color; }
.call-meta-missed { color: @error_fg_color; font-weight: bold; }
.call-video-dot {
  background: @window_bg_color;
  border-radius: 9999px;
  padding: 2px;
  color: @wa_muted_fg_color;
}
.call-dot {
  background: @error_fg_color;
  border-radius: 9999px;
}
.call-empty-circle {
  background: alpha(@window_fg_color, 0.08);
  border-radius: 9999px;
  color: @wa_muted_fg_color;
}
.call-detail {
  border-left: 1px solid @wa_border_color;
}
.call-hero {
  background: alpha(@accent_bg_color, 0.12);
  border-radius: 9999px;
}
.call-hero-missed {
  background: alpha(@error_fg_color, 0.12);
}
.detail-value { font-weight: bold; }

/* Call mode window */
.call-pill {
  border: 1px solid @wa_border_color;
  border-radius: 9999px;
  padding: 3px 12px;
  font-size: 11px;
  font-weight: bold;
  color: @wa_muted_fg_color;
}
.call-round {
  border-radius: 9999px;
  min-width: 64px;
  min-height: 64px;
  padding: 0;
}
.call-status {
  font-size: 14px;
  color: alpha(@window_fg_color, 0.65);
}
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
.msg-highlight {
  background-color: alpha(@accent_bg_color, 0.25);
  border-radius: 8px;
}
`

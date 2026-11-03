//! Drawing. The look is a Neovim one: flat panes, a one-row statusline with a
//! mode pill, rounded input box, Tokyo Night colours.

use chrono::{Datelike, Local};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;
use ratatui::layout::Size;
use ratatui_image::picker::ProtocolType;
use ratatui_image::sliced::{SignedPosition, SlicedImage};
use ratatui_image::{FilterType, Resize, StatefulImage};
use wabot_backend_client::Message;

use crate::app::{is_from_me, kind_label, with_tag, App, ChatLog, Focus, Mode, MsgView};
use crate::images::{self, Img, Kind, Slot};
use crate::browser::draw_browser;
use crate::form::draw_form;
use crate::stickers::draw_stickers;
use crate::overlay::{draw_attach, draw_menu};
use crate::panel::draw_panel;
use crate::text::{truncate, width, wrap_rows, LinkSeg};
use crate::theme::{self as c, local, name_color};

/// Below this width only one pane shows at a time.
const NARROW: u16 = 70;

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    f.render_widget(Block::default().style(Style::new().bg(c::BG).fg(c::FG)), area);

    let [main, status] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(area);
    let narrow = area.width < NARROW;
    app.narrow = narrow;
    let show_sidebar = !narrow || app.focus == Focus::Chats || app.open.is_none();
    let show_messages = !narrow || !show_sidebar;

    app.hit = Default::default();
    app.build_budget = 2;
    app.pending_builds = false;
    app.hide_images = (app.menu.is_some() || app.attach.is_some() || app.browser.is_some() || app.stickers.is_some() || app.form.is_some())
        && app.picker.as_ref().is_some_and(|p| matches!(p.protocol_type(), ProtocolType::Sixel | ProtocolType::Iterm2));
    app.want.clear();
    app.media_hits.clear();
    app.bubble_hits.clear();
    app.link_hits.clear();
    app.quote_hits.clear();
    if app.viewer.is_some() {
        draw_viewer(f, app, main);
        draw_status(f, app, status);
        return;
    }
    match (show_sidebar, show_messages) {
        (true, true) => {
            if app.sidebar_collapsed && app.open.is_some() {
                // Folded away completely, like the desktop app: the conversation takes the width.
                draw_conversation(f, app, main);
            } else {
                let side_w = (main.width * 30 / 100).clamp(26, 40);
                let [side, msgs] = Layout::horizontal([Constraint::Length(side_w), Constraint::Min(1)]).areas(main);
                draw_sidebar(f, app, side);
                draw_conversation(f, app, msgs);
            }
        }
        (true, false) => draw_sidebar(f, app, main),
        _ => draw_conversation(f, app, main),
    }
    draw_status(f, app, status);
    draw_overlays(f, app, main);
}

/// The conversation, with the info / search drawer on its right when one is open.
fn draw_conversation(f: &mut Frame, app: &mut App, area: Rect) {
    if app.panel.is_none() {
        return draw_messages(f, app, area);
    }
    if area.width < 76 {
        // Not enough room for both: the drawer takes the pane while it has focus.
        if app.focus == Focus::Panel {
            draw_panel(f, app, area);
        } else {
            draw_messages(f, app, area);
        }
        return;
    }
    let pw = (area.width * 36 / 100).clamp(32, 46);
    let [left, right] = Layout::horizontal([Constraint::Min(1), Constraint::Length(pw)]).areas(area);
    draw_messages(f, app, left);
    draw_panel(f, app, right);
}

/// Menu, message-info box and the pasted-picture box go over everything else.
fn draw_overlays(f: &mut Frame, app: &mut App, area: Rect) {
    let font = app.picker.as_ref().map(|p| {
        let fs = p.font_size();
        (fs.width, fs.height)
    });
    let chat_name = app.attach.as_ref().and_then(|a| app.chat(&a.chat_id)).map_or(String::new(), |c| c.name.clone());
    if let Some(b) = app.browser.as_mut() {
        draw_browser(f, b, area);
    }
    if app.stickers.is_some() {
        draw_stickers(f, app, area);
    }
    if let Some(form) = app.form.as_mut() {
        draw_form(f, form, area);
    }
    if let Some(a) = app.attach.as_mut() {
        draw_attach(f, a, &chat_name, font, area);
    }
    if let Some(m) = app.menu.as_mut() {
        draw_menu(f, m, area);
    }
}

// ---- sidebar ---------------------------------------------------------------

fn draw_sidebar(f: &mut Frame, app: &mut App, area: Rect) {
    let picker = app.picker.clone();
    let focused = app.focus == Focus::Chats;
    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::new().fg(if focused { c::BLUE } else { c::BORDER }))
        .style(Style::new().bg(c::BG_DARK));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let [head, filter, list] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Min(0)]).areas(inner);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" WA Bot", Style::new().fg(c::BLUE).add_modifier(Modifier::BOLD)),
            Span::styled(if app.connected { "  ●" } else { "  ○" }, Style::new().fg(if app.connected { c::GREEN } else { c::RED })),
        ])),
        head,
    );

    let filtering = app.mode == Mode::Filter;
    let filter_line = if filtering || !app.filter.text.is_empty() {
        Line::from(vec![
            Span::styled(" / ", Style::new().fg(c::YELLOW)),
            Span::styled(app.filter.text.clone(), Style::new().fg(c::FG)),
        ])
    } else {
        Line::styled(" / to filter", Style::new().fg(c::DIM))
    };
    f.render_widget(Paragraph::new(filter_line), filter);
    if filtering {
        f.set_cursor_position((filter.x + 3 + app.filter.cursor_col() as u16, filter.y));
    }

    app.hit.sidebar_list = list;
    let visible = app.visible();
    if visible.is_empty() {
        let msg = if !app.chats_loaded {
            " Loading chats…"
        } else if app.filter.text.is_empty() {
            " No chats"
        } else {
            " No match"
        };
        f.render_widget(Paragraph::new(Line::styled(msg, Style::new().fg(c::DIM))), list);
        return;
    }

    let rows = (list.height / 2).max(1) as usize;
    let pos = app.cursor_pos(&visible);
    app.side_offset = app.side_offset.min(visible.len().saturating_sub(rows));
    if app.side_follow {
        if pos < app.side_offset {
            app.side_offset = pos;
        } else if pos >= app.side_offset + rows {
            app.side_offset = pos + 1 - rows;
        }
    }

    let w = list.width as usize;
    let mut lines: Vec<Line> = Vec::new();
    // (row on screen, picture url) for chats that have a profile photo.
    let mut avatar_rows: Vec<(usize, String)> = Vec::new();
    for (n, &i) in visible.iter().enumerate().skip(app.side_offset).take(rows) {
        let chat = &app.chats[i];
        let is_open = app.open.as_deref() == Some(chat.id.as_str());
        let is_cursor = n == pos && focused;
        let bg = if is_cursor {
            c::SELECT
        } else if is_open {
            c::SURFACE
        } else {
            c::BG_DARK
        };
        let bar = if is_open { "▎" } else { " " };
        let muted = chat.mute_mode != "off";

        let time = format_list_time(chat.last_time);
        let badge = if chat.unread > 0 {
            format!(" {} ", if chat.unread > 99 { "99+".to_string() } else { chat.unread.to_string() })
        } else {
            String::new()
        };
        let pin = if chat.pinned_at.is_some() { "* " } else { "" };
        let right_w = width(&time).max(width(&badge));
        let (initial, tile_color) = avatar_tile(&chat.name);
        let tile = Style::new().fg(c::BG_DARK).bg(tile_color).add_modifier(Modifier::BOLD);
        if !chat.avatar.is_empty() {
            avatar_rows.push((n - app.side_offset, chat.avatar.clone()));
        }
        let name_w = w.saturating_sub(AV_PREFIX + width(pin) + right_w + 1);
        let name = truncate(&chat.name, name_w);
        let name_style = if chat.unread > 0 {
            Style::new().fg(c::FG).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(c::FG)
        };
        let gap = w.saturating_sub(AV_PREFIX + width(pin) + width(&name) + width(&time) + 1);
        lines.push(
            Line::from(vec![
                Span::styled(bar, Style::new().fg(c::BLUE)),
                Span::raw(" "),
                Span::styled(format!(" {initial}  "), tile),
                Span::raw(" "),
                Span::styled(pin, Style::new().fg(c::YELLOW)),
                Span::styled(name, name_style),
                Span::raw(" ".repeat(gap)),
                Span::styled(time, Style::new().fg(if chat.unread > 0 { c::BLUE } else { c::DIM })),
                Span::raw(" "),
            ])
            .style(Style::new().bg(bg)),
        );

        // Second row: preview (or typing) on the left, unread badge on the right.
        let typing = app.typing_line(&chat.id);
        let (text, style) = match typing {
            Some(t) => (t, Style::new().fg(c::GREEN)),
            None => {
                let tick = if chat.last_message_from_me { tick_for(&chat.last_message_status) } else { String::new() };
                let sender = if chat.last_message_from_me { "" } else { "" };
                (format!("{tick}{sender}{}", chat.last_msg), Style::new().fg(c::DIM))
            }
        };
        let badge_w = width(&badge);
        let prev_w = w.saturating_sub(AV_PREFIX + badge_w + 1);
        let preview = truncate(&text, prev_w);
        let gap = w.saturating_sub(AV_PREFIX + width(&preview) + badge_w + 1);
        let badge_style = if muted {
            Style::new().fg(c::BG).bg(c::DIM)
        } else if chat.has_unread_mention {
            Style::new().fg(c::BG).bg(c::RED)
        } else {
            Style::new().fg(c::BG).bg(c::BLUE)
        };
        lines.push(
            Line::from(vec![
                Span::styled(bar, Style::new().fg(c::BLUE)),
                Span::raw(" "),
                Span::styled("    ", tile),
                Span::raw(" "),
                Span::styled(preview.clone(), style),
                Span::raw(" ".repeat(gap)),
                Span::styled(badge, badge_style),
                Span::raw(" "),
            ])
            .style(Style::new().bg(bg)),
        );
    }
    f.render_widget(Paragraph::new(lines), list);

    // Profile photos go over the initial tiles once they are loaded.
    for (row, url) in avatar_rows {
        if app.hide_images {
            break;
        }
        let rect = Rect::new(list.x + 2, list.y + (row * 2) as u16, AV_W, 2);
        if rect.bottom() > list.bottom() {
            continue;
        }
        match app.avatars.get_mut(&url) {
            Some(Img::Ready(img)) => {
                if let Some(pk) = &picker {
                    f.render_stateful_widget(
                        StatefulImage::default().resize(Resize::Scale(Some(FilterType::Triangle))),
                        rect,
                        img.proto(pk, Slot::Avatar),
                    );
                }
            }
            Some(_) => {}
            None => app.want.push((Kind::Avatar, url)),
        }
    }
}

/// Width of the profile-photo tile in cells (2 rows tall), and everything left of the name.
const AV_W: u16 = 4;
const AV_PREFIX: usize = 2 + AV_W as usize + 1;

fn avatar_tile(name: &str) -> (char, Color) {
    let initial = name
        .chars()
        .find(|c| c.is_alphanumeric())
        .or_else(|| name.chars().next())
        .map_or('#', |c| c.to_uppercase().next().unwrap_or(c));
    (initial, name_color(name))
}

fn tick_for(status: &str) -> String {
    match status {
        "read" | "delivered" => format!("{} ", crate::ticks::double()),
        "failed" => "! ".to_string(),
        "" | "received" => String::new(),
        _ => format!("{} ", crate::ticks::single()),
    }
}

fn format_list_time(ts: i64) -> String {
    let Some(t) = local(ts).filter(|_| ts > 0) else { return String::new() };
    let today = Local::now().date_naive();
    let d = t.date_naive();
    if d == today {
        t.format("%H:%M").to_string()
    } else if (today - d).num_days() == 1 {
        "Yesterday".to_string()
    } else if d.year() == today.year() {
        t.format("%d/%m").to_string()
    } else {
        t.format("%d/%m/%y").to_string()
    }
}

fn day_label(ts: i64) -> String {
    let Some(t) = local(ts) else { return String::new() };
    if t.date_naive() == Local::now().date_naive() {
        "TODAY".to_string()
    } else {
        t.format("%-d %B %Y").to_string().to_uppercase()
    }
}

// ---- messages --------------------------------------------------------------

fn draw_messages(f: &mut Frame, app: &mut App, area: Rect) {
    let picker = app.picker.clone();
    let Some(chat_id) = app.open.clone() else {
        draw_welcome(f, area);
        return;
    };
    let focused = app.focus == Focus::Messages;
    let compose = app.compose_bar();
    let bar_rows = u16::from(compose.is_some());
    let [head, body, input_all] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(1), Constraint::Length(3 + bar_rows)]).areas(area);
    let (compose_area, input) = if bar_rows == 1 {
        let [b, i] = Layout::vertical([Constraint::Length(1), Constraint::Length(3)]).areas(input_all);
        (Some(b), i)
    } else {
        (None, input_all)
    };

    // Header: [toggle] [photo] name / subtitle ... [Info] [Search], and a rule below.
    let chat = app.chat(&chat_id).cloned();
    let name = chat.as_ref().map_or(chat_id.clone(), |c| c.name.clone());
    let is_group = chat.as_ref().is_some_and(|c| c.is_group);
    let avatar_url = chat.as_ref().map(|c| c.avatar.clone()).unwrap_or_default();
    let btn = Style::new().fg(c::FG).bg(c::SURFACE);

    // Folds / unfolds the chat list; in the one-pane layout it goes back to the list.
    // One row high so the glyph sits in the middle of its box; the click area is two rows.
    let glyph = if app.narrow { "‹" } else if app.sidebar_collapsed { "»" } else { "«" };
    let toggle = Rect::new(head.x + 1, head.y, 3, 2);
    f.render_widget(Paragraph::new(Line::styled(format!(" {glyph} "), btn)), Rect::new(toggle.x, toggle.y, 3, 1));
    app.hit.toggle_btn = toggle;

    // Profile photo: an initial tile until the picture is loaded.
    let (initial, tile_color) = avatar_tile(&name);
    let tile = Style::new().fg(c::BG_DARK).bg(tile_color).add_modifier(Modifier::BOLD);
    let av = Rect::new(head.x + 5, head.y, AV_W, 2);
    f.render_widget(Paragraph::new(vec![Line::styled(format!(" {initial}  "), tile), Line::styled("    ", tile)]), av);
    if !avatar_url.is_empty() && !app.hide_images {
        match app.avatars.get_mut(&avatar_url) {
            Some(Img::Ready(img)) => {
                if let Some(pk) = &picker {
                    f.render_stateful_widget(
                        StatefulImage::default().resize(Resize::Scale(Some(FilterType::Triangle))),
                        av,
                        img.proto(pk, Slot::Header),
                    );
                }
            }
            Some(_) => {}
            None => app.want.push((Kind::Avatar, avatar_url.clone())),
        }
    }

    // Name, and under it: who is typing, or what kind of chat this is.
    let text_x = head.x + 5 + AV_W + 1;
    let btn_room = if area.width >= 50 { 16 } else { 0 };
    let text_w = (head.width as usize).saturating_sub((text_x - head.x) as usize + btn_room + 1);
    let sub = match (app.typing_line(&chat_id), is_group) {
        (Some(t), _) => Span::styled(truncate(&t, text_w), Style::new().fg(c::GREEN)),
        (None, true) => Span::styled("group", Style::new().fg(c::DIM)),
        (None, false) => Span::raw(""),
    };
    f.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(truncate(&name, text_w), Style::new().fg(c::FG).add_modifier(Modifier::BOLD))),
            Line::from(sub),
        ]),
        Rect::new(text_x, head.y, text_w as u16, 2),
    );
    f.render_widget(
        Paragraph::new(Line::styled("─".repeat(area.width as usize), Style::new().fg(if focused { c::BLUE } else { c::BORDER }))),
        Rect::new(head.x, head.y + 2, head.width, 1),
    );
    // Info / Search buttons at the right end of the header.
    if area.width >= 50 {
        let x = head.x + head.width - 16;
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(" Info ", btn),
                Span::raw(" "),
                Span::styled(" Search ", btn),
            ])),
            Rect::new(x, head.y, 15, 1),
        );
        app.hit.info_btn = Rect::new(x, head.y, 6, 1);
        app.hit.search_btn = Rect::new(x + 7, head.y, 8, 1);
    }

    // Body.
    app.hit.messages = body;
    app.page = body.height as usize;
    // The last column is the scrollbar.
    let text_w = (body.width as usize).saturating_sub(1);
    let text_area = Rect { width: body.width.saturating_sub(1), ..body };
    let bar_area = Rect { x: body.x + body.width.saturating_sub(1), width: 1, ..body };
    // The lines only change when something happened (see `lines_gen`) or the pane was resized.
    let cached = match app.lines_cache.take() {
        Some(c) if c.chat == chat_id && c.width == text_w && c.gen == app.lines_gen => c,
        _ => {
            let b = match app.logs.get(&chat_id) {
                Some(log) if log.loaded || !log.msgs.is_empty() => build_lines(app, &chat_id, log, text_w),
                Some(_) | None => Built {
                    lines: vec![Line::styled("  Loading messages…", Style::new().fg(c::DIM))],
                    placements: Vec::new(),
                    spans: Vec::new(),
                    links: Vec::new(),
                    quotes: Vec::new(),
                },
            };
            LinesCache {
                chat: chat_id.clone(),
                width: text_w,
                gen: app.lines_gen,
                lines: b.lines,
                placements: b.placements,
                spans: b.spans,
                links: b.links,
                quotes: b.quotes,
            }
        }
    };
    let (lines, placements, spans) = (&cached.lines, &cached.placements, &cached.spans);
    let h = body.height as usize;
    let total = lines.len();
    app.max_scroll = total.saturating_sub(h);
    // Bring a picked or searched-for message into view.
    if let Some((id, center)) = app.reveal.take() {
        if let Some(sp) = spans.iter().find(|sp| sp.msg_id == id) {
            let end_now = total - app.scroll.min(app.max_scroll);
            let start_now = end_now.saturating_sub(h);
            let new_end = if center {
                ((sp.line0 + sp.line1) / 2 + h / 2).min(total)
            } else if sp.line0 < start_now {
                (sp.line0 + h).min(total)
            } else if sp.line1 > end_now {
                sp.line1
            } else {
                end_now
            };
            let goal = total - new_end.max(h.min(total));
            if goal != app.scroll {
                app.smooth.aim(app.scroll, goal.min(app.max_scroll));
            }
        }
    }
    app.scroll = app.scroll.min(app.max_scroll);
    app.at_top = app.scroll >= app.max_scroll;
    let end = total - app.scroll;
    let start = end.saturating_sub(h);
    let mut shown: Vec<Line> = Vec::with_capacity(h);
    // Short conversations sit at the bottom, next to the input.
    for _ in 0..h.saturating_sub(end - start) {
        shown.push(Line::raw(""));
    }
    shown.extend(lines[start..end].iter().cloned());
    let pad = h.saturating_sub(end - start);
    f.render_widget(Paragraph::new(shown), text_area);

    // Bubbles on screen: for right-click, and a marker beside the picked one.
    for sp in spans.iter() {
        let (a, b) = (sp.line0.max(start), sp.line1.min(end));
        if a >= b {
            continue;
        }
        let y = text_area.y + (pad + a - start) as u16;
        let rect = Rect::new(
            text_area.x + sp.x0 as u16,
            y,
            ((sp.x1 - sp.x0) as u16).min(text_area.width.saturating_sub(sp.x0 as u16)),
            (b - a) as u16,
        );
        app.bubble_hits.push((rect, sp.msg_id.clone()));
        if app.sel_msg.as_deref() == Some(sp.msg_id.as_str()) {
            let marks: Vec<Line> = (0..b - a).map(|_| Line::styled("▌", Style::new().fg(c::YELLOW))).collect();
            f.render_widget(Paragraph::new(marks), Rect::new(text_area.x, y, 1, (b - a) as u16));
        }
    }

    // Links and quoted lines on screen: clickable.
    let on_screen = |line: usize, x0: usize, x1: usize| -> Option<Rect> {
        if line < start || line >= end || x0 >= text_area.width as usize {
            return None;
        }
        let w = (x1.min(text_area.width as usize) - x0) as u16;
        Some(Rect::new(text_area.x + x0 as u16, text_area.y + (pad + line - start) as u16, w, 1))
    };
    for l in &cached.links {
        if let Some(r) = on_screen(l.line, l.x0, l.x1) {
            app.link_hits.push((r, l.url.clone()));
        }
    }
    for q in &cached.quotes {
        if let Some(r) = on_screen(q.line, q.x0, q.x1) {
            app.quote_hits.push((r, q.target.clone()));
        }
    }
    // Text picked with the mouse: remember how rows map to lines, paint the selection.
    app.msg_view = MsgView { area: text_area, start, pad };
    if let Some(sel) = app.selection.filter(|s| !s.is_empty()) {
        let ((l0, c0), (l1, c1)) = sel.ordered();
        for line in l0.max(start)..=l1.min(end.saturating_sub(1)) {
            if line < start || line >= end {
                continue;
            }
            let from = if line == l0 { c0 } else { 0 };
            let to = if line == l1 { c1 } else { text_area.width as usize };
            let (from, to) = (from.min(text_area.width as usize), to.min(text_area.width as usize));
            if from < to {
                let r = Rect::new(text_area.x + from as u16, text_area.y + (pad + line - start) as u16, (to - from) as u16, 1);
                f.buffer_mut().set_style(r, Style::new().add_modifier(Modifier::REVERSED));
            }
        }
    }

    // Pictures: fetch what is near the view; draw every one that touches it, cut by
    // the pane's edges so it shows partly while scrolling.
    for p in placements.iter() {
        let near = p.line + p.h as usize > start.saturating_sub(h) && p.line < end + h;
        if near && app.media.get(&p.url).is_none() {
            app.want.push((Kind::Media, p.url.clone()));
        }
        let (a, b) = (p.line.max(start), (p.line + p.h as usize).min(end));
        if a >= b {
            continue;
        }
        let width = p.w.min(text_area.width.saturating_sub(p.x));
        if width == 0 {
            continue;
        }
        let rect = Rect::new(text_area.x + p.x, text_area.y + (pad + a - start) as u16, width, (b - a) as u16);
        app.media_hits.push((rect, p.msg_id.clone()));
        if app.hide_images {
            continue;
        }
        if let (Some(Img::Ready(img)), Some(pk)) = (app.media.get_mut(&p.url), &picker) {
            let size = Size::new(p.w, p.h);
            // Encoding is the slow part: two per frame, the rest on the next one.
            let ok = img.is_built(Slot::Bubble, size) || images::take_budget(&mut app.build_budget, &mut app.pending_builds);
            let sl = if ok { img.sliced(pk, Slot::Bubble, size) } else { None };
                    if let Some(sl) = sl {
                // Row of the picture's top edge inside the pane; negative when it starts above it.
                let top = (pad as i64 + p.line as i64 - start as i64).clamp(-3000, 3000) as i16;
                f.render_widget(SlicedImage::new(sl, SignedPosition::from((p.x as i16, top))), text_area);
            }
        }
    }
    app.hit.msg_bar = if app.max_scroll > 0 { bar_area } else { Rect::default() };
    if app.max_scroll > 0 {
        let mut bar = ScrollbarState::new(app.max_scroll).position(app.max_scroll - app.scroll);
        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .track_symbol(Some("│"))
                .thumb_symbol("┃")
                .track_style(Style::new().fg(c::BORDER))
                .thumb_style(Style::new().fg(c::BLUE)),
            bar_area,
            &mut bar,
        );
    }

    // A button back to the newest message while scrolled up (or looking at an old
    // stretch around a search hit): click it, or press G.
    let detached = app.logs.get(&chat_id).is_some_and(|l| l.detached);
    if app.scroll > 0 || detached {
        let tag = if app.scroll > 0 {
            format!(" ↓ {} more lines · click or G for latest ", app.scroll)
        } else {
            " ↓ older messages · click or G for latest ".to_string()
        };
        let w = width(&tag) as u16;
        if body.width > w + 2 {
            let r = Rect::new(body.x + body.width - w - 2, body.y + body.height - 1, w, 1);
            f.render_widget(Paragraph::new(Span::styled(tag, Style::new().fg(c::BG).bg(c::YELLOW).add_modifier(Modifier::BOLD))), r);
            app.hit.latest_btn = r;
        }
    }

    // Input.
    if let (Some(r), Some((title, quoted))) = (compose_area, compose) {
        let hint = "esc cancel ";
        let room = (r.width as usize).saturating_sub(width(&title) + width(hint) + 6);
        let q = if quoted.is_empty() { String::new() } else { format!("  {}", truncate(&quoted, room)) };
        let gap = (r.width as usize).saturating_sub(width(&title) + width(&q) + width(hint) + 3);
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(" ↪ ", Style::new().fg(c::CYAN)),
                Span::styled(title, Style::new().fg(c::CYAN).add_modifier(Modifier::BOLD)),
                Span::styled(q, Style::new().fg(c::DIM)),
                Span::raw(" ".repeat(gap)),
                Span::styled(hint, Style::new().fg(c::DIM)),
            ])),
            r,
        );
    }
    // The "+" button sits left of the message field.
    let [attach_btn, input] = Layout::horizontal([Constraint::Length(5), Constraint::Min(1)]).areas(input);
    let plus = Block::bordered().border_type(BorderType::Rounded).border_style(Style::new().fg(c::BORDER));
    let plus_inner = plus.inner(attach_btn);
    f.render_widget(plus, attach_btn);
    f.render_widget(
        Paragraph::new(Line::styled(" + ", Style::new().fg(c::BLUE).add_modifier(Modifier::BOLD))),
        plus_inner,
    );
    app.hit.attach_btn = attach_btn;
    app.hit.input = input;
    let inserting = app.mode == Mode::Insert;
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(if inserting { c::GREEN } else { c::BORDER }));
    let inner = block.inner(input);
    f.render_widget(block, input);
    let iw = inner.width as usize;
    if app.input.text.is_empty() {
        let hint = if inserting { "Type a message…" } else { "Press i to write a message" };
        f.render_widget(Paragraph::new(Span::styled(format!(" {hint}"), Style::new().fg(c::DIM))), inner);
    } else {
        // Scroll horizontally so the cursor stays in view.
        let col = app.input.cursor_col();
        let skip = (col + 2).saturating_sub(iw);
        let mut cells = 0;
        let mut shown = String::new();
        for ch in app.input.text.chars() {
            let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
            if cells >= skip {
                shown.push(ch);
            }
            cells += w;
        }
        f.render_widget(Paragraph::new(Span::styled(format!(" {shown}"), Style::new().fg(c::FG))), inner);
        if inserting {
            f.set_cursor_position((inner.x + 1 + (col - skip) as u16, inner.y));
        }
    }
    if inserting && app.input.text.is_empty() {
        f.set_cursor_position((inner.x + 1, inner.y));
    }
    app.lines_cache = Some(cached);
}

fn draw_welcome(f: &mut Frame, area: Rect) {
    let lines = vec![
        Line::styled("WA Bot", Style::new().fg(c::BLUE).add_modifier(Modifier::BOLD)),
        Line::raw(""),
        Line::styled("j / k   move through chats", Style::new().fg(c::DIM)),
        Line::styled("Enter   open the chat", Style::new().fg(c::DIM)),
        Line::styled("/       filter chats", Style::new().fg(c::DIM)),
        Line::styled("q       quit", Style::new().fg(c::DIM)),
    ];
    // One left-aligned block, centered as a whole.
    let h = lines.len() as u16;
    let w = 28.min(area.width);
    let r = Rect::new(
        area.x + (area.width - w) / 2,
        area.y + area.height.saturating_sub(h) / 2,
        w,
        h.min(area.height),
    );
    f.render_widget(Paragraph::new(lines), r);
}

/// Every line of the conversation at this width, oldest first. Messages are
/// bubbles: mine on the right, theirs on the left.
fn build_lines(app: &App, chat_id: &str, log: &ChatLog, pane_w: usize) -> Built {
    let chat = app.chat(chat_id);
    let is_group = chat.is_some_and(|c| c.is_group);
    let chat_name = chat.map_or("", |c| c.name.as_str());

    let label = |m: &Message| -> String {
        if is_from_me(m) {
            return "You".to_string();
        }
        if let Some(n) = m.sender_name.as_deref().filter(|n| !n.is_empty()) {
            return n.to_string();
        }
        if !is_group && !chat_name.is_empty() {
            return chat_name.to_string();
        }
        m.from.split('@').next().unwrap_or("?").to_string()
    };

    // A bubble takes at most ~72% of the pane; its text has 1 cell of padding each side.
    let outer_max = (pane_w * 72 / 100).clamp(24, pane_w.saturating_sub(2).max(24));
    let inner_max = outer_max - 2;

    let mut out: Vec<Line<'static>> = Vec::new();
    let mut placements: Vec<Placement> = Vec::new();
    let mut spans: Vec<BubbleSpan> = Vec::new();
    let mut links: Vec<LinkSpan> = Vec::new();
    let mut quotes: Vec<QuoteSpan> = Vec::new();
    let centered = |text: &str, style: Style| Line::from(Span::styled(text.to_string(), style)).centered();
    if log.loading && !log.msgs.is_empty() {
        out.push(centered("loading older messages…", Style::new().fg(c::DIM)));
    } else if log.exhausted {
        out.push(centered("— start of the conversation —", Style::new().fg(c::DIM)));
    }
    if log.msgs.is_empty() {
        out.push(centered("No messages yet", Style::new().fg(c::DIM)));
    }

    let mut last_day = String::new();
    let mut last_sender = String::new();
    let mut last_ts = 0i64;
    for m in &log.msgs {
        let day = day_label(m.timestamp);
        if day != last_day {
            out.push(Line::raw(""));
            out.push(centered(&format!(" {day} "), Style::new().fg(c::DIM).bg(c::SURFACE)));
            last_day = day;
            last_sender.clear();
        }

        let sender = label(m);
        let mine = is_from_me(m);
        let same_run = sender == last_sender && m.timestamp - last_ts < 5 * 60_000;
        last_sender = sender.clone();
        last_ts = m.timestamp;
        // One blank row between bubbles, so neighbours do not touch.
        out.push(Line::raw(""));
        if !same_run {
            // Who is talking: only in groups, only for others, once per run.
            if is_group && !mine {
                out.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        truncate(&sender, inner_max),
                        Style::new().fg(name_color(&sender)).add_modifier(Modifier::BOLD),
                    ),
                ]));
            }
        }

        let bg = if mine { c::SELECT } else { c::SURFACE };
        let body_style = if m.revoked_at.is_some() {
            Style::new().fg(c::DIM).add_modifier(Modifier::ITALIC)
        } else {
            Style::new().fg(c::FG)
        };
        let mut body: Vec<Row> = wrap_rows(&message_text(m), inner_max)
            .into_iter()
            .map(|w| Row { text: w.text, style: body_style, links: w.links })
            .collect();
        if let Some(title) = m
            .extra
            .as_ref()
            .and_then(|e| e.link_preview.as_ref())
            .and_then(|lp| lp.title.as_deref())
            .filter(|t| !t.is_empty())
        {
            body.push(Row::plain(format!("↳ {}", truncate(title, inner_max.saturating_sub(2))), Style::new().fg(c::CYAN)));
        }

        let time = local(m.timestamp).map(|t| t.format("%H:%M").to_string()).unwrap_or_default();
        let mut meta = vec![Span::styled(time, Style::new().fg(c::DIM))];
        if mine {
            let (tick, color) = match m.status.as_str() {
                "read" => (format!(" {}", crate::ticks::double()), c::CYAN),
                "delivered" => (format!(" {}", crate::ticks::double()), c::DIM),
                "failed" => (" !".to_string(), c::RED),
                "pending" => (" …".to_string(), c::DIM),
                _ => (format!(" {}", crate::ticks::single()), c::DIM),
            };
            meta.push(Span::styled(tick, Style::new().fg(color)));
        }
        // A picture goes first in the bubble: reserve its rows, the renderer draws over them.
        let mut thumb: Option<(String, u16, u16)> = None;
        if let (Some(url), Some(picker)) = (images::media_url(m), app.picker.as_ref()) {
            let font = picker.font_size();
            // Stickers are small and sit without a bubble; pictures can be big.
            let sticker = m.message_type == "sticker";
            let (max_cols, max_rows) = if sticker { (inner_max.min(16) as u16, 8) } else { (inner_max.min(40) as u16, 14) };
            let (cols, rows, note) = match app.media.get(&url) {
                Some(Img::Ready(r)) => {
                    let (cols, rows) = images::fit(r.w, r.h, (font.width, font.height), max_cols, max_rows, false);
                    (cols, rows, "")
                }
                Some(Img::Failed) => (0, 0, ""),
                _ => (max_cols.min(24), 6, "loading picture…"),
            };
            if cols > 0 {
                let mut rows_text: Vec<Row> =
                    (0..rows).map(|_| Row::plain(" ".repeat(cols as usize), Style::new())).collect();
                if !note.is_empty() {
                    rows_text[0] = Row::plain(pad_to(note, cols as usize), Style::new().fg(c::DIM));
                }
                body.splice(0..0, rows_text);
                // The picture says "image" already: drop the tag from the caption.
                if m.message_type != "video" {
                    if let Some(tag) = kind_label(m) {
                        let at = rows as usize;
                        // The backend writes "[Sticker]" / "[Image]": match the tag ignoring case.
                        let rest = body.get(at).and_then(|row| {
                            let t = &row.text;
                            let hit = t.is_char_boundary(tag.len()) && t[..tag.len()].eq_ignore_ascii_case(&tag);
                            hit.then(|| t[tag.len()..].trim_start().to_string())
                        });
                        if let Some(r) = rest {
                            // The text moved: byte offsets of its links no longer apply.
                            body[at].text = r;
                            body[at].links.clear();
                        }
                    }
                }
                thumb = Some((url, cols, rows));
            }
        }
        let bg = if m.message_type == "sticker" && thumb.is_some() { c::BG } else { bg };
        // A reply quotes what it answers, above everything else.
        let mut quote_rows = 0;
        if let Some(rid) = m.reply_to_id.as_deref().filter(|r| !r.is_empty()) {
            let q = match log.msgs.iter().find(|x| x.id == rid) {
                Some(orig) => format!("↪ {}: {}", app.sender_label(chat_id, orig), crate::app::preview(orig)),
                None => "↪ Reply".to_string(),
            };
            body.insert(0, Row::plain(truncate(&q, inner_max), Style::new().fg(c::CYAN)));
            quote_rows = 1;
        }
        let first_line = out.len();
        let (margin, outer, found) = push_bubble(&mut out, pane_w, mine, &body, meta, bg, inner_max);
        for (row, x0, x1, url) in found {
            links.push(LinkSpan { line: first_line + row, x0, x1, url });
        }
        if quote_rows == 1 {
            if let Some(rid) = m.reply_to_id.as_deref().filter(|r| !r.is_empty()) {
                let w = width(&body[0].text);
                quotes.push(QuoteSpan { line: first_line, x0: margin + 1, x1: margin + 1 + w, target: rid.to_string() });
            }
        }
        if let Some((url, w, h)) = thumb {
            placements.push(Placement { line: first_line + quote_rows, x: (margin + 1) as u16, w, h, url, msg_id: m.id.clone() });
        }

        if let Some(reactions) = m.reactions.as_ref().filter(|r| !r.is_empty()) {
            let line = reactions
                .iter()
                .map(|r| format!("{} {}", r.emoji, r.senders.len().max(1)))
                .collect::<Vec<_>>()
                .join("  ");
            let line = truncate(&line, inner_max);
            let margin = if mine { pane_w.saturating_sub(width(&line) + 2) } else { 2 };
            out.push(Line::from(vec![
                Span::raw(" ".repeat(margin)),
                Span::styled(line, Style::new().fg(c::YELLOW)),
            ]));
        }
        spans.push(BubbleSpan { msg_id: m.id.clone(), line0: first_line, line1: out.len(), x0: margin, x1: margin + outer });
    }
    Built { lines: out, placements, spans, links, quotes }
}

/// Everything `build_lines` makes: the lines and where things sit among them.
pub(crate) struct Built {
    lines: Vec<Line<'static>>,
    placements: Vec<Placement>,
    spans: Vec<BubbleSpan>,
    links: Vec<LinkSpan>,
    quotes: Vec<QuoteSpan>,
}

/// One row of a bubble: text, its base style and the links inside it.
struct Row {
    text: String,
    style: Style,
    links: Vec<LinkSeg>,
}

impl Row {
    fn plain(text: String, style: Style) -> Self {
        Self { text, style, links: Vec::new() }
    }
}

/// A link on screen-to-be: line among all lines, columns inside the pane, address.
pub(crate) struct LinkSpan {
    line: usize,
    x0: usize,
    x1: usize,
    url: String,
}

/// The quoted line of a reply: clicking it goes to the message it answers.
pub(crate) struct QuoteSpan {
    line: usize,
    x0: usize,
    x1: usize,
    target: String,
}

/// The text between two points of the conversation, rows joined by newlines. Cells
/// are counted like on screen, so wide characters count two.
pub fn selected_text(app: &App, sel: crate::app::Selection) -> String {
    let Some(cache) = app.lines_cache.as_ref() else { return String::new() };
    let ((l0, c0), (l1, c1)) = sel.ordered();
    let mut rows: Vec<String> = Vec::new();
    for line in l0..=l1.min(cache.lines.len().saturating_sub(1)) {
        let full: String = cache.lines[line].spans.iter().map(|s| s.content.as_ref()).collect();
        let from = if line == l0 { c0 } else { 0 };
        let to = if line == l1 { c1 } else { usize::MAX };
        let mut text = String::new();
        let mut col = 0;
        for ch in full.chars() {
            let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
            if col >= from && col + w <= to {
                text.push(ch);
            }
            col += w;
        }
        rows.push(text.trim().to_string());
    }
    // Blank rows between bubbles stay as one empty line at most; ends are trimmed.
    rows.join("\n").trim().to_string()
}

/// The conversation's lines for one chat at one width, kept between frames.
pub struct LinesCache {
    chat: String,
    width: usize,
    gen: u64,
    lines: Vec<Line<'static>>,
    placements: Vec<Placement>,
    spans: Vec<BubbleSpan>,
    links: Vec<LinkSpan>,
    quotes: Vec<QuoteSpan>,
}

/// Where one message's bubble sits among the conversation lines.
pub(crate) struct BubbleSpan {
    msg_id: String,
    /// First line, and one past the last (reactions included).
    line0: usize,
    line1: usize,
    /// Columns inside the pane.
    x0: usize,
    x1: usize,
}

/// A picture inside the conversation lines: where it sits and what to draw there.
pub(crate) struct Placement {
    /// Index of its first line among all conversation lines.
    line: usize,
    /// Column inside the message pane.
    x: u16,
    w: u16,
    h: u16,
    url: String,
    msg_id: String,
}

fn pad_to(s: &str, w: usize) -> String {
    format!("{s}{}", " ".repeat(w.saturating_sub(width(s))))
}

/// One bubble: `body` rows on `bg`, then `meta` (time, ticks) at the bottom right,
/// on the last text row when it fits and on a row of its own otherwise. Besides the
/// margin and width it says where the links are: (row, first column, end column, address).
fn push_bubble(
    out: &mut Vec<Line<'static>>,
    pane_w: usize,
    mine: bool,
    body: &[Row],
    meta: Vec<Span<'static>>,
    bg: Color,
    inner_max: usize,
) -> (usize, usize, Vec<(usize, usize, usize, String)>) {
    let meta_w: usize = meta.iter().map(|s| width(&s.content)).sum();
    let widest = body.iter().map(|r| width(&r.text)).max().unwrap_or(0);
    let last_w = body.last().map_or(0, |r| width(&r.text));
    let inline = last_w + 2 + meta_w <= inner_max;
    let inner = if inline { widest.max(last_w + 2 + meta_w) } else { widest.max(meta_w) };
    let margin = if mine { pane_w.saturating_sub(inner + 2 + 1) } else { 1 };

    let row = |mut spans: Vec<Span<'static>>| {
        let mut all = vec![Span::raw(" ".repeat(margin)), Span::styled(" ", Style::new().bg(bg))];
        all.append(&mut spans);
        all.push(Span::styled(" ", Style::new().bg(bg)));
        Line::from(all)
    };
    let with_bg = |spans: Vec<Span<'static>>| -> Vec<Span<'static>> {
        spans.into_iter().map(|s| Span { style: s.style.bg(bg), content: s.content }).collect()
    };

    let link_style = Style::new().fg(c::BLUE).add_modifier(Modifier::UNDERLINED);
    let mut found = Vec::new();
    let count = body.len();
    for (i, r) in body.iter().enumerate() {
        // Plain text and links alternate; a link is underlined and clickable.
        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut at = 0;
        for l in &r.links {
            let Some(before) = r.text.get(at..l.start) else { continue };
            let Some(label) = r.text.get(l.start..l.end) else { continue };
            if !before.is_empty() {
                spans.push(Span::styled(before.to_string(), r.style.bg(bg)));
            }
            spans.push(Span::styled(label.to_string(), r.style.patch(link_style).bg(bg)));
            let x0 = margin + 1 + width(&r.text[..l.start]);
            found.push((i, x0, x0 + width(label), l.url.clone()));
            at = l.end;
        }
        if let Some(rest) = r.text.get(at..).filter(|t| !t.is_empty() || spans.is_empty()) {
            spans.push(Span::styled(rest.to_string(), r.style.bg(bg)));
        }
        if i + 1 == count && inline {
            let gap = inner - width(&r.text) - meta_w;
            spans.push(Span::styled(" ".repeat(gap), Style::new().bg(bg)));
            spans.extend(with_bg(meta.clone()));
        } else {
            spans.push(Span::styled(" ".repeat(inner - width(&r.text)), Style::new().bg(bg)));
        }
        out.push(row(spans));
    }
    if !inline {
        let mut spans = vec![Span::styled(" ".repeat(inner - meta_w), Style::new().bg(bg))];
        spans.extend(with_bg(meta));
        out.push(row(spans));
    }
    (margin, inner + 2, found)
}

/// The text of one message as shown: tag, mentions by name, flags.
fn message_text(m: &Message) -> String {
    let mut content = m.content.clone();
    if let Some(extra) = &m.extra {
        for (jid, name) in &extra.mention_names {
            let user = jid.split('@').next().unwrap_or(jid);
            content = content.replace(&format!("@{user}"), &format!("@{name}"));
        }
    }
    let mut out = String::new();
    if m.forwarded == Some(true) {
        out.push_str("⤳ ");
    }
    out.push_str(&with_tag(m, &content));
    if m.extra.as_ref().is_some_and(|e| e.edited_at.is_some()) {
        out.push_str(" (edited)");
    }
    if m.revoked_at.is_some() {
        out.push_str(" (deleted)");
    }
    if m.starred {
        out.push_str(" ★");
    }
    out
}

// ---- viewer ------------------------------------------------------------------

fn draw_viewer(f: &mut Frame, app: &mut App, area: Rect) {
    let picker = app.picker.clone();
    let Some(v) = app.viewer.clone() else { return };
    if app.image_msgs(&v.chat_id).is_empty() {
        app.viewer = None;
        return;
    }
    // Everything needed from the message list, owned, before borrowing the cache.
    let (n, idx, url, caption, around) = {
        let list = app.image_msgs(&v.chat_id);
        let idx = list.iter().position(|m| m.id == v.msg_id).unwrap_or(0);
        let m = list[idx];
        let who = if is_from_me(m) {
            "You".to_string()
        } else {
            m.sender_name.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| m.from.split('@').next().unwrap_or("").to_string())
        };
        let time = local(m.timestamp).map(|t| t.format("%-d %b %Y, %H:%M").to_string()).unwrap_or_default();
        let mut caption = format!("{who}  ·  {time}");
        if m.message_type == "video" {
            caption.push_str("  ·  video (preview only)");
        }
        let text = m.content.trim().lines().next().unwrap_or("");
        if !text.is_empty() && !text.starts_with('[') {
            caption.push_str(&format!("  ·  {text}"));
        }
        let around: Vec<String> = [idx.checked_sub(1), Some(idx + 1)]
            .into_iter()
            .flatten()
            .filter_map(|i| list.get(i).and_then(|m| images::media_url(m)))
            .collect();
        (list.len(), idx, images::media_url(m).unwrap_or_default(), caption, around)
    };
    // Load this one and its neighbours so stepping is instant.
    for u in std::iter::once(url.clone()).chain(around) {
        if app.media.get(&u).is_none() {
            app.want.push((Kind::Media, u));
        }
    }

    f.render_widget(Block::default().style(Style::new().bg(c::BG_DARK)), area);
    let [top, body, bottom] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(1), Constraint::Length(1)]).areas(area);
    let title = format!(" {}  {}/{}", app.chat(&v.chat_id).map_or("", |c| c.name.as_str()), idx + 1, n);
    f.render_widget(
        Paragraph::new(Span::styled(title, Style::new().fg(c::FG).add_modifier(Modifier::BOLD))),
        top,
    );
    f.render_widget(
        Paragraph::new(Span::styled(format!(" {}", truncate(&caption, area.width as usize - 2)), Style::new().fg(c::DIM))),
        bottom,
    );

    let inner = Rect::new(body.x + 1, body.y, body.width.saturating_sub(2), body.height);
    let font = app.picker.as_ref().map(|p| p.font_size());
    match (app.media.get_mut(&url), font) {
        (Some(Img::Ready(img)), Some(font)) => {
            let (cols, rows) = images::fit(img.w, img.h, (font.width, font.height), inner.width, inner.height, true);
            let rect = Rect::new(
                inner.x + (inner.width - cols) / 2,
                inner.y + (inner.height - rows) / 2,
                cols,
                rows,
            );
            if let Some(pk) = &picker {
                f.render_stateful_widget(
                    StatefulImage::default().resize(Resize::Scale(Some(FilterType::CatmullRom))),
                    rect,
                    img.proto(pk, Slot::Viewer),
                );
            }
        }
        (Some(Img::Failed), _) => centered_note(f, inner, "Could not load this picture (r to retry)", c::RED),
        _ => centered_note(f, inner, "Loading…", c::DIM),
    }
}

fn centered_note(f: &mut Frame, area: Rect, text: &str, color: Color) {
    let y = area.y + area.height / 2;
    f.render_widget(
        Paragraph::new(Span::styled(text.to_string(), Style::new().fg(color))).centered(),
        Rect::new(area.x, y, area.width, 1),
    );
}

// ---- statusline --------------------------------------------------------------

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let (label0, color0) = match app.mode {
        Mode::Normal => (" NORMAL ", c::BLUE),
        Mode::Insert => (" INSERT ", c::GREEN),
        Mode::Filter => (" FILTER ", c::YELLOW),
        Mode::Search => (" SEARCH ", c::YELLOW),
        Mode::Caption => (" PICTURE ", c::GREEN),
    };
    let (label, color) = if app.menu.is_some() {
        (" MENU ", c::MAGENTA)
    } else if app.browser.is_some() {
        (" FILES ", c::BLUE)
    } else if app.stickers.is_some() {
        (" STICKERS ", c::MAGENTA)
    } else if app.form.is_some() {
        (" FORM ", c::GREEN)
    } else if app.viewer.is_some() {
        (" IMAGE ", c::MAGENTA)
    } else {
        (label0, color0)
    };
    let mut left = vec![
        Span::styled(label, Style::new().fg(c::BG).bg(color).add_modifier(Modifier::BOLD)),
        Span::styled(" ", Style::new().bg(c::SURFACE)),
    ];
    let place = match &app.open {
        Some(id) => app.chat(id).map_or(id.as_str(), |c| c.name.as_str()).to_string(),
        None => "no chat open".to_string(),
    };
    left.push(Span::styled(place, Style::new().fg(c::FG).bg(c::SURFACE)));

    let mut right_text = String::new();
    if let Some(n) = app.live_notice() {
        right_text = format!("{n}  ");
    }
    let hints_mode = match app.mode {
        Mode::Normal => "j/k move  ⏎ open  i write  . menu  f find  I info  o pic  q quit",
        Mode::Insert => "⏎ send  esc normal",
        Mode::Filter => "⏎ keep  esc clear",
        Mode::Search => "⏎ search  esc leave",
        Mode::Caption => "⏎ send  esc cancel",
    };
    let hints = if app.menu.is_some() {
        "j/k move  ⏎ select  esc back"
    } else if app.browser.is_some() {
        "j/k move  ⏎ open  h up  ~ home  . hidden  esc close"
    } else if app.stickers.is_some() {
        "arrows move  ⏎ send  tab switch  esc close"
    } else if app.form.is_some() {
        "tab next  ⏎ send  ctrl-s send  esc cancel"
    } else if app.viewer.is_some() {
        "h/l prev/next  g/G first/last  esc close"
    } else if app.focus == Focus::Panel && app.mode == Mode::Normal {
        "h/l tab  j/k move  ⏎ open  esc close"
    } else {
        hints_mode
    };
    let left_w: usize = left.iter().map(|s| width(&s.content)).sum();
    let room = (area.width as usize).saturating_sub(left_w + 2);
    let notice_w = width(&right_text);
    let mut spans = left;
    let mut used = 0;
    if notice_w > 0 && notice_w <= room {
        used += notice_w;
    } else {
        right_text.clear();
    }
    let hint = if width(hints) + used + 2 <= room { hints } else { "" };
    used += width(hint);
    spans.push(Span::styled(" ".repeat(room.saturating_sub(used)), Style::new().bg(c::SURFACE)));
    if !right_text.is_empty() {
        spans.push(Span::styled(right_text, Style::new().fg(c::YELLOW).bg(c::SURFACE)));
    }
    spans.push(Span::styled(hint, Style::new().fg(c::DIM).bg(c::SURFACE)));
    spans.push(Span::styled("  ", Style::new().bg(c::SURFACE)));
    f.render_widget(Paragraph::new(Line::from(spans)).style(Style::new().bg(c::SURFACE)), area);
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use image::{DynamicImage, ImageBuffer, Rgba};
    use ratatui::crossterm::event::{Event as CtEvent, KeyCode, KeyEvent, KeyModifiers};
    use ratatui_image::picker::Picker;
    use wabot_backend_client::{Chat, HttpClient, Message, ReactionEntry};

    use super::*;
    use crate::app::{App, ChatLog, Event};
    use crate::images::Ready;

    fn msg(id: &str, from: &str, text: &str, ts: i64) -> Message {
        Message {
            id: id.into(),
            chat_id: "g@g.us".into(),
            from: from.into(),
            content: text.into(),
            timestamp: ts,
            status: if from == "me" { "read".into() } else { "received".into() },
            message_type: "text".into(),
            sender_name: (from != "me").then(|| "Angger".to_string()),
            ..Default::default()
        }
    }

    /// Fake data only: nothing here touches a backend.
    fn render(width: u16, height: u16) -> Vec<String> {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let (tx, _rx) = flume::unbounded();
        let mut app = App::new(Arc::new(HttpClient::from_port(1)), rt.handle().clone(), tx, Some(Picker::halfblocks()));
        app.chats.push(Chat {
            id: "g@g.us".into(),
            name: "AkuKatiga".into(),
            is_group: true,
            last_time: 1,
            ..serde_json::from_str(r#"{"id":"x"}"#).unwrap()
        });
        let now = crate::app::now_ms();
        let mut hi = msg("1", "628@s.whatsapp.net", "halo, besok jadi ngumpul jam berapa? tolong dikabari ya", now - 90_000);
        hi.reactions = Some(vec![ReactionEntry { emoji: "👍".into(), senders: vec!["a".into(), "b".into()] }]);
        let log = ChatLog {
            msgs: vec![
                hi,
                msg("2", "628@s.whatsapp.net", "yang lain juga belum tau", now - 80_000),
                msg("3", "me", "jam 7 aja", now - 60_000),
                msg("4", "me", "nanti aku kabari lagi kalau ada perubahan, jangan lupa bawa laptop", now - 30_000),
            ],
            loaded: true,
            exhausted: true,
            ..Default::default()
        };
        app.logs.insert("g@g.us".into(), log);
        app.open = Some("g@g.us".into());
        app.cursor = app.open.clone();

        let mut term = Terminal::new(TestBackend::new(width, height)).unwrap();
        term.draw(|f| draw(f, &mut app)).unwrap();
        let buf = term.backend().buffer().clone();
        (0..height)
            .map(|y| (0..width).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>())
            .collect()
    }

    #[test]
    fn my_messages_sit_right_and_theirs_left() {
        let rows = render(100, 30);
        for r in &rows {
            println!("{r}");
        }
        let col = |needle: &str| {
            let row = rows.iter().find(|r| r.contains(needle)).unwrap_or_else(|| panic!("no row with {needle}"));
            row.find(needle).map(|b| row[..b].chars().count()).unwrap()
        };
        assert!(col("halo, besok") < 45, "theirs on the left");
        assert!(col("jam 7 aja") > 55, "mine on the right");
        // Name only in groups, only for others, once per run.
        assert_eq!(rows.iter().filter(|r| r.contains("Angger")).count(), 1);
    }

    #[test]
    fn narrow_window_still_draws() {
        let rows = render(40, 16);
        assert!(rows.iter().any(|r| r.contains("AkuKatiga")));
    }

    fn gradient(w: u32, h: u32) -> DynamicImage {
        ImageBuffer::from_fn(w, h, |x, y| Rgba([(x * 255 / w) as u8, (y * 255 / h) as u8, 160, 255])).into()
    }

    fn ready(picker: &Picker, w: u32, h: u32) -> Ready {
        let _ = picker;
        Ready::new(gradient(w, h))
    }

    fn key(app: &mut App, code: KeyCode) {
        app.handle(Event::Input(CtEvent::Key(KeyEvent::new(code, KeyModifiers::NONE))));
    }

    fn screen(term: &Terminal<TestBackend>) -> Vec<String> {
        let buf = term.backend().buffer();
        let a = buf.area;
        (0..a.height).map(|y| (0..a.width).map(|x| buf[(x, y)].symbol().to_string()).collect()).collect()
    }

    /// A chat with one picture message and a profile photo, all pre-loaded; no network.
    fn app_with_picture(rt: &tokio::runtime::Runtime) -> App {
        let (tx, _rx) = flume::unbounded();
        let picker = Picker::halfblocks();
        let mut app = App::new(Arc::new(HttpClient::from_port(1)), rt.handle().clone(), tx, Some(picker.clone()));
        app.chats.push(Chat {
            id: "g@g.us".into(),
            name: "AkuKatiga".into(),
            avatar: "https://cdn.example/a.png".into(),
            is_group: true,
            last_time: 1,
            ..serde_json::from_str(r#"{"id":"x"}"#).unwrap()
        });
        let now = crate::app::now_ms();
        let mut pic = msg("p1", "628@s.whatsapp.net", "liat ini", now - 60_000);
        pic.message_type = "image".into();
        pic.media_url = Some("/media/p1.png".into());
        app.logs.insert(
            "g@g.us".into(),
            ChatLog {
                msgs: vec![msg("0", "me", "halo", now - 90_000), pic, msg("2", "me", "bagus", now - 30_000)],
                loaded: true,
                exhausted: true,
                ..Default::default()
            },
        );
        app.media.begin("/media/p1.png");
        app.media.finish("/media/p1.png", Some(ready(&picker, 400, 300)));
        app.avatars.begin("https://cdn.example/a.png");
        app.avatars.finish("https://cdn.example/a.png", Some(ready(&picker, 64, 64)));
        app.open = Some("g@g.us".into());
        app.cursor = app.open.clone();
        app
    }

    #[test]
    fn thumbnails_avatars_and_the_viewer_render() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        let mut term = Terminal::new(TestBackend::new(110, 36)).unwrap();

        term.draw(|f| draw(f, &mut app)).unwrap();
        let rows = screen(&term);
        for r in &rows {
            println!("{r}");
        }
        // The thumbnail took rows in the bubble and is hit-testable.
        assert_eq!(app.media_hits.len(), 1);
        let (rect, id) = app.media_hits[0].clone();
        assert_eq!(id, "p1");
        assert!(rect.width >= 10 && rect.height >= 3, "thumbnail box {rect:?}");
        // Halfblock pixels (the gradient) are inside that box ...
        let in_box = rows[rect.y as usize..(rect.y + rect.height) as usize]
            .iter()
            .any(|r| r.chars().any(|c| c == '▀' || c == '▄'));
        assert!(in_box, "thumbnail not drawn");
        // ... and the profile photo replaced the initial tile in the sidebar.
        assert!(rows[2].chars().take(8).any(|c| c == '▀' || c == '▄'), "avatar not drawn: {:?}", rows[2]);

        // `o` opens the viewer on it; the picture fills the middle of the screen.
        key(&mut app, KeyCode::Char('o'));
        assert!(app.viewer.is_some());
        term.draw(|f| draw(f, &mut app)).unwrap();
        let rows = screen(&term);
        for r in &rows {
            println!("{r}");
        }
        assert!(rows.last().unwrap().contains("IMAGE"));
        let blocks: usize = rows.iter().map(|r| r.chars().filter(|c| *c == '▀' || *c == '▄').count()).sum();
        assert!(blocks > 500, "viewer picture too small: {blocks}");
        assert!(rows.iter().any(|r| r.contains("1/1")));

        key(&mut app, KeyCode::Esc);
        assert!(app.viewer.is_none());
    }

    #[test]
    fn pictures_off_keeps_text_only() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        app.picker = None;
        let mut term = Terminal::new(TestBackend::new(110, 36)).unwrap();
        term.draw(|f| draw(f, &mut app)).unwrap();
        assert!(app.media_hits.is_empty() && app.want.is_empty());
        assert!(screen(&term).iter().any(|r| r.contains("[image] liat ini")));
        key(&mut app, KeyCode::Char('o'));
        assert!(app.viewer.is_none());
    }

    // ---- menus, drawer, paste: all against the offline app, nothing leaves the process ----

    use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

    fn mouse(app: &mut App, kind: MouseEventKind, col: u16, row: u16) {
        app.handle(Event::Input(CtEvent::Mouse(MouseEvent { kind, column: col, row, modifiers: KeyModifiers::NONE })));
    }

    fn draw_once(app: &mut App, term: &mut Terminal<TestBackend>) -> Vec<String> {
        term.draw(|f| draw(f, app)).unwrap();
        screen(term)
    }

    #[test]
    fn right_click_opens_the_menu_and_hover_moves_the_highlight() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        let mut term = Terminal::new(TestBackend::new(110, 40)).unwrap();
        draw_once(&mut app, &mut term);

        // Right-click the picture bubble.
        let (rect, id) = app.bubble_hits.iter().find(|(_, id)| id == "p1").cloned().expect("bubble on screen");
        mouse(&mut app, MouseEventKind::Down(MouseButton::Right), rect.x + 3, rect.y + 1);
        assert!(app.menu.is_some(), "no menu after right-click on {id}");
        let rows = draw_once(&mut app, &mut term);
        for r in &rows {
            println!("{r}");
        }
        let text = rows.join("\n");
        for want in ["Reply", "React", "Forward", "Copy image", "Star", "Delete"] {
            assert!(text.contains(want), "menu lacks {want}");
        }
        assert!(text.contains("MENU"));

        // Hover the second item, click it: Reply is item 0, so React (a submenu) opens.
        let m = app.menu.as_ref().unwrap();
        let (row, idx) = m.rows[1];
        let col = m.rect.x + 3;
        mouse(&mut app, MouseEventKind::Moved, col, row);
        assert_eq!(app.menu.as_ref().unwrap().sel, idx);
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), col, row);
        let labels: Vec<_> = app.menu.as_ref().unwrap().items.iter().map(|i| i.label.clone()).collect();
        assert_eq!(labels, ["👍", "❤️", "😂", "😮", "😢", "🙏"]);

        // Clicking outside closes it without doing anything.
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), 0, 39);
        assert!(app.menu.is_none());
    }

    #[test]
    fn right_click_on_a_chat_row_opens_the_chat_menu() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        let mut term = Terminal::new(TestBackend::new(110, 40)).unwrap();
        draw_once(&mut app, &mut term);
        let list = app.hit.sidebar_list;
        mouse(&mut app, MouseEventKind::Down(MouseButton::Right), list.x + 8, list.y);
        let rows = draw_once(&mut app, &mut term);
        let text = rows.join("\n");
        for want in ["Group info", "Archive chat", "Mute notifications", "Pin chat"] {
            assert!(text.contains(want), "chat menu lacks {want}");
        }
    }

    #[test]
    fn keyboard_selects_a_bubble_and_dot_opens_its_menu() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        app.focus = crate::app::Focus::Messages;
        let mut term = Terminal::new(TestBackend::new(110, 40)).unwrap();
        draw_once(&mut app, &mut term);
        key(&mut app, KeyCode::Char('K'));
        assert_eq!(app.sel_msg.as_deref(), Some("2"), "K starts from the newest");
        key(&mut app, KeyCode::Char('K'));
        assert_eq!(app.sel_msg.as_deref(), Some("p1"));
        let rows = draw_once(&mut app, &mut term);
        assert!(rows.iter().any(|r| r.contains('▌')), "selection marker");
        key(&mut app, KeyCode::Char('.'));
        assert!(app.menu.is_some());
        key(&mut app, KeyCode::Esc);
        assert!(app.menu.is_none());
        key(&mut app, KeyCode::Esc);
        assert!(app.sel_msg.is_none());
    }

    #[test]
    fn reply_shows_a_quote_in_the_bubble_and_a_bar_over_the_input() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        let mut term = Terminal::new(TestBackend::new(110, 40)).unwrap();
        app.run_action(crate::menu::Action::Reply("p1".into()));
        for c in "bagus ya".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        let rows = draw_once(&mut app, &mut term);
        assert!(rows.iter().any(|r| r.contains("Replying to Angger")), "reply bar");
        // Send: the offline runtime never runs the call, but the pending bubble shows the quote.
        key(&mut app, KeyCode::Enter);
        assert!(app.reply.is_none());
        let rows = draw_once(&mut app, &mut term);
        assert!(rows.iter().any(|r| r.contains("↪ Angger")), "quote line");
        assert!(rows.iter().any(|r| r.contains("bagus ya")));
    }

    #[test]
    fn info_drawer_shows_group_members_and_media_rows() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        key(&mut app, KeyCode::Char('I'));
        let group: wabot_backend_client::GroupCache = serde_json::from_str(
            r#"{"jid":"g@g.us","name":"AkuKatiga","description":"Tempat ngobrol santai tiap hari","participantCount":2,
                "participants":[{"jid":"1@s.whatsapp.net","name":"Budi","isAdmin":true},{"jid":"2@s.whatsapp.net","name":"Sari"}]}"#,
        )
        .unwrap();
        app.on_group("g@g.us", Ok(group));
        let mut photo = msg("m1", "1@s.whatsapp.net", "foto kemarin", crate::app::now_ms());
        photo.message_type = "image".into();
        photo.media_url = Some("/media/p1.png".into());
        app.on_panel_page("g@g.us", crate::panel::Tab::Media, Ok(vec![photo]));
        let rows = draw_once(&mut app, &mut term);
        for r in &rows {
            println!("{r}");
        }
        let text = rows.join("\n");
        for want in ["Group info", "Group · 2 participants", "Tempat ngobrol", "Media", "Docs", "Links", "Members", "foto kemarin"] {
            assert!(text.contains(want), "drawer lacks {want}");
        }
        // Members tab lists names and roles.
        key(&mut app, KeyCode::Char('l'));
        key(&mut app, KeyCode::Char('l'));
        key(&mut app, KeyCode::Char('l'));
        let text = draw_once(&mut app, &mut term).join("\n");
        assert!(text.contains("Budi") && text.contains("admin") && text.contains("Sari"), "{text}");
        // Esc closes the drawer.
        key(&mut app, KeyCode::Esc);
        assert!(app.panel.is_none());
    }

    #[test]
    fn search_drawer_lists_hits_and_jumps_to_one() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        key(&mut app, KeyCode::Char('f'));
        assert_eq!(app.mode, Mode::Search);
        for c in "bagus".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        // Enter would call the backend (never run here); feed the answer ourselves.
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, Mode::Normal);
        let hit = app.logs["g@g.us"].msgs[2].clone();
        app.on_search("g@g.us", "bagus", Ok(vec![hit]));
        let rows = draw_once(&mut app, &mut term);
        for r in &rows {
            println!("{r}");
        }
        let text = rows.join("\n");
        assert!(text.contains("1 result for \"bagus\""), "{text}");
        assert!(text.contains("bagus"));
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.sel_msg.as_deref(), Some("2"), "jumped to the hit");
        assert_eq!(app.focus, crate::app::Focus::Messages);
    }

    #[test]
    fn pasted_picture_gets_a_preview_and_waits_for_enter() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        let mut term = Terminal::new(TestBackend::new(110, 40)).unwrap();
        let img = gradient(320, 200);
        let picked = crate::attach::Picked { bytes: vec![0u8; 5000], name: "pasted-image.png".into(), mime: "image/png", kind: crate::attach::AttachKind::Image, preview: Some(img) };
        app.on_pasted(Ok(crate::actions::Pasted::File(picked)));
        assert_eq!(app.mode, Mode::Caption);
        for c in "lihat ini".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        let rows = draw_once(&mut app, &mut term);
        for r in &rows {
            println!("{r}");
        }
        let text = rows.join("\n");
        assert!(text.contains("Send picture to AkuKatiga"), "{text}");
        assert!(text.contains("pasted-image.png") && text.contains("320×200"));
        assert!(text.contains("lihat ini") && text.contains("PICTURE"));
        let blocks: usize = rows.iter().map(|r| r.chars().filter(|c| *c == '▀' || *c == '▄').count()).sum();
        assert!(blocks > 100, "preview not drawn ({blocks})");
        // Esc drops it without sending.
        key(&mut app, KeyCode::Esc);
        assert!(app.attach.is_none() && app.mode == Mode::Normal);
    }

    #[test]
    fn pasting_text_goes_into_the_focused_field() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        app.mode = Mode::Insert;
        app.on_paste_text("baris satu\nbaris dua".into());
        assert_eq!(app.input.text, "baris satu baris dua");
        app.mode = Mode::Filter;
        app.on_paste_text("tenshi".into());
        assert_eq!(app.filter.text, "tenshi");
    }

    #[test]
    fn the_whole_info_drawer_scrolls_not_just_the_list() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        // A small window so the drawer is taller than the screen.
        let mut term = Terminal::new(TestBackend::new(120, 24)).unwrap();
        key(&mut app, KeyCode::Char('I'));
        let long = (1..=12).map(|i| format!("baris deskripsi nomor {i}")).collect::<Vec<_>>().join("\n");
        let group: wabot_backend_client::GroupCache = serde_json::from_str(&format!(
            r#"{{"jid":"g@g.us","name":"AkuKatiga","description":{},"participantCount":1,"participants":[]}}"#,
            serde_json::to_string(&long).unwrap()
        ))
        .unwrap();
        app.on_group("g@g.us", Ok(group));
        let items: Vec<Message> = (0..8)
            .map(|i| {
                let mut m = msg(&format!("m{i}"), "1@s.whatsapp.net", &format!("foto {i}"), crate::app::now_ms() - i * 1000);
                m.message_type = "image".into();
                m.media_url = Some(format!("/media/{i}.png"));
                m
            })
            .collect();
        app.on_panel_page("g@g.us", crate::panel::Tab::Media, Ok(items));

        let before = draw_once(&mut app, &mut term).join("\n");
        assert!(before.contains("Group · 1 participants") || before.contains("Group"), "{before}");
        assert!(before.contains("baris deskripsi nomor 1"), "description starts visible");
        assert!(!before.contains("foto 7"), "the last media row is below the fold");

        // Wheel over the drawer: the profile block moves up and out, the list comes in.
        let r = app.panel.as_ref().unwrap().rect;
        for _ in 0..30 {
            mouse(&mut app, MouseEventKind::ScrollDown, r.x + 5, r.y + 5);
            app.finish_scroll();
            draw_once(&mut app, &mut term);
        }
        let after = draw_once(&mut app, &mut term);
        let text = after.join("\n");
        for r in &after {
            println!("{r}");
        }
        assert!(text.contains("Group info"), "the title stays pinned");
        assert!(!text.contains("baris deskripsi nomor 1\n"), "the description scrolled away");
        assert!(text.contains("foto 7"), "the end of the list is reachable: {text}");
        let p = app.panel.as_ref().unwrap();
        assert_eq!(p.scroll_y, p.max_scroll, "scrolled to the very end");
        assert_eq!(p.sel, 0, "the wheel moves the view, not the highlight");

        // And back up.
        for _ in 0..30 {
            mouse(&mut app, MouseEventKind::ScrollUp, r.x + 5, r.y + 5);
            app.finish_scroll();
            draw_once(&mut app, &mut term);
        }
        assert!(draw_once(&mut app, &mut term).join("\n").contains("baris deskripsi nomor 1"));

        // Keyboard: moving the highlight down drags the view with it.
        for _ in 0..7 {
            key(&mut app, KeyCode::Char('j'));
            draw_once(&mut app, &mut term);
            app.finish_scroll();
        }
        assert!(draw_once(&mut app, &mut term).join("\n").contains("foto 7"), "view follows the highlighted row");
    }

    #[test]
    fn a_picture_scrolled_half_out_of_view_still_shows_its_visible_part() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        let mut term = Terminal::new(TestBackend::new(110, 22)).unwrap();
        draw_once(&mut app, &mut term);
        assert!(app.max_scroll > 6, "the window must be too short for the whole chat");

        let glyph_rows = |rows: &[String], r: Rect| {
            rows[r.y as usize..(r.y + r.height) as usize].iter().filter(|l| l.chars().any(|c| c == '▀' || c == '▄')).count()
        };
        let mut full = 0;
        let mut partial = None;
        for k in 0..=app.max_scroll {
            app.scroll = k;
            let rows = draw_once(&mut app, &mut term);
            if let Some((r, _)) = app.media_hits.iter().find(|(_, id)| id == "p1").cloned() {
                full = full.max(r.height);
                if r.height < full || (k > 0 && r.height < 14) {
                    // Cut by the pane's edge: rows that are on screen are still drawn.
                    assert_eq!(glyph_rows(&rows, r), r.height as usize, "k={k} rect={r:?}");
                    if partial.is_none() {
                        println!("--- scroll {k}: picture shows {} rows", r.height);
                        for l in rows.iter().take(14) {
                            println!("{l}");
                        }
                    }
                    partial = Some((k, r.height));
                }
            }
        }
        assert!(full >= 10, "the picture is shown whole at some point (full={full})");
        let (k, h) = partial.expect("some scroll position cuts the picture");
        assert!(h > 0 && h < full, "partial at scroll {k}: {h} of {full} rows");
    }

    #[test]
    fn the_scrollbar_can_be_dragged_with_the_mouse() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        let mut term = Terminal::new(TestBackend::new(110, 22)).unwrap();
        draw_once(&mut app, &mut term);
        let bar = app.hit.msg_bar;
        assert!(bar.height > 4 && app.max_scroll > 0, "bar={bar:?}");
        let max = app.max_scroll;

        // Pressing at the top of the track jumps to the oldest messages ...
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), bar.x, bar.y);
        assert_eq!(app.scroll, max);
        assert!(app.drag.is_some());
        // ... dragging down follows the pointer (also outside the column) ...
        mouse(&mut app, MouseEventKind::Drag(MouseButton::Left), bar.x - 15, bar.y + bar.height / 2);
        assert!(app.scroll > 0 && app.scroll < max, "middle of the track: {}", app.scroll);
        mouse(&mut app, MouseEventKind::Drag(MouseButton::Left), bar.x, bar.y + bar.height - 1);
        assert_eq!(app.scroll, 0, "bottom of the track is the newest");
        // ... and releasing ends it: further movement does nothing.
        mouse(&mut app, MouseEventKind::Up(MouseButton::Left), bar.x, bar.y + bar.height - 1);
        assert!(app.drag.is_none());
        mouse(&mut app, MouseEventKind::Moved, bar.x, bar.y);
        assert_eq!(app.scroll, 0);

        // The column next to the bar counts too (the bar is only one cell wide).
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), bar.x - 1, bar.y);
        assert_eq!(app.scroll, max);
        mouse(&mut app, MouseEventKind::Up(MouseButton::Left), bar.x - 1, bar.y);
    }

    #[test]
    fn the_wheel_glides_instead_of_jumping() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        let mut term = Terminal::new(TestBackend::new(110, 22)).unwrap();
        draw_once(&mut app, &mut term);
        let area = app.hit.messages;
        mouse(&mut app, MouseEventKind::ScrollUp, area.x + 10, area.y + 3);
        assert_eq!(app.scroll, 0, "nothing moves until the first frame");
        assert!(app.animating());
        // Two more notches pile onto the same glide.
        mouse(&mut app, MouseEventKind::ScrollUp, area.x + 10, area.y + 3);
        mouse(&mut app, MouseEventKind::ScrollUp, area.x + 10, area.y + 3);
        app.finish_scroll();
        assert_eq!(app.scroll, 6.min(app.max_scroll), "three notches of two lines");
        assert!(!app.animating());
    }

    #[test]
    fn stickers_are_small_and_do_not_repeat_their_tag() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        let picker = Picker::halfblocks();
        let mut st = msg("s1", "628@s.whatsapp.net", "[Sticker]", crate::app::now_ms() - 10_000);
        st.message_type = "sticker".into();
        st.media_url = Some("/media/s1.webp".into());
        app.logs.get_mut("g@g.us").unwrap().msgs.push(st);
        app.media.begin("/media/s1.webp");
        app.media.finish("/media/s1.webp", Some(ready(&picker, 512, 512)));
        let mut term = Terminal::new(TestBackend::new(110, 40)).unwrap();
        let rows = draw_once(&mut app, &mut term);
        for r in &rows {
            println!("{r}");
        }
        let (rect, _) = app.media_hits.iter().find(|(_, id)| id == "s1").cloned().expect("sticker on screen");
        assert!(rect.width <= 16 && rect.height <= 8, "sticker box {rect:?}");
        assert!(!rows.join("\n").to_lowercase().contains("[sticker]"), "the tag under the picture is noise");
        // A normal picture is still allowed to be bigger.
        let (big, _) = app.media_hits.iter().find(|(_, id)| id == "p1").cloned().unwrap();
        assert!(big.width > 16, "picture box {big:?}");
    }

    #[test]
    fn scrolling_reuses_the_built_lines_and_events_rebuild_them() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        let mut term = Terminal::new(TestBackend::new(110, 22)).unwrap();
        draw_once(&mut app, &mut term);
        let ptr = |a: &App| a.lines_cache.as_ref().map(|c| c.lines.as_ptr() as usize);
        let first = ptr(&app).expect("cached after a draw");
        let area = app.hit.messages;
        for _ in 0..5 {
            mouse(&mut app, MouseEventKind::ScrollUp, area.x + 10, area.y + 3);
            app.finish_scroll();
            draw_once(&mut app, &mut term);
            app.handle(Event::Tick);
        }
        assert!(app.scroll > 0);
        assert_eq!(ptr(&app), Some(first), "scrolling must not rebuild the conversation");
        // Anything that can change a message does.
        let gen = app.lines_gen;
        app.handle(Event::Done(crate::actions::Done { label: String::new(), result: Ok(()), then: crate::actions::Then::Star("g@g.us".into(), "p1".into(), true) }));
        assert_eq!(app.lines_gen, gen + 1);
        let rows = draw_once(&mut app, &mut term).join("\n");
        assert!(rows.contains('★') || app.logs["g@g.us"].msgs[1].starred, "starred message shows its star");
    }

    #[test]
    fn many_new_pictures_are_encoded_a_couple_per_frame() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        let picker = Picker::halfblocks();
        let now = crate::app::now_ms();
        // Replace the chat with six small pictures, all already downloaded.
        let mut msgs = Vec::new();
        for i in 0..6 {
            let mut m = msg(&format!("q{i}"), "628@s.whatsapp.net", "", now - 60_000 + i * 1000);
            m.message_type = "image".into();
            m.media_url = Some(format!("/media/q{i}.png"));
            app.media.begin(&format!("/media/q{i}.png"));
            app.media.finish(&format!("/media/q{i}.png"), Some(ready(&picker, 160, 40)));
            msgs.push(m);
        }
        app.logs.get_mut("g@g.us").unwrap().msgs = msgs;
        let mut term = Terminal::new(TestBackend::new(110, 60)).unwrap();
        let glyph_blocks = |rows: &[String]| rows.iter().filter(|r| r.contains("▀") || r.contains("▄")).count();

        let first = glyph_blocks(&draw_once(&mut app, &mut term));
        assert!(app.pending_builds && app.animating(), "work is left, so frames keep coming");
        let mut last = first;
        for _ in 0..4 {
            last = glyph_blocks(&draw_once(&mut app, &mut term));
        }
        assert!(last > first, "later frames show more pictures ({first} -> {last})");
        assert!(!app.pending_builds && !app.animating(), "everything built, nothing left to do");
    }

    #[test]
    fn esc_close_in_a_drawer_title_is_a_button() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        let mut term = Terminal::new(TestBackend::new(120, 30)).unwrap();

        // Info drawer.
        key(&mut app, KeyCode::Char('I'));
        draw_once(&mut app, &mut term);
        let btn = app.panel.as_ref().unwrap().close_btn;
        assert!(btn.width >= 8 && btn.height == 1, "close button {btn:?}");
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), btn.x + 2, btn.y);
        assert!(app.panel.is_none(), "clicking close closes the info drawer");

        // Search drawer, even while typing in its field.
        key(&mut app, KeyCode::Char('f'));
        assert_eq!(app.mode, Mode::Search);
        draw_once(&mut app, &mut term);
        let btn = app.panel.as_ref().unwrap().close_btn;
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), btn.x + btn.width - 1, btn.y);
        assert!(app.panel.is_none() && app.mode == Mode::Normal, "and leaves search mode");
    }

    #[test]
    fn message_info_is_a_right_hand_drawer_like_the_others() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        let mut term = Terminal::new(TestBackend::new(120, 34)).unwrap();
        // My own message "bagus"; it is the last one in the chat.
        app.run_action(crate::menu::Action::MsgInfo("2".into()));
        let p = app.panel.as_ref().expect("a drawer, not a popup");
        assert_eq!(p.kind, crate::panel::PanelKind::MsgInfo);
        assert_eq!(app.focus, crate::app::Focus::Panel);

        let rows = draw_once(&mut app, &mut term);
        let text = rows.join("\n");
        assert!(text.contains("Message info") && text.contains("esc close"));
        assert!(text.contains("You") && text.contains("bagus"), "the message itself");
        assert!(text.contains("Loading receipts"), "own messages ask for receipts");
        // The conversation stays visible on the left.
        assert!(text.contains("liat ini"));

        let receipts: wabot_backend_client::MessageReceipts = serde_json::from_str(
            r#"{"total":3,"recipients":["1@s.whatsapp.net","2@s.whatsapp.net","3@s.whatsapp.net"],
                "receipts":[{"messageId":"2","chatId":"g@g.us","jid":"1@s.whatsapp.net","deliveredAt":1790000000000,"readAt":1790000060000},
                            {"messageId":"2","chatId":"g@g.us","jid":"2@s.whatsapp.net","deliveredAt":1790000000000,"readAt":0}]}"#,
        )
        .unwrap();
        let group: wabot_backend_client::GroupCache = serde_json::from_str(
            r#"{"jid":"g@g.us","name":"Group","participants":[{"jid":"1@s.whatsapp.net","name":"Budi"},{"jid":"2@s.whatsapp.net","name":"Sari"},{"jid":"3@s.whatsapp.net","name":"Joko"}]}"#,
        )
        .unwrap();
        app.on_group("g@g.us", Ok(group));
        app.on_receipts("2", Ok(receipts));
        let rows = draw_once(&mut app, &mut term);
        for r in &rows {
            println!("{r}");
        }
        let text = rows.join("\n");
        for want in ["Read by 1 of 3", "Budi", "Delivered to 1", "Sari", "Not delivered yet 1", "Joko"] {
            assert!(text.contains(want), "receipts lack {want}");
        }
        // Wrong message's receipts are ignored; Esc closes the drawer.
        app.on_receipts("other", Err("x".into()));
        key(&mut app, KeyCode::Esc);
        assert!(app.panel.is_none());
    }

    #[test]
    fn the_header_button_hides_the_chat_list_completely_and_brings_it_back() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        app.chats.push(wabot_backend_client::Chat {
            id: "b@s.whatsapp.net".into(),
            name: "Budi".into(),
            last_time: crate::app::now_ms(),
            last_msg: "halo bro".into(),
            unread: 2,
            ..serde_json::from_str(r#"{"id":"x"}"#).unwrap()
        });
        let mut term = Terminal::new(TestBackend::new(110, 30)).unwrap();
        let rows = draw_once(&mut app, &mut term);
        let text = rows.join("\n");
        assert!(text.contains("WA Bot") && text.contains("halo bro"), "the list is there at first");
        assert!(rows[0].contains('«'), "collapse button: {:?}", rows[0]);
        let toggle = app.hit.toggle_btn;
        assert_eq!((toggle.width, toggle.height), (3, 2), "easy to click: two rows high");
        let before = app.hit.messages.width;

        // Click: no list, no strip, no border. Only the conversation.
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), toggle.x + 1, toggle.y);
        assert!(app.sidebar_collapsed);
        let rows = draw_once(&mut app, &mut term);
        let text = rows.join("\n");
        for r in rows.iter().take(4) {
            println!("{r}");
        }
        assert!(!text.contains("WA Bot") && !text.contains("halo bro") && !text.contains("/ to filter"), "{text}");
        assert_eq!(app.hit.sidebar_list, Rect::default(), "nothing to click on the left");
        assert!(rows[0].contains('»'), "now it says expand: {:?}", rows[0]);
        assert_eq!(app.hit.messages.width, 110, "the conversation has the whole window");
        assert!(app.hit.messages.width > before + 25);
        assert_ne!(app.focus, crate::app::Focus::Chats, "keyboard focus must not sit on a hidden list");
        // Tab does not land on the hidden list either.
        key(&mut app, KeyCode::Tab);
        assert_ne!(app.focus, crate::app::Focus::Chats);

        // Click again: the list is back.
        draw_once(&mut app, &mut term);
        let toggle = app.hit.toggle_btn;
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), toggle.x + 1, toggle.y + 1);
        assert!(!app.sidebar_collapsed);
        assert!(draw_once(&mut app, &mut term).join("\n").contains("halo bro"));

        // `B` toggles from the keyboard, `h` (go left) and `/` (filter) bring the list back.
        key(&mut app, KeyCode::Char('B'));
        assert!(app.sidebar_collapsed);
        key(&mut app, KeyCode::Char('h'));
        assert!(!app.sidebar_collapsed && app.focus == crate::app::Focus::Chats);
        key(&mut app, KeyCode::Char('B'));
        key(&mut app, KeyCode::Char('/'));
        assert!(!app.sidebar_collapsed);
    }

    #[test]
    fn in_a_narrow_window_the_same_button_goes_back_to_the_list() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        app.focus = crate::app::Focus::Messages;
        let mut term = Terminal::new(TestBackend::new(60, 24)).unwrap();
        let rows = draw_once(&mut app, &mut term);
        assert!(rows[0].contains('‹'), "a back arrow: {:?}", rows[0]);
        let toggle = app.hit.toggle_btn;
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), toggle.x + 1, toggle.y);
        assert_eq!(app.focus, crate::app::Focus::Chats);
        assert!(!app.sidebar_collapsed);
    }

    #[test]
    fn a_flood_of_wheel_events_cannot_fly_across_the_conversation() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        // A tall conversation so there is far to go.
        let now = crate::app::now_ms();
        let many: Vec<Message> = (0..120).map(|i| msg(&format!("n{i}"), "628@s.whatsapp.net", &format!("pesan {i}"), now - 200_000 + i * 1000)).collect();
        app.logs.get_mut("g@g.us").unwrap().msgs = many;
        let mut term = Terminal::new(TestBackend::new(110, 30)).unwrap();
        draw_once(&mut app, &mut term);
        assert!(app.max_scroll > 100);
        let area = app.hit.messages;
        // A touchpad flick: fifty events before a single frame is drawn.
        for _ in 0..50 {
            mouse(&mut app, MouseEventKind::ScrollUp, area.x + 10, area.y + 3);
        }
        app.finish_scroll();
        assert!(app.scroll > 0 && app.scroll as isize <= crate::app::MAX_LEAD, "went {} lines", app.scroll);
        // One notch is a small step, not a page.
        app.scroll = 0;
        app.smooth.stop();
        mouse(&mut app, MouseEventKind::ScrollUp, area.x + 10, area.y + 3);
        app.finish_scroll();
        assert_eq!(app.scroll, 2);
    }

    #[test]
    fn the_picture_box_works_with_the_mouse_alone() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        let mut term = Terminal::new(TestBackend::new(110, 40)).unwrap();
        let attach = |app: &mut App| {
            let picked = crate::attach::Picked { bytes: vec![0u8; 3000], name: "pasted-image.png".into(), mime: "image/png", kind: crate::attach::AttachKind::Image, preview: Some(gradient(200, 100)) };
            app.on_pasted(Ok(crate::actions::Pasted::File(picked)));
        };

        attach(&mut app);
        let text = draw_once(&mut app, &mut term).join("\n");
        assert!(text.contains("⏎ Send") && text.contains("Cancel"), "buttons: {text}");
        let (send, cancel) = {
            let a = app.attach.as_ref().unwrap();
            (a.send_btn, a.cancel_btn)
        };
        assert!(send.width > 4 && cancel.width > 4 && send.y == cancel.y && cancel.x > send.x + send.width);

        // A click on empty space, or on the conversation behind the box, does nothing.
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), 0, 0);
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), send.x, send.y.saturating_sub(5));
        assert!(app.attach.is_some() && !app.attach.as_ref().unwrap().sending);

        // Cancel: the box closes, nothing was sent, we are back in normal mode.
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), cancel.x + 2, cancel.y);
        assert!(app.attach.is_none() && app.mode == Mode::Normal);

        // Send: the box turns into "Sending…" (the offline runtime never runs the call).
        attach(&mut app);
        draw_once(&mut app, &mut term);
        let send = app.attach.as_ref().unwrap().send_btn;
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), send.x + 1, send.y);
        assert!(app.attach.as_ref().unwrap().sending);
        let text = draw_once(&mut app, &mut term).join("\n");
        assert!(text.contains("Sending…") && !text.contains("Cancel"), "{text}");
        // While it is being sent, neither a second click nor Esc can drop it.
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), send.x + 1, send.y);
        key(&mut app, KeyCode::Esc);
        assert!(app.attach.is_some(), "an upload in flight is not cancelled by a stray key");
    }

    fn scratch_dir() -> std::path::PathBuf {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("wabot-ui-attach-{}-{n}", std::process::id()));
        std::fs::create_dir_all(dir.join("Laporan")).unwrap();
        std::fs::write(dir.join("Laporan").join("inner.pdf"), b"%PDF").unwrap();
        std::fs::write(dir.join("notes.pdf"), b"%PDF-1.4 notes").unwrap();
        std::fs::write(dir.join(".hidden.pdf"), b"x").unwrap();
        dir
    }

    #[test]
    fn the_plus_button_opens_the_attach_menu_and_the_file_chooser_works_by_mouse() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        let dir = scratch_dir();
        app.last_dir = Some(dir.clone());
        let mut term = Terminal::new(TestBackend::new(110, 40)).unwrap();

        let rows = draw_once(&mut app, &mut term);
        let btn = app.hit.attach_btn;
        assert!(btn.width >= 5 && rows.iter().any(|r| r.contains(" + ")), "a + button beside the message field");
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), btn.x + 2, btn.y + 1);
        let text = draw_once(&mut app, &mut term).join("\n");
        for want in ["Attach", "Photo / video", "Document", "Audio", "GIF", "Paste picture"] {
            assert!(text.contains(want), "attach menu lacks {want}");
        }

        // Pick "Document" with the mouse.
        let m = app.menu.as_ref().unwrap();
        let (row, _) = m.rows.iter().find(|(_, i)| m.items[*i].label == "Document").copied().unwrap();
        let col = m.rect.x + 3;
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), col, row);
        assert!(app.menu.is_none());
        assert_eq!(app.browser.as_ref().map(|b| b.mode), Some(crate::attach::AttachMode::Document));
        let rows = draw_once(&mut app, &mut term);
        let text = rows.join("\n");
        for r in rows.iter().take(14) {
            println!("{r}");
        }
        for want in ["Choose a document", "Up", "Home", "Hidden: off", "Cancel", "Laporan/", "notes.pdf"] {
            assert!(text.contains(want), "chooser lacks {want}");
        }
        assert!(!text.contains(".hidden.pdf"));

        let b = app.browser.as_ref().unwrap();
        let row_of = |name: &str| {
            let i = b.entries.iter().position(|e| e.name == name).unwrap();
            b.list.y + (i - b.offset) as u16
        };
        let (laporan_row, hidden_btn, up_btn) = (
            row_of("Laporan"),
            b.buttons.iter().find(|(_, k)| *k == crate::browser::Btn::Hidden).unwrap().0,
            b.buttons.iter().find(|(_, k)| *k == crate::browser::Btn::Up).unwrap().0,
        );
        let list_x = b.list.x + 3;

        // A click on a folder goes in; "Up" comes back.
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), list_x, laporan_row);
        assert!(app.browser.as_ref().unwrap().dir.ends_with("Laporan"));
        assert!(draw_once(&mut app, &mut term).join("\n").contains("inner.pdf"));
        let up_btn = app.browser.as_ref().unwrap().buttons.iter().find(|(_, k)| *k == crate::browser::Btn::Up).map_or(up_btn, |x| x.0);
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), up_btn.x + 1, up_btn.y);
        assert_eq!(app.browser.as_ref().unwrap().dir, dir);

        // "Hidden" shows dotfiles.
        draw_once(&mut app, &mut term);
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), hidden_btn.x + 1, hidden_btn.y);
        assert!(draw_once(&mut app, &mut term).join("\n").contains(".hidden.pdf"));

        // A click on a file picks it: the chooser closes and remembers the folder.
        let b = app.browser.as_ref().unwrap();
        let i = b.entries.iter().position(|e| e.name == "notes.pdf").unwrap();
        let row = b.list.y + (i - b.offset) as u16;
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), list_x, row);
        assert!(app.browser.is_none());
        assert_eq!(app.last_dir.as_deref(), Some(dir.as_path()));

        // Cancel closes the chooser without picking anything.
        app.run_action(crate::menu::Action::Attach(crate::attach::AttachMode::Media));
        draw_once(&mut app, &mut term);
        let cancel = app.browser.as_ref().unwrap().buttons.iter().find(|(_, k)| *k == crate::browser::Btn::Cancel).unwrap().0;
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), cancel.x + 1, cancel.y);
        assert!(app.browser.is_none() && app.attach.is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_document_gets_a_file_tile_and_audio_has_no_caption() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        let mut term = Terminal::new(TestBackend::new(110, 40)).unwrap();
        let file = |name: &str, kind, mime| crate::attach::Picked { bytes: vec![0u8; 4096], name: name.into(), mime, kind, preview: None };

        app.on_pasted(Ok(crate::actions::Pasted::File(file("laporan-akhir.pdf", crate::attach::AttachKind::Document, "application/pdf"))));
        for c in "mohon dicek".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        let rows = draw_once(&mut app, &mut term);
        let text = rows.join("\n");
        for r in rows.iter().take(20) {
            println!("{r}");
        }
        for want in ["Send document to AkuKatiga", "PDF", "laporan-akhir.pdf", "application/pdf", "mohon dicek", "⏎ Send"] {
            assert!(text.contains(want), "box lacks {want}");
        }
        key(&mut app, KeyCode::Esc);
        assert!(app.attach.is_none());

        app.on_pasted(Ok(crate::actions::Pasted::File(file("suara.mp3", crate::attach::AttachKind::Audio, "audio/mpeg"))));
        key(&mut app, KeyCode::Char('x'));
        assert!(app.attach.as_ref().unwrap().caption.text.is_empty(), "typing is ignored: audio has no caption");
        let text = draw_once(&mut app, &mut term).join("\n");
        assert!(text.contains("Send audio to") && text.contains("Audio has no caption") && text.contains("MP3"));
    }

    #[test]
    fn the_sticker_picker_shows_a_grid_and_a_click_sends_one() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        let picker = Picker::halfblocks();
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();

        // Opened from the attach menu.
        app.run_action(crate::menu::Action::Stickers);
        assert!(app.stickers.is_some());
        let text = draw_once(&mut app, &mut term).join("\n");
        assert!(text.contains("Stickers") && text.contains("Loading stickers"), "{text}");

        let st = |id: &str| wabot_backend_client::StickerFavorite { id: id.into(), media_url: format!("/media/{id}.webp"), is_animated: false, source: None };
        app.on_stickers(Ok((vec![st("r1"), st("r2"), st("r3")], vec![st("f1")])));
        for id in ["r1", "r2", "r3", "f1"] {
            let url = format!("/media/{id}.webp");
            app.sticker_imgs.begin(&url);
            app.sticker_imgs.finish(&url, Some(ready(&picker, 256, 256)));
        }
        let rows = draw_once(&mut app, &mut term);
        for r in rows.iter().take(16) {
            println!("{r}");
        }
        let text = rows.join("\n");
        assert!(text.contains("Recent") && text.contains("Favourites") && text.contains("esc close"));
        let blocks: usize = rows.iter().map(|r| r.chars().filter(|c| *c == '▀' || *c == '▄').count()).sum();
        assert!(blocks > 40, "stickers drawn ({blocks})");

        // The second tab.
        let fav = app.stickers.as_ref().unwrap().tab_hits[1].0;
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), fav.x + 2, fav.y);
        assert_eq!(app.stickers.as_ref().unwrap().tab, crate::stickers::StickerTab::Favourites);
        assert_eq!(app.stickers.as_ref().unwrap().items().len(), 1);
        let recent_tab = app.stickers.as_ref().unwrap().tab_hits[0].0;
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), recent_tab.x + 1, fav.y);

        // A click on a sticker sends it (the offline runtime never runs the call); it closes on success only.
        let g = app.stickers.as_ref().unwrap().grid;
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), g.x + 12, g.y + 1);
        assert_eq!(app.stickers.as_ref().unwrap().sel, 1, "the second cell was clicked");
        assert!(app.stickers.is_some(), "still open until the backend says it went");
        app.on_done(crate::actions::Done { label: "Sticker sent".into(), result: Ok(()), then: crate::actions::Then::CloseStickers });
        assert!(app.stickers.is_none() && app.live_notice() == Some("Sticker sent"));

        // The close button and Esc both close it.
        app.run_action(crate::menu::Action::Stickers);
        draw_once(&mut app, &mut term);
        let close = app.stickers.as_ref().unwrap().close_btn;
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), close.x + 1, close.y);
        assert!(app.stickers.is_none());
    }

    #[test]
    fn the_attach_menu_lists_everything_gpui_offers() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        let mut term = Terminal::new(TestBackend::new(110, 40)).unwrap();
        draw_once(&mut app, &mut term);
        key(&mut app, KeyCode::Char('+'));
        let text = draw_once(&mut app, &mut term).join("\n");
        for want in ["Sticker", "Photo / video", "Document", "Audio", "GIF", "Poll", "Location", "Contact", "Paste picture"] {
            assert!(text.contains(want), "menu lacks {want}");
        }
    }

    #[test]
    fn the_poll_form_is_filled_checked_and_sent_with_the_mouse_or_keyboard() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        let mut term = Terminal::new(TestBackend::new(110, 40)).unwrap();
        app.run_action(crate::menu::Action::Compose(crate::form::FormKind::Poll));
        let rows = draw_once(&mut app, &mut term);
        for r in rows.iter().take(26) {
            println!("{r}");
        }
        let text = rows.join("\n");
        for want in ["New poll", "Question", "Option 1", "Option 8", "Allow several answers", "⏎ Send", "Cancel", "FORM"] {
            assert!(text.contains(want), "form lacks {want}");
        }

        // Sending an empty form says what is missing and sends nothing.
        let send = app.form.as_ref().unwrap().send_btn;
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), send.x + 2, send.y);
        assert_eq!(app.form.as_ref().unwrap().error.as_deref(), Some("Write the question"));
        assert!(!app.form.as_ref().unwrap().sending);

        // Type with the keyboard: Enter moves to the next field.
        for c in "Makan apa?".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        key(&mut app, KeyCode::Enter);
        for c in "Soto".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        // Click straight into Option 2 and type there.
        draw_once(&mut app, &mut term);
        let opt2 = app.form.as_ref().unwrap().field_hits[2].0;
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), opt2.x + 14, opt2.y);
        assert_eq!(app.form.as_ref().unwrap().focus, 2);
        for c in "Bakso".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        // The checkbox is clickable too.
        draw_once(&mut app, &mut term);
        let multi = app.form.as_ref().unwrap().multi_btn;
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), multi.x + 2, multi.y);
        assert!(app.form.as_ref().unwrap().multi);
        assert!(draw_once(&mut app, &mut term).join("\n").contains("[x] Allow several"));

        // Now it is valid: Send goes (the offline runtime never runs the call) and the form locks.
        let send = app.form.as_ref().unwrap().send_btn;
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), send.x + 2, send.y);
        assert!(app.form.as_ref().unwrap().sending, "{:?}", app.form.as_ref().unwrap().error);
        key(&mut app, KeyCode::Esc);
        assert!(app.form.is_some(), "an upload in flight is not cancelled by a stray key");
        // A failure unlocks it and says why; success closes it.
        app.on_done(crate::actions::Done { label: String::new(), result: Err("offline".into()), then: crate::actions::Then::CloseForm });
        assert!(!app.form.as_ref().unwrap().sending && app.form.as_ref().unwrap().error.as_deref() == Some("offline"));
        key(&mut app, KeyCode::Esc);
        assert!(app.form.is_none());
    }

    #[test]
    fn location_and_contact_forms_open_from_the_menu_and_take_pasted_text() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        let mut term = Terminal::new(TestBackend::new(110, 40)).unwrap();
        app.run_action(crate::menu::Action::Compose(crate::form::FormKind::Location));
        let text = draw_once(&mut app, &mut term).join("\n");
        assert!(text.contains("Send location") && text.contains("Latitude") && text.contains("Longitude"));
        app.on_paste_text("-6.2088".into());
        assert_eq!(app.form.as_ref().unwrap().fields[0].edit.text, "-6.2088");
        key(&mut app, KeyCode::Esc);

        app.run_action(crate::menu::Action::Compose(crate::form::FormKind::Contact));
        let text = draw_once(&mut app, &mut term).join("\n");
        assert!(text.contains("Send contact") && text.contains("Phone"));
        key(&mut app, KeyCode::Char('B'));
        assert_eq!(app.form.as_ref().unwrap().fields[0].edit.text, "B", "keys go to the form, not to the app behind it");
        assert!(!app.sidebar_collapsed);
    }

    #[test]
    fn the_more_lines_badge_is_a_button_that_jumps_to_the_latest() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        let mut term = Terminal::new(TestBackend::new(110, 22)).unwrap();
        draw_once(&mut app, &mut term);
        assert_eq!(app.hit.latest_btn, Rect::default(), "no badge while already at the bottom");

        // Scroll up: the badge appears and says it can be clicked.
        app.scroll = 9.min(app.max_scroll);
        let text = draw_once(&mut app, &mut term).join("\n");
        assert!(text.contains("more lines") && text.contains("click or G for latest"), "{text}");
        let btn = app.hit.latest_btn;
        assert!(btn.width > 20 && btn.height == 1);

        // A click on it glides to the newest messages.
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), btn.x + 3, btn.y);
        assert!(app.animating(), "it glides, not jumps");
        app.finish_scroll();
        assert_eq!(app.scroll, 0);
        draw_once(&mut app, &mut term);
        assert_eq!(app.hit.latest_btn, Rect::default(), "the badge goes away once there");

        // Looking at an old stretch around a search hit shows it too, even at scroll 0.
        app.logs.get_mut("g@g.us").unwrap().detached = true;
        let text = draw_once(&mut app, &mut term).join("\n");
        assert!(text.contains("older messages"), "{text}");
        let btn = app.hit.latest_btn;
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), btn.x + 3, btn.y);
        assert!(app.logs["g@g.us"].loading, "the newest page is fetched again (the offline runtime never runs it)");
    }

    fn add_message(app: &mut App, m: Message) {
        app.logs.get_mut("g@g.us").unwrap().msgs.push(m);
        app.lines_gen += 1;
    }

    fn click(app: &mut App, col: u16, row: u16) {
        mouse(app, MouseEventKind::Down(MouseButton::Left), col, row);
        mouse(app, MouseEventKind::Up(MouseButton::Left), col, row);
    }

    #[test]
    fn links_are_underlined_and_a_click_opens_them() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        let now = crate::app::now_ms();
        add_message(&mut app, msg("L1", "me", "cek https://example.com/halaman ya, atau www.contoh.id.", now - 10_000));
        let mut term = Terminal::new(TestBackend::new(110, 40)).unwrap();
        let rows = draw_once(&mut app, &mut term);
        assert!(rows.iter().any(|r| r.contains("https://example.com/halaman")));
        assert_eq!(app.link_hits.len(), 2, "both addresses are clickable");

        // Drawn underlined, and clicking the label opens the full address.
        let (r, url) = app.link_hits[0].clone();
        assert_eq!(url, "https://example.com/halaman");
        let cell = term.backend().buffer()[(r.x + 2, r.y)].clone();
        assert!(cell.modifier.contains(Modifier::UNDERLINED), "{cell:?}");
        click(&mut app, r.x + 2, r.y);
        assert_eq!(app.last_opened.as_deref(), Some("https://example.com/halaman"));

        // "www." gets a scheme, and the trailing dot is not part of the link.
        let (r2, url2) = app.link_hits[1].clone();
        assert_eq!(url2, "https://www.contoh.id");
        click(&mut app, r2.x, r2.y);
        assert_eq!(app.last_opened.as_deref(), Some("https://www.contoh.id"));

        // Plain text next to it opens nothing.
        app.last_opened = None;
        click(&mut app, r.x.saturating_sub(2), r.y);
        assert_eq!(app.last_opened, None);
    }

    #[test]
    fn a_link_cut_over_several_rows_opens_whole_from_any_row() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        let now = crate::app::now_ms();
        let long = "https://example.com/sebuah/alamat/yang/panjang/sekali/sampai/harus/dipotong/beberapa/baris";
        add_message(&mut app, msg("L2", "me", long, now - 10_000));
        let mut term = Terminal::new(TestBackend::new(110, 40)).unwrap();
        draw_once(&mut app, &mut term);
        let pieces: Vec<_> = app.link_hits.iter().filter(|(_, u)| u == long).collect();
        assert!(pieces.len() >= 2, "one piece per row: {}", pieces.len());
        let (last, _) = pieces.last().unwrap();
        let (last_x, last_y) = (last.x, last.y);
        click(&mut app, last_x, last_y);
        assert_eq!(app.last_opened.as_deref(), Some(long));
    }

    #[test]
    fn clicking_a_quoted_reply_goes_to_the_original_message() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        let now = crate::app::now_ms();
        let mut reply = msg("R1", "me", "setuju!", now - 5_000);
        reply.reply_to_id = Some("0".into());
        add_message(&mut app, reply);
        // Plenty of messages in between, so the original is far above the view.
        for i in 0..40 {
            add_message(&mut app, msg(&format!("f{i}"), "628@s.whatsapp.net", &format!("pengisi nomor {i}"), now - 4_000 + i));
        }
        let mut reply2 = msg("R2", "me", "balasan terakhir", now - 1_000);
        reply2.reply_to_id = Some("0".into());
        add_message(&mut app, reply2);
        let mut term = Terminal::new(TestBackend::new(110, 24)).unwrap();
        draw_once(&mut app, &mut term);
        assert!(app.bubble_hits.iter().all(|(_, id)| id != "0"), "the original is out of view");
        let (r, target) = app.quote_hits.first().cloned().expect("the visible reply has a clickable quote");
        assert_eq!(target, "0");

        click(&mut app, r.x + 1, r.y);
        assert_eq!(app.sel_msg.as_deref(), Some("0"));
        draw_once(&mut app, &mut term);
        app.finish_scroll();
        draw_once(&mut app, &mut term);
        assert!(app.bubble_hits.iter().any(|(_, id)| id == "0"), "the original is on screen now");
        assert!(app.scroll > 0);
    }

    #[test]
    fn a_quote_of_an_unloaded_message_still_asks_for_it() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        let now = crate::app::now_ms();
        let mut reply = msg("R9", "me", "ini balasan lama", now - 5_000);
        reply.reply_to_id = Some("jauh-sekali".into());
        add_message(&mut app, reply);
        let mut term = Terminal::new(TestBackend::new(110, 40)).unwrap();
        let rows = draw_once(&mut app, &mut term);
        assert!(rows.iter().any(|r| r.contains("↪ Reply")));
        let (r, target) = app.quote_hits[0].clone();
        assert_eq!(target, "jauh-sekali");
        // The offline runtime never runs the fetch; it must simply not crash or select anything.
        click(&mut app, r.x, r.y);
        assert_eq!(app.sel_msg, None);
    }

    #[test]
    fn dragging_over_text_selects_it_and_copies_on_release() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        let now = crate::app::now_ms();
        add_message(&mut app, msg("T1", "me", "salin kalimat ini saja", now - 10_000));
        let mut term = Terminal::new(TestBackend::new(110, 40)).unwrap();
        let rows = draw_once(&mut app, &mut term);
        let y = rows.iter().position(|r| r.contains("salin kalimat")).unwrap() as u16;
        let x0 = rows[y as usize].find("salin").map(|b| rows[y as usize][..b].chars().count()).unwrap() as u16;

        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x0, y);
        mouse(&mut app, MouseEventKind::Drag(MouseButton::Left), x0 + 5, y);
        mouse(&mut app, MouseEventKind::Drag(MouseButton::Left), x0 + 13, y);
        // The picked cells are drawn reversed.
        draw_once(&mut app, &mut term);
        assert!(term.backend().buffer()[(x0 + 3, y)].modifier.contains(Modifier::REVERSED));
        assert!(!term.backend().buffer()[(x0 + 20, y)].modifier.contains(Modifier::REVERSED));
        mouse(&mut app, MouseEventKind::Up(MouseButton::Left), x0 + 13, y);
        assert_eq!(app.last_copied.as_deref(), Some("salin kalimat"));
        assert!(app.notice.as_ref().is_some_and(|(t, _)| t.contains("Copied 13")));

        // A plain click afterwards clears the highlight and copies nothing more.
        app.last_copied = None;
        click(&mut app, x0, y);
        draw_once(&mut app, &mut term);
        assert!(!term.backend().buffer()[(x0 + 3, y)].modifier.contains(Modifier::REVERSED));
        assert_eq!(app.last_copied, None);
    }

    #[test]
    fn a_selection_over_several_bubbles_copies_every_row() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        let now = crate::app::now_ms();
        add_message(&mut app, msg("T2", "me", "baris pertama", now - 10_000));
        add_message(&mut app, msg("T3", "me", "baris kedua", now - 9_000));
        let mut term = Terminal::new(TestBackend::new(110, 40)).unwrap();
        let rows = draw_once(&mut app, &mut term);
        let y1 = rows.iter().position(|r| r.contains("baris pertama")).unwrap() as u16;
        let y2 = rows.iter().position(|r| r.contains("baris kedua")).unwrap() as u16;
        // Dragged upwards, from the end of the second to the start of the first.
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), 109, y2);
        mouse(&mut app, MouseEventKind::Drag(MouseButton::Left), 0, y1);
        mouse(&mut app, MouseEventKind::Up(MouseButton::Left), 0, y1);
        let copied = app.last_copied.clone().unwrap();
        assert!(copied.contains("baris pertama") && copied.contains("baris kedua"), "{copied:?}");
        assert!(copied.find("pertama") < copied.find("kedua"));
    }

    #[test]
    fn forward_lists_every_chat_and_typing_narrows_it() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        for i in 0..50 {
            app.chats.push(Chat {
                id: format!("c{i}@s.whatsapp.net"),
                name: format!("Teman {i:02}"),
                last_time: 2 + i,
                ..serde_json::from_str(r#"{"id":"x"}"#).unwrap()
            });
        }
        app.chats.push(Chat {
            id: "keluarga@g.us".into(),
            name: "Keluarga Besar".into(),
            is_group: true,
            last_time: 99,
            ..serde_json::from_str(r#"{"id":"x"}"#).unwrap()
        });
        let mut term = Terminal::new(TestBackend::new(110, 40)).unwrap();
        draw_once(&mut app, &mut term);
        app.open_message_menu((10, 10), "p1");
        // Down to "Forward" and open it.
        while app.menu.as_ref().unwrap().current().unwrap().label != "Forward" {
            key(&mut app, KeyCode::Char('j'));
        }
        key(&mut app, KeyCode::Enter);
        let m = app.menu.as_ref().unwrap();
        assert!(m.search.is_some());
        assert!(m.items.len() > 50, "not cut to 30: {}", m.items.len());
        let rows = draw_once(&mut app, &mut term);
        assert!(rows.iter().any(|r| r.contains("type to search")));

        // Letters are text, not j/k movement.
        for ch in "kelu".chars() {
            key(&mut app, KeyCode::Char(ch));
        }
        let m = app.menu.as_ref().unwrap();
        assert_eq!(m.items.iter().map(|i| i.label.as_str()).collect::<Vec<_>>(), ["Keluarga Besar"]);
        let rows = draw_once(&mut app, &mut term);
        assert!(rows.iter().any(|r| r.contains("⌕ kelu")), "the field shows what was typed");

        key(&mut app, KeyCode::Backspace);
        key(&mut app, KeyCode::Backspace);
        key(&mut app, KeyCode::Backspace);
        key(&mut app, KeyCode::Backspace);
        assert!(app.menu.as_ref().unwrap().items.len() > 50, "empty search lists all again");
        for ch in "zzz".chars() {
            key(&mut app, KeyCode::Char(ch));
        }
        let rows = draw_once(&mut app, &mut term);
        assert!(rows.iter().any(|r| r.contains("No match")));

        // Esc clears the text, a second Esc goes back to the message menu.
        key(&mut app, KeyCode::Esc);
        assert!(app.menu.as_ref().unwrap().search.is_some());
        key(&mut app, KeyCode::Esc);
        assert!(app.menu.as_ref().unwrap().search.is_none());
        assert!(app.menu.as_ref().unwrap().items.iter().any(|i| i.label == "Reply"));
    }

    #[test]
    fn a_click_on_a_search_result_forwards_there() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = app_with_picture(&rt);
        app.chats.push(Chat {
            id: "keluarga@g.us".into(),
            name: "Keluarga Besar".into(),
            is_group: true,
            last_time: 99,
            ..serde_json::from_str(r#"{"id":"x"}"#).unwrap()
        });
        let mut term = Terminal::new(TestBackend::new(110, 40)).unwrap();
        draw_once(&mut app, &mut term);
        app.open_message_menu((10, 10), "p1");
        while app.menu.as_ref().unwrap().current().unwrap().label != "Forward" {
            key(&mut app, KeyCode::Char('j'));
        }
        key(&mut app, KeyCode::Enter);
        for ch in "besar".chars() {
            key(&mut app, KeyCode::Char(ch));
        }
        let rows = draw_once(&mut app, &mut term);
        let y = rows.iter().position(|r| r.contains("Keluarga Besar")).unwrap() as u16;
        let x = rows[y as usize].find("Keluarga").map(|b| rows[y as usize][..b].chars().count()).unwrap() as u16;
        click(&mut app, x + 2, y);
        assert!(app.menu.is_none(), "the menu closes after picking");
    }
}

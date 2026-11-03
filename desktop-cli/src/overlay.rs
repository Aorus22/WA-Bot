//! Things drawn on top of everything: the context menu and the
//! "send this picture?" box.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};
use ratatui::Frame;
use ratatui_image::{FilterType, Resize, StatefulImage};

use crate::actions::Attach;
use crate::images;
use crate::menu::{Action, Menu};
use crate::text::{truncate, width};
use crate::theme as c;

pub fn draw_menu(f: &mut Frame, menu: &mut Menu, area: Rect) {
    let title_w = menu.title.as_deref().map_or(0, |t| width(t) + 2);
    let widest = menu
        .items
        .iter()
        .map(|i| width(&i.label) + if i.hint.is_empty() && !matches!(i.action, Action::Submenu { .. } | Action::SearchMenu { .. }) { 0 } else { width(&i.hint) + 2 } + 2)
        .max()
        .unwrap_or(10);
    let min_w = if menu.search.is_some() { 32 } else { 18 };
    let w = (widest.max(title_w) + 4).clamp(min_w, 46).min(area.width.saturating_sub(2) as usize) as u16;

    let rules = |items: &[crate::menu::MenuItem]| items.iter().filter(|i| i.rule_above).count();
    // Border, title, search field and the "no match" line when nothing is left.
    let searching = menu.search.is_some();
    let chrome = 2 + u16::from(menu.title.is_some()) + u16::from(searching) + u16::from(searching && menu.items.is_empty());
    let max_rows = area.height.saturating_sub(2 + chrome) as usize;

    // Keep the highlighted item inside a window when the list is taller than the screen.
    let n = menu.items.len();
    let shown = n.min(max_rows.max(1));
    if shown == 0 {
        menu.offset = 0;
    } else if menu.sel < menu.offset {
        menu.offset = menu.sel;
    } else if menu.sel >= menu.offset + shown {
        menu.offset = menu.sel + 1 - shown;
    }
    let window = &menu.items[menu.offset..(menu.offset + shown).min(n)];
    let h = (window.len() + rules(window)) as u16 + chrome;
    let h = h.min(area.height);

    let x = menu.at.0.min(area.x + area.width.saturating_sub(w));
    let y = menu.at.1.min(area.y + area.height.saturating_sub(h));
    let rect = Rect::new(x, y, w, h);
    menu.rect = rect;

    f.render_widget(Clear, rect);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(c::BLUE))
        .style(Style::new().bg(c::SURFACE));
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    let mut lines: Vec<Line> = Vec::new();
    menu.rows.clear();
    let mut row = inner.y;
    if let Some(t) = &menu.title {
        lines.push(Line::styled(format!(" {t}"), Style::new().fg(c::DIM).add_modifier(Modifier::BOLD)));
        row += 1;
    }
    let iw = inner.width as usize;
    if let Some(s) = &menu.search {
        let line = if s.query.is_empty() {
            Line::from(vec![Span::styled(" ⌕ ", Style::new().fg(c::BLUE)), Span::styled("type to search", Style::new().fg(c::DIM))])
        } else {
            Line::from(vec![
                Span::styled(" ⌕ ", Style::new().fg(c::BLUE)),
                Span::styled(truncate(&s.query, iw.saturating_sub(5)), Style::new().fg(c::FG)),
                Span::styled("▏", Style::new().fg(c::BLUE)),
            ])
        };
        lines.push(line);
        row += 1;
        if menu.items.is_empty() {
            lines.push(Line::styled(" No match", Style::new().fg(c::DIM)));
            row += 1;
        }
    }
    for (k, item) in window.iter().enumerate() {
        let idx = menu.offset + k;
        if item.rule_above {
            lines.push(Line::styled("─".repeat(iw), Style::new().fg(c::BORDER)));
            row += 1;
        }
        let selected = idx == menu.sel;
        let fg = if item.danger { c::RED } else { c::FG };
        let arrow = if matches!(item.action, Action::Submenu { .. } | Action::SearchMenu { .. }) { "›" } else { "" };
        let right = if item.hint.is_empty() { arrow.to_string() } else { item.hint.clone() };
        let label = truncate(&item.label, iw.saturating_sub(width(&right) + 3));
        let gap = iw.saturating_sub(1 + width(&label) + width(&right) + 1);
        let mut style = Style::new().fg(fg);
        let mut line = Line::from(vec![
            Span::raw(" "),
            Span::styled(label, style),
            Span::raw(" ".repeat(gap)),
            Span::styled(right, Style::new().fg(c::DIM)),
            Span::raw(" "),
        ]);
        if selected {
            style = style.bg(c::SELECT).add_modifier(Modifier::BOLD);
            line = line.style(Style::new().bg(c::SELECT));
            // Re-style the label so bold applies to it only.
            line.spans[1].style = style;
        }
        lines.push(line);
        menu.rows.push((row, idx));
        row += 1;
    }
    f.render_widget(Paragraph::new(lines), inner);
}

/// The pasted picture with its caption field; Enter sends, Esc drops it.
pub fn draw_attach(f: &mut Frame, a: &mut Attach, chat_name: &str, font: Option<(u16, u16)>, area: Rect) {
    let w = (area.width * 4 / 5).clamp(40.min(area.width), 90);
    let h = (area.height * 4 / 5).clamp(14.min(area.height), 40);
    let rect = Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h);
    f.render_widget(Clear, rect);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(c::GREEN))
        .title(Span::styled(
            format!(" Send {} to {} ", a.picked.kind.label(), truncate(chat_name, w as usize - 24)),
            Style::new().fg(c::FG).add_modifier(Modifier::BOLD),
        ))
        .style(Style::new().bg(c::SURFACE));
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    let [pic, info, caption, hints] =
        Layout::vertical([Constraint::Min(2), Constraint::Length(1), Constraint::Length(3), Constraint::Length(1)]).areas(inner);

    let dims = a.picked.size();
    match (dims, a.proto.as_mut(), font) {
        (Some((pw, ph)), Some(proto), Some(font)) => {
            let (cols, rows) = images::fit(pw, ph, font, pic.width, pic.height, true);
            let r = Rect::new(pic.x + (pic.width - cols) / 2, pic.y + (pic.height - rows) / 2, cols, rows);
            f.render_stateful_widget(StatefulImage::default().resize(Resize::Fit(Some(FilterType::Triangle))), r, proto);
        }
        (Some(_), _, _) => {
            let y = pic.y + pic.height / 2;
            f.render_widget(
                Paragraph::new(Line::styled("(preview needs a graphics terminal)", Style::new().fg(c::DIM))).centered(),
                Rect::new(pic.x, y, pic.width, 1),
            );
        }
        (None, _, _) => {
            // Not a picture: a file tile with its extension.
            let ext = crate::attach::ext_of(&a.picked.name).to_uppercase();
            let label = if ext.is_empty() { a.picked.kind.label().to_uppercase() } else { truncate(&ext, 6) };
            let (tw, th) = (24.min(pic.width), 7.min(pic.height));
            let tile = Rect::new(pic.x + (pic.width - tw) / 2, pic.y + (pic.height.saturating_sub(th)) / 2, tw, th);
            let tb = Block::bordered().border_type(BorderType::Rounded).border_style(Style::new().fg(c::BLUE)).style(Style::new().bg(c::BG_DARK));
            let ti = tb.inner(tile);
            f.render_widget(tb, tile);
            let mut lines = vec![Line::raw(""); (ti.height / 2) as usize];
            lines.push(Line::styled(label, Style::new().fg(c::BLUE).add_modifier(Modifier::BOLD)).centered());
            lines.push(Line::styled(a.picked.kind.label(), Style::new().fg(c::DIM)).centered());
            f.render_widget(Paragraph::new(lines), ti);
        }
    }

    let kb = (a.picked.bytes.len() as f64 / 1024.0).max(1.0);
    let size = if kb >= 1024.0 { format!("{:.1} MB", kb / 1024.0) } else { format!("{kb:.0} KB") };
    let detail = match dims {
        Some((pw, ph)) => format!("{pw}×{ph}"),
        None => a.picked.mime.to_string(),
    };
    f.render_widget(
        Paragraph::new(Line::styled(format!(" {} · {detail} · {size}", truncate(&a.picked.name, 30)), Style::new().fg(c::DIM))),
        info,
    );

    let cb = Block::bordered().border_type(BorderType::Rounded).border_style(Style::new().fg(if a.sending { c::DIM } else { c::GREEN }));
    let ci = cb.inner(caption);
    f.render_widget(cb, caption);
    let text = if !a.picked.kind.allows_caption() {
        Span::styled(" Audio has no caption", Style::new().fg(c::DIM))
    } else if a.caption.text.is_empty() {
        Span::styled(" Add a caption…", Style::new().fg(c::DIM))
    } else {
        Span::styled(format!(" {}", a.caption.text), Style::new().fg(c::FG))
    };
    f.render_widget(Paragraph::new(text), ci);
    if !a.sending && a.picked.kind.allows_caption() {
        f.set_cursor_position((ci.x + 1 + a.caption.cursor_col().min(ci.width.saturating_sub(2) as usize) as u16, ci.y));
    }
    if a.sending {
        a.send_btn = Rect::default();
        a.cancel_btn = Rect::default();
        f.render_widget(Paragraph::new(Line::styled(" Sending…", Style::new().fg(c::YELLOW))), hints);
    } else {
        // Two buttons, so the whole box works with the mouse; Enter / Esc still do the same.
        let (send, cancel) = (" ⏎ Send ", " Cancel ");
        let (sw, cw) = (width(send) as u16, width(cancel) as u16);
        let send_r = Rect::new(hints.x + 1, hints.y, sw, 1);
        let cancel_r = Rect::new(send_r.x + sw + 2, hints.y, cw, 1);
        f.render_widget(Paragraph::new(Span::styled(send, Style::new().fg(c::BG).bg(c::GREEN).add_modifier(Modifier::BOLD))), send_r);
        f.render_widget(Paragraph::new(Span::styled(cancel, Style::new().fg(c::FG).bg(c::SELECT))), cancel_r);
        let note = Rect::new(cancel_r.x + cw + 2, hints.y, hints.width.saturating_sub(sw + cw + 5), 1);
        f.render_widget(Paragraph::new(Span::styled("or Enter / Esc", Style::new().fg(c::DIM))), note);
        a.send_btn = send_r;
        a.cancel_btn = cancel_r;
    }
}

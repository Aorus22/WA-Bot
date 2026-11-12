//! The sticker picker: a grid of the recent and favourite stickers. A click (or
//! Enter) sends the sticker, as in WhatsApp.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};
use ratatui::Frame;
use ratatui_image::{FilterType, Resize, StatefulImage};
use wabot_backend_client::StickerFavorite;

use crate::actions::Then;
use crate::app::{App, Event};
use crate::images::{Img, Kind, Slot};
use crate::text::{truncate, width};
use crate::theme as c;

/// One grid cell: the sticker (8x4 cells) plus a margin.
const CELL_W: u16 = 10;
const CELL_H: u16 = 5;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StickerTab {
    Recent,
    Favourites,
}

pub struct Stickers {
    pub tab: StickerTab,
    pub recent: Vec<StickerFavorite>,
    pub favourites: Vec<StickerFavorite>,
    pub loaded: bool,
    pub error: Option<String>,
    pub sel: usize,
    /// First visible grid row.
    pub row_offset: usize,
    // Set by the last draw, for the mouse:
    pub rect: Rect,
    pub grid: Rect,
    pub cols: usize,
    pub tab_hits: Vec<(Rect, StickerTab)>,
    pub close_btn: Rect,
}

impl Stickers {
    pub fn new() -> Self {
        Self {
            tab: StickerTab::Recent,
            recent: Vec::new(),
            favourites: Vec::new(),
            loaded: false,
            error: None,
            sel: 0,
            row_offset: 0,
            rect: Rect::default(),
            grid: Rect::default(),
            cols: 1,
            tab_hits: Vec::new(),
            close_btn: Rect::default(),
        }
    }

    pub fn items(&self) -> &[StickerFavorite] {
        match self.tab {
            StickerTab::Recent => &self.recent,
            StickerTab::Favourites => &self.favourites,
        }
    }

    fn set_tab(&mut self, tab: StickerTab) {
        self.tab = tab;
        self.sel = 0;
        self.row_offset = 0;
    }

    /// Move the highlight by whole cells (`dx`) or rows (`dy`).
    fn step(&mut self, dx: isize, dy: isize) {
        let n = self.items().len();
        if n == 0 {
            return;
        }
        let delta = dx + dy * self.cols.max(1) as isize;
        self.sel = (self.sel as isize + delta).clamp(0, n as isize - 1) as usize;
    }
}

impl App {
    pub fn open_stickers(&mut self) {
        if self.open.is_none() {
            self.notice("Open a chat first");
            return;
        }
        if self.stickers.is_none() {
            self.stickers = Some(Stickers::new());
        }
        let loaded = self.stickers.as_ref().is_some_and(|s| s.loaded);
        if !loaded {
            let (http, tx) = (self.http.clone(), self.tx.clone());
            self.rt.spawn(async move {
                let result = async {
                    let favourites = http.get_favorite_stickers().await.map_err(|e| e.to_string())?;
                    let recent = http.get_recent_stickers().await.unwrap_or_default();
                    Ok::<_, String>((recent, favourites))
                }
                .await;
                let _ = tx.send(Event::Stickers(result));
            });
        }
    }

    pub fn on_stickers(&mut self, result: Result<(Vec<StickerFavorite>, Vec<StickerFavorite>), String>) {
        let Some(s) = self.stickers.as_mut() else { return };
        s.loaded = true;
        match result {
            Ok((recent, favourites)) => {
                s.recent = recent;
                s.favourites = favourites;
                // Nothing recent but favourites exist: start there.
                if s.recent.is_empty() && !s.favourites.is_empty() {
                    s.tab = StickerTab::Favourites;
                }
                s.error = None;
            }
            Err(e) => s.error = Some(e),
        }
    }

    /// Send the highlighted sticker to the open chat.
    fn send_sticker(&mut self) {
        let Some(chat) = self.open.clone() else { return };
        let Some(sticker) = self.stickers.as_ref().and_then(|s| s.items().get(s.sel).cloned()) else { return };
        self.spawn_call("Sticker sent", Then::CloseStickers, move |h| async move {
            h.send_sticker(&chat, &sticker.media_url, sticker.is_animated).await.map(|_| ()).map_err(|e| e.to_string())
        });
    }

    pub fn key_stickers(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let Some(s) = self.stickers.as_mut() else { return };
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.stickers = None,
            KeyCode::Char('c') if ctrl => self.stickers = None,
            KeyCode::Char('h') | KeyCode::Left => s.step(-1, 0),
            KeyCode::Char('l') | KeyCode::Right => s.step(1, 0),
            KeyCode::Char('j') | KeyCode::Down => s.step(0, 1),
            KeyCode::Char('k') | KeyCode::Up => s.step(0, -1),
            KeyCode::Char('g') | KeyCode::Home => s.sel = 0,
            KeyCode::Char('G') | KeyCode::End => s.sel = s.items().len().saturating_sub(1),
            KeyCode::Tab | KeyCode::BackTab | KeyCode::Char('1') | KeyCode::Char('2') => {
                let next = match (key.code, s.tab) {
                    (KeyCode::Char('1'), _) => StickerTab::Recent,
                    (KeyCode::Char('2'), _) => StickerTab::Favourites,
                    (_, StickerTab::Recent) => StickerTab::Favourites,
                    (_, StickerTab::Favourites) => StickerTab::Recent,
                };
                s.set_tab(next);
            }
            KeyCode::Enter => self.send_sticker(),
            _ => {}
        }
    }

    pub fn mouse_stickers(&mut self, m: MouseEvent) {
        let Some(s) = self.stickers.as_mut() else { return };
        let at = |r: Rect| r.width > 0 && m.column >= r.x && m.column < r.x + r.width && m.row >= r.y && m.row < r.y + r.height;
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if at(s.close_btn) {
                    self.stickers = None;
                } else if let Some(tab) = s.tab_hits.iter().find(|(r, _)| at(*r)).map(|(_, t)| *t) {
                    s.set_tab(tab);
                } else if at(s.grid) {
                    let col = ((m.column - s.grid.x) / CELL_W) as usize;
                    let row = ((m.row - s.grid.y) / CELL_H) as usize + s.row_offset;
                    let idx = row * s.cols + col;
                    if col < s.cols && idx < s.items().len() {
                        s.sel = idx;
                        self.send_sticker();
                    }
                }
            }
            MouseEventKind::ScrollUp => s.step(0, -1),
            MouseEventKind::ScrollDown => s.step(0, 1),
            _ => {}
        }
    }
}

pub fn draw_stickers(f: &mut Frame, app: &mut App, area: Rect) {
    let picker = app.picker.clone();
    let Some(s) = app.stickers.as_mut() else { return };
    let w = (area.width * 7 / 10).clamp(32.min(area.width), 100);
    let h = (area.height * 8 / 10).clamp(12.min(area.height), 36);
    let rect = Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h);
    s.rect = rect;
    f.render_widget(Clear, rect);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(c::magenta()))
        .title(Span::styled(" Stickers ", Style::new().fg(c::fg()).add_modifier(Modifier::BOLD)))
        .style(Style::new().bg(c::surface()));
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    let [tabs, grid, hint] = Layout::vertical([Constraint::Length(1), Constraint::Min(1), Constraint::Length(1)]).areas(inner);

    // Tabs and the close button.
    let mut spans = vec![Span::raw(" ")];
    let mut x = tabs.x + 1;
    s.tab_hits.clear();
    for (tab, label) in [(StickerTab::Recent, " Recent "), (StickerTab::Favourites, " Favourites ")] {
        let lw = width(label) as u16;
        let style = if tab == s.tab { Style::new().fg(c::on_accent()).bg(c::magenta()).add_modifier(Modifier::BOLD) } else { Style::new().fg(c::dim()) };
        spans.push(Span::styled(label, style));
        s.tab_hits.push((Rect::new(x, tabs.y, lw, 1), tab));
        x += lw;
    }
    let close = " esc close ";
    let cw = width(close) as u16;
    let gap = (tabs.width as usize).saturating_sub(1 + (x - tabs.x - 1) as usize + cw as usize);
    spans.push(Span::raw(" ".repeat(gap)));
    spans.push(Span::styled(close, Style::new().fg(c::dim()).bg(c::bg_dark())));
    f.render_widget(Paragraph::new(Line::from(spans)), tabs);
    s.close_btn = Rect::new(tabs.x + tabs.width - cw, tabs.y, cw, 1);

    // The grid.
    s.grid = grid;
    s.cols = (grid.width / CELL_W).max(1) as usize;
    let rows_visible = (grid.height / CELL_H).max(1) as usize;
    let count = s.items().len();
    let sel_row = s.sel / s.cols;
    if sel_row < s.row_offset {
        s.row_offset = sel_row;
    } else if sel_row >= s.row_offset + rows_visible {
        s.row_offset = sel_row + 1 - rows_visible;
    }
    let total_rows = count.div_ceil(s.cols);
    s.row_offset = s.row_offset.min(total_rows.saturating_sub(rows_visible));

    if count == 0 {
        let msg = if !s.loaded {
            " Loading stickers…".to_string()
        } else if let Some(e) = &s.error {
            format!(" Could not load stickers: {e}")
        } else if s.tab == StickerTab::Recent {
            " No recent stickers yet".to_string()
        } else {
            " No favourites yet. Right-click a sticker in a chat and choose \"Add to favourites\".".to_string()
        };
        f.render_widget(Paragraph::new(Line::styled(truncate(&msg, grid.width as usize), Style::new().fg(c::dim()))), grid);
    }

    // Draw cells, then pictures (kept apart so the cache can be borrowed mutably).
    let (first, last) = (s.row_offset * s.cols, ((s.row_offset + rows_visible) * s.cols).min(count));
    let mut cells: Vec<(Rect, String, bool)> = Vec::new();
    for i in first..last {
        let (row, col) = (i / s.cols - s.row_offset, i % s.cols);
        let cell = Rect::new(grid.x + col as u16 * CELL_W, grid.y + row as u16 * CELL_H, CELL_W, CELL_H);
        cells.push((cell, s.items()[i].media_url.clone(), i == s.sel));
    }
    let hint_text = if s.items().is_empty() { "" } else { " click a sticker to send it · arrows move · ⏎ send" };
    f.render_widget(Paragraph::new(Line::styled(hint_text, Style::new().fg(c::dim()))), hint);
    for (cell, url, selected) in &cells {
        let bg = if *selected { c::select() } else { c::surface() };
        f.render_widget(Block::default().style(Style::new().bg(bg)), Rect::new(cell.x, cell.y, cell.width.saturating_sub(1), cell.height.saturating_sub(1)));
        let img_rect = Rect::new(cell.x + 1, cell.y, 8.min(cell.width), 4.min(cell.height));
        match app.sticker_imgs.get_mut(url) {
            Some(Img::Ready(img)) => {
                if let (Some(pk), false) = (&picker, app.hide_images) {
                    f.render_stateful_widget(
                        StatefulImage::default().resize(Resize::Fit(Some(FilterType::Triangle))),
                        img_rect,
                        img.proto(pk, Slot::Sticker),
                    );
                }
            }
            Some(_) => {}
            None => app.want.push((Kind::Sticker, url.clone())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sticker(id: &str) -> StickerFavorite {
        StickerFavorite { id: id.into(), media_url: format!("/media/{id}.webp"), is_animated: false, source: None }
    }

    #[test]
    fn the_highlight_moves_by_cells_and_rows_and_stays_inside() {
        let mut s = Stickers::new();
        s.recent = (0..7).map(|i| sticker(&format!("s{i}"))).collect();
        s.cols = 3;
        s.step(1, 0);
        assert_eq!(s.sel, 1);
        s.step(0, 1);
        assert_eq!(s.sel, 4, "one row down is three cells on");
        s.step(0, 5);
        assert_eq!(s.sel, 6, "clamped to the last sticker");
        s.step(-1, -9);
        assert_eq!(s.sel, 0);
        s.set_tab(StickerTab::Favourites);
        s.step(1, 0);
        assert_eq!(s.sel, 0, "no favourites: nothing to move to");
    }
}

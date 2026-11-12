//! The right-hand drawer: chat info (profile, media, docs, links, members) and
//! message search. Same two sheets as the gpui and web clients.

use std::collections::HashMap;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::layout::Size;
use ratatui::widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;
use ratatui_image::sliced::{SignedPosition, SlicedImage};
use wabot_backend_client::{GroupCache, Message, MessageReceipts};

use crate::app::{is_from_me, upsert, App, Event, Focus, LineEdit, Mode};
use crate::images::{self, Img, Kind, Slot};
use crate::smooth::Smooth;
use crate::text::{truncate, width, wrap};
use crate::theme::{self as c, local, name_color};

const PAGE: usize = 30;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PanelKind {
    Info,
    Search,
    /// Who received and read one message.
    MsgInfo,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Tab {
    Media,
    Docs,
    Links,
    Members,
}

impl Tab {
    fn label(self) -> &'static str {
        match self {
            Tab::Media => "Media",
            Tab::Docs => "Docs",
            Tab::Links => "Links",
            Tab::Members => "Members",
        }
    }

    /// Terminal rows one entry takes.
    fn row_h(self) -> u16 {
        match self {
            Tab::Media => 4,
            Tab::Docs | Tab::Links => 2,
            Tab::Members => 1,
        }
    }
}

#[derive(Default)]
pub struct TabData {
    /// Newest first.
    pub items: Vec<Message>,
    pub loaded: bool,
    pub loading: bool,
    pub exhausted: bool,
}

pub struct Panel {
    pub kind: PanelKind,
    pub chat_id: String,
    pub tab: Tab,
    pub sel: usize,
    pub offset: usize,
    pub group: Option<GroupCache>,
    pub tabs: HashMap<Tab, TabData>,
    pub query: LineEdit,
    pub results: Vec<Message>,
    /// The query the results belong to.
    pub searched: Option<String>,
    pub searching: bool,
    // Set by the last draw, for the mouse:
    pub rect: Rect,
    pub list: Rect,
    pub tab_hits: Vec<(Rect, Tab)>,
    /// Info drawer: its document scrolls as a whole.
    pub scroll_y: usize,
    pub max_scroll: usize,
    /// Keep the highlighted row in view (set by the keyboard, cleared by the wheel).
    pub follow: bool,
    pub at_end: bool,
    pub row_hits: Vec<(Rect, usize)>,
    pub rect_view: Rect,
    /// Eases `scroll_y`.
    pub smooth: Smooth,
    /// The drawer's scrollbar column.
    pub bar: Rect,
    /// The "esc close" button in the title row.
    pub close_btn: Rect,
    /// Message info: which message, and who received / read it.
    pub msg_id: Option<String>,
    pub receipts: Option<Result<MessageReceipts, String>>,
}

impl Panel {
    fn new(kind: PanelKind, chat_id: &str) -> Self {
        Self {
            kind,
            chat_id: chat_id.to_string(),
            tab: Tab::Media,
            sel: 0,
            offset: 0,
            group: None,
            tabs: HashMap::new(),
            query: LineEdit::default(),
            results: Vec::new(),
            searched: None,
            searching: false,
            rect: Rect::default(),
            list: Rect::default(),
            tab_hits: Vec::new(),
            scroll_y: 0,
            max_scroll: 0,
            follow: true,
            at_end: false,
            row_hits: Vec::new(),
            rect_view: Rect::default(),
            smooth: Smooth::new(),
            bar: Rect::default(),
            close_btn: Rect::default(),
            msg_id: None,
            receipts: None,
        }
    }

    pub fn tab_list(&self, is_group: bool) -> Vec<Tab> {
        let mut t = vec![Tab::Media, Tab::Docs, Tab::Links];
        if is_group {
            t.push(Tab::Members);
        }
        t
    }
}

impl App {
    fn open_chat_is_group(&self) -> bool {
        self.open.as_deref().and_then(|id| self.chat(id)).is_some_and(|c| c.is_group)
    }

    pub fn open_info(&mut self, chat_id: &str) {
        if self.open.as_deref() != Some(chat_id) {
            self.open_chat(chat_id);
        }
        let mut panel = Panel::new(PanelKind::Info, chat_id);
        self.fetch_group(chat_id);
        panel.tabs.insert(Tab::Media, TabData::default());
        self.panel = Some(panel);
        self.focus = Focus::Panel;
        self.mode = Mode::Normal;
        self.load_tab(Tab::Media, false);
    }

    /// Names of members come from the group's info.
    fn fetch_group(&self, chat_id: &str) {
        if self.chat(chat_id).is_some_and(|c| c.is_group) {
            let (http, tx, id) = (self.http.clone(), self.tx.clone(), chat_id.to_string());
            self.rt.spawn(async move {
                let result = http.get_group(&id).await.map_err(|e| e.to_string());
                let _ = tx.send(Event::Group { chat_id: id, result });
            });
        }
    }

    /// The message-info drawer: the message, when it was sent, who received and read it.
    pub fn open_msg_info(&mut self, chat_id: &str, msg_id: &str) {
        let mut panel = Panel::new(PanelKind::MsgInfo, chat_id);
        panel.msg_id = Some(msg_id.to_string());
        self.fetch_group(chat_id);
        if self.find_message(chat_id, msg_id).is_some_and(|m| is_from_me(&m)) {
            let (http, tx, chat, mid) = (self.http.clone(), self.tx.clone(), chat_id.to_string(), msg_id.to_string());
            self.rt.spawn(async move {
                let result = http.get_message_receipts(&chat, &mid).await.map_err(|e| e.to_string());
                let _ = tx.send(Event::Receipts { msg_id: mid, result });
            });
        }
        self.panel = Some(panel);
        self.focus = Focus::Panel;
        self.mode = Mode::Normal;
    }

    pub fn open_search(&mut self) {
        let Some(chat_id) = self.open.clone() else {
            self.notice("Open a chat first");
            return;
        };
        self.panel = Some(Panel::new(PanelKind::Search, &chat_id));
        self.focus = Focus::Panel;
        self.mode = Mode::Search;
    }

    pub fn close_panel(&mut self) {
        self.panel = None;
        if self.focus == Focus::Panel {
            self.focus = Focus::Messages;
        }
        if self.mode == Mode::Search {
            self.mode = Mode::Normal;
        }
    }

    fn load_tab(&mut self, tab: Tab, older: bool) {
        let Some(panel) = self.panel.as_mut() else { return };
        if tab == Tab::Members {
            return;
        }
        let data = panel.tabs.entry(tab).or_default();
        if data.loading || (older && data.exhausted) || (!older && data.loaded) {
            return;
        }
        data.loading = true;
        let before = if older { data.items.iter().map(|m| m.timestamp).min() } else { None };
        let (http, tx, chat_id) = (self.http.clone(), self.tx.clone(), panel.chat_id.clone());
        self.rt.spawn(async move {
            let result = match tab {
                Tab::Media => http.get_chat_media(&chat_id, Some(PAGE), before).await,
                Tab::Docs => http.get_chat_docs(&chat_id, Some(PAGE), before).await,
                _ => http.get_chat_links(&chat_id, Some(PAGE), before).await,
            }
            .map_err(|e| e.to_string());
            let _ = tx.send(Event::PanelPage { chat_id, tab, result });
        });
    }

    pub fn on_group(&mut self, chat_id: &str, result: Result<GroupCache, String>) {
        let Some(panel) = self.panel.as_mut().filter(|p| p.chat_id == chat_id) else { return };
        match result {
            Ok(g) => panel.group = Some(g),
            Err(e) => self.notice(format!("Could not load group info: {e}")),
        }
    }

    pub fn on_panel_page(&mut self, chat_id: &str, tab: Tab, result: Result<Vec<Message>, String>) {
        let Some(panel) = self.panel.as_mut().filter(|p| p.chat_id == chat_id) else { return };
        let data = panel.tabs.entry(tab).or_default();
        data.loading = false;
        match result {
            Ok(page) => {
                data.loaded = true;
                if page.len() < PAGE {
                    data.exhausted = true;
                }
                for m in page {
                    if !data.items.iter().any(|x| x.id == m.id) {
                        data.items.push(m);
                    }
                }
                data.items.sort_by_key(|m| std::cmp::Reverse(m.timestamp));
            }
            Err(e) => {
                data.exhausted = true;
                self.notice(format!("Could not load {}: {e}", tab.label().to_lowercase()));
            }
        }
    }

    pub fn on_search(&mut self, chat_id: &str, query: &str, result: Result<Vec<Message>, String>) {
        let Some(panel) = self.panel.as_mut().filter(|p| p.chat_id == chat_id) else { return };
        if panel.searched.as_deref() != Some(query) {
            return; // an older search that lost the race
        }
        panel.searching = false;
        match result {
            Ok(mut found) => {
                found.sort_by_key(|m| std::cmp::Reverse(m.timestamp));
                panel.results = found;
                panel.sel = 0;
                panel.offset = 0;
            }
            Err(e) => self.notice(format!("Search failed: {e}")),
        }
    }

    pub fn on_context(&mut self, chat_id: &str, msg_id: &str, result: Result<Vec<Message>, String>) {
        match result {
            Ok(mut msgs) if !msgs.is_empty() => {
                msgs.sort_by_key(|m| m.timestamp);
                let log = self.logs.entry(chat_id.to_string()).or_default();
                log.msgs = msgs;
                log.loaded = true;
                log.loading = false;
                log.exhausted = false;
                log.detached = true;
                self.sel_msg = Some(msg_id.to_string());
                self.reveal = Some((msg_id.to_string(), true));
            }
            Ok(_) => self.notice("That message is no longer available"),
            Err(e) => self.notice(format!("Could not open the message: {e}")),
        }
    }

    fn run_search(&mut self) {
        let Some(panel) = self.panel.as_mut() else { return };
        let q = panel.query.text.trim().to_string();
        if q.is_empty() {
            return;
        }
        panel.searched = Some(q.clone());
        panel.searching = true;
        panel.results.clear();
        let (http, tx, chat_id) = (self.http.clone(), self.tx.clone(), panel.chat_id.clone());
        self.rt.spawn(async move {
            let result = http.search_messages(&chat_id, &q, Some(50)).await.map_err(|e| e.to_string());
            let _ = tx.send(Event::Search { chat_id, query: q, result });
        });
    }

    /// Show a message in the conversation, loading its surroundings if needed.
    pub(crate) fn jump_to(&mut self, chat_id: &str, msg_id: &str) {
        let present = self.logs.get(chat_id).is_some_and(|l| l.msgs.iter().any(|m| m.id == msg_id));
        if present {
            self.sel_msg = Some(msg_id.to_string());
            self.reveal = Some((msg_id.to_string(), true));
            return;
        }
        let (http, tx, chat, id) = (self.http.clone(), self.tx.clone(), chat_id.to_string(), msg_id.to_string());
        self.rt.spawn(async move {
            let result = http.get_message_context(&chat, &id, Some(40)).await.map_err(|e| e.to_string());
            let _ = tx.send(Event::Context { chat_id: chat, msg_id: id, result });
        });
    }

    // ---- keys ---------------------------------------------------------------------------

    /// Typing the search query.
    pub fn key_search(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                // First Esc leaves the field; a second one (in the list) closes the drawer.
                let has_results = self.panel.as_ref().is_some_and(|p| p.searched.is_some());
                if has_results {
                    self.mode = Mode::Normal;
                } else {
                    self.close_panel();
                }
            }
            KeyCode::Enter => {
                self.run_search();
                self.mode = Mode::Normal;
            }
            KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => self.paste_clipboard(),
            _ => {
                if let Some(p) = self.panel.as_mut() {
                    p.query.key(key);
                }
            }
        }
    }

    /// Navigation inside the drawer (normal mode, panel focused).
    pub fn key_panel(&mut self, key: KeyEvent) {
        let is_group = self.open_chat_is_group();
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let Some(panel) = self.panel.as_mut() else { return };
        if panel.kind == PanelKind::MsgInfo {
            let max = panel.max_scroll as isize;
            let base = panel.smooth.goal().unwrap_or(panel.scroll_y) as isize;
            let go = |panel: &mut Panel, goal: isize| panel.smooth.aim(panel.scroll_y, goal.clamp(0, max) as usize);
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => self.close_panel(),
                KeyCode::Char('j') | KeyCode::Down => go(panel, base + 1),
                KeyCode::Char('k') | KeyCode::Up => go(panel, base - 1),
                KeyCode::Char('d') if ctrl => go(panel, base + 8),
                KeyCode::Char('u') if ctrl => go(panel, base - 8),
                KeyCode::Char('g') | KeyCode::Home => go(panel, 0),
                KeyCode::Char('G') | KeyCode::End => go(panel, max),
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.close_panel(),
            KeyCode::Char('j') | KeyCode::Down => self.panel_move(1),
            KeyCode::Char('k') | KeyCode::Up => self.panel_move(-1),
            KeyCode::Char('d') if ctrl => self.panel_move(5),
            KeyCode::Char('u') if ctrl => self.panel_move(-5),
            KeyCode::Char('g') | KeyCode::Home => {
                panel.sel = 0;
                panel.smooth.aim(panel.scroll_y, 0);
                panel.follow = true;
            }
            KeyCode::Char('G') | KeyCode::End => self.panel_move(isize::MAX / 2),
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Tab if panel.kind == PanelKind::Info => {
                let tabs = panel.tab_list(is_group);
                let i = tabs.iter().position(|t| *t == panel.tab).unwrap_or(0);
                let next = tabs[(i + 1) % tabs.len()];
                self.switch_tab(next);
            }
            KeyCode::Char('h') | KeyCode::Left | KeyCode::BackTab if panel.kind == PanelKind::Info => {
                let tabs = panel.tab_list(is_group);
                let i = tabs.iter().position(|t| *t == panel.tab).unwrap_or(0);
                let prev = tabs[(i + tabs.len() - 1) % tabs.len()];
                self.switch_tab(prev);
            }
            KeyCode::Char('/') | KeyCode::Char('i') if panel.kind == PanelKind::Search => self.mode = Mode::Search,
            KeyCode::Enter | KeyCode::Char('o') => self.panel_enter(),
            _ => {}
        }
    }

    fn switch_tab(&mut self, tab: Tab) {
        if let Some(p) = self.panel.as_mut() {
            p.tab = tab;
            p.sel = 0;
            p.offset = 0;
            p.follow = false;
        }
        self.load_tab(tab, false);
    }

    fn panel_len(&self) -> usize {
        let Some(p) = self.panel.as_ref() else { return 0 };
        match p.kind {
            PanelKind::Search => p.results.len(),
            PanelKind::MsgInfo => 0,
            PanelKind::Info if p.tab == Tab::Members => p.group.as_ref().map_or(0, |g| g.participants.len()),
            PanelKind::Info => p.tabs.get(&p.tab).map_or(0, |d| d.items.len()),
        }
    }

    fn panel_move(&mut self, delta: isize) {
        let len = self.panel_len();
        let Some(p) = self.panel.as_mut() else { return };
        if len == 0 {
            return;
        }
        p.sel = (p.sel as isize + delta).clamp(0, len as isize - 1) as usize;
        p.follow = true;
        let (kind, tab, near_end) = (p.kind, p.tab, p.sel + 3 >= len);
        if kind == PanelKind::Info && near_end {
            self.load_tab(tab, true);
        }
    }

    fn panel_enter(&mut self) {
        let Some(p) = self.panel.as_ref() else { return };
        let (kind, tab, sel, chat_id) = (p.kind, p.tab, p.sel, p.chat_id.clone());
        match kind {
            PanelKind::MsgInfo => {}
            PanelKind::Search => {
                if let Some(m) = p.results.get(sel).cloned() {
                    self.jump_to(&chat_id, &m.id);
                    self.focus = Focus::Messages;
                }
            }
            PanelKind::Info => match tab {
                Tab::Media => {
                    let items = p.tabs.get(&Tab::Media).map(|d| d.items.clone()).unwrap_or_default();
                    let Some(target) = items.get(sel).map(|m| m.id.clone()) else { return };
                    // The viewer steps through the chat's loaded pictures: add these to it.
                    let log = self.logs.entry(chat_id).or_default();
                    for m in items {
                        upsert(&mut log.msgs, m);
                    }
                    self.open_viewer(&target);
                }
                Tab::Links => {
                    let url = p.tabs.get(&Tab::Links).and_then(|d| d.items.get(sel)).and_then(link_of);
                    match url {
                        Some(u) => match crate::clip::write_text(&u) {
                            Ok(()) => self.notice("Link copied"),
                            Err(e) => self.notice(format!("Could not copy: {e}")),
                        },
                        None => self.notice("No link in that message"),
                    }
                }
                Tab::Docs => self.notice("Documents can be saved from the desktop app"),
                Tab::Members => {
                    let jid = p.group.as_ref().and_then(|g| g.participants.get(sel)).map(|m| m.jid.clone());
                    match jid.filter(|j| self.chat(j).is_some()) {
                        Some(j) => self.run_action(crate::menu::Action::OpenChat(j)),
                        None => self.notice("No chat with this member yet"),
                    }
                }
            },
        }
    }

    pub fn mouse_panel(&mut self, m: MouseEvent) -> bool {
        let Some(p) = self.panel.as_ref() else { return false };
        let at = |r: Rect| m.column >= r.x && m.column < r.x + r.width && m.row >= r.y && m.row < r.y + r.height;
        if !at(p.rect) {
            return false;
        }
        let info = p.kind != PanelKind::Search;
        let on_close = at(p.close_btn);
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if on_close {
                    self.close_panel();
                    return true;
                }
                self.focus = Focus::Panel;
                let tab = p.tab_hits.iter().find(|(r, _)| at(*r)).map(|(_, t)| *t);
                let row = if info {
                    p.row_hits.iter().find(|(r, _)| at(*r)).map(|(_, i)| *i)
                } else if at(p.list) {
                    Some(p.offset + ((m.row - p.list.y) / 2) as usize)
                } else {
                    None
                };
                if let Some(tab) = tab {
                    self.switch_tab(tab);
                } else if let Some(idx) = row.filter(|i| *i < self.panel_len()) {
                    let Some(p) = self.panel.as_mut() else { return true };
                    let was = p.sel == idx;
                    p.sel = idx;
                    p.follow = false;
                    // Pictures and search hits open on one click; the rest on the second.
                    let one_click = p.kind == PanelKind::Search || p.tab == Tab::Media;
                    if was || one_click {
                        self.panel_enter();
                    }
                }
            }
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let up = m.kind == MouseEventKind::ScrollUp;
                if info {
                    // The wheel scrolls the whole drawer, not the highlight.
                    if let Some(p) = self.panel.as_mut() {
                        p.follow = false;
                        let step = self.wheel_step;
                        let base = p.smooth.goal().unwrap_or(p.scroll_y) as isize;
                        let lead = p.scroll_y as isize;
                        let goal = (base + if up { -step } else { step })
                            .clamp(lead - crate::app::MAX_LEAD, lead + crate::app::MAX_LEAD)
                            .clamp(0, p.max_scroll as isize) as usize;
                        p.smooth.aim(p.scroll_y, goal);
                    }
                } else {
                    self.panel_move(if up { -1 } else { 1 });
                }
            }
            _ => {}
        }
        true
    }

    /// After a draw: load the next page when the drawer is scrolled to its end.
    pub fn panel_after_draw(&mut self) {
        let Some(p) = self.panel.as_ref() else { return };
        if p.kind == PanelKind::Info && p.at_end && p.tab != Tab::Members {
            let tab = p.tab;
            self.load_tab(tab, true);
        }
    }
}

/// The first web link in a message (or its preview card).
fn link_of(m: &Message) -> Option<String> {
    if let Some(lp) = m.extra.as_ref().and_then(|e| e.link_preview.as_ref()) {
        if !lp.url.is_empty() {
            return Some(lp.url.clone());
        }
    }
    m.content.split_whitespace().find(|w| w.starts_with("http://") || w.starts_with("https://")).map(str::to_string)
}

// ---- drawing -------------------------------------------------------------------------------

pub fn draw_panel(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(kind) = app.panel.as_ref().map(|p| p.kind) else { return };
    let focused = app.focus == Focus::Panel;
    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::new().fg(if focused { c::blue() } else { c::border() }))
        .style(Style::new().bg(c::bg_dark()));
    let inner = block.inner(area);
    f.render_widget(block, area);
    if let Some(p) = app.panel.as_mut() {
        p.rect = area;
        p.tab_hits.clear();
        p.row_hits.clear();
    }
    match kind {
        PanelKind::Info => draw_info(f, app, inner),
        PanelKind::Search => draw_search(f, app, inner),
        PanelKind::MsgInfo => draw_msg_info(f, app, inner),
    }
}

/// The drawer's title row: the title, and a clickable "esc close" at the right.
fn draw_title(f: &mut Frame, app: &mut App, area: Rect, text: &str) {
    let hint = " esc close ";
    let hw = (width(hint) as u16).min(area.width);
    let gap = (area.width as usize).saturating_sub(width(text) + 1 + hw as usize);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!(" {text}"), Style::new().fg(c::fg()).add_modifier(Modifier::BOLD)),
            Span::raw(" ".repeat(gap)),
            Span::styled(hint, Style::new().fg(c::dim()).bg(c::surface())),
        ])),
        area,
    );
    if let Some(p) = app.panel.as_mut() {
        p.close_btn = Rect::new(area.x + area.width - hw, area.y, hw, 1);
    }
}

/// A row of the drawer padded to the full width, so a highlight spans the whole row.
fn row_line(mut spans: Vec<Span<'static>>, w: usize, bg: Color) -> Line<'static> {
    let used: usize = spans.iter().map(|s| width(&s.content)).sum();
    if used < w {
        spans.push(Span::raw(" ".repeat(w - used)));
    }
    Line::from(spans).style(Style::new().bg(bg))
}

/// The info drawer. Everything under the title is one document that scrolls as a
/// whole: profile photo, name, description, the tab strip and the list.
fn draw_info(f: &mut Frame, app: &mut App, area: Rect) {
    let picker = app.picker.clone();
    let Some(panel) = app.panel.as_ref() else { return };
    let chat_id = panel.chat_id.clone();
    let chat = app.chat(&chat_id).cloned();
    let is_group = chat.as_ref().is_some_and(|c| c.is_group);
    let name = chat.as_ref().map_or(chat_id.clone(), |c| c.name.clone());
    let avatar_url = chat.as_ref().map(|c| c.avatar.clone()).unwrap_or_default();
    let desc = panel.group.as_ref().and_then(|g| g.description.clone()).unwrap_or_default();
    let subtitle = if is_group {
        let n = panel.group.as_ref().map_or(0, |g| g.participant_count.max(g.participants.len()));
        if n > 0 { format!("Group · {n} participants") } else { "Group".to_string() }
    } else {
        let digits: String = chat_id.split('@').next().unwrap_or("").chars().filter(char::is_ascii_digit).collect();
        if digits.is_empty() { String::new() } else { format!("+{digits}") }
    };

    let [title, view_full, hints] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)]).areas(area);
    draw_title(f, app, title, if is_group { "Group info" } else { "Contact info" });
    // The last column is the scrollbar.
    let view = Rect { width: view_full.width.saturating_sub(1), ..view_full };
    let bar_area = Rect { x: view_full.x + view_full.width.saturating_sub(1), width: 1, ..view_full };

    let w = view.width as usize;
    let dim = Style::new().fg(c::dim());

    // ---- the document ----------------------------------------------------------------
    let mut lines: Vec<Line<'static>> = vec![Line::raw("")];
    let av_w = 12u16.min(view.width.saturating_sub(2));
    let av_h = 6u16;
    let avatar_y = lines.len();
    let initial = name.chars().find(|c| c.is_alphanumeric()).map_or('#', |c| c.to_uppercase().next().unwrap_or(c));
    let tile = Style::new().fg(c::on_accent()).bg(name_color(&name)).add_modifier(Modifier::BOLD);
    for r in 0..av_h {
        let text = if r == av_h / 2 {
            let left = av_w as usize / 2;
            format!("{}{initial}{}", " ".repeat(left), " ".repeat((av_w as usize).saturating_sub(left + 1)))
        } else {
            " ".repeat(av_w as usize)
        };
        lines.push(Line::from(Span::styled(text, tile)).centered());
    }
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(truncate(&name, w.saturating_sub(2)), Style::new().fg(c::fg()).add_modifier(Modifier::BOLD))).centered());
    if !subtitle.is_empty() {
        lines.push(Line::from(Span::styled(subtitle, dim)).centered());
    }
    if !desc.trim().is_empty() {
        lines.push(Line::raw(""));
        for l in wrap(desc.trim(), w.saturating_sub(2)) {
            lines.push(Line::styled(format!(" {l}"), dim));
        }
    }
    lines.push(Line::raw(""));

    // Tab strip.
    let tabs_y = lines.len();
    let Some(panel) = app.panel.as_ref() else { return };
    let mut tab_spans: Vec<Span<'static>> = vec![Span::raw(" ")];
    let mut tab_x: Vec<(u16, u16, Tab)> = Vec::new();
    let mut x = 1u16;
    for tab in panel.tab_list(is_group) {
        let label = format!(" {} ", tab.label());
        let lw = width(&label) as u16;
        let style = if tab == panel.tab { Style::new().fg(c::on_accent()).bg(c::blue()).add_modifier(Modifier::BOLD) } else { dim };
        tab_spans.push(Span::styled(label, style));
        tab_x.push((x, lw, tab));
        x += lw;
    }
    lines.push(Line::from(tab_spans));
    lines.push(Line::raw(""));

    // The list for the current tab.
    let (tab, sel) = (panel.tab, panel.sel);
    let rh = tab.row_h() as usize;
    let focused = app.focus == Focus::Panel;
    let list_y = lines.len();
    let (len, loaded, loading_more) = match tab {
        Tab::Members => (panel.group.as_ref().map_or(0, |g| g.participants.len()), panel.group.is_some(), false),
        t => {
            let d = panel.tabs.get(&t);
            (d.map_or(0, |d| d.items.len()), d.is_some_and(|d| d.loaded), d.is_some_and(|d| d.loading))
        }
    };
    if len == 0 {
        let msg = if loaded {
            match tab {
                Tab::Media => " No media",
                Tab::Docs => " No documents",
                Tab::Links => " No links",
                Tab::Members => " No members",
            }
        } else {
            " Loading…"
        };
        lines.push(Line::styled(msg, dim));
    }
    let mut thumbs: Vec<(usize, String)> = Vec::new();
    for i in 0..len {
        let bg = if i == sel && focused { c::select() } else if i == sel { c::surface() } else { c::bg_dark() };
        match tab {
            Tab::Members => {
                let Some(m) = panel.group.as_ref().and_then(|g| g.participants.get(i)) else { continue };
                let who = m.name.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| m.jid.split('@').next().unwrap_or("").to_string());
                let role = if m.is_super_admin == Some(true) { "owner" } else if m.is_admin { "admin" } else { "" };
                let who = truncate(&who, w.saturating_sub(10));
                let gap = w.saturating_sub(2 + width(&who) + width(role) + 1);
                lines.push(row_line(
                    vec![
                        Span::raw("  "),
                        Span::styled(who.clone(), Style::new().fg(name_color(&who))),
                        Span::raw(" ".repeat(gap)),
                        Span::styled(role.to_string(), Style::new().fg(c::green())),
                    ],
                    w,
                    bg,
                ));
            }
            Tab::Media => {
                let Some(m) = panel.tabs.get(&tab).and_then(|d| d.items.get(i)) else { continue };
                let when = local(m.timestamp).map(|t| t.format("%-d %b %Y, %H:%M").to_string()).unwrap_or_default();
                let kind = if m.message_type == "video" { "video" } else { "image" };
                let text_w = w.saturating_sub(10);
                let pad = || Span::raw(" ".repeat(10));
                lines.push(row_line(vec![pad(), Span::styled(truncate(kind, text_w), Style::new().fg(c::fg()))], w, bg));
                lines.push(row_line(vec![pad(), Span::styled(truncate(&when, text_w), dim)], w, bg));
                lines.push(row_line(vec![pad(), Span::styled(truncate(m.content.trim(), text_w), Style::new().fg(c::fg()))], w, bg));
                lines.push(row_line(vec![], w, bg));
                if let Some(url) = images::media_url(m) {
                    thumbs.push((list_y + i * rh, url));
                }
            }
            Tab::Docs | Tab::Links => {
                let Some(m) = panel.tabs.get(&tab).and_then(|d| d.items.get(i)) else { continue };
                let when = local(m.timestamp).map(|t| t.format("%-d %b %Y").to_string()).unwrap_or_default();
                let main = if tab == Tab::Links {
                    m.extra
                        .as_ref()
                        .and_then(|e| e.link_preview.as_ref())
                        .and_then(|lp| lp.title.clone())
                        .filter(|t| !t.is_empty())
                        .or_else(|| link_of(m))
                        .unwrap_or_else(|| m.content.clone())
                } else {
                    m.content.trim().to_string()
                };
                let who = app.sender_label(&chat_id, m);
                lines.push(row_line(vec![Span::raw(" "), Span::styled(truncate(&main, w.saturating_sub(2)), Style::new().fg(c::fg()))], w, bg));
                lines.push(row_line(vec![Span::raw(" "), Span::styled(truncate(&format!("{who} · {when}"), w.saturating_sub(2)), dim)], w, bg));
            }
        }
    }
    if loading_more {
        lines.push(Line::styled(" loading more…", dim));
    }
    lines.push(Line::raw(""));

    // ---- scrolling ---------------------------------------------------------------------
    let total = lines.len();
    let vh = view.height as usize;
    let max_scroll = total.saturating_sub(vh);
    let Some(panel) = app.panel.as_mut() else { return };
    panel.max_scroll = max_scroll;
    if panel.follow && len > 0 {
        // Keyboard moved the highlight: keep its row on screen.
        let (top, bottom) = (list_y + sel * rh, list_y + (sel + 1) * rh);
        let desired = if top < panel.scroll_y {
            Some(top)
        } else if bottom > panel.scroll_y + vh {
            Some(bottom.saturating_sub(vh))
        } else {
            None
        };
        if let Some(d) = desired {
            panel.smooth.aim(panel.scroll_y, d.min(max_scroll));
        }
    }
    panel.scroll_y = panel.scroll_y.min(max_scroll);
    panel.at_end = panel.scroll_y >= max_scroll;
    let scroll = panel.scroll_y;
    let on_screen = |doc_y: usize, h: usize| -> Option<u16> {
        (doc_y >= scroll && doc_y + h <= scroll + vh).then(|| view.y + (doc_y - scroll) as u16)
    };

    // Mouse targets.
    panel.tab_hits = tab_x
        .iter()
        .filter_map(|(x, tw, t)| on_screen(tabs_y, 1).map(|y| (Rect::new(view.x + x, y, *tw, 1), *t)))
        .collect();
    panel.row_hits.clear();
    for i in 0..len {
        let (top, bottom) = (list_y + i * rh, list_y + (i + 1) * rh);
        if bottom <= scroll || top >= scroll + vh {
            continue;
        }
        let (a, b) = (top.max(scroll), bottom.min(scroll + vh));
        panel.row_hits.push((Rect::new(view.x, view.y + (a - scroll) as u16, view.width, (b - a) as u16), i));
    }
    panel.rect_view = view;
    panel.bar = if max_scroll > 0 { bar_area } else { Rect::default() };

    f.render_widget(Paragraph::new(lines).scroll((scroll as u16, 0)), view);
    if max_scroll > 0 {
        let mut state = ScrollbarState::new(max_scroll).position(scroll);
        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .track_symbol(Some("│"))
                .thumb_symbol("┃")
                .track_style(Style::new().fg(c::border()))
                .thumb_style(Style::new().fg(c::blue())),
            bar_area,
            &mut state,
        );
    }

    // ---- pictures: cut by the view's edges, so they show partly while scrolling ---------------
    let at = |x: u16, doc_y: usize| SignedPosition::from((x as i16, (doc_y as i64 - scroll as i64).clamp(-3000, 3000) as i16));
    if !avatar_url.is_empty() && !app.hide_images {
        match app.avatars.get_mut(&avatar_url) {
            Some(Img::Ready(img)) => {
                if let Some(pk) = &picker {
                    let size = Size::new(av_w, av_h);
                    let ok = img.is_built(Slot::Profile, size)
                        || images::take_budget(&mut app.build_budget, &mut app.pending_builds);
                    let sl = if ok { img.sliced(pk, Slot::Profile, size) } else { None };
                    if let Some(sl) = sl {
                        let x = view.width.saturating_sub(av_w) / 2;
                        f.render_widget(SlicedImage::new(sl, at(x, avatar_y)), view);
                    }
                }
            }
            Some(_) => {}
            None => app.want.push((Kind::Avatar, avatar_url.clone())),
        }
    }
    for (doc_y, url) in thumbs {
        let near = doc_y + 4 > scroll.saturating_sub(vh) && doc_y < scroll + vh * 2;
        match app.media.get_mut(&url) {
            Some(Img::Ready(img)) => {
                if app.hide_images {
                    continue;
                }
                if let Some(pk) = &picker {
                    let size = Size::new(8.min(view.width.saturating_sub(1)), 4);
                    let ok = img.is_built(Slot::Row, size)
                        || images::take_budget(&mut app.build_budget, &mut app.pending_builds);
                    let sl = if ok { img.sliced(pk, Slot::Row, size) } else { None };
                    if let Some(sl) = sl {
                        f.render_widget(SlicedImage::new(sl, at(1, doc_y)), view);
                    }
                }
            }
            Some(_) => {}
            None if near => app.want.push((Kind::Media, url)),
            None => {}
        }
    }

    let hint = match tab {
        Tab::Media => "h/l tab  j/k move  ⏎ view",
        Tab::Links => "h/l tab  j/k move  ⏎ copy link",
        Tab::Members => "h/l tab  j/k move  ⏎ open chat",
        Tab::Docs => "h/l tab  j/k move",
    };
    f.render_widget(Paragraph::new(Line::styled(format!(" {hint}"), dim)), hints);
}

/// Render `lines` as the drawer's scrolling document, with its scrollbar.
fn render_scrolling(f: &mut Frame, app: &mut App, lines: Vec<Line<'static>>, view: Rect, bar_area: Rect) {
    let max_scroll = lines.len().saturating_sub(view.height as usize);
    let Some(panel) = app.panel.as_mut() else { return };
    panel.max_scroll = max_scroll;
    panel.scroll_y = panel.scroll_y.min(max_scroll);
    panel.bar = if max_scroll > 0 { bar_area } else { Rect::default() };
    let scroll = panel.scroll_y;
    f.render_widget(Paragraph::new(lines).scroll((scroll as u16, 0)), view);
    if max_scroll > 0 {
        let mut state = ScrollbarState::new(max_scroll).position(scroll);
        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .track_symbol(Some("│"))
                .thumb_symbol("┃")
                .track_style(Style::new().fg(c::border()))
                .thumb_style(Style::new().fg(c::blue())),
            bar_area,
            &mut state,
        );
    }
}

/// Message info: the message itself, when it was sent, and who has received / read it.
fn draw_msg_info(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(panel) = app.panel.as_ref() else { return };
    let chat_id = panel.chat_id.clone();
    let msg_id = panel.msg_id.clone().unwrap_or_default();
    let receipts = panel.receipts.clone();
    let group = panel.group.clone();
    let msg = app.find_message(&chat_id, &msg_id);
    let chat = app.chat(&chat_id).cloned();

    let [title, view_full, hints] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)]).areas(area);
    draw_title(f, app, title, "Message info");
    let view = Rect { width: view_full.width.saturating_sub(1), ..view_full };
    let bar_area = Rect { x: view_full.x + view_full.width.saturating_sub(1), width: 1, ..view_full };
    let w = view.width as usize;
    let dim = Style::new().fg(c::dim());
    let fg = Style::new().fg(c::fg());

    // Who is a jid: group members by name, the contact in a private chat, else the number.
    let who = |jid: &str| -> String {
        if let Some(n) = group.as_ref().and_then(|g| g.participants.iter().find(|p| p.jid == jid)).and_then(|p| p.name.clone()).filter(|n| !n.is_empty()) {
            return n;
        }
        match &chat {
            Some(c) if !c.is_group => c.name.clone(),
            _ => jid.split('@').next().unwrap_or(jid).to_string(),
        }
    };
    let when = |ts: i64| local(ts).map(|t| t.format("%-d %b %Y, %H:%M").to_string()).unwrap_or_default();

    let mut lines: Vec<Line<'static>> = vec![Line::raw("")];
    let Some(m) = msg else {
        lines.push(Line::styled(" That message is no longer here", dim));
        render_scrolling(f, app, lines, view, bar_area);
        return;
    };
    let mine = is_from_me(&m);

    // The message, as in the conversation.
    let sender = app.sender_label(&chat_id, &m);
    lines.push(row_line(vec![Span::raw(" "), Span::styled(sender.clone(), Style::new().fg(name_color(&sender)).add_modifier(Modifier::BOLD))], w, c::surface()));
    for l in wrap(&crate::app::preview(&m), w.saturating_sub(2)).into_iter().take(14) {
        lines.push(row_line(vec![Span::raw(" "), Span::styled(l, fg)], w, c::surface()));
    }
    lines.push(row_line(vec![Span::raw(" "), Span::styled(when(m.timestamp), dim)], w, c::surface()));
    lines.push(Line::raw(""));

    let row = |label: &str, value: String| {
        Line::from(vec![Span::styled(format!(" {label:<8}"), dim), Span::styled(truncate(&value, w.saturating_sub(10)), fg)])
    };
    let status = if m.status.is_empty() { "received" } else { m.status.as_str() };
    lines.push(row("Sent", when(m.timestamp)));
    lines.push(row("Status", status.to_string()));

    if mine {
        lines.push(Line::raw(""));
        match &receipts {
            None => lines.push(Line::styled(" Loading receipts…", dim)),
            Some(Err(e)) => lines.push(Line::styled(truncate(&format!(" Receipts: {e}"), w), Style::new().fg(c::red()))),
            Some(Ok(r)) => {
                let section = |lines: &mut Vec<Line<'static>>, title: String, rows: Vec<(String, i64)>| {
                    lines.push(Line::styled(format!(" {title}"), Style::new().fg(c::blue()).add_modifier(Modifier::BOLD)));
                    if rows.is_empty() {
                        lines.push(Line::styled("   —", dim));
                    }
                    for (name, ts) in rows {
                        let t = if ts > 0 { when(ts) } else { String::new() };
                        let room = w.saturating_sub(3 + width(&t) + 2);
                        let name = truncate(&name, room);
                        let gap = w.saturating_sub(3 + width(&name) + width(&t) + 1);
                        lines.push(Line::from(vec![
                            Span::raw("   "),
                            Span::styled(name, fg),
                            Span::raw(" ".repeat(gap)),
                            Span::styled(t, dim),
                        ]));
                    }
                    lines.push(Line::raw(""));
                };
                let mut read: Vec<(String, i64)> = r.receipts.iter().filter(|x| x.read_at > 0).map(|x| (who(&x.jid), x.read_at)).collect();
                read.sort_by_key(|(_, t)| std::cmp::Reverse(*t));
                let mut delivered: Vec<(String, i64)> =
                    r.receipts.iter().filter(|x| x.delivered_at > 0 && x.read_at == 0).map(|x| (who(&x.jid), x.delivered_at)).collect();
                delivered.sort_by_key(|(_, t)| std::cmp::Reverse(*t));
                let seen: Vec<&str> = r.receipts.iter().filter(|x| x.delivered_at > 0 || x.read_at > 0).map(|x| x.jid.as_str()).collect();
                let waiting: Vec<(String, i64)> = r.recipients.iter().filter(|j| !seen.contains(&j.as_str())).map(|j| (who(j), 0)).collect();
                section(&mut lines, format!("Read by {} of {}", read.len(), r.total), read);
                section(&mut lines, format!("Delivered to {}", delivered.len()), delivered);
                if !waiting.is_empty() {
                    section(&mut lines, format!("Not delivered yet {}", waiting.len()), waiting);
                }
            }
        }
    }
    lines.push(Line::raw(""));
    render_scrolling(f, app, lines, view, bar_area);
    f.render_widget(Paragraph::new(Line::styled(" j/k scroll  esc close", dim)), hints);
}

fn draw_search(f: &mut Frame, app: &mut App, area: Rect) {
    let typing = app.mode == Mode::Search;
    let focused = app.focus == Focus::Panel;
    let chat_id = app.panel.as_ref().map(|p| p.chat_id.clone()).unwrap_or_default();
    let [title, input, status, list, hints] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);
    draw_title(f, app, title, "Search messages");

    let Some(panel) = app.panel.as_mut() else { return };
    let block = Block::bordered()
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::new().fg(if typing { c::yellow() } else { c::border() }));
    let inner = block.inner(input);
    f.render_widget(block, input);
    let text = if panel.query.text.is_empty() && !typing {
        Span::styled(" / to search this chat", Style::new().fg(c::dim()))
    } else {
        Span::styled(format!(" {}", panel.query.text), Style::new().fg(c::fg()))
    };
    f.render_widget(Paragraph::new(text), inner);
    if typing {
        f.set_cursor_position((inner.x + 1 + panel.query.cursor_col().min(inner.width.saturating_sub(2) as usize) as u16, inner.y));
    }

    let status_text = match (&panel.searched, panel.searching) {
        (None, _) => " Type a word and press Enter".to_string(),
        (Some(_), true) => " Searching…".to_string(),
        (Some(q), false) => format!(" {} result{} for \"{}\"", panel.results.len(), if panel.results.len() == 1 { "" } else { "s" }, truncate(q, 20)),
    };
    f.render_widget(Paragraph::new(Line::styled(status_text, Style::new().fg(c::dim()))), status);

    panel.list = list;
    let visible = (list.height / 2).max(1) as usize;
    let len = panel.results.len();
    if panel.sel < panel.offset {
        panel.offset = panel.sel;
    } else if panel.sel >= panel.offset + visible {
        panel.offset = panel.sel + 1 - visible;
    }
    let (offset, sel) = (panel.offset, panel.sel);
    let needle = panel.searched.clone().unwrap_or_default().to_lowercase();
    let w = list.width as usize;
    let mut rows: Vec<Line> = Vec::new();
    for i in (offset..len).take(visible) {
        let m = &app.panel.as_ref().expect("open").results[i];
        let bg = if i == sel && focused { c::select() } else if i == sel { c::surface() } else { c::bg_dark() };
        let when = local(m.timestamp).map(|t| t.format("%-d %b %Y, %H:%M").to_string()).unwrap_or_default();
        let who = app.sender_label(&chat_id, m);
        rows.push(
            Line::from(vec![
                Span::raw(" "),
                Span::styled(truncate(&who, w / 2), Style::new().fg(name_color(&who)).add_modifier(Modifier::BOLD)),
                Span::styled(format!("  {when}"), Style::new().fg(c::dim())),
            ])
            .style(Style::new().bg(bg)),
        );
        rows.push(highlight_line(&m.content, &needle, w.saturating_sub(2), bg));
    }
    f.render_widget(Paragraph::new(rows), list);
    f.render_widget(Paragraph::new(Line::styled(" / edit  j/k move  ⏎ show in chat", Style::new().fg(c::dim()))), hints);
}

/// One snippet line with the match in colour; starts at the match when it would be cut off.
fn highlight_line(content: &str, needle_lower: &str, max: usize, bg: ratatui::style::Color) -> Line<'static> {
    let flat: String = content.replace(['\n', '\r'], " ");
    let lower = flat.to_lowercase();
    let mut start = 0usize;
    let found = (!needle_lower.is_empty()).then(|| lower.find(needle_lower)).flatten();
    if let Some(pos) = found {
        // Keep the match in view: begin a little before it.
        let chars_before = flat[..pos.min(flat.len())].chars().count();
        if chars_before > max.saturating_sub(needle_lower.chars().count()) {
            start = chars_before.saturating_sub(8);
        }
    }
    let shown: String = flat.chars().skip(start).collect();
    let shown = if start > 0 { format!("…{shown}") } else { shown };
    let shown = truncate(&shown, max);
    let low = shown.to_lowercase();
    let mut spans = vec![Span::raw(" ")];
    match (!needle_lower.is_empty()).then(|| low.find(needle_lower)).flatten() {
        Some(p) if shown.is_char_boundary(p) && shown.is_char_boundary(p + needle_lower.len().min(shown.len() - p)) => {
            let end = p + needle_lower.len().min(shown.len() - p);
            spans.push(Span::styled(shown[..p].to_string(), Style::new().fg(c::fg())));
            spans.push(Span::styled(shown[p..end].to_string(), Style::new().fg(c::on_accent()).bg(c::yellow())));
            spans.push(Span::styled(shown[end..].to_string(), Style::new().fg(c::fg())));
        }
        _ => spans.push(Span::styled(shown, Style::new().fg(c::fg()))),
    }
    Line::from(spans).style(Style::new().bg(bg))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlight_marks_the_match_and_keeps_it_in_view() {
        let line = highlight_line("kita ketemu besok di kampus jam tujuh pagi sama Budi", "budi", 20, c::bg());
        let text: String = line.spans.iter().map(|s| s.content.to_string()).collect();
        assert!(text.to_lowercase().contains("budi"), "match scrolled out of view: {text:?}");
        assert!(line.spans.iter().any(|s| s.content.eq_ignore_ascii_case("budi")));
    }

    #[test]
    fn finds_the_first_link() {
        let mut m = Message { content: "lihat https://example.com/x dulu".into(), ..Default::default() };
        assert_eq!(link_of(&m).as_deref(), Some("https://example.com/x"));
        m.content = "tanpa link".into();
        assert!(link_of(&m).is_none());
    }
}

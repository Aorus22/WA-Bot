//! Application state and event handling (no drawing in here).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ratatui::crossterm::event::{
    Event as CtEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;
use tokio::runtime::Handle;
use wabot_backend_client::{Chat, ChatState, GroupCache, HttpClient, Message, MessageReceipts, PinnedMessage, WsEvent};

use image::DynamicImage;
use ratatui_image::picker::Picker;

use crate::actions::{Attach, Done, Pasted, Reply};
use crate::images::{self, ImageCache, Kind, Ready};
use crate::browser::Browser;
use crate::form::Form;
use crate::menu::Menu;
use crate::panel::{Panel, Tab};
use crate::smooth::Smooth;
use crate::stickers::Stickers;
use crate::text;

/// How many messages one page request asks for.
const PAGE: usize = 50;
/// A typing indicator lasts this long without being renewed.
const TYPING_TTL: Duration = Duration::from_secs(7);
/// How long a status-line notice stays.
const NOTICE_TTL: Duration = Duration::from_secs(5);

pub enum Event {
    Input(CtEvent),
    Ws(WsEvent),
    Tick,
    Chats(Result<Vec<Chat>, String>),
    Messages {
        chat_id: String,
        older: bool,
        result: Result<Vec<Message>, String>,
    },
    Sent {
        chat_id: String,
        temp_id: String,
        result: Result<String, String>,
    },
    MarkedRead {
        chat_id: String,
        result: Result<(), String>,
    },
    Done(Done),
    ChatState(Result<ChatState, String>),
    Pins {
        chat_id: String,
        result: Result<Vec<PinnedMessage>, String>,
    },
    Receipts {
        msg_id: String,
        result: Result<MessageReceipts, String>,
    },
    Group {
        chat_id: String,
        result: Result<GroupCache, String>,
    },
    PanelPage {
        chat_id: String,
        tab: Tab,
        result: Result<Vec<Message>, String>,
    },
    Search {
        chat_id: String,
        query: String,
        result: Result<Vec<Message>, String>,
    },
    Context {
        chat_id: String,
        msg_id: String,
        result: Result<Vec<Message>, String>,
    },
    Pasted(Result<Pasted, String>),
    Stickers(Result<(Vec<wabot_backend_client::StickerFavorite>, Vec<wabot_backend_client::StickerFavorite>), String>),
    /// A picture finished downloading and decoding.
    Image {
        kind: Kind,
        url: String,
        result: Result<DynamicImage, String>,
    },
}

/// The full-screen image viewer: one picture of a chat.
#[derive(Clone, Debug)]
pub struct Viewer {
    pub chat_id: String,
    pub msg_id: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Focus {
    Chats,
    Messages,
    /// The right-hand info / search drawer.
    Panel,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Normal,
    Insert,
    Filter,
    /// Typing in the drawer's search field.
    Search,
    /// Typing the caption of a pasted picture.
    Caption,
}

/// A single-line text field with a byte cursor.
#[derive(Default, Debug)]
pub struct LineEdit {
    pub text: String,
    /// Byte index, always on a char boundary.
    pub cursor: usize,
}

impl LineEdit {
    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }

    pub fn insert(&mut self, ch: char) {
        self.text.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
    }

    fn prev_boundary(&self, from: usize) -> usize {
        self.text[..from].char_indices().next_back().map_or(0, |(i, _)| i)
    }

    fn next_boundary(&self, from: usize) -> usize {
        self.text[from..].chars().next().map_or(from, |c| from + c.len_utf8())
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            let start = self.prev_boundary(self.cursor);
            self.text.drain(start..self.cursor);
            self.cursor = start;
        }
    }

    pub fn delete(&mut self) {
        if self.cursor < self.text.len() {
            let end = self.next_boundary(self.cursor);
            self.text.drain(self.cursor..end);
        }
    }

    pub fn delete_word_back(&mut self) {
        let before = &self.text[..self.cursor];
        let trimmed = before.trim_end_matches(' ');
        let start = trimmed.rfind(' ').map_or(0, |i| i + 1);
        self.text.drain(start..self.cursor);
        self.cursor = start;
    }

    pub fn left(&mut self) {
        self.cursor = self.prev_boundary(self.cursor);
    }

    pub fn right(&mut self) {
        self.cursor = self.next_boundary(self.cursor);
    }

    /// Terminal column of the cursor within the text.
    pub fn cursor_col(&self) -> usize {
        text::width(&self.text[..self.cursor])
    }

    /// Apply an editing key. Returns whether the key was an editing key.
    pub fn key(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('w') if ctrl => self.delete_word_back(),
            KeyCode::Char('u') if ctrl => self.clear(),
            KeyCode::Char('a') if ctrl => self.cursor = 0,
            KeyCode::Char('e') if ctrl => self.cursor = self.text.len(),
            KeyCode::Char(c) if !ctrl && !key.modifiers.contains(KeyModifiers::ALT) => self.insert(c),
            KeyCode::Backspace => self.backspace(),
            KeyCode::Delete => self.delete(),
            KeyCode::Left => self.left(),
            KeyCode::Right => self.right(),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.text.len(),
            _ => return false,
        }
        true
    }
}

#[derive(Default)]
pub struct ChatLog {
    /// Oldest first.
    pub msgs: Vec<Message>,
    pub loaded: bool,
    pub loading: bool,
    /// No older page left on the server.
    pub exhausted: bool,
    /// Showing a stretch around a search hit, not the newest messages.
    pub detached: bool,
}

/// Screen areas of the last frame, for mouse hit-testing.
#[derive(Default, Clone, Copy)]
pub struct Hit {
    pub sidebar_list: Rect,
    pub messages: Rect,
    pub input: Rect,
    /// The conversation's scrollbar column.
    pub msg_bar: Rect,
    /// The header button that collapses / expands the chat list.
    pub toggle_btn: Rect,
    /// The "+" button beside the message field.
    pub attach_btn: Rect,
    /// "Info" and "Search" buttons in the conversation header.
    pub info_btn: Rect,
    pub search_btn: Rect,
}

pub struct App {
    pub(crate) http: Arc<HttpClient>,
    pub(crate) rt: Handle,
    pub(crate) tx: flume::Sender<Event>,

    pub quit: bool,
    pub connected: bool,
    pub chats: Vec<Chat>,
    pub chats_loaded: bool,
    pub notice: Option<(String, Instant)>,

    pub focus: Focus,
    pub mode: Mode,
    pub filter: LineEdit,
    /// Highlighted chat in the sidebar (by id, so it survives re-sorting).
    pub cursor: Option<String>,
    pub side_offset: usize,
    pub side_follow: bool,
    /// The chat shown on the right.
    pub open: Option<String>,
    pub logs: HashMap<String, ChatLog>,
    /// Lines scrolled up from the newest message; 0 sticks to the bottom.
    pub scroll: usize,
    /// Eases `scroll` toward a target (wheel, keys, jumps).
    pub smooth: Smooth,
    /// A scrollbar thumb being dragged with the mouse.
    pub drag: Option<Drag>,
    /// Lines one wheel event scrolls (`--scroll N`).
    pub wheel_step: isize,
    // Set by the last draw:
    pub max_scroll: usize,
    pub at_top: bool,
    pub page: usize,
    pub hit: Hit,

    pub input: LineEdit,
    pub typing: HashMap<String, HashMap<String, (String, bool, Instant)>>,

    /// Terminal graphics; `None` when pictures are off.
    pub picker: Option<Picker>,
    pub media: ImageCache,
    pub avatars: ImageCache,
    /// Pictures the last draw wants (set by the renderer, fetched in `after_draw`).
    pub want: Vec<(Kind, String)>,
    /// Thumbnails fully on screen in the last draw: area and message id.
    pub media_hits: Vec<(Rect, String)>,
    pub viewer: Option<Viewer>,
    /// Sixel and iTerm2 pictures cannot be covered by text, so they hide while an overlay is open.
    pub hide_images: bool,
    /// The chat list is folded into a narrow strip of profile photos.
    pub sidebar_collapsed: bool,
    /// The window is too narrow for list and conversation side by side.
    pub narrow: bool,
    /// Bumped whenever something that changes the conversation's lines happens;
    /// the lines are only rebuilt then, not on every animation frame.
    pub lines_gen: u64,
    pub lines_cache: Option<crate::ui::LinesCache>,
    /// Picture encodings allowed in the current frame (they are slow; the rest wait a frame).
    pub build_budget: u8,
    pub pending_builds: bool,

    pub menu: Option<Menu>,
    /// The file chooser of the attach menu.
    pub browser: Option<Browser>,
    pub stickers: Option<Stickers>,
    pub form: Option<Form>,
    pub sticker_imgs: ImageCache,
    /// The folder last browsed, so the next attach starts there.
    pub last_dir: Option<std::path::PathBuf>,
    /// The bubble picked with J/K or by a right-click.
    pub sel_msg: Option<String>,
    /// Scroll this message into view on the next draw (id, centered).
    pub reveal: Option<(String, bool)>,
    /// Bubbles on screen in the last draw: area and message id.
    pub bubble_hits: Vec<(Rect, String)>,
    pub reply: Option<Reply>,
    pub editing: Option<String>,
    /// Active in-chat pins by chat id.
    pub pins: HashMap<String, HashSet<String>>,
    pub panel: Option<Panel>,
    pub attach: Option<Attach>,
    pending_g: bool,
    pub(crate) temp_seq: u64,
}

impl App {
    pub fn new(http: Arc<HttpClient>, rt: Handle, tx: flume::Sender<Event>, picker: Option<Picker>) -> Self {
        Self {
            http,
            rt,
            tx,
            quit: false,
            connected: false,
            chats: Vec::new(),
            chats_loaded: false,
            notice: None,
            focus: Focus::Chats,
            mode: Mode::Normal,
            filter: LineEdit::default(),
            cursor: None,
            side_offset: 0,
            side_follow: true,
            open: None,
            logs: HashMap::new(),
            scroll: 0,
            smooth: Smooth::new(),
            drag: None,
            wheel_step: 2,
            max_scroll: 0,
            at_top: false,
            page: 10,
            hit: Hit::default(),
            input: LineEdit::default(),
            typing: HashMap::new(),
            picker,
            media: ImageCache::new(20, 1280),
            avatars: ImageCache::new(400, 96),
            want: Vec::new(),
            media_hits: Vec::new(),
            viewer: None,
            hide_images: false,
            sidebar_collapsed: false,
            narrow: false,
            lines_gen: 0,
            lines_cache: None,
            build_budget: 0,
            pending_builds: false,
            menu: None,
            browser: None,
            stickers: None,
            form: None,
            sticker_imgs: ImageCache::new(160, 256),
            last_dir: None,
            sel_msg: None,
            reveal: None,
            bubble_hits: Vec::new(),
            reply: None,
            editing: None,
            pins: HashMap::new(),
            panel: None,
            attach: None,
            pending_g: false,
            temp_seq: 0,
        }
    }

    // ---- derived views -------------------------------------------------

    /// Indices into `chats` in sidebar order: pinned first, then newest first;
    /// archived chats and chats not matching the filter are left out.
    pub fn visible(&self) -> Vec<usize> {
        let needle = self.filter.text.to_lowercase();
        let mut idx: Vec<usize> = self
            .chats
            .iter()
            .enumerate()
            .filter(|(_, c)| !c.archived && (needle.is_empty() || c.name.to_lowercase().contains(&needle)))
            .map(|(i, _)| i)
            .collect();
        idx.sort_by(|&a, &b| {
            let (a, b) = (&self.chats[a], &self.chats[b]);
            b.pinned_at
                .is_some()
                .cmp(&a.pinned_at.is_some())
                .then(b.pinned_at.cmp(&a.pinned_at))
                .then(b.last_time.cmp(&a.last_time))
        });
        idx
    }

    /// Every chat that is not archived, in sidebar order, ignoring the filter.
    pub fn visible_all(&self) -> Vec<&Chat> {
        let mut v: Vec<&Chat> = self.chats.iter().filter(|c| !c.archived).collect();
        v.sort_by(|a, b| {
            b.pinned_at.is_some().cmp(&a.pinned_at.is_some()).then(b.last_time.cmp(&a.last_time))
        });
        v
    }

    /// Position of the highlighted chat within `visible`.
    pub fn cursor_pos(&self, visible: &[usize]) -> usize {
        self.cursor
            .as_ref()
            .and_then(|id| visible.iter().position(|&i| self.chats[i].id == *id))
            .unwrap_or(0)
    }

    pub fn chat(&self, id: &str) -> Option<&Chat> {
        self.chats.iter().find(|c| c.id == id)
    }

    pub fn typing_line(&self, chat_id: &str) -> Option<String> {
        let now = Instant::now();
        let entries = self.typing.get(chat_id)?;
        let mut names: Vec<&str> = Vec::new();
        let mut audio = false;
        for (name, is_audio, at) in entries.values() {
            if now.duration_since(*at) < TYPING_TTL {
                names.push(name.as_str());
                audio |= *is_audio;
            }
        }
        if names.is_empty() {
            return None;
        }
        names.sort_unstable();
        let verb = if audio { "recording audio…" } else { "typing…" };
        let is_group = self.chat(chat_id).is_some_and(|c| c.is_group);
        Some(if is_group && !names[0].is_empty() {
            format!("{} {verb}", names.join(", "))
        } else {
            verb.to_string()
        })
    }

    pub(crate) fn notice(&mut self, msg: impl Into<String>) {
        self.notice = Some((msg.into(), Instant::now()));
    }

    pub fn live_notice(&self) -> Option<&str> {
        self.notice
            .as_ref()
            .filter(|(_, at)| at.elapsed() < NOTICE_TTL)
            .map(|(m, _)| m.as_str())
    }

    // ---- backend calls ---------------------------------------------------

    pub fn reload_chats(&self) {
        let (http, tx) = (self.http.clone(), self.tx.clone());
        self.rt.spawn(async move {
            let result = http.get_chats().await.map_err(|e| e.to_string());
            let _ = tx.send(Event::Chats(result));
        });
    }

    pub(crate) fn load_messages(&mut self, chat_id: &str, before: Option<i64>) {
        let log = self.logs.entry(chat_id.to_string()).or_default();
        if log.loading {
            return;
        }
        log.loading = true;
        let (http, tx, id) = (self.http.clone(), self.tx.clone(), chat_id.to_string());
        self.rt.spawn(async move {
            let result = http
                .get_messages(&id, Some(PAGE), before, None)
                .await
                .map_err(|e| e.to_string());
            let _ = tx.send(Event::Messages {
                chat_id: id,
                older: before.is_some(),
                result,
            });
        });
    }

    pub fn open_chat(&mut self, id: &str) {
        if self.open.as_deref() != Some(id) {
            self.sel_msg = None;
            self.reply = None;
            if self.editing.take().is_some() {
                self.input.clear();
            }
            if self.panel.is_some() {
                self.close_panel();
            }
            self.load_pins(id);
        }
        self.open = Some(id.to_string());
        self.cursor = Some(id.to_string());
        self.focus = Focus::Messages;
        self.scroll = 0;
        self.smooth.stop();
        self.max_scroll = 0;
        self.at_top = false;
        if !self.logs.get(id).is_some_and(|l| l.loaded) {
            self.load_messages(id, None);
        }
        // Typing indicators only come for chats we subscribed to.
        let (http, chat) = (self.http.clone(), id.to_string());
        self.rt.spawn(async move {
            let _ = http.subscribe_chat_presence(&chat).await;
        });
    }

    /// Called after every draw: fetch the next older page when the view reached the top.
    pub fn after_draw(&mut self) {
        for (kind, url) in std::mem::take(&mut self.want) {
            self.fetch_image(kind, &url);
        }
        self.panel_after_draw();
        let Some(id) = self.open.clone() else { return };
        if !self.at_top {
            return;
        }
        let Some(log) = self.logs.get(&id) else { return };
        if !log.loaded || log.loading || log.exhausted {
            return;
        }
        if let Some(first) = log.msgs.first() {
            let before = first.timestamp;
            self.load_messages(&id, Some(before));
        }
    }

    fn fetch_image(&mut self, kind: Kind, url: &str) {
        if self.picker.is_none() {
            return;
        }
        let cache = match kind {
            Kind::Media => &mut self.media,
            Kind::Avatar => &mut self.avatars,
            Kind::Sticker => &mut self.sticker_imgs,
        };
        let max = cache.max_dim;
        if !cache.begin(url) {
            return;
        }
        let (http, tx, url) = (self.http.clone(), self.tx.clone(), url.to_string());
        self.rt.spawn(async move {
            let result = load_image(&http, &url, max).await;
            let _ = tx.send(Event::Image { kind, url, result });
        });
    }

    /// The pictures of a chat, oldest first.
    pub fn image_msgs(&self, chat_id: &str) -> Vec<&Message> {
        self.logs
            .get(chat_id)
            .map(|l| l.msgs.iter().filter(|m| images::media_url(m).is_some()).collect())
            .unwrap_or_default()
    }

    pub(crate) fn open_viewer(&mut self, msg_id: &str) {
        if self.picker.is_none() {
            self.notice("Pictures are off (start without --images off)");
            return;
        }
        if let Some(chat_id) = self.open.clone() {
            self.viewer = Some(Viewer { chat_id, msg_id: msg_id.to_string() });
        }
    }

    /// `o`: open the lowest picture that is fully on screen.
    fn open_viewer_here(&mut self) {
        match self.media_hits.last().map(|(_, id)| id.clone()) {
            Some(id) => self.open_viewer(&id),
            None => self.notice("No picture in view"),
        }
    }

    fn viewer_step(&mut self, delta: isize, edge: Option<bool>) {
        let Some(v) = self.viewer.clone() else { return };
        let ids: Vec<String> = self.image_msgs(&v.chat_id).iter().map(|m| m.id.clone()).collect();
        if ids.is_empty() {
            self.viewer = None;
            return;
        }
        let pos = ids.iter().position(|i| *i == v.msg_id).unwrap_or(0) as isize;
        let next = match edge {
            Some(false) => 0,
            Some(true) => ids.len() as isize - 1,
            None => (pos + delta).clamp(0, ids.len() as isize - 1),
        };
        self.viewer = Some(Viewer { msg_id: ids[next as usize].clone(), ..v });
    }

    fn key_viewer(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('c') if ctrl => self.quit = true,
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('o') | KeyCode::Enter => self.viewer = None,
            KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('k') | KeyCode::Up => self.viewer_step(-1, None),
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('j') | KeyCode::Down => self.viewer_step(1, None),
            KeyCode::Home | KeyCode::Char('g') => self.viewer_step(0, Some(false)),
            KeyCode::End | KeyCode::Char('G') => self.viewer_step(0, Some(true)),
            _ => {}
        }
    }

    fn mark_read(&mut self) {
        let target = match self.focus {
            Focus::Chats => self.cursor.clone().or_else(|| self.open.clone()),
            Focus::Messages | Focus::Panel => self.open.clone(),
        };
        if let Some(chat_id) = target {
            self.mark_read_chat(chat_id);
        }
    }

    // ---- events ----------------------------------------------------------

    pub fn handle(&mut self, ev: Event) {
        // Scrolling and pointer movement do not change what the lines say.
        let calm = matches!(
            &ev,
            Event::Tick
                | Event::Input(CtEvent::Mouse(MouseEvent {
                    kind: MouseEventKind::Moved
                        | MouseEventKind::Drag(_)
                        | MouseEventKind::ScrollUp
                        | MouseEventKind::ScrollDown,
                    ..
                }))
        );
        if !calm {
            self.lines_gen = self.lines_gen.wrapping_add(1);
        }
        match ev {
            Event::Input(CtEvent::Key(key)) if key.kind != KeyEventKind::Release => self.on_key(key),
            Event::Input(CtEvent::Mouse(m)) => self.on_mouse(m),
            Event::Input(CtEvent::Paste(text)) => self.on_paste_text(text),
            Event::Input(_) => {}
            Event::Tick => {
                let now = Instant::now();
                for entries in self.typing.values_mut() {
                    entries.retain(|_, (_, _, at)| now.duration_since(*at) < TYPING_TTL);
                }
                self.typing.retain(|_, e| !e.is_empty());
            }
            Event::Chats(Ok(chats)) => {
                self.chats = chats;
                self.chats_loaded = true;
            }
            Event::Chats(Err(e)) => self.notice(format!("Could not load chats: {e}")),
            Event::Messages { chat_id, older, result } => self.on_messages(&chat_id, older, result),
            Event::Sent { chat_id, temp_id, result } => self.on_sent(&chat_id, &temp_id, result),
            Event::MarkedRead { chat_id, result } => match result {
                Ok(()) => {
                    if let Some(c) = self.chats.iter_mut().find(|c| c.id == chat_id) {
                        c.unread = 0;
                        c.has_unread_mention = false;
                    }
                }
                Err(e) => self.notice(format!("Could not mark as read: {e}")),
            },
            Event::Ws(ev) => self.on_ws(ev),
            Event::Done(done) => self.on_done(done),
            Event::ChatState(Ok(state)) => self.apply_chat_state(state),
            Event::ChatState(Err(e)) => self.notice(e),
            Event::Pins { chat_id, result } => self.on_pins(chat_id, result),
            Event::Receipts { msg_id, result } => self.on_receipts(&msg_id, result),
            Event::Group { chat_id, result } => self.on_group(&chat_id, result),
            Event::PanelPage { chat_id, tab, result } => self.on_panel_page(&chat_id, tab, result),
            Event::Search { chat_id, query, result } => self.on_search(&chat_id, &query, result),
            Event::Context { chat_id, msg_id, result } => self.on_context(&chat_id, &msg_id, result),
            Event::Pasted(result) => self.on_pasted(result),
            Event::Stickers(result) => self.on_stickers(result),
            Event::Image { kind, url, result } => {
                let ready = match (&self.picker, result) {
                    (Some(_), Ok(img)) => Some(Ready::new(img)),
                    _ => None,
                };
                match kind {
                    Kind::Media => self.media.finish(&url, ready),
                    Kind::Sticker => self.sticker_imgs.finish(&url, ready),
                    Kind::Avatar => self.avatars.finish(&url, ready),
                }
            }
        }
    }

    fn on_messages(&mut self, chat_id: &str, older: bool, result: Result<Vec<Message>, String>) {
        let log = self.logs.entry(chat_id.to_string()).or_default();
        log.loading = false;
        match result {
            Ok(page) => {
                if !older && log.detached {
                    log.msgs.clear();
                    log.exhausted = false;
                    log.detached = false;
                }
                if page.len() < PAGE {
                    log.exhausted = true;
                }
                let before = log.msgs.len();
                for m in page {
                    upsert(&mut log.msgs, m);
                }
                if older && log.msgs.len() == before {
                    log.exhausted = true;
                }
                log.loaded = true;
            }
            Err(e) => {
                if older {
                    // Do not hammer a failing endpoint on every redraw.
                    log.exhausted = true;
                }
                self.notice(format!("Could not load messages: {e}"));
            }
        }
    }

    pub(crate) fn apply_chat_state(&mut self, state: ChatState) {
        if let Some(c) = self.chats.iter_mut().find(|c| c.id == state.chat_id) {
            c.archived = state.archived;
            c.pinned_at = state.pinned_at;
            c.mute_mode = state.mute_mode;
            c.muted_until = state.muted_until;
        }
    }

    fn on_sent(&mut self, chat_id: &str, temp_id: &str, result: Result<String, String>) {
        let Some(log) = self.logs.get_mut(chat_id) else { return };
        match result {
            Ok(real_id) if real_id.is_empty() => {
                // A reply answers without an id; the echo brings the real message.
                if let Some(m) = log.msgs.iter_mut().find(|m| m.id == temp_id) {
                    m.status = "sent".into();
                }
            }
            Ok(real_id) => {
                // The WebSocket echo may already have replaced the temp message.
                if log.msgs.iter().any(|m| m.id == real_id) {
                    log.msgs.retain(|m| m.id != temp_id);
                } else if let Some(m) = log.msgs.iter_mut().find(|m| m.id == temp_id) {
                    m.id = real_id;
                    m.status = "sent".into();
                }
            }
            Err(e) => {
                if let Some(m) = log.msgs.iter_mut().find(|m| m.id == temp_id) {
                    m.status = "failed".into();
                }
                self.notice(format!("Send failed: {e}"));
            }
        }
    }

    /// Update the sidebar row for a message in `chat_id`.
    pub(crate) fn apply_to_chat_row(&mut self, msg: &Message, unread_here: bool) {
        let mentions_me = msg.extra.as_ref().is_some_and(|e| e.mentions_me);
        let preview = preview(msg);
        let from_me = is_from_me(msg);
        match self.chats.iter_mut().find(|c| c.id == msg.chat_id) {
            Some(c) => {
                c.last_msg = preview;
                c.last_time = msg.timestamp;
                c.last_message_id = msg.id.clone();
                c.last_message_status = msg.status.clone();
                c.last_message_from_me = from_me;
                if unread_here {
                    c.unread += 1;
                    c.has_unread_mention |= mentions_me;
                }
            }
            None => {
                // A chat we have not listed yet: ask the backend instead of guessing its fields.
                self.reload_chats();
            }
        }
    }

    fn on_ws(&mut self, ev: WsEvent) {
        match ev {
            WsEvent::Connected => {
                let reconnect = self.connected_once();
                self.connected = true;
                if reconnect {
                    self.reload_chats();
                    if let Some(id) = self.open.clone() {
                        // Catch up on whatever arrived while the socket was down.
                        if let Some(log) = self.logs.get_mut(&id) {
                            log.loading = false;
                        }
                        self.load_messages(&id, None);
                    }
                }
            }
            WsEvent::Disconnected => self.connected = false,
            WsEvent::NewMessage(msg) => {
                let msg = *msg;
                let incoming = msg.status == "received";
                let viewing = self.open.as_deref() == Some(msg.chat_id.as_str());
                self.apply_to_chat_row(&msg, incoming && !viewing);
                if let Some(log) = self.logs.get_mut(&msg.chat_id) {
                    if is_from_me(&msg) {
                        // The echo of what we just sent replaces the pending copy.
                        if let Some(pos) = log
                            .msgs
                            .iter()
                            .position(|m| m.id.starts_with("temp-") && m.content == msg.content)
                        {
                            log.msgs.remove(pos);
                        }
                    }
                    upsert(&mut log.msgs, msg);
                }
            }
            WsEvent::MessageStatus { chat_id, id, status } => {
                for (cid, log) in self.logs.iter_mut() {
                    if chat_id.as_deref().is_some_and(|c| c != cid) {
                        continue;
                    }
                    if let Some(m) = log.msgs.iter_mut().find(|m| m.id == id) {
                        m.status = status.clone();
                    }
                }
            }
            WsEvent::MessageDeleted { chat_id, id } => {
                if let Some(log) = self.logs.get_mut(&chat_id) {
                    log.msgs.retain(|m| m.id != id);
                }
            }
            WsEvent::MessageRevoked { chat_id, id, revoked_at } => {
                if let Some(m) = self.find_msg(&chat_id, &id) {
                    m.revoked_at = Some(revoked_at);
                }
            }
            WsEvent::MessageEdited { chat_id, id, content, extra } => {
                if let Some(m) = self.find_msg(&chat_id, &id) {
                    m.content = content;
                    if let Some(extra) = extra {
                        m.extra = Some(*extra);
                    }
                }
            }
            WsEvent::MessageReaction { chat_id, id, reactions } => {
                if let Some(m) = self.find_msg(&chat_id, &id) {
                    m.reactions = Some(reactions);
                }
            }
            WsEvent::MessageStarred { chat_id, id, starred } => {
                if let Some(m) = self.find_msg(&chat_id, &id) {
                    m.starred = starred;
                }
            }
            WsEvent::ChatState(state) => {
                if let Some(c) = self.chats.iter_mut().find(|c| c.id == state.chat_id) {
                    c.archived = state.archived;
                    c.pinned_at = state.pinned_at;
                    c.mute_mode = state.mute_mode;
                    c.muted_until = state.muted_until;
                }
            }
            WsEvent::ChatNameUpdate { chat_id, name } => {
                if let Some(c) = self.chats.iter_mut().find(|c| c.id == chat_id) {
                    c.name = name;
                }
            }
            WsEvent::ChatUpdate { chat_id, unread, last_msg, disappearing_secs } => {
                if let Some(c) = self.chats.iter_mut().find(|c| c.id == chat_id) {
                    if let Some(u) = unread {
                        c.unread = u;
                        if u == 0 {
                            c.has_unread_mention = false;
                        }
                    }
                    if let Some(l) = last_msg {
                        c.last_msg = l;
                    }
                    if let Some(d) = disappearing_secs {
                        c.disappearing_secs = d;
                    }
                }
            }
            WsEvent::ChatsChanged { .. } => self.reload_chats(),
            WsEvent::ChatPinsChanged { chat_id } => self.load_pins(&chat_id),
            WsEvent::Typing { chat_id, sender, sender_name, composing, audio } => {
                let entries = self.typing.entry(chat_id).or_default();
                if composing {
                    let name = sender_name.unwrap_or_default();
                    entries.insert(sender, (name, audio, Instant::now()));
                } else {
                    entries.remove(&sender);
                }
            }
            _ => {}
        }
    }

    fn connected_once(&self) -> bool {
        // True from the second connect on: the first one is the startup load.
        self.chats_loaded
    }

    fn find_msg(&mut self, chat_id: &str, id: &str) -> Option<&mut Message> {
        self.logs.get_mut(chat_id)?.msgs.iter_mut().find(|m| m.id == id)
    }

    // ---- input -----------------------------------------------------------

    fn on_key(&mut self, key: KeyEvent) {
        if self.menu.is_some() {
            return self.key_menu(key);
        }
        if self.browser.is_some() {
            return self.key_browser(key);
        }
        if self.stickers.is_some() {
            return self.key_stickers(key);
        }
        if self.form.is_some() {
            return self.key_form(key);
        }
        if self.viewer.is_some() {
            return self.key_viewer(key);
        }
        match self.mode {
            Mode::Insert => self.key_insert(key),
            Mode::Filter => self.key_filter(key),
            Mode::Search => self.key_search(key),
            Mode::Caption => self.key_caption(key),
            Mode::Normal if self.focus == Focus::Panel => self.key_panel(key),
            Mode::Normal => self.key_normal(key),
        }
    }

    fn key_insert(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Char('c') if ctrl => self.mode = Mode::Normal,
            KeyCode::Enter => self.send(),
            KeyCode::Char('v') if ctrl => self.paste_clipboard(),
            _ => {
                self.input.key(key);
            }
        }
    }

    fn key_filter(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.filter.clear();
                self.mode = Mode::Normal;
            }
            KeyCode::Enter => self.mode = Mode::Normal,
            _ => {
                if self.filter.key(key) {
                    // The list changed: start from its top again.
                    self.cursor = self.visible().first().map(|&i| self.chats[i].id.clone());
                    self.side_offset = 0;
                    self.side_follow = true;
                }
            }
        }
    }

    fn key_normal(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let was_g = std::mem::take(&mut self.pending_g);
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('c') if ctrl => self.quit = true,
            KeyCode::Tab => {
                self.focus = match (self.focus, &self.open, self.panel.is_some()) {
                    (Focus::Chats, Some(_), _) => Focus::Messages,
                    (Focus::Messages, _, true) => Focus::Panel,
                    // With the list folded away, Tab must not land on it.
                    _ if self.sidebar_collapsed && self.open.is_some() => Focus::Messages,
                    _ => Focus::Chats,
                }
            }
            KeyCode::Char('h') | KeyCode::Left => {
                // "Left" from the conversation brings the list back.
                self.sidebar_collapsed = false;
                self.focus = Focus::Chats;
            }
            KeyCode::Char('l') | KeyCode::Right => {
                if self.focus == Focus::Chats {
                    self.open_cursor();
                } else {
                    self.focus = Focus::Messages;
                }
            }
            KeyCode::Char('/') => {
                // Filtering needs the full list.
                self.sidebar_collapsed = false;
                self.focus = Focus::Chats;
                self.mode = Mode::Filter;
            }
            KeyCode::Char('i') | KeyCode::Char('a') => {
                if self.open.is_some() {
                    self.focus = Focus::Messages;
                    self.mode = Mode::Insert;
                } else {
                    self.notice("Open a chat first (Enter)");
                }
            }
            KeyCode::Char('m') => self.mark_read(),
            KeyCode::Char('o') => self.open_viewer_here(),
            KeyCode::Char('r') => {
                self.media.clear_failed();
                self.avatars.clear_failed();
                self.sticker_imgs.clear_failed();
                self.reload_chats();
                if let Some(id) = self.open.clone() {
                    if let Some(log) = self.logs.get_mut(&id) {
                        log.loading = false;
                    }
                    self.load_messages(&id, None);
                }
            }
            KeyCode::Esc => {
                if self.sel_msg.take().is_some() || self.cancel_compose() {
                    // Dropped the highlight or a half-made reply.
                } else if !self.filter.text.is_empty() {
                    self.filter.clear();
                }
            }
            KeyCode::Char('.') => self.open_menu_here(),
            KeyCode::Char('B') => self.toggle_sidebar(),
            KeyCode::Char('+') | KeyCode::Char('A') => self.open_attach_menu(),
            KeyCode::Char('J') if self.focus == Focus::Messages => self.select_msg(1),
            KeyCode::Char('K') if self.focus == Focus::Messages => self.select_msg(-1),
            KeyCode::Char('f') => self.open_search(),
            KeyCode::Char('I') => match self.open.clone() {
                Some(id) => self.open_info(&id),
                None => self.notice("Open a chat first"),
            },
            KeyCode::Char('v') if ctrl => self.paste_clipboard(),
            KeyCode::Enter => match self.focus {
                Focus::Chats => self.open_cursor(),
                Focus::Panel => {}
                Focus::Messages => {
                    if self.open.is_some() {
                        self.mode = Mode::Insert;
                    }
                }
            },
            KeyCode::Char('j') | KeyCode::Down => self.step(1),
            KeyCode::Char('k') | KeyCode::Up => self.step(-1),
            KeyCode::Char('d') if ctrl => self.step(self.page as isize / 2 + 1),
            KeyCode::Char('u') if ctrl => self.step(-(self.page as isize / 2 + 1)),
            KeyCode::PageDown => self.step(self.page as isize),
            KeyCode::PageUp => self.step(-(self.page as isize)),
            KeyCode::Char('G') | KeyCode::End => self.jump(true),
            KeyCode::Char('g') => {
                if was_g {
                    self.jump(false);
                } else {
                    self.pending_g = true;
                }
            }
            KeyCode::Home => self.jump(false),
            _ => {}
        }
    }

    fn open_cursor(&mut self) {
        let visible = self.visible();
        let Some(&i) = visible.get(self.cursor_pos(&visible)) else { return };
        let id = self.chats[i].id.clone();
        self.open_chat(&id);
    }

    /// Move down (`n` > 0) or up in whichever pane has focus. In the message
    /// pane "down" means towards newer messages.
    fn step(&mut self, n: isize) {
        match self.focus {
            Focus::Chats => {
                let visible = self.visible();
                if visible.is_empty() {
                    return;
                }
                let pos = self.cursor_pos(&visible) as isize;
                let next = (pos + n).clamp(0, visible.len() as isize - 1) as usize;
                self.cursor = Some(self.chats[visible[next]].id.clone());
                self.side_follow = true;
            }
            Focus::Messages | Focus::Panel => {
                self.scroll_by(-n);
            }
        }
    }

    /// `gg` (top) and `G` (bottom).
    fn jump(&mut self, bottom: bool) {
        match self.focus {
            Focus::Chats => {
                let visible = self.visible();
                let pick = if bottom { visible.last() } else { visible.first() };
                if let Some(&i) = pick {
                    self.cursor = Some(self.chats[i].id.clone());
                    self.side_follow = true;
                }
            }
            Focus::Messages | Focus::Panel => {
                self.scroll_to(if bottom { 0 } else { self.max_scroll });
                // Back from a search hit: fetch the newest messages again.
                if bottom {
                    if let Some(id) = self.open.clone().filter(|id| self.logs.get(id).is_some_and(|l| l.detached)) {
                        self.load_messages(&id, None);
                    }
                }
            }
        }
    }

    fn on_mouse(&mut self, m: MouseEvent) {
        let at = |r: Rect| {
            m.column >= r.x && m.column < r.x + r.width && m.row >= r.y && m.row < r.y + r.height
        };
        if self.attach.is_some() {
            self.mouse_attach(m);
            return;
        }
        if self.menu.is_some() && self.mouse_menu(m) {
            return;
        }
        if self.browser.is_some() {
            self.mouse_browser(m);
            return;
        }
        if self.stickers.is_some() {
            self.mouse_stickers(m);
            return;
        }
        if self.form.is_some() {
            self.mouse_form(m);
            return;
        }
        if self.viewer.is_some() {
            match m.kind {
                MouseEventKind::Down(MouseButton::Left) => self.viewer = None,
                MouseEventKind::ScrollUp => self.viewer_step(-1, None),
                MouseEventKind::ScrollDown => self.viewer_step(1, None),
                _ => {}
            }
            return;
        }
        // A scrollbar thumb being dragged follows the pointer wherever it goes.
        if let Some(d) = self.drag {
            match m.kind {
                MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Moved => {
                    self.drag_to(d, m.row);
                    return;
                }
                MouseEventKind::Up(_) => {
                    self.drag = None;
                    return;
                }
                _ => {}
            }
        }
        if m.kind == MouseEventKind::Down(MouseButton::Left) {
            if let Some(d) = self.bar_at(m.column, m.row) {
                self.drag = Some(d);
                self.drag_to(d, m.row);
                return;
            }
        }
        if self.mouse_panel(m) {
            return;
        }
        match m.kind {
            MouseEventKind::Down(MouseButton::Right) => {
                if at(self.hit.sidebar_list) {
                    let row = (m.row - self.hit.sidebar_list.y) as usize / 2 + self.side_offset;
                    let visible = self.visible();
                    if let Some(&i) = visible.get(row) {
                        let id = self.chats[i].id.clone();
                        self.cursor = Some(id.clone());
                        self.open_chat_menu((m.column, m.row), &id);
                    }
                } else if let Some(id) = self.bubble_hits.iter().find(|(r, _)| at(*r)).map(|(_, id)| id.clone()) {
                    self.focus = Focus::Messages;
                    self.open_message_menu((m.column, m.row), &id);
                }
            }
            MouseEventKind::Down(MouseButton::Left) if at(self.hit.toggle_btn) && self.open.is_some() => self.toggle_sidebar(),
            MouseEventKind::Down(MouseButton::Left) if at(self.hit.attach_btn) && self.open.is_some() => self.open_attach_menu(),
            MouseEventKind::Down(MouseButton::Left) if at(self.hit.info_btn) && self.open.is_some() => {
                if let Some(id) = self.open.clone() {
                    self.open_info(&id);
                }
            }
            MouseEventKind::Down(MouseButton::Left) if at(self.hit.search_btn) && self.open.is_some() => self.open_search(),
            MouseEventKind::Down(MouseButton::Left) => {
                if at(self.hit.sidebar_list) {
                    self.focus = Focus::Chats;
                    let row = (m.row - self.hit.sidebar_list.y) as usize / 2 + self.side_offset;
                    let visible = self.visible();
                    if let Some(&i) = visible.get(row) {
                        let id = self.chats[i].id.clone();
                        self.open_chat(&id);
                        self.mode = Mode::Normal;
                    }
                } else if let Some(id) = self.media_hits.iter().find(|(r, _)| at(*r)).map(|(_, id)| id.clone()) {
                    self.focus = Focus::Messages;
                    self.open_viewer(&id);
                } else if at(self.hit.input) && self.open.is_some() {
                    self.focus = Focus::Messages;
                    self.mode = Mode::Insert;
                } else if at(self.hit.messages) {
                    self.focus = Focus::Messages;
                }
            }
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let up = m.kind == MouseEventKind::ScrollUp;
                if at(self.hit.sidebar_list) {
                    self.side_follow = false;
                    let total = self.visible().len();
                    self.side_offset = if up {
                        self.side_offset.saturating_sub(1)
                    } else {
                        (self.side_offset + 1).min(total.saturating_sub(1))
                    };
                } else if at(self.hit.messages) || at(self.hit.input) {
                    self.wheel(if up { self.wheel_step } else { -self.wheel_step });
                }
            }
            _ => {}
        }
    }
}

/// How far (in lines) a wheel-driven scroll may run ahead of the view.
pub const MAX_LEAD: isize = 8;

/// Which scrollbar is being dragged.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Drag {
    Messages,
    Panel,
}

impl App {
    /// Collapse or expand the chat list. In the one-pane layout there is nothing to
    /// fold, so the same button goes back to the list.
    pub fn toggle_sidebar(&mut self) {
        if self.narrow {
            self.focus = Focus::Chats;
        } else {
            self.sidebar_collapsed = !self.sidebar_collapsed;
            // The list is gone: keyboard focus has to be somewhere that is still on screen.
            if self.sidebar_collapsed && self.focus == Focus::Chats && self.open.is_some() {
                self.focus = Focus::Messages;
            }
        }
    }

    /// Scroll the conversation by `delta` lines (positive = towards older), gliding there.
    pub fn scroll_by(&mut self, delta: isize) {
        let base = self.smooth.goal().unwrap_or(self.scroll) as isize;
        let goal = (base + delta).clamp(0, self.max_scroll as isize) as usize;
        self.scroll_to(goal);
    }

    /// One wheel event. Terminals and touchpads send several per notch, so the target
    /// may only run a little ahead of what is on screen; a flood of events then makes
    /// the view move faster, but it cannot fly across the whole conversation.
    pub fn wheel(&mut self, delta: isize) {
        let base = self.smooth.goal().unwrap_or(self.scroll) as isize;
        let lead = self.scroll as isize;
        let goal = (base + delta).clamp(lead - MAX_LEAD, lead + MAX_LEAD).clamp(0, self.max_scroll as isize);
        self.scroll_to(goal as usize);
    }

    pub fn scroll_to(&mut self, goal: usize) {
        self.smooth.aim(self.scroll, goal.min(self.max_scroll));
    }

    /// True while something is gliding, so the main loop keeps frames coming.
    pub fn animating(&self) -> bool {
        self.smooth.active() || self.pending_builds || self.panel.as_ref().is_some_and(|p| p.smooth.active())
    }

    /// One animation step; called every loop turn.
    pub fn animate(&mut self) {
        let now = Instant::now();
        if let Some(v) = self.smooth.step(now) {
            self.scroll = v.min(self.max_scroll);
        }
        if let Some(p) = self.panel.as_mut() {
            if let Some(v) = p.smooth.step(now) {
                p.scroll_y = v.min(p.max_scroll);
            }
        }
    }

    #[cfg(test)]
    /// Land every glide on its target at once (tests; also when a jump must be exact).
    pub fn finish_scroll(&mut self) {
        if let Some(v) = self.smooth.finish() {
            self.scroll = v.min(self.max_scroll);
        }
        if let Some(p) = self.panel.as_mut() {
            if let Some(v) = p.smooth.finish() {
                p.scroll_y = v.min(p.max_scroll);
            }
        }
    }

    /// The scrollbar under the pointer, if any. The column left of it counts too:
    /// the bar itself is one cell wide.
    fn bar_at(&self, col: u16, row: u16) -> Option<Drag> {
        let near = |r: Rect| {
            r.height > 0 && row >= r.y && row < r.y + r.height && col + 1 >= r.x && col <= r.x
        };
        if near(self.hit.msg_bar) && self.max_scroll > 0 {
            return Some(Drag::Messages);
        }
        match self.panel.as_ref() {
            Some(p) if p.max_scroll > 0 && near(p.bar) => Some(Drag::Panel),
            _ => None,
        }
    }

    /// Put the view where the pointer is along the bar (top of the bar = oldest).
    fn drag_to(&mut self, d: Drag, row: u16) {
        let place = |bar: Rect, max: usize| -> usize {
            if bar.height <= 1 || max == 0 {
                return 0;
            }
            let t = f32::from(row.saturating_sub(bar.y).min(bar.height - 1)) / f32::from(bar.height - 1);
            (t * max as f32).round() as usize
        };
        match d {
            Drag::Messages => {
                let from_top = place(self.hit.msg_bar, self.max_scroll);
                self.scroll = self.max_scroll - from_top.min(self.max_scroll);
                self.smooth.stop();
            }
            Drag::Panel => {
                if let Some(p) = self.panel.as_mut() {
                    p.scroll_y = place(p.bar, p.max_scroll).min(p.max_scroll);
                    p.follow = false;
                    p.smooth.stop();
                }
            }
        }
    }
}

// ---- message helpers (shared with the renderer) --------------------------

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

pub(crate) async fn load_image(http: &HttpClient, url: &str, max_dim: u32) -> Result<DynamicImage, String> {
    // Backend paths ("/media/x.jpg") live under /api; `media_url` adds it, and leaves
    // absolute URLs (profile photos) alone.
    let target = http.media_url(Some(url)).unwrap_or_else(|| url.to_string());
    let bytes = http.download_bytes(&target).await.map_err(|e| e.to_string())?;
    tokio::task::spawn_blocking(move || images::decode(&bytes, max_dim))
        .await
        .map_err(|e| e.to_string())?
}

pub fn is_from_me(m: &Message) -> bool {
    m.from == "me" || m.id.starts_with("temp-") || m.status == "pending"
}

/// Insert or replace by id, keeping the list oldest-first.
pub(crate) fn upsert(msgs: &mut Vec<Message>, msg: Message) {
    if let Some(slot) = msgs.iter_mut().find(|m| m.id == msg.id) {
        *slot = msg;
        return;
    }
    let pos = msgs.partition_point(|m| m.timestamp <= msg.timestamp);
    msgs.insert(pos, msg);
}

/// `[image]`-style tag for non-text messages.
pub fn kind_label(m: &Message) -> Option<String> {
    let kind = m.message_type.as_str();
    match kind {
        "" | "text" => None,
        "ptt" | "audio" => Some("[audio]".into()),
        "image" if m.extra.as_ref().and_then(|e| e.gif) == Some(true) => Some("[gif]".into()),
        other => Some(format!("[{other}]")),
    }
}

/// `body` with the media tag in front, unless the body already starts with it
/// (the backend sends "[Sticker]" as the text of a sticker).
pub fn with_tag(m: &Message, body: &str) -> String {
    let body = body.trim();
    match kind_label(m) {
        None => body.to_string(),
        Some(tag) if body.is_empty() => tag,
        Some(tag) if body.to_lowercase().starts_with(&tag.to_lowercase()) => body.to_string(),
        Some(tag) => format!("{tag} {body}"),
    }
}

/// One-line text for the sidebar and the notice.
pub fn preview(m: &Message) -> String {
    with_tag(m, &m.content)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(id: &str, ts: i64) -> Message {
        Message { id: id.into(), chat_id: "c".into(), timestamp: ts, ..Default::default() }
    }

    #[test]
    fn upsert_keeps_order_and_replaces() {
        let mut v = vec![msg("a", 1), msg("c", 3)];
        upsert(&mut v, msg("b", 2));
        assert_eq!(v.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);
        let mut again = msg("b", 2);
        again.content = "x".into();
        upsert(&mut v, again);
        assert_eq!(v.len(), 3);
        assert_eq!(v[1].content, "x");
    }

    #[test]
    fn line_edit_is_unicode_safe() {
        let mut e = LineEdit::default();
        for c in "héllo 👍".chars() {
            e.insert(c);
        }
        e.backspace();
        assert_eq!(e.text, "héllo ");
        e.left();
        e.left();
        e.delete();
        assert_eq!(e.text, "héll ");
        e.delete_word_back();
        assert_eq!(e.text, " ");
    }

    #[test]
    fn previews_tag_media() {
        let mut m = msg("a", 1);
        m.message_type = "image".into();
        assert_eq!(preview(&m), "[image]");
        m.content = "liat ini".into();
        assert_eq!(preview(&m), "[image] liat ini");
        m.message_type = "sticker".into();
        m.content = "[Sticker]".into();
        assert_eq!(preview(&m), "[Sticker]");
    }
}

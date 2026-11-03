//! What the user can *do*: run a menu action, reply, edit, paste a picture.
//! Everything that talks to the backend goes through `spawn_call`, so a failure
//! always ends as one notice on the status line.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui_image::protocol::StatefulProtocol;
use wabot_backend_client::{HttpClient, Message, PinnedMessage, SendMediaBuilder};

use crate::app::{is_from_me, now_ms, App, Event, Focus, LineEdit, Mode};
use crate::attach::Picked;
use crate::clip;
use crate::images;
use crate::menu::{self, Action, Menu};

/// The message being answered; shown as a bar above the input.
#[derive(Clone, Debug)]
pub struct Reply {
    pub id: String,
    pub who: String,
    pub text: String,
}

/// A picture waiting for a caption and Enter.
pub struct Attach {
    pub chat_id: String,
    pub picked: Picked,
    pub proto: Option<StatefulProtocol>,
    pub caption: LineEdit,
    pub sending: bool,
    /// Buttons of the last draw, for the mouse.
    pub send_btn: Rect,
    pub cancel_btn: Rect,
}

/// What to change locally once a call succeeded.
#[derive(Clone, Debug)]
pub enum Then {
    Nothing,
    RemoveMsg(String, String),
    RevokeMsg(String, String),
    Star(String, String, bool),
    ReloadPins(String),
    CloseAttach,
    CloseStickers,
    CloseForm,
}

pub struct Done {
    /// Shown on the status line when it worked ("" = nothing).
    pub label: String,
    pub result: Result<(), String>,
    pub then: Then,
}

/// What a paste produced.
pub enum Pasted {
    File(Picked),
    Text(String),
}

impl App {
    /// Run one backend call off the UI thread and report it as [`Event::Done`].
    pub(crate) fn spawn_call<F, Fut>(&self, label: &str, then: Then, call: F)
    where
        F: FnOnce(std::sync::Arc<HttpClient>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<(), String>> + Send + 'static,
    {
        let (http, tx, label) = (self.http.clone(), self.tx.clone(), label.to_string());
        self.rt.spawn(async move {
            let result = call(http).await;
            let _ = tx.send(Event::Done(Done { label, result, then }));
        });
    }

    /// Put text on the clipboard; says why on the status line when that fails.
    /// (Tests keep it in `last_copied` and leave the real clipboard alone.)
    pub(crate) fn copy_out(&mut self, text: &str) -> bool {
        if cfg!(test) {
            self.last_copied = Some(text.to_string());
            return true;
        }
        match clip::write_text(text) {
            Ok(()) => {
                self.last_copied = Some(text.to_string());
                true
            }
            Err(e) => {
                self.notice(format!("Could not copy: {e}"));
                false
            }
        }
    }

    /// Open an address in the desktop's browser.
    pub(crate) fn open_link(&mut self, url: &str) {
        self.last_opened = Some(url.to_string());
        if cfg!(test) {
            return;
        }
        match clip::open_url(url) {
            Ok(()) => self.notice(format!("Opening {}", crate::text::truncate(url, 60))),
            Err(e) => self.notice(format!("Could not open the link: {e}")),
        }
    }

    // ---- opening menus ------------------------------------------------------

    /// The attach (+) menu, opened above the button.
    pub fn open_attach_menu(&mut self) {
        if self.open.is_none() {
            self.notice("Open a chat first");
            return;
        }
        let items = menu::attach_items();
        let btn = self.hit.attach_btn;
        let at = (btn.x, btn.y.saturating_sub(items.len() as u16 + 3));
        self.menu = Some(Menu::new(at, Some("Attach".into()), items));
    }

    pub fn open_chat_menu(&mut self, at: (u16, u16), chat_id: &str) {
        let Some(chat) = self.chat(chat_id).cloned() else { return };
        let items = menu::chat_items(self, &chat);
        self.menu = Some(Menu::new(at, None, items));
    }

    pub fn open_message_menu(&mut self, at: (u16, u16), msg_id: &str) {
        let Some(chat_id) = self.open.clone() else { return };
        let Some(m) = self.logs.get(&chat_id).and_then(|l| l.msgs.iter().find(|m| m.id == msg_id)).cloned() else {
            return;
        };
        let items = menu::message_items(self, &chat_id, &m);
        if items.is_empty() {
            return;
        }
        self.sel_msg = Some(m.id.clone());
        self.menu = Some(Menu::new(at, None, items));
    }

    /// `.`: the menu of whatever is highlighted in the focused pane.
    pub fn open_menu_here(&mut self) {
        match self.focus {
            Focus::Chats => {
                let visible = self.visible();
                let pos = self.cursor_pos(&visible);
                let Some(&i) = visible.get(pos) else { return };
                let id = self.chats[i].id.clone();
                let row = pos.saturating_sub(self.side_offset) as u16 * 2;
                let r = self.hit.sidebar_list;
                self.open_chat_menu((r.x + 3, r.y + row + 1), &id);
            }
            Focus::Messages | Focus::Panel => {
                // The selected bubble, else the lowest one fully on screen.
                let id = self.sel_msg.clone().or_else(|| self.bubble_hits.last().map(|(_, id)| id.clone()));
                let Some(id) = id else {
                    self.notice("No message to act on (J/K selects one)");
                    return;
                };
                let at = self
                    .bubble_hits
                    .iter()
                    .find(|(_, mid)| *mid == id)
                    .map_or((self.hit.messages.x + 2, self.hit.messages.y + 1), |(r, _)| (r.x + 2, r.y + 1));
                self.open_message_menu(at, &id);
            }
        }
    }

    // ---- keys and mouse while a menu is open ----------------------------------

    pub fn key_menu(&mut self, key: KeyEvent) {
        let Some(menu) = self.menu.as_mut() else { return };
        // With a search field every letter is text; arrows, Enter and Esc still steer.
        if menu.search.is_some() {
            let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
            let none: Option<fn(&mut String)> = None;
            match key.code {
                KeyCode::Char('c') if ctrl => self.menu = None,
                KeyCode::Char('u') if ctrl => menu.edit_query(none),
                KeyCode::Char('n') if ctrl => menu.step(1),
                KeyCode::Char('p') if ctrl => menu.step(-1),
                KeyCode::Char(ch) if !ctrl => menu.edit_query(Some(move |q: &mut String| q.push(ch))),
                KeyCode::Backspace => menu.edit_query(Some(|q: &mut String| {
                    q.pop();
                })),
                KeyCode::Down | KeyCode::Tab => menu.step(1),
                KeyCode::Up | KeyCode::BackTab => menu.step(-1),
                KeyCode::Home => menu.sel = 0,
                KeyCode::End => menu.sel = menu.items.len().saturating_sub(1),
                // Esc clears the text first, then goes back a level.
                KeyCode::Esc => {
                    if !menu.query_is_empty() {
                        menu.edit_query(none);
                    } else if !menu.back() {
                        self.menu = None;
                    }
                }
                KeyCode::Enter => self.activate_menu_item(),
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Tab => menu.step(1),
            KeyCode::Char('k') | KeyCode::Up | KeyCode::BackTab => menu.step(-1),
            KeyCode::Char('g') | KeyCode::Home => menu.sel = 0,
            KeyCode::Char('G') | KeyCode::End => menu.sel = menu.items.len().saturating_sub(1),
            KeyCode::Esc | KeyCode::Char('h') | KeyCode::Left => {
                if !menu.back() {
                    self.menu = None;
                }
            }
            KeyCode::Char('q') => self.menu = None,
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right | KeyCode::Char(' ') => self.activate_menu_item(),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => self.menu = None,
            _ => {}
        }
    }

    fn activate_menu_item(&mut self) {
        let Some(menu) = self.menu.as_mut() else { return };
        let Some(item) = menu.current().cloned() else { return };
        match item.action {
            Action::Submenu { title, items } => menu.open_sub(title, items),
            Action::SearchMenu { title, items } => menu.open_search(title, items),
            action => {
                self.menu = None;
                self.run_action(action);
            }
        }
    }

    /// Mouse while a menu is open. Returns true when the event was used up.
    pub fn mouse_menu(&mut self, m: MouseEvent) -> bool {
        let Some(menu) = self.menu.as_mut() else { return false };
        let hit = menu.item_at(m.column, m.row);
        match m.kind {
            MouseEventKind::Moved => {
                if let Some(i) = hit {
                    menu.sel = i;
                }
                true
            }
            MouseEventKind::ScrollUp => {
                menu.step(-1);
                true
            }
            MouseEventKind::ScrollDown => {
                menu.step(1);
                true
            }
            MouseEventKind::Down(MouseButton::Left) => {
                match hit {
                    Some(i) => {
                        menu.sel = i;
                        self.activate_menu_item();
                    }
                    None => {
                        let r = menu.rect;
                        let inside = m.column >= r.x && m.column < r.x + r.width && m.row >= r.y && m.row < r.y + r.height;
                        if !inside {
                            self.menu = None;
                        }
                    }
                }
                true
            }
            // A right-click elsewhere closes this menu and opens the next one.
            MouseEventKind::Down(MouseButton::Right) => {
                self.menu = None;
                false
            }
            MouseEventKind::Down(_) => {
                self.menu = None;
                true
            }
            _ => true,
        }
    }

    // ---- J / K: pick a message with the keyboard --------------------------------

    pub fn select_msg(&mut self, dir: isize) {
        let Some(chat_id) = self.open.clone() else { return };
        let Some(log) = self.logs.get(&chat_id) else { return };
        if log.msgs.is_empty() {
            return;
        }
        let pos = self.sel_msg.as_ref().and_then(|id| log.msgs.iter().position(|m| m.id == *id));
        let next = match pos {
            Some(p) => (p as isize + dir).clamp(0, log.msgs.len() as isize - 1) as usize,
            None => log.msgs.len() - 1,
        };
        let id = log.msgs[next].id.clone();
        self.sel_msg = Some(id.clone());
        self.reveal = Some((id, false));
    }

    // ---- running an action --------------------------------------------------------

    pub fn run_action(&mut self, action: Action) {
        let chat = self.open.clone().unwrap_or_default();
        match action {
            Action::OpenChat(id) => self.open_chat(&id),
            Action::MarkRead(id) => self.mark_read_chat(id),
            Action::PinChat(id, pinned) => self.chat_state_call(move |h| async move { h.pin_chat(&id, pinned).await }),
            Action::ArchiveChat(id, archived) => {
                self.chat_state_call(move |h| async move { h.archive_chat(&id, archived).await })
            }
            Action::MuteChat(id, mode) => self.chat_state_call(move |h| async move { h.mute_chat(&id, mode).await }),
            Action::ExitGroup(id) => self.spawn_call("Left the group", Then::Nothing, move |h| async move {
                h.leave_group(&id).await.map(|_| ()).map_err(|e| e.to_string())
            }),
            Action::ChatInfo(id) => self.open_info(&id),
            Action::Attach(mode) => self.open_browser(mode),
            Action::PasteImage => self.paste_clipboard(),
            Action::Stickers => self.open_stickers(),
            Action::Compose(kind) => self.open_form(kind),

            Action::Reply(id) => {
                if let Some(m) = self.find_message(&chat, &id) {
                    let who = self.sender_label(&chat, &m);
                    let text = crate::app::preview(&m);
                    self.reply = Some(Reply { id, who, text });
                    self.editing = None;
                    self.focus = Focus::Messages;
                    self.mode = Mode::Insert;
                }
            }
            Action::React(id, emoji) => self.spawn_call("", Then::Nothing, move |h| async move {
                h.react_to_message(&chat, &id, &emoji, None).await.map(|_| ()).map_err(|e| e.to_string())
            }),
            Action::ForwardTo { msg, chat: to } => self.spawn_call("Forwarded", Then::Nothing, move |h| async move {
                h.forward_message(&chat, &msg, &[to]).await.map(|_| ()).map_err(|e| e.to_string())
            }),
            Action::Copy(text) => {
                if self.copy_out(&text) {
                    self.notice("Copied");
                }
            }
            Action::CopyImage(id) => self.copy_image(&chat, &id),
            Action::Star(id, starred) => {
                let then = Then::Star(chat.clone(), id.clone(), starred);
                self.spawn_call(if starred { "Starred" } else { "Unstarred" }, then, move |h| async move {
                    h.star_message(&chat, &id, starred).await.map(|_| ()).map_err(|e| e.to_string())
                })
            }
            Action::PinMsg(id, secs) => {
                let then = Then::ReloadPins(chat.clone());
                self.spawn_call("Pinned", then, move |h| async move {
                    h.pin_message(&chat, &id, true, secs).await.map(|_| ()).map_err(|e| e.to_string())
                })
            }
            Action::UnpinMsg(id) => {
                let then = Then::ReloadPins(chat.clone());
                self.spawn_call("Unpinned", then, move |h| async move {
                    h.pin_message(&chat, &id, false, 0).await.map(|_| ()).map_err(|e| e.to_string())
                })
            }
            Action::Edit(id) => {
                if let Some(m) = self.find_message(&chat, &id) {
                    self.input.text = m.content.clone();
                    self.input.cursor = self.input.text.len();
                    self.editing = Some(id);
                    self.reply = None;
                    self.focus = Focus::Messages;
                    self.mode = Mode::Insert;
                }
            }
            Action::DeleteForMe(id) => {
                let then = Then::RemoveMsg(chat.clone(), id.clone());
                self.spawn_call("Deleted", then, move |h| async move {
                    h.delete_message_for_me(&chat, &id).await.map(|_| ()).map_err(|e| e.to_string())
                })
            }
            Action::DeleteForAll(id) => {
                let then = Then::RevokeMsg(chat.clone(), id.clone());
                self.spawn_call("Deleted for everyone", then, move |h| async move {
                    h.delete_message(&chat, &id).await.map(|_| ()).map_err(|e| e.to_string())
                })
            }
            Action::MsgInfo(id) => self.open_msg_info(&chat, &id),
            Action::Favorite(id) => {
                let Some(m) = self.find_message(&chat, &id) else { return };
                let url = m.media_url.clone().unwrap_or_default();
                self.spawn_call("Added to favourites", Then::Nothing, move |h| async move {
                    h.favorite_sticker(&id, &url, false).await.map(|_| ()).map_err(|e| e.to_string())
                })
            }
            // Handled by the menu itself.
            Action::Submenu { .. } | Action::SearchMenu { .. } => {}
        }
    }

    /// Pin / archive / mute answer with the chat's new state.
    fn chat_state_call<F, Fut>(&self, call: F)
    where
        F: FnOnce(std::sync::Arc<HttpClient>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = wabot_backend_client::Result<wabot_backend_client::ChatState>> + Send + 'static,
    {
        let (http, tx) = (self.http.clone(), self.tx.clone());
        self.rt.spawn(async move {
            let result = call(http).await.map_err(|e| e.to_string());
            let _ = tx.send(Event::ChatState(result));
        });
    }

    pub fn mark_read_chat(&self, chat_id: String) {
        let (http, tx) = (self.http.clone(), self.tx.clone());
        self.rt.spawn(async move {
            let result = http.mark_as_read_explicit(&chat_id).await.map(|_| ()).map_err(|e| e.to_string());
            let _ = tx.send(Event::MarkedRead { chat_id, result });
        });
    }

    pub fn find_message(&self, chat_id: &str, id: &str) -> Option<Message> {
        self.logs.get(chat_id)?.msgs.iter().find(|m| m.id == id).cloned()
    }

    /// Who wrote `m`, as shown above bubbles and in reply bars.
    pub fn sender_label(&self, chat_id: &str, m: &Message) -> String {
        if is_from_me(m) {
            return "You".into();
        }
        if let Some(n) = m.sender_name.as_deref().filter(|n| !n.is_empty()) {
            return n.to_string();
        }
        let chat = self.chat(chat_id);
        if chat.is_some_and(|c| !c.is_group) {
            return chat.map(|c| c.name.clone()).unwrap_or_default();
        }
        m.from.split('@').next().unwrap_or("?").to_string()
    }

    fn copy_image(&self, chat_id: &str, id: &str) {
        let Some(url) = self.find_message(chat_id, id).and_then(|m| images::media_url(&m)) else { return };
        let (http, tx) = (self.http.clone(), self.tx.clone());
        self.rt.spawn(async move {
            let result = match crate::app::load_image(&http, &url, 4096).await {
                Ok(img) => tokio::task::spawn_blocking(move || clip::write_image(&img))
                    .await
                    .map_err(|e| e.to_string())
                    .and_then(|r| r),
                Err(e) => Err(e),
            };
            let _ = tx.send(Event::Done(Done { label: "Image copied".into(), result, then: Then::Nothing }));
        });
    }

    pub fn on_receipts(&mut self, msg_id: &str, result: Result<wabot_backend_client::MessageReceipts, String>) {
        if let Some(p) = self.panel.as_mut().filter(|p| p.kind == crate::panel::PanelKind::MsgInfo && p.msg_id.as_deref() == Some(msg_id)) {
            p.receipts = Some(result);
        }
    }

    pub fn on_done(&mut self, done: Done) {
        match done.result {
            Ok(()) => {
                if !done.label.is_empty() {
                    self.notice(done.label);
                }
                match done.then {
                    Then::Nothing => {}
                    Then::RemoveMsg(chat, id) => {
                        if let Some(log) = self.logs.get_mut(&chat) {
                            log.msgs.retain(|m| m.id != id);
                        }
                    }
                    Then::RevokeMsg(chat, id) => {
                        if let Some(m) = self.logs.get_mut(&chat).and_then(|l| l.msgs.iter_mut().find(|m| m.id == id)) {
                            m.revoked_at = Some(now_ms());
                        }
                    }
                    Then::Star(chat, id, starred) => {
                        if let Some(m) = self.logs.get_mut(&chat).and_then(|l| l.msgs.iter_mut().find(|m| m.id == id)) {
                            m.starred = starred;
                        }
                    }
                    Then::ReloadPins(chat) => self.load_pins(&chat),
                    Then::CloseStickers => self.stickers = None,
                    Then::CloseForm => self.form = None,
                    Then::CloseAttach => {
                        self.attach = None;
                        if self.mode == Mode::Caption {
                            self.mode = Mode::Normal;
                        }
                    }
                }
            }
            Err(e) => {
                if let Some(a) = self.attach.as_mut() {
                    a.sending = false;
                }
                if let Some(f) = self.form.as_mut() {
                    f.sending = false;
                    f.error = Some(e.clone());
                }
                self.notice(e);
            }
        }
    }

    // ---- pins ------------------------------------------------------------------------

    pub fn is_pinned(&self, chat_id: &str, msg_id: &str) -> bool {
        self.pins.get(chat_id).is_some_and(|p| p.contains(msg_id))
    }

    pub fn load_pins(&self, chat_id: &str) {
        let (http, tx, chat) = (self.http.clone(), self.tx.clone(), chat_id.to_string());
        self.rt.spawn(async move {
            let result = http.get_chat_pins(&chat).await.map_err(|e| e.to_string());
            let _ = tx.send(Event::Pins { chat_id: chat, result });
        });
    }

    pub fn on_pins(&mut self, chat_id: String, result: Result<Vec<PinnedMessage>, String>) {
        if let Ok(pins) = result {
            self.pins.insert(chat_id, pins.into_iter().map(|p| p.message_id).collect());
        }
    }

    // ---- composer: send, reply, edit ----------------------------------------------------

    /// Enter in the input: send, answer, or save an edit.
    pub fn send(&mut self) {
        let body = self.input.text.trim().to_string();
        let Some(chat_id) = self.open.clone() else { return };
        if body.is_empty() {
            return;
        }
        if let Some(id) = self.editing.take() {
            self.input.clear();
            self.spawn_call("Edited", Then::Nothing, move |h| async move {
                h.edit_message(&chat_id, &id, &body).await.map(|_| ()).map_err(|e| e.to_string())
            });
            return;
        }
        let reply = self.reply.take();
        self.temp_seq += 1;
        let temp_id = format!("temp-{}", self.temp_seq);
        let msg = Message {
            id: temp_id.clone(),
            chat_id: chat_id.clone(),
            from: "me".into(),
            to: chat_id.clone(),
            content: body.clone(),
            timestamp: now_ms(),
            status: "pending".into(),
            message_type: "text".into(),
            reply_to_id: reply.as_ref().map(|r| r.id.clone()),
            ..Default::default()
        };
        self.apply_to_chat_row(&msg, false);
        // Looking at an old part of the chat: bring the latest back so the new message lands at the end.
        let detached = self.logs.get(&chat_id).is_some_and(|l| l.detached);
        if detached {
            self.load_messages(&chat_id, None);
        }
        self.logs.entry(chat_id.clone()).or_default().msgs.push(msg);
        self.input.clear();
        self.scroll = 0;
        self.smooth.stop();

        let (http, tx) = (self.http.clone(), self.tx.clone());
        self.rt.spawn(async move {
            let result = match reply {
                Some(r) => http.reply_message(&chat_id, &r.id, &body, &[]).await.map(|_| String::new()),
                None => http.send_message(&chat_id, &body, &[]).await.map(|r| r.id),
            }
            .map_err(|e| e.to_string());
            let _ = tx.send(Event::Sent { chat_id, temp_id, result });
        });
    }

    /// Esc in normal mode: drop a half-made reply or edit.
    pub fn cancel_compose(&mut self) -> bool {
        let had = self.reply.is_some() || self.editing.is_some();
        if self.editing.take().is_some() {
            self.input.clear();
        }
        self.reply = None;
        had
    }

    // ---- paste ----------------------------------------------------------------------------

    /// `Ctrl-v`, or an empty paste: a picture if the clipboard has one, else its text.
    pub fn paste_clipboard(&mut self) {
        if self.open.is_none() && self.mode != Mode::Filter {
            self.notice("Open a chat first");
            return;
        }
        let tx = self.tx.clone();
        let want_text = matches!(self.mode, Mode::Insert | Mode::Filter | Mode::Search | Mode::Caption);
        self.rt.spawn_blocking(move || {
            let result = match clip::pick_from_clipboard() {
                Ok(p) => Ok(Pasted::File(p)),
                Err(e) if want_text => clip::read_text().map(Pasted::Text).map_err(|_| e),
                Err(e) => Err(e),
            };
            let _ = tx.send(Event::Pasted(result));
        });
    }

    /// Text that arrived through the terminal's bracketed paste.
    pub fn on_paste_text(&mut self, text: String) {
        // A form field takes the text as it is.
        if self.form.is_some() {
            return self.insert_pasted(&text);
        }
        // Dropping a file onto the terminal pastes its path.
        if self.mode != Mode::Filter && self.mode != Mode::Search {
            if let Some(path) = clip::file_path_in(&text) {
                match clip::pick_from_path(&path) {
                    Ok(p) => self.begin_attach(p),
                    Err(e) => self.notice(e),
                }
                return;
            }
            if text.trim().is_empty() {
                return self.paste_clipboard();
            }
        }
        self.insert_pasted(&text);
    }

    pub fn insert_pasted(&mut self, text: &str) {
        let flat: String = text.chars().map(|c| if c == '\n' || c == '\r' || c == '\t' { ' ' } else { c }).collect();
        if let Some(f) = self.form.as_mut() {
            let focus = f.focus;
            for c in flat.chars() {
                f.fields[focus].edit.insert(c);
            }
            return;
        }
        let edit = match self.mode {
            Mode::Insert => &mut self.input,
            Mode::Filter => &mut self.filter,
            Mode::Search => match self.panel.as_mut() {
                Some(p) => &mut p.query,
                None => return,
            },
            Mode::Caption => match self.attach.as_mut() {
                Some(a) => &mut a.caption,
                None => return,
            },
            Mode::Normal => return,
        };
        for c in flat.chars() {
            edit.insert(c);
        }
    }

    pub fn on_pasted(&mut self, result: Result<Pasted, String>) {
        match result {
            Ok(Pasted::File(p)) => self.begin_attach(p),
            Ok(Pasted::Text(t)) => self.insert_pasted(&t),
            Err(e) => self.notice(e),
        }
    }

    pub fn begin_attach(&mut self, picked: Picked) {
        let Some(chat_id) = self.open.clone() else {
            self.notice("Open a chat first");
            return;
        };
        let proto = match (&self.picker, &picked.preview) {
            (Some(pk), Some(img)) => Some(pk.new_resize_protocol(img.clone())),
            _ => None,
        };
        self.attach = Some(Attach {
            chat_id,
            picked,
            proto,
            caption: LineEdit::default(),
            sending: false,
            send_btn: Rect::default(),
            cancel_btn: Rect::default(),
        });
        self.mode = Mode::Caption;
    }

    pub fn key_caption(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.cancel_attach(),
            KeyCode::Enter => self.send_attachment(),
            KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => self.paste_clipboard(),
            _ => {
                if let Some(a) = self.attach.as_mut().filter(|a| a.picked.kind.allows_caption()) {
                    a.caption.key(key);
                }
            }
        }
    }

    /// Drop the pasted picture without sending it. Not while it is being sent.
    pub fn cancel_attach(&mut self) {
        if self.attach.as_ref().is_some_and(|a| a.sending) {
            return;
        }
        self.attach = None;
        if self.mode == Mode::Caption {
            self.mode = Mode::Normal;
        }
    }

    /// Mouse while the picture box is open: its two buttons. Clicks elsewhere do nothing,
    /// so a stray click cannot send or lose the picture.
    pub fn mouse_attach(&mut self, m: MouseEvent) {
        if m.kind != MouseEventKind::Down(MouseButton::Left) {
            return;
        }
        let Some(a) = self.attach.as_ref() else { return };
        let at = |r: Rect| r.width > 0 && m.column >= r.x && m.column < r.x + r.width && m.row >= r.y && m.row < r.y + r.height;
        if at(a.send_btn) {
            self.send_attachment();
        } else if at(a.cancel_btn) {
            self.cancel_attach();
        }
    }

    fn send_attachment(&mut self) {
        let Some(a) = self.attach.as_mut() else { return };
        if a.sending {
            return;
        }
        a.sending = true;
        let kind = a.picked.kind;
        let caption = if kind.allows_caption() { a.caption.text.trim().to_string() } else { String::new() };
        let (chat, bytes, name, mime) = (a.chat_id.clone(), a.picked.bytes.clone(), a.picked.name.clone(), a.picked.mime);
        let done = format!("{} sent", kind.label());
        let done = done[..1].to_uppercase() + &done[1..];
        self.spawn_call(&done, Then::CloseAttach, move |h| async move {
            let builder = SendMediaBuilder::new(chat, kind.media_type(), bytes, name).caption(caption).mime_type(mime);
            h.send_media(builder).await.map(|_| ()).map_err(|e| e.to_string())
        });
    }

    /// Where the "reply" / "editing" bar sits is decided by the renderer; this is its text.
    pub fn compose_bar(&self) -> Option<(String, String)> {
        if self.editing.is_some() {
            return Some(("Editing message".into(), String::new()));
        }
        self.reply.as_ref().map(|r| (format!("Replying to {}", r.who), r.text.clone()))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::app::{ChatLog, Event};
    use std::sync::Arc;
    use wabot_backend_client::{Chat, HttpClient};

    /// An app wired to nothing: the runtime never runs, so no call leaves the process.
    pub(crate) fn offline_app(rt: &tokio::runtime::Runtime) -> App {
        let (tx, _rx) = flume::unbounded();
        let mut app = App::new(Arc::new(HttpClient::from_port(1)), rt.handle().clone(), tx, None);
        app.chats.push(Chat {
            id: "g@g.us".into(),
            name: "Group".into(),
            is_group: true,
            last_time: 1,
            ..serde_json::from_str(r#"{"id":"x"}"#).unwrap()
        });
        let msgs = vec![
            Message { id: "a".into(), chat_id: "g@g.us".into(), from: "62@s.whatsapp.net".into(), content: "halo".into(), status: "received".into(), message_type: "text".into(), timestamp: 1, ..Default::default() },
            Message { id: "b".into(), chat_id: "g@g.us".into(), from: "me".into(), content: "hai".into(), status: "sent".into(), message_type: "text".into(), timestamp: 2, ..Default::default() },
        ];
        app.logs.insert("g@g.us".into(), ChatLog { msgs, loaded: true, ..Default::default() });
        app.open = Some("g@g.us".into());
        app
    }

    fn labels(app: &App) -> Vec<String> {
        app.menu.as_ref().unwrap().items.iter().map(|i| i.label.clone()).collect()
    }

    #[test]
    fn message_menu_differs_for_mine_and_theirs() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = offline_app(&rt);
        app.open_message_menu((5, 5), "a");
        let theirs = labels(&app);
        assert!(theirs.contains(&"Reply".to_string()) && theirs.contains(&"Copy".to_string()));
        assert!(!theirs.contains(&"Edit".to_string()), "cannot edit someone else's message");
        app.menu = None;
        app.open_message_menu((5, 5), "b");
        let mine = labels(&app);
        assert!(mine.contains(&"Edit".to_string()));
        // Delete is a submenu; "for everyone" only on my own messages.
        app.menu.as_mut().unwrap().sel = mine.iter().position(|l| l == "Delete").unwrap();
        app.key_menu(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(labels(&app), ["Delete for me", "Delete for everyone"]);
        // Esc goes back one level, a second Esc closes.
        app.key_menu(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(labels(&app).contains(&"Delete".to_string()));
        app.key_menu(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(app.menu.is_none());
    }

    #[test]
    fn reply_and_edit_prepare_the_composer_without_sending() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = offline_app(&rt);
        app.run_action(Action::Reply("a".into()));
        let (title, text) = app.compose_bar().unwrap();
        assert_eq!((title.as_str(), text.as_str()), ("Replying to 62", "halo"));
        assert_eq!(app.mode, Mode::Insert);
        assert!(app.cancel_compose() && app.compose_bar().is_none());

        app.run_action(Action::Edit("b".into()));
        assert_eq!((app.input.text.as_str(), app.compose_bar().unwrap().0.as_str()), ("hai", "Editing message"));
        assert!(app.cancel_compose());
        assert!(app.input.text.is_empty());
    }

    #[test]
    fn chat_menu_follows_chat_state() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = offline_app(&rt);
        app.chats[0].unread = 3;
        app.open_chat_menu((0, 0), "g@g.us");
        let l = labels(&app);
        for want in ["Group info", "Mark as read", "Archive chat", "Mute notifications", "Pin chat", "Exit group"] {
            assert!(l.contains(&want.to_string()), "missing {want}: {l:?}");
        }
        assert!(!l.contains(&"Open".to_string()), "already open");
        // Exiting a group asks again.
        app.menu.as_mut().unwrap().sel = l.iter().position(|x| x == "Exit group").unwrap();
        app.key_menu(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(labels(&app), ["Yes, exit group"]);
    }

    #[test]
    fn done_applies_local_changes() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut app = offline_app(&rt);
        app.on_done(Done { label: "Deleted".into(), result: Ok(()), then: Then::RemoveMsg("g@g.us".into(), "a".into()) });
        assert_eq!(app.logs["g@g.us"].msgs.len(), 1);
        app.on_done(Done { label: String::new(), result: Ok(()), then: Then::Star("g@g.us".into(), "b".into(), true) });
        assert!(app.logs["g@g.us"].msgs[0].starred);
        app.on_done(Done { label: String::new(), result: Err("boom".into()), then: Then::Nothing });
        assert_eq!(app.live_notice(), Some("boom"));
        let _ = Event::Tick;
    }
}

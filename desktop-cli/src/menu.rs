//! Context menus: right-click (or `.`) on a message bubble or a chat in the list.
//!
//! A menu is a list of items; an item either runs an [`Action`] or opens a
//! submenu (Delete, Mute, Pin, React, Forward). `Esc` goes back one level.

use ratatui::layout::Rect;
use wabot_backend_client::{Chat, Message};

use crate::app::{is_from_me, App};
use crate::attach::AttachMode;
use crate::form::FormKind;
use crate::images;

/// What picking an item does. Executed by `App::run_action`.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    // Chats
    OpenChat(String),
    MarkRead(String),
    PinChat(String, bool),
    ArchiveChat(String, bool),
    MuteChat(String, &'static str),
    ExitGroup(String),
    ChatInfo(String),
    // Messages
    Reply(String),
    React(String, String),
    ForwardTo { msg: String, chat: String },
    Copy(String),
    CopyImage(String),
    Star(String, bool),
    PinMsg(String, u32),
    UnpinMsg(String),
    Edit(String),
    DeleteForMe(String),
    DeleteForAll(String),
    MsgInfo(String),
    Favorite(String),
    // The attach (+) menu
    Attach(AttachMode),
    PasteImage,
    Stickers,
    Compose(FormKind),
    /// Open another menu level.
    Submenu { title: String, items: Vec<MenuItem> },
    /// A menu level with a search field: typing narrows the list.
    SearchMenu { title: String, items: Vec<MenuItem> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct MenuItem {
    pub label: String,
    /// Dim text at the right edge (a hint such as a duration or a key).
    pub hint: String,
    pub action: Action,
    pub danger: bool,
    /// A thin rule above this item.
    pub rule_above: bool,
}

impl MenuItem {
    pub fn new(label: impl Into<String>, action: Action) -> Self {
        Self { label: label.into(), hint: String::new(), action, danger: false, rule_above: false }
    }

    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    pub fn rule(mut self) -> Self {
        self.rule_above = true;
        self
    }

    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = hint.into();
        self
    }
}

/// The search field of a menu level: what was typed and the full list.
#[derive(Clone, Debug, Default)]
pub struct Search {
    pub query: String,
    pub all: Vec<MenuItem>,
}

#[derive(Clone, Debug)]
pub struct Menu {
    /// Where it opened (the click, or the target's corner).
    pub at: (u16, u16),
    pub title: Option<String>,
    pub items: Vec<MenuItem>,
    pub sel: usize,
    /// First visible item when the list is taller than the screen.
    pub offset: usize,
    /// Levels above this one, for `Esc`/back.
    back: Vec<(Option<String>, Vec<MenuItem>, usize, Option<Search>)>,
    /// Set on a level with a search field.
    pub search: Option<Search>,
    /// Screen area and first item row of the last draw, for the mouse.
    pub rect: Rect,
    pub rows: Vec<(u16, usize)>,
    /// The title row of a submenu: clicking it goes back (like `Esc`).
    pub back_row: Rect,
}

impl Menu {
    pub fn new(at: (u16, u16), title: Option<String>, items: Vec<MenuItem>) -> Self {
        Self { at, title, items, sel: 0, offset: 0, back: Vec::new(), search: None, rect: Rect::default(), rows: Vec::new(), back_row: Rect::default() }
    }

    pub fn step(&mut self, delta: isize) {
        if self.items.is_empty() {
            return;
        }
        let n = self.items.len() as isize;
        self.sel = (self.sel as isize + delta).rem_euclid(n) as usize;
    }

    /// True inside a submenu.
    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn current(&self) -> Option<&MenuItem> {
        self.items.get(self.sel)
    }

    pub fn open_sub(&mut self, title: String, items: Vec<MenuItem>) {
        let prev = (self.title.take(), std::mem::take(&mut self.items), self.sel, self.search.take());
        self.back.push(prev);
        self.title = Some(title);
        self.items = items;
        self.sel = 0;
        self.offset = 0;
    }

    /// Like `open_sub`, with a search field above the list.
    pub fn open_search(&mut self, title: String, items: Vec<MenuItem>) {
        self.open_sub(title, items.clone());
        self.search = Some(Search { query: String::new(), all: items });
    }

    /// Edit the search text and narrow the list. `None` = clear it.
    pub fn edit_query(&mut self, edit: Option<impl FnOnce(&mut String)>) {
        if let Some(s) = self.search.as_mut() {
            match edit {
                Some(f) => f(&mut s.query),
                None => s.query.clear(),
            }
        }
        self.refilter();
    }

    pub fn query_is_empty(&self) -> bool {
        self.search.as_ref().is_none_or(|s| s.query.is_empty())
    }

    /// The typed text changed: keep the entries that contain it, best match first.
    pub fn refilter(&mut self) {
        let Some(s) = self.search.as_ref() else { return };
        let q = s.query.trim().to_lowercase();
        let mut hits: Vec<(usize, &MenuItem)> = s
            .all
            .iter()
            .filter_map(|i| {
                let l = i.label.to_lowercase();
                // Words starting with the text come before the text found in the middle.
                let rank = if l.starts_with(&q) {
                    0
                } else if l.split_whitespace().any(|w| w.starts_with(&q)) {
                    1
                } else if l.contains(&q) {
                    2
                } else {
                    return None;
                };
                Some((rank, i))
            })
            .collect();
        hits.sort_by_key(|(rank, _)| *rank);
        self.items = hits.into_iter().map(|(_, i)| i.clone()).collect();
        self.sel = 0;
        self.offset = 0;
    }

    /// One level up. False when already at the top (the caller closes the menu).
    pub fn back(&mut self) -> bool {
        match self.back.pop() {
            Some((title, items, sel, search)) => {
                self.title = title;
                self.items = items;
                self.search = search;
                self.sel = sel;
                self.offset = 0;
                true
            }
            None => false,
        }
    }

    /// The item drawn on screen row `row`, if any.
    pub fn item_at(&self, col: u16, row: u16) -> Option<usize> {
        let r = self.rect;
        if col < r.x || col >= r.x + r.width {
            return None;
        }
        self.rows.iter().find(|(y, _)| *y == row).map(|(_, i)| *i)
    }
}

pub const REACTIONS: [&str; 6] = ["👍", "❤️", "😂", "😮", "😢", "🙏"];
pub const PIN_DURATIONS: [(u32, &str); 3] = [(86_400, "24 hours"), (604_800, "7 days"), (2_592_000, "30 days")];

/// The attach (+) menu above the message field.
pub fn attach_items() -> Vec<MenuItem> {
    vec![
        MenuItem::new("Sticker", Action::Stickers),
        MenuItem::new("Photo / video", Action::Attach(AttachMode::Media)),
        MenuItem::new("Document", Action::Attach(AttachMode::Document)),
        MenuItem::new("Audio", Action::Attach(AttachMode::Audio)),
        MenuItem::new("GIF", Action::Attach(AttachMode::Gif)),
        MenuItem::new("Poll", Action::Compose(FormKind::Poll)).rule(),
        MenuItem::new("Location", Action::Compose(FormKind::Location)),
        MenuItem::new("Contact", Action::Compose(FormKind::Contact)),
        MenuItem::new("Paste picture", Action::PasteImage).hint("Ctrl-v").rule(),
    ]
}

/// Entries for a chat row.
pub fn chat_items(app: &App, chat: &Chat) -> Vec<MenuItem> {
    let id = chat.id.clone();
    let mut items = Vec::new();
    if app.open.as_deref() != Some(id.as_str()) {
        items.push(MenuItem::new("Open", Action::OpenChat(id.clone())));
    }
    items.push(MenuItem::new(if chat.is_group { "Group info" } else { "Contact info" }, Action::ChatInfo(id.clone())));
    if chat.unread > 0 {
        items.push(MenuItem::new("Mark as read", Action::MarkRead(id.clone())));
    }
    items.push(MenuItem::new(
        if chat.archived { "Unarchive chat" } else { "Archive chat" },
        Action::ArchiveChat(id.clone(), !chat.archived),
    ));
    if chat.mute_mode != "off" {
        items.push(MenuItem::new("Unmute notifications", Action::MuteChat(id.clone(), "off")));
    } else {
        items.push(MenuItem::new(
            "Mute notifications",
            Action::Submenu {
                title: "Mute for".into(),
                items: vec![
                    MenuItem::new("8 hours", Action::MuteChat(id.clone(), "8h")),
                    MenuItem::new("1 week", Action::MuteChat(id.clone(), "1w")),
                    MenuItem::new("Always", Action::MuteChat(id.clone(), "forever")),
                ],
            },
        ));
    }
    let pinned = chat.pinned_at.is_some();
    items.push(MenuItem::new(if pinned { "Unpin chat" } else { "Pin chat" }, Action::PinChat(id.clone(), !pinned)));
    if chat.is_group {
        items.push(
            MenuItem::new(
                "Exit group",
                Action::Submenu {
                    title: format!("Exit {}?", crate::text::truncate(&chat.name, 24)),
                    items: vec![MenuItem::new("Yes, exit group", Action::ExitGroup(id.clone())).danger()],
                },
            )
            .danger()
            .rule(),
        );
    }
    items
}

/// Entries for a message bubble.
pub fn message_items(app: &App, chat_id: &str, m: &Message) -> Vec<MenuItem> {
    let id = m.id.clone();
    let mine = is_from_me(m);
    // A message still on its way has no id the backend knows yet.
    let settled = !m.id.starts_with("temp-") && m.status != "pending" && m.status != "failed";
    let revoked = m.revoked_at.is_some();
    let mut items = Vec::new();

    if settled && !revoked {
        items.push(MenuItem::new("Reply", Action::Reply(id.clone())));
        items.push(MenuItem::new(
            "React",
            Action::Submenu {
                title: "React".into(),
                items: REACTIONS.iter().map(|e| MenuItem::new(*e, Action::React(id.clone(), (*e).to_string()))).collect(),
            },
        ));
        items.push(MenuItem::new("Message info", Action::MsgInfo(id.clone())));
        items.push(MenuItem::new(
            "Forward",
            Action::SearchMenu { title: "Forward to".into(), items: forward_targets(app, chat_id, &id) },
        ));
        if app.is_pinned(chat_id, &id) {
            items.push(MenuItem::new("Unpin", Action::UnpinMsg(id.clone())));
        } else {
            items.push(MenuItem::new(
                "Pin",
                Action::Submenu {
                    title: "Pin for".into(),
                    items: PIN_DURATIONS.iter().map(|(s, l)| MenuItem::new(*l, Action::PinMsg(id.clone(), *s))).collect(),
                },
            ));
        }
        items.push(MenuItem::new(if m.starred { "Unstar" } else { "Star" }, Action::Star(id.clone(), !m.starred)));
    }

    let copyable = !m.content.trim().is_empty()
        && !matches!(m.content.trim(), "[Image]" | "[Sticker]" | "[Video]" | "[Audio]" | "[Document]");
    if copyable {
        items.push(MenuItem::new("Copy", Action::Copy(m.content.clone())));
    }
    let is_picture = matches!(m.message_type.as_str(), "image" | "sticker") && images::media_url(m).is_some();
    if is_picture {
        items.push(MenuItem::new("Copy image", Action::CopyImage(id.clone())));
    }
    if m.message_type == "sticker" && settled {
        items.push(MenuItem::new("Add to favourites", Action::Favorite(id.clone())));
    }
    let editable = matches!(m.message_type.as_str(), "" | "text") && !is_picture;
    if mine && settled && editable && !revoked {
        items.push(MenuItem::new("Edit", Action::Edit(id.clone())));
    }
    if settled {
        let mut choices = vec![MenuItem::new("Delete for me", Action::DeleteForMe(id.clone()))];
        if mine && !revoked {
            choices.push(MenuItem::new("Delete for everyone", Action::DeleteForAll(id.clone())).danger());
        }
        items.push(MenuItem::new("Delete", Action::Submenu { title: "Delete message".into(), items: choices }).danger().rule());
    }
    items
}

/// Chats a message can be forwarded to: all of them, newest first (the menu searches).
fn forward_targets(app: &App, from_chat: &str, msg_id: &str) -> Vec<MenuItem> {
    app.visible_all()
        .into_iter()
        .map(|c| {
            let item = MenuItem::new(
                crate::text::truncate(&c.name, 30),
                Action::ForwardTo { msg: msg_id.to_string(), chat: c.id.clone() },
            );
            if c.id == from_chat {
                item.hint("this chat")
            } else if c.is_group {
                item.hint("group")
            } else {
                item
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_steps_wrap_and_go_back() {
        let a = MenuItem::new("A", Action::OpenChat("a".into()));
        let b = MenuItem::new("B", Action::OpenChat("b".into()));
        let mut m = Menu::new((0, 0), None, vec![a.clone(), b.clone()]);
        m.step(1);
        assert_eq!(m.sel, 1);
        m.step(1);
        assert_eq!(m.sel, 0);
        m.step(-1);
        assert_eq!(m.sel, 1);
        m.open_sub("Sub".into(), vec![a]);
        assert_eq!((m.items.len(), m.sel, m.title.as_deref()), (1, 0, Some("Sub")));
        assert!(m.back());
        assert_eq!((m.items.len(), m.sel), (2, 1));
        assert!(!m.back());
    }
}

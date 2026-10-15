use std::collections::{HashMap, HashSet};
use wabot_backend_client::dto::{Chat, ChatState, Message};

#[derive(Debug, Clone, PartialEq)]
pub struct ChatMessagesEntry {
    pub messages: Vec<Message>,
    pub has_more: bool,
    pub has_more_next: bool,
    pub loaded: bool,
    pub loading: bool,
    pub loading_more: bool,
    pub loading_newer: bool,
}

impl Default for ChatMessagesEntry {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            has_more: true,
            has_more_next: false,
            loaded: false,
            loading: false,
            loading_more: false,
            loading_newer: false,
        }
    }
}

pub fn sort_asc(mut msgs: Vec<Message>) -> Vec<Message> {
    msgs.sort_by_key(|m| m.timestamp);
    msgs
}

/// How long a typing indicator lasts without being renewed.
pub const TYPING_TTL: std::time::Duration = std::time::Duration::from_secs(6);

#[derive(Debug, Clone)]
pub struct TypingEntry {
    pub name: Option<String>,
    pub audio: bool,
    pub at: std::time::Instant,
}

#[derive(Debug, Clone, Default)]
pub struct ChatStore {
    pub chats: Vec<Chat>,
    pub chats_loaded: bool,
    pub chats_loading: bool,
    /// Set by the backend's `chats_changed`; the chat view reloads the list
    /// (and clears this) the next time it renders.
    pub chats_stale: bool,
    /// Set when contact names were re-resolved: the open chat's messages and
    /// group members carry the old names and must be fetched again.
    pub names_changed: bool,
    pub chats_version: u64,
    pub messages_by_chat: HashMap<String, ChatMessagesEntry>,
    pub active_chat_id: Option<String>,
    /// Who is typing where: chat id → sender → entry. Entries go stale after
    /// [`TYPING_TTL`] unless renewed (WhatsApp repeats "composing" every few seconds).
    pub typing: HashMap<String, HashMap<String, TypingEntry>>,
    pub peer_presence: HashMap<String, String>, // chat_id -> presence string (e.g. "available", "composing")
    /// Bumped on every message mutation (status, reaction, new/deleted/edited
    /// message) so views can rebuild their render caches even when the chat
    /// id is unchanged.
    pub messages_version: u64,
    /// Chats whose pinned messages changed (`chat_pins_changed`); the open
    /// chat's view takes its entry and refetches the pins.
    pub pins_stale: HashSet<String>,
    /// Bumped when a message is starred or unstarred elsewhere, so an open
    /// starred-messages list reloads.
    pub starred_version: u64,
}

use gpui::{App, Global};
use wabot_backend_client::ws::WsEvent;

impl Global for ChatStore {}

impl ChatStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }

    pub fn set_chats(&mut self, chats: Vec<Chat>) {
        self.chats = chats;
        self.chats_loaded = true;
        self.chats_loading = false;
        self.chats_version = self.chats_version.wrapping_add(1);
    }

    pub fn upsert_chat(&mut self, chat: Chat) {
        self.chats_version = self.chats_version.wrapping_add(1);
        if let Some(pos) = self.chats.iter().position(|c| c.id == chat.id) {
            let existing = &self.chats[pos];
            let last_changed = (!chat.last_msg.is_empty() && chat.last_msg != existing.last_msg)
                || (chat.last_time != 0 && chat.last_time != existing.last_time);

            // Preserve unread unless updated explicitly (if unread == 0 and existing > 0, check if we keep existing)
            // Web parity: if unread is provided, merge. If not explicitly changed, preserve existing.
            let merged = Chat {
                id: chat.id.clone(),
                name: if !chat.name.is_empty() { chat.name } else { existing.name.clone() },
                avatar: if !chat.avatar.is_empty() { chat.avatar } else { existing.avatar.clone() },
                last_msg: if !chat.last_msg.is_empty() { chat.last_msg } else { existing.last_msg.clone() },
                last_time: if chat.last_time != 0 { chat.last_time } else { existing.last_time },
                unread: chat.unread,
                has_unread_mention: chat.unread > 0 && chat.has_unread_mention,
                is_active: chat.is_active,
                is_group: chat.is_group || existing.is_group,
                archived: chat.archived,
                pinned_at: chat.pinned_at.or(existing.pinned_at),
                mute_mode: chat.mute_mode,
                muted_until: chat.muted_until.or(existing.muted_until),
                disappearing_secs: chat.disappearing_secs,
            };

            if last_changed {
                self.chats.remove(pos);
                self.chats.insert(0, merged);
            } else {
                self.chats[pos] = merged;
            }
        } else {
            self.chats.insert(0, chat);
        }
    }

    pub fn patch_chat_state(&mut self, state: &ChatState) {
        self.chats_version = self.chats_version.wrapping_add(1);
        if let Some(chat) = self.chats.iter_mut().find(|c| c.id == state.chat_id) {
            chat.archived = state.archived;
            chat.pinned_at = state.pinned_at;
            chat.mute_mode = state.mute_mode.clone();
            chat.muted_until = state.muted_until;
        }
    }

    /// Set a chat's disappearing-messages timer (0 = off).
    pub fn set_disappearing(&mut self, chat_id: &str, seconds: u32) {
        if let Some(chat) = self.chats.iter_mut().find(|c| c.id == chat_id) {
            chat.disappearing_secs = seconds;
            self.chats_version = self.chats_version.wrapping_add(1);
        }
    }

    /// Chats whose pins changed since the last call (drained).
    pub fn take_stale_pins(&mut self) -> Vec<String> {
        self.pins_stale.drain().collect()
    }

    pub fn invalidate_messages(&mut self) {
        for entry in self.messages_by_chat.values_mut() {
            entry.loaded = false;
            entry.loading = false;
        }
    }

    pub fn ensure_chat_state(&mut self, chat_id: &str) -> &mut ChatMessagesEntry {
        self.messages_by_chat
            .entry(chat_id.to_string())
            .or_insert_with(ChatMessagesEntry::default)
    }

    pub fn set_messages(&mut self, chat_id: &str, msgs: Vec<Message>, has_more: bool) {
        let entry = self.ensure_chat_state(chat_id);
        entry.messages = sort_asc(msgs);
        entry.has_more = has_more;
        entry.has_more_next = false;
        entry.loaded = true;
        entry.loading = false;
        self.messages_version = self.messages_version.wrapping_add(1);
    }

    pub fn prepend_messages(&mut self, chat_id: &str, msgs: Vec<Message>, has_more: bool) {
        let entry = self.ensure_chat_state(chat_id);
        let mut combined = msgs;
        combined.extend(entry.messages.clone());
        entry.messages = sort_asc(combined);
        entry.has_more = has_more;
        entry.loading_more = false;
    }

    pub fn append_messages(&mut self, chat_id: &str, msgs: Vec<Message>) {
        let entry = self.ensure_chat_state(chat_id);
        let has_next = !msgs.is_empty();
        let mut combined = entry.messages.clone();
        combined.extend(msgs);
        entry.messages = sort_asc(combined);
        entry.has_more_next = has_next;
        entry.loading_newer = false;
    }

    pub fn upsert_message(&mut self, chat_id: &str, msg: Message) {
        let entry = self.ensure_chat_state(chat_id);
        let existing_index = entry.messages.iter().position(|m| m.id == msg.id);

        // Rule 4: If incoming message has a real id and matches a pending temp message,
        // replace the temp one in place without duplicating
        if existing_index.is_none() && !msg.id.starts_with("temp-") {
            let pending_index = entry.messages.iter().position(|m| {
                m.status == "pending"
                    && m.id.starts_with("temp-")
                    && (m.content == msg.content
                        || (m.message_type == msg.message_type
                            && matches!(
                                m.message_type.as_str(),
                                "image" | "video" | "sticker" | "document" | "audio" | "ptt" | "voice"
                            )))
            });

            if let Some(p_idx) = pending_index {
                entry.messages[p_idx] = msg;
                self.messages_version = self.messages_version.wrapping_add(1);
                return;
            }
        }

        if let Some(idx) = existing_index {
            entry.messages[idx] = msg;
        } else {
            let mut msgs = entry.messages.clone();
            msgs.push(msg);
            entry.messages = sort_asc(msgs);
        }
        self.messages_version = self.messages_version.wrapping_add(1);
    }

    pub fn patch_message<F>(&mut self, chat_id: &str, msg_id: &str, patch_fn: F)
    where
        F: FnOnce(&mut Message),
    {
        if let Some(entry) = self.messages_by_chat.get_mut(chat_id) {
            if let Some(msg) = entry.messages.iter_mut().find(|m| m.id == msg_id) {
                patch_fn(msg);
                self.messages_version = self.messages_version.wrapping_add(1);
            }
        }
    }

    pub fn delete_message(&mut self, chat_id: &str, msg_id: &str) {
        if let Some(entry) = self.messages_by_chat.get_mut(chat_id) {
            let before = entry.messages.len();
            entry.messages.retain(|m| m.id != msg_id);
            if entry.messages.len() != before {
                self.messages_version = self.messages_version.wrapping_add(1);
            }
        }
    }

    pub fn set_presence(&mut self, chat_id: &str, presence: &str) {
        self.peer_presence.insert(chat_id.to_string(), presence.to_string());
    }

    /// Record (or clear) that `sender` is typing in `chat_id`.
    pub fn set_typing(&mut self, chat_id: &str, sender: &str, name: Option<String>, composing: bool, audio: bool) {
        let sender = if sender.is_empty() { chat_id } else { sender };
        if composing {
            self.typing.entry(chat_id.to_string()).or_default().insert(
                sender.to_string(),
                TypingEntry { name, audio, at: std::time::Instant::now() },
            );
        } else if let Some(entries) = self.typing.get_mut(chat_id) {
            entries.remove(sender);
            if entries.is_empty() {
                self.typing.remove(chat_id);
            }
        }
    }

    /// Who is typing in `chat_id` right now: (sender JID, display name, recording audio).
    pub fn typers(&self, chat_id: &str) -> Vec<(String, String, bool)> {
        let Some(entries) = self.typing.get(chat_id) else { return Vec::new() };
        let mut fresh: Vec<_> = entries.iter().filter(|(_, entry)| entry.at.elapsed() < TYPING_TTL).collect();
        fresh.sort_by_key(|(_, entry)| entry.at);
        fresh
            .into_iter()
            .map(|(sender, entry)| {
                let name = entry
                    .name
                    .clone()
                    .or_else(|| self.chats.iter().find(|c| c.id == *sender).map(|c| c.name.clone()))
                    .unwrap_or_else(|| sender.split('@').next().unwrap_or(sender).to_string());
                (sender.clone(), name, entry.audio)
            })
            .collect()
    }

    /// The chat-list line for a typing chat, like WhatsApp's: "typing…" in a
    /// one-to-one chat, "Name is typing…" in a group.
    pub fn typing_label(&self, chat_id: &str, is_group: bool) -> Option<String> {
        let typers = self.typers(chat_id);
        let (_, first, audio) = typers.first()?.clone();
        let verb = if audio { "recording audio…" } else { "typing…" };
        Some(match (is_group, typers.len()) {
            (false, _) => verb.to_string(),
            (true, 1) => format!("{first} is {verb}"),
            (true, 2) => format!("{first} and {} are {verb}", typers[1].1),
            (true, n) => format!("{first} and {} others are {verb}", n - 1),
        })
    }

    pub fn get_presence(&self, chat_id: &str) -> Option<&String> {
        self.peer_presence.get(chat_id)
    }

    pub fn mark_chat_read(&mut self, chat_id: &str) {
        if let Some(chat) = self.chats.iter_mut().find(|c| c.id == chat_id) {
            chat.unread = 0;
            chat.has_unread_mention = false;
            self.chats_version = self.chats_version.wrapping_add(1);
        }
    }

    pub fn handle_ws_event(&mut self, event: &WsEvent) -> bool {
        self.handle_ws_event_with_visibility(event, true)
    }

    pub fn handle_ws_event_with_visibility(&mut self, event: &WsEvent, chat_is_visible: bool) -> bool {
        match event {
            // Another account may link next: drop everything cached from this one.
            WsEvent::LoggedOut => {
                *self = Self::default();
                true
            }
            WsEvent::MessageStatus { chat_id, id, status } => {
                if let Some(cid) = chat_id {
                    self.patch_message(cid, id, |m| m.status = status.clone());
                } else {
                    for entry in self.messages_by_chat.values_mut() {
                        if let Some(m) = entry.messages.iter_mut().find(|m| m.id == *id) {
                            m.status = status.clone();
                        }
                    }
                }
                true
            }
            WsEvent::MessageDeleted { chat_id, id } => {
                self.delete_message(chat_id, id);
                true
            }
            WsEvent::MessageEdited { chat_id, id, content, extra } => {
                self.patch_message(chat_id, id, |m| {
                    m.content = content.clone();
                    if extra.is_some() {
                        m.extra = extra.clone();
                    }
                });
                true
            }
            WsEvent::MessageReaction { chat_id, id, reactions } => {
                self.patch_message(chat_id, id, |m| m.reactions = Some(reactions.clone()));
                true
            }
            WsEvent::MessageStarred { chat_id, id, starred } => {
                self.patch_message(chat_id, id, |m| m.starred = *starred);
                self.starred_version = self.starred_version.wrapping_add(1);
                true
            }
            WsEvent::ChatPinsChanged { chat_id } => {
                self.pins_stale.insert(chat_id.clone());
                true
            }
            // A fresh login brings the chats along; the view loads them next render.
            WsEvent::AuthSuccess { .. } => {
                self.chats_stale = true;
                true
            }
            WsEvent::PollUpdate { chat_id, message_id, payload } => {
                let extra = payload
                    .get("extra")
                    .and_then(|v| serde_json::from_value::<wabot_backend_client::dto::MessageExtra>(v.clone()).ok());
                match extra {
                    Some(extra) => {
                        self.patch_message(chat_id, message_id, |m| m.extra = Some(extra));
                        true
                    }
                    None => false,
                }
            }
            WsEvent::NewMessage(msg) => {
                let chat_id = msg.chat_id.clone();
                // Only what others send is unread; the echo of my own message
                // (sent from the phone or another device) is not.
                let incoming = msg.status == "received";
                let mentions_me = incoming && msg.extra.as_ref().is_some_and(|extra| extra.mentions_me);
                let unread_here = incoming
                    && (!chat_is_visible || self.active_chat_id.as_deref() != Some(&chat_id));
                self.upsert_message(&chat_id, *msg.clone());

                if let Some(pos) = self.chats.iter().position(|c| c.id == chat_id) {
                    let mut existing = self.chats.remove(pos);
                    existing.last_msg = msg.content.clone();
                    existing.last_time = msg.timestamp;
                    if unread_here {
                        existing.unread += 1;
                        existing.has_unread_mention |= mentions_me;
                    }
                    self.chats.insert(0, existing);
                } else {
                    let new_chat = Chat {
                        id: chat_id.clone(),
                        name: msg.sender_name.clone().unwrap_or_else(|| chat_id.clone()),
                        avatar: String::new(),
                        last_msg: msg.content.clone(),
                        last_time: msg.timestamp,
                        unread: if unread_here { 1 } else { 0 },
                        has_unread_mention: unread_here && mentions_me,
                        is_active: true,
                        is_group: chat_id.contains("@g.us"),
                        archived: false,
                        pinned_at: None,
                        mute_mode: "off".to_string(),
                        muted_until: None,
                        disappearing_secs: 0,
                    };
                    self.chats.insert(0, new_chat);
                }
                self.chats_version = self.chats_version.wrapping_add(1);
                true
            }
            WsEvent::ChatState(state) => {
                self.patch_chat_state(state);
                true
            }
            WsEvent::ChatNameUpdate { chat_id, name } => {
                if let Some(chat) = self.chats.iter_mut().find(|c| c.id == *chat_id) {
                    chat.name = name.clone();
                    self.chats_version = self.chats_version.wrapping_add(1);
                    true
                } else {
                    false
                }
            }
            WsEvent::Typing { chat_id, sender, sender_name, composing, audio } => {
                self.set_typing(chat_id, sender, sender_name.clone(), *composing, *audio);
                true
            }
            WsEvent::ChatPresence { chat_id, presence } => {
                self.set_presence(chat_id, presence);
                true
            }
            WsEvent::ChatUpdate { chat_id, unread, last_msg, disappearing_secs } => {
                if let Some(chat) = self.chats.iter_mut().find(|c| c.id == *chat_id) {
                    if let Some(unread) = unread {
                        chat.unread = *unread;
                        if *unread == 0 { chat.has_unread_mention = false; }
                    }
                    if let Some(last_msg) = last_msg {
                        chat.last_msg = last_msg.clone();
                    }
                    if let Some(secs) = disappearing_secs {
                        chat.disappearing_secs = *secs;
                    }
                    self.chats_version = self.chats_version.wrapping_add(1);
                    true
                } else {
                    false
                }
            }
            WsEvent::ChatDeleted { chat_id } => {
                let before = self.chats.len();
                self.chats.retain(|c| c.id != *chat_id);
                self.messages_by_chat.remove(chat_id);
                if self.active_chat_id.as_deref() == Some(chat_id.as_str()) {
                    self.active_chat_id = None;
                }
                self.chats_version = self.chats_version.wrapping_add(1);
                self.messages_version = self.messages_version.wrapping_add(1);
                self.chats.len() != before
            }
            WsEvent::ChatCleared { chat_id } => {
                if let Some(entry) = self.messages_by_chat.get_mut(chat_id) {
                    entry.messages.clear();
                    entry.has_more = false;
                    entry.has_more_next = false;
                    self.messages_version = self.messages_version.wrapping_add(1);
                }
                if let Some(chat) = self.chats.iter_mut().find(|c| c.id == *chat_id) {
                    chat.last_msg.clear();
                    chat.unread = 0;
                    chat.has_unread_mention = false;
                    self.chats_version = self.chats_version.wrapping_add(1);
                }
                true
            }
            WsEvent::ChatsChanged { reason } => {
                self.chats_loaded = false;
                self.chats_stale = true;
                if reason == "names" || reason == "history_sync" {
                    // Cached messages carry sender names from before the
                    // change; the open chat is refetched in place, the rest reload.
                    self.names_changed = true;
                    let active = self.active_chat_id.clone();
                    self.messages_by_chat.retain(|id, _| Some(id) == active.as_ref());
                    self.messages_version = self.messages_version.wrapping_add(1);
                }
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wabot_backend_client::dto::Message;

    fn make_test_msg(id: &str, timestamp: i64, content: &str, status: &str) -> Message {
        Message {
            id: id.to_string(),
            chat_id: "chat-1".to_string(),
            from: "user".to_string(),
            to: "peer".to_string(),
            content: content.to_string(),
            timestamp,
            status: status.to_string(),
            message_type: "text".to_string(),
            media_url: None,
            is_automatic: None,
            notification_suppressed: false,
            sender_name: None,
            reply_to_id: None,
            forwarded: None,
            reactions: None,
            extra: None,
            ..Default::default()
        }
    }

    #[test]
    fn typing_shows_who_and_expires() {
        let mut store = ChatStore::new();
        assert_eq!(store.typing_label("a@s.whatsapp.net", false), None);

        store.set_typing("a@s.whatsapp.net", "a@s.whatsapp.net", None, true, false);
        assert_eq!(store.typing_label("a@s.whatsapp.net", false).as_deref(), Some("typing…"));

        store.set_typing("g@g.us", "628@s.whatsapp.net", Some("Budi".into()), true, true);
        assert_eq!(store.typing_label("g@g.us", true).as_deref(), Some("Budi is recording audio…"));
        store.set_typing("g@g.us", "629@s.whatsapp.net", None, true, false);
        assert_eq!(store.typing_label("g@g.us", true).as_deref(), Some("Budi and 629 are recording audio…"));

        store.set_typing("g@g.us", "628@s.whatsapp.net", None, false, false);
        assert_eq!(store.typing_label("g@g.us", true).as_deref(), Some("629 is typing…"));

        // A renewal that never comes: the entry goes stale.
        store.typing.get_mut("a@s.whatsapp.net").unwrap().values_mut().for_each(|e| {
            e.at = std::time::Instant::now() - TYPING_TTL - std::time::Duration::from_secs(1);
        });
        assert_eq!(store.typing_label("a@s.whatsapp.net", false), None);
    }

    #[test]
    fn my_own_messages_never_count_as_unread() {
        let mut store = ChatStore::new();
        let mut chat = Chat {
            id: "chat-1".into(), name: "A".into(), avatar: String::new(), last_msg: String::new(),
            last_time: 0, unread: 0, has_unread_mention: false, is_active: true, is_group: false,
            archived: false, pinned_at: None, mute_mode: "off".into(), muted_until: None, disappearing_secs: 0,
        };
        store.set_chats(vec![chat.clone()]);

        let mine = make_test_msg("m1", 1, "hi", "sent");
        store.handle_ws_event_with_visibility(&WsEvent::NewMessage(Box::new(mine)), false);
        assert_eq!(store.chats[0].unread, 0, "an echo of my message is not unread");

        let theirs = make_test_msg("m2", 2, "yo", "received");
        store.handle_ws_event_with_visibility(&WsEvent::NewMessage(Box::new(theirs)), false);
        assert_eq!(store.chats[0].unread, 1);

        // A chat first seen through my own message starts read.
        chat.id = "chat-2".into();
        let mut first = make_test_msg("m3", 3, "hello", "sent");
        first.chat_id = "chat-2".into();
        store.handle_ws_event_with_visibility(&WsEvent::NewMessage(Box::new(first)), false);
        assert_eq!(store.chats.iter().find(|c| c.id == "chat-2").map(|c| c.unread), Some(0));
    }

    #[test]
    fn name_changes_ask_the_view_to_refetch_the_open_chat() {
        let mut store = ChatStore::new();
        store.active_chat_id = Some("chat-1".to_string());
        store.upsert_message("chat-1", make_test_msg("a", 1, "x", "delivered"));
        store.upsert_message("chat-2", make_test_msg("b", 1, "y", "delivered"));

        assert!(store.handle_ws_event(&WsEvent::ChatsChanged { reason: "names".to_string() }));

        assert!(store.chats_stale && store.names_changed);
        assert!(store.messages_by_chat.contains_key("chat-1"), "open chat is refetched in place");
        assert!(!store.messages_by_chat.contains_key("chat-2"), "other chats reload on open");
    }

    #[test]
    fn poll_update_replaces_the_message_extra() {
        let mut store = ChatStore::new();
        let mut poll = make_test_msg("p1", 100, "📊 Lunch?", "delivered");
        poll.message_type = "poll".to_string();
        store.upsert_message("chat-1", poll);

        // Shape of the backend's `poll_update` broadcast.
        let event = wabot_backend_client::ws::WsMessage {
            msg_type: "poll_update".to_string(),
            payload: serde_json::json!({
                "chatId": "chat-1",
                "id": "p1",
                "extra": {"poll": {
                    "question": "Lunch?",
                    "options": [{"name": "Nasi"}, {"name": "Mie"}],
                    "votes": {"628123@s.whatsapp.net": ["Nasi"]}
                }}
            }),
        }
        .to_event();
        assert!(store.handle_ws_event(&event));

        let stored = &store.messages_by_chat["chat-1"].messages[0];
        let poll = stored.extra.as_ref().and_then(|e| e.poll.as_ref()).expect("poll extra");
        assert_eq!(poll.votes.as_ref().map(|v| v.len()), Some(1));
    }

    #[test]
    fn test_chat_store_sort_asc() {
        let m1 = make_test_msg("m1", 200, "second", "delivered");
        let m2 = make_test_msg("m2", 100, "first", "delivered");
        let m3 = make_test_msg("m3", 300, "third", "delivered");

        let sorted = sort_asc(vec![m1, m2, m3]);
        assert_eq!(sorted[0].id, "m2");
        assert_eq!(sorted[1].id, "m1");
        assert_eq!(sorted[2].id, "m3");
    }

    #[test]
    fn test_chat_store_temp_message_replacement() {
        let mut store = ChatStore::new();
        let temp = make_test_msg("temp-12345", 1000, "Halo dunia", "pending");
        store.upsert_message("chat-1", temp);

        assert_eq!(store.messages_by_chat["chat-1"].messages.len(), 1);
        assert_eq!(store.messages_by_chat["chat-1"].messages[0].id, "temp-12345");
        assert_eq!(store.messages_by_chat["chat-1"].messages[0].status, "pending");

        // Backend echo arrives with real ID
        let real = make_test_msg("msg-real-999", 1001, "Halo dunia", "sent");
        store.upsert_message("chat-1", real);

        // Temp message must be replaced in place, no duplicate!
        assert_eq!(store.messages_by_chat["chat-1"].messages.len(), 1);
        assert_eq!(store.messages_by_chat["chat-1"].messages[0].id, "msg-real-999");
        assert_eq!(store.messages_by_chat["chat-1"].messages[0].status, "sent");
    }

    #[test]
    fn test_chat_store_upsert_moves_to_top_on_last_msg_change() {
        let mut store = ChatStore::new();
        let c1 = Chat {
            id: "chat-1".to_string(),
            name: "Alice".to_string(),
            avatar: String::new(),
            last_msg: "Hello".to_string(),
            last_time: 100,
            unread: 0,
            has_unread_mention: false,
            is_active: false,
            is_group: false,
            archived: false,
            pinned_at: None,
            mute_mode: "off".to_string(),
            muted_until: None,
            disappearing_secs: 0,
        };
        let c2 = Chat {
            id: "chat-2".to_string(),
            name: "Bob".to_string(),
            avatar: String::new(),
            last_msg: "Hey".to_string(),
            last_time: 200,
            unread: 0,
            has_unread_mention: false,
            is_active: false,
            is_group: false,
            archived: false,
            pinned_at: None,
            mute_mode: "off".to_string(),
            muted_until: None,
            disappearing_secs: 0,
        };
        store.set_chats(vec![c2, c1]);
        assert_eq!(store.chats[0].id, "chat-2");
        assert_eq!(store.chats[1].id, "chat-1");

        // Now Alice sends a new message -> moves to top
        let c1_updated = Chat {
            id: "chat-1".to_string(),
            name: "Alice".to_string(),
            avatar: String::new(),
            last_msg: "New message from Alice".to_string(),
            last_time: 300,
            unread: 1,
            has_unread_mention: false,
            is_active: false,
            is_group: false,
            archived: false,
            pinned_at: None,
            mute_mode: "off".to_string(),
            muted_until: None,
            disappearing_secs: 0,
        };
        store.upsert_chat(c1_updated);
        assert_eq!(store.chats[0].id, "chat-1");
        assert_eq!(store.chats[1].id, "chat-2");
        assert_eq!(store.chats[0].unread, 1);
    }

    fn store_with_chat(id: &str, unread: u32) -> ChatStore {
        let mut store = ChatStore::default();
        let chat: Chat = serde_json::from_value(serde_json::json!({
            "id": id, "name": "n", "lastMsg": "hi", "unread": unread
        }))
        .unwrap();
        store.set_chats(vec![chat]);
        store
    }

    fn mention_message(id: &str, mentions_me: bool) -> WsEvent {
        let message: Message = serde_json::from_value(serde_json::json!({
            "id": id, "chatId": "group@g.us", "from": "peer", "status": "received",
            "extra": {"mentions": ["123@lid"], "mentionsMe": mentions_me}
        })).unwrap();
        WsEvent::NewMessage(Box::new(message))
    }

    #[test]
    fn unread_mention_survives_newer_messages_and_clears_on_read() {
        let mut store = ChatStore::new();
        store.handle_ws_event(&mention_message("mention", true));
        assert!(store.chats[0].has_unread_mention);
        store.handle_ws_event(&mention_message("other-member", false));
        assert!(store.chats[0].has_unread_mention);
        store.mark_chat_read("group@g.us");
        assert!(!store.chats[0].has_unread_mention);
        store.handle_ws_event(&mention_message("plain-after-read", false));
        assert!(!store.chats[0].has_unread_mention);
        store.handle_ws_event(&mention_message("new-mention", true));
        store.handle_ws_event(&WsEvent::ChatUpdate { chat_id: "group@g.us".into(), unread: Some(0), last_msg: None, disappearing_secs: None });
        assert!(!store.chats[0].has_unread_mention);
    }

    #[test]
    fn mentions_in_the_open_chat_do_not_raise_a_sidebar_badge() {
        let mut store = ChatStore::new();
        store.active_chat_id = Some("group@g.us".into());
        store.handle_ws_event(&mention_message("mention", true));
        assert!(!store.chats[0].has_unread_mention);
        assert_eq!(store.chats[0].unread, 0);
    }

    #[test]
    fn mentions_in_a_hidden_conversation_remain_unread() {
        let mut store = ChatStore::new();
        store.active_chat_id = Some("group@g.us".into());
        store.handle_ws_event_with_visibility(&mention_message("mention", true), false);
        assert!(store.chats[0].has_unread_mention);
        assert_eq!(store.chats[0].unread, 1);
    }

    #[test]
    fn unread_mention_is_restored_from_chat_list_and_cleared_with_chat() {
        let chat: Chat = serde_json::from_value(serde_json::json!({
            "id": "group@g.us", "unread": 3, "hasUnreadMention": true
        })).unwrap();
        let mut store = ChatStore::new();
        store.set_chats(vec![chat]);
        assert!(store.chats[0].has_unread_mention);
        store.handle_ws_event(&WsEvent::ChatCleared { chat_id: "group@g.us".into() });
        assert!(!store.chats[0].has_unread_mention);
    }

    #[test]
    fn test_ws_chat_update_patches_only_given_fields() {
        let mut store = store_with_chat("a@s.whatsapp.net", 3);
        store.handle_ws_event(&WsEvent::ChatUpdate {
            chat_id: "a@s.whatsapp.net".into(),
            unread: Some(0),
            last_msg: None,
            disappearing_secs: None,
        });
        assert_eq!(store.chats[0].unread, 0);
        assert_eq!(store.chats[0].last_msg, "hi");

        store.handle_ws_event(&WsEvent::ChatUpdate {
            chat_id: "a@s.whatsapp.net".into(),
            unread: None,
            last_msg: Some("bye".into()),
            disappearing_secs: None,
        });
        assert_eq!(store.chats[0].unread, 0);
        assert_eq!(store.chats[0].last_msg, "bye");
    }

    #[test]
    fn test_ws_chat_deleted_and_cleared() {
        let mut store = store_with_chat("a@s.whatsapp.net", 2);
        store.upsert_message("a@s.whatsapp.net", make_test_msg("m1", 1, "x", "received"));

        store.handle_ws_event(&WsEvent::ChatCleared { chat_id: "a@s.whatsapp.net".into() });
        assert!(store.messages_by_chat["a@s.whatsapp.net"].messages.is_empty());
        assert_eq!((store.chats[0].unread, store.chats[0].last_msg.as_str()), (0, ""));

        store.handle_ws_event(&WsEvent::ChatDeleted { chat_id: "a@s.whatsapp.net".into() });
        assert!(store.chats.is_empty());
        assert!(!store.messages_by_chat.contains_key("a@s.whatsapp.net"));
    }

    #[test]
    fn test_chat_store_invalidate_messages() {
        let mut store = ChatStore::new();
        let m = make_test_msg("m1", 100, "hello", "sent");
        store.set_messages("chat-1", vec![m], false);
        assert!(store.messages_by_chat["chat-1"].loaded);

        store.invalidate_messages();
        assert!(!store.messages_by_chat["chat-1"].loaded);
    }

    #[test]
    fn starring_patches_the_message_and_signals_open_lists() {
        let mut store = ChatStore::new();
        store.upsert_message("chat-1", make_test_msg("m1", 1, "x", "received"));
        let version = store.messages_version;
        let event = WsEvent::MessageStarred { chat_id: "chat-1".into(), id: "m1".into(), starred: true };
        assert!(store.handle_ws_event(&event));
        assert!(store.messages_by_chat["chat-1"].messages[0].starred);
        assert_ne!(store.messages_version, version, "bubbles must repaint");
        assert_eq!(store.starred_version, 1);

        store.handle_ws_event(&WsEvent::MessageStarred { chat_id: "chat-1".into(), id: "m1".into(), starred: false });
        assert!(!store.messages_by_chat["chat-1"].messages[0].starred);
    }

    #[test]
    fn pin_changes_are_queued_once_per_chat() {
        let mut store = ChatStore::new();
        store.handle_ws_event(&WsEvent::ChatPinsChanged { chat_id: "g@g.us".into() });
        store.handle_ws_event(&WsEvent::ChatPinsChanged { chat_id: "g@g.us".into() });
        assert_eq!(store.take_stale_pins(), vec!["g@g.us".to_string()]);
        assert!(store.take_stale_pins().is_empty());
    }

    #[test]
    fn chat_update_carries_the_disappearing_timer() {
        let mut store = store_with_chat("a@s.whatsapp.net", 0);
        assert_eq!(store.chats[0].disappearing_secs, 0);
        store.handle_ws_event(&WsEvent::ChatUpdate {
            chat_id: "a@s.whatsapp.net".into(),
            unread: None,
            last_msg: None,
            disappearing_secs: Some(86_400),
        });
        assert_eq!(store.chats[0].disappearing_secs, 86_400);
        // Other updates leave it alone.
        store.handle_ws_event(&WsEvent::ChatUpdate {
            chat_id: "a@s.whatsapp.net".into(),
            unread: Some(2),
            last_msg: None,
            disappearing_secs: None,
        });
        assert_eq!(store.chats[0].disappearing_secs, 86_400);
        store.set_disappearing("a@s.whatsapp.net", 0);
        assert_eq!(store.chats[0].disappearing_secs, 0);
    }
}

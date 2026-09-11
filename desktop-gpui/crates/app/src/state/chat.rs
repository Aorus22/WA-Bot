use std::collections::HashMap;
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

#[derive(Debug, Clone, Default)]
pub struct ChatStore {
    pub chats: Vec<Chat>,
    pub chats_loaded: bool,
    pub chats_loading: bool,
    pub chats_version: u64,
    pub messages_by_chat: HashMap<String, ChatMessagesEntry>,
    pub active_chat_id: Option<String>,
    pub peer_presence: HashMap<String, String>, // chat_id -> presence string (e.g. "available", "composing")
    /// Bumped on every message mutation (status, reaction, new/deleted/edited
    /// message) so views can rebuild their render caches even when the chat
    /// id is unchanged.
    pub messages_version: u64,
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
                is_active: chat.is_active,
                is_group: chat.is_group || existing.is_group,
                archived: chat.archived,
                pinned_at: chat.pinned_at.or(existing.pinned_at),
                mute_mode: chat.mute_mode,
                muted_until: chat.muted_until.or(existing.muted_until),
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

    pub fn get_presence(&self, chat_id: &str) -> Option<&String> {
        self.peer_presence.get(chat_id)
    }

    pub fn mark_chat_read(&mut self, chat_id: &str) {
        if let Some(chat) = self.chats.iter_mut().find(|c| c.id == chat_id) {
            chat.unread = 0;
            self.chats_version = self.chats_version.wrapping_add(1);
        }
    }

    pub fn handle_ws_event(&mut self, event: &WsEvent) -> bool {
        match event {
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
            WsEvent::MessageEdited { chat_id, id, content } => {
                self.patch_message(chat_id, id, |m| m.content = content.clone());
                true
            }
            WsEvent::MessageReaction { chat_id, id, reactions } => {
                self.patch_message(chat_id, id, |m| m.reactions = Some(reactions.clone()));
                true
            }
            WsEvent::NewMessage(msg) => {
                let chat_id = msg.chat_id.clone();
                self.upsert_message(&chat_id, *msg.clone());

                if let Some(pos) = self.chats.iter().position(|c| c.id == chat_id) {
                    let mut existing = self.chats.remove(pos);
                    existing.last_msg = msg.content.clone();
                    existing.last_time = msg.timestamp;
                    if self.active_chat_id.as_deref() != Some(&chat_id) {
                        existing.unread += 1;
                    }
                    self.chats.insert(0, existing);
                } else {
                    let new_chat = Chat {
                        id: chat_id.clone(),
                        name: msg.sender_name.clone().unwrap_or_else(|| chat_id.clone()),
                        avatar: String::new(),
                        last_msg: msg.content.clone(),
                        last_time: msg.timestamp,
                        unread: if self.active_chat_id.as_deref() == Some(&chat_id) { 0 } else { 1 },
                        is_active: true,
                        is_group: chat_id.contains("@g.us"),
                        archived: false,
                        pinned_at: None,
                        mute_mode: "off".to_string(),
                        muted_until: None,
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
            WsEvent::ChatPresence { chat_id, presence } => {
                self.set_presence(chat_id, presence);
                true
            }
            WsEvent::ChatsChanged { .. } => {
                self.chats_loaded = false;
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
            sender_name: None,
            reply_to_id: None,
            forwarded: None,
            reactions: None,
            extra: None,
        }
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
            is_active: false,
            is_group: false,
            archived: false,
            pinned_at: None,
            mute_mode: "off".to_string(),
            muted_until: None,
        };
        let c2 = Chat {
            id: "chat-2".to_string(),
            name: "Bob".to_string(),
            avatar: String::new(),
            last_msg: "Hey".to_string(),
            last_time: 200,
            unread: 0,
            is_active: false,
            is_group: false,
            archived: false,
            pinned_at: None,
            mute_mode: "off".to_string(),
            muted_until: None,
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
            is_active: false,
            is_group: false,
            archived: false,
            pinned_at: None,
            mute_mode: "off".to_string(),
            muted_until: None,
        };
        store.upsert_chat(c1_updated);
        assert_eq!(store.chats[0].id, "chat-1");
        assert_eq!(store.chats[1].id, "chat-2");
        assert_eq!(store.chats[0].unread, 1);
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
}

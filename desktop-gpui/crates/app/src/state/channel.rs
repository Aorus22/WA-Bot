use std::collections::HashMap;
use wabot_backend_client::dto::{Channel, ChannelMessage};

#[derive(Clone, Debug, Default)]
pub struct ChannelStore {
    pub channels: Vec<Channel>,
    pub active_channel_jid: Option<String>,
    pub messages_by_channel: HashMap<String, Vec<ChannelMessage>>,
}

impl ChannelStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_channels(&mut self, channels: Vec<Channel>) {
        self.channels = channels;
    }

    pub fn is_following(&self, jid: &str) -> bool {
        self.channels.iter().any(|c| c.jid == jid)
    }

    pub fn follow(&mut self, channel: Channel) {
        if !self.is_following(&channel.jid) {
            self.channels.push(channel);
        }
    }

    pub fn unfollow(&mut self, jid: &str) {
        self.channels.retain(|c| c.jid != jid);
        if self.active_channel_jid.as_deref() == Some(jid) {
            self.active_channel_jid = None;
        }
    }

    pub fn toggle_mute(&mut self, jid: &str) {
        if let Some(c) = self.channels.iter_mut().find(|c| c.jid == jid) {
            c.muted = !c.muted;
        }
    }

    pub fn set_messages(&mut self, jid: &str, mut messages: Vec<ChannelMessage>) {
        messages.sort_by_key(|m| m.timestamp);
        self.messages_by_channel.insert(jid.to_string(), messages);
    }

    pub fn append_message(&mut self, jid: &str, msg: ChannelMessage) {
        let msgs = self.messages_by_channel.entry(jid.to_string()).or_default();
        if let Some(idx) = msgs.iter().position(|m| m.id == msg.id) {
            msgs[idx] = msg;
        } else {
            msgs.push(msg);
            msgs.sort_by_key(|m| m.timestamp);
        }
    }

    pub fn toggle_reaction(&mut self, jid: &str, msg_id: &str, emoji: &str) {
        if let Some(msgs) = self.messages_by_channel.get_mut(jid) {
            if let Some(msg) = msgs.iter_mut().find(|m| m.id == msg_id) {
                let reactions = msg.reactions.get_or_insert_with(HashMap::new);
                let count = reactions.entry(emoji.to_string()).or_insert(0);
                *count += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_channel(jid: &str, name: &str) -> Channel {
        Channel {
            jid: jid.to_string(),
            name: name.to_string(),
            description: Some("Description".to_string()),
            subscribers: 1000,
            invite_code: Some("abc".to_string()),
            avatar: None,
            muted: false,
            verified: true,
        }
    }

    #[test]
    fn test_channel_store_follow_and_mute() {
        let mut store = ChannelStore::new();
        let ch = make_channel("ch-1", "Tech News");

        assert!(!store.is_following("ch-1"));
        store.follow(ch);
        assert!(store.is_following("ch-1"));
        assert!(!store.channels[0].muted);

        store.toggle_mute("ch-1");
        assert!(store.channels[0].muted);

        store.unfollow("ch-1");
        assert!(!store.is_following("ch-1"));
    }

    #[test]
    fn test_channel_messages_and_reactions() {
        let mut store = ChannelStore::new();
        let msg = ChannelMessage {
            id: "cm-1".to_string(),
            server_id: 1,
            message_type: "text".to_string(),
            timestamp: 1000,
            views_count: 50,
            reactions: None,
            content: Some("Breaking update".to_string()),
            media_type: None,
        };

        store.append_message("ch-1", msg);
        assert_eq!(store.messages_by_channel["ch-1"].len(), 1);

        store.toggle_reaction("ch-1", "cm-1", "🔥");
        let reactions = store.messages_by_channel["ch-1"][0].reactions.as_ref().unwrap();
        assert_eq!(reactions["🔥"], 1);
    }
}

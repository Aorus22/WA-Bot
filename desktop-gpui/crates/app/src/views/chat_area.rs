use wabot_backend_client::dto::{Chat, Message};
use crate::components::dialogs::forward::ForwardDialog;
use crate::components::emoji_picker::EmojiPickerState;

#[derive(Clone)]
pub struct ChatAreaState {
    pub active_chat: Option<Chat>,
    pub compose_text: String,
    pub is_markdown_mode: bool,
    pub reply_to: Option<Message>,
    pub editing_message: Option<Message>,
    pub emoji_picker: EmojiPickerState,
    pub forward_dialog: ForwardDialog,
    pub in_chat_search: Option<String>,
}

impl ChatAreaState {
    pub fn new() -> Self {
        Self {
            active_chat: None,
            compose_text: String::new(),
            is_markdown_mode: false,
            reply_to: None,
            editing_message: None,
            emoji_picker: EmojiPickerState::new(),
            forward_dialog: ForwardDialog::new(),
            in_chat_search: None,
        }
    }

    pub fn set_active_chat(&mut self, chat: Chat) {
        self.active_chat = Some(chat);
        self.reply_to = None;
        self.editing_message = None;
        self.in_chat_search = None;
    }

    pub fn create_optimistic_message(&self, content: &str, current_user: &str) -> Option<Message> {
        let chat = self.active_chat.as_ref()?;
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        let final_content = if self.is_markdown_mode {
            let encoded = base64_encode(content.as_bytes());
            format!("{{{{md:{}}}}}", encoded)
        } else {
            content.to_string()
        };

        Some(Message {
            id: format!("temp-{}", now_ms),
            chat_id: chat.id.clone(),
            from: current_user.to_string(),
            to: chat.id.clone(),
            content: final_content,
            timestamp: now_ms,
            status: "pending".to_string(),
            message_type: "text".to_string(),
            media_url: None,
            is_automatic: None,
            sender_name: Some("You".to_string()),
            reply_to_id: self.reply_to.as_ref().map(|r| r.id.clone()),
            forwarded: None,
            reactions: None,
            extra: None,
        })
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    const B64_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let b0 = bytes[i];
        let b1 = if i + 1 < bytes.len() { bytes[i + 1] } else { 0 };
        let b2 = if i + 2 < bytes.len() { bytes[i + 2] } else { 0 };

        out.push(B64_TABLE[(b0 >> 2) as usize] as char);
        out.push(B64_TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if i + 1 < bytes.len() {
            out.push(B64_TABLE[(((b1 & 0x0F) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if i + 2 < bytes.len() {
            out.push(B64_TABLE[(b2 & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        i += 3;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::message_bubble::MessageBubbleHelper;

    #[test]
    fn test_optimistic_message_creation() {
        let mut state = ChatAreaState::new();
        let chat = Chat {
            id: "chat-123".to_string(),
            name: "Partner".to_string(),
            avatar: String::new(),
            last_msg: String::new(),
            last_time: 0,
            unread: 0,
            is_active: true,
            is_group: false,
            archived: false,
            pinned_at: None,
            mute_mode: "off".to_string(),
            muted_until: None,
        };
        state.set_active_chat(chat);

        let opt_msg = state.create_optimistic_message("Hello there!", "me").unwrap();
        assert!(opt_msg.id.starts_with("temp-"));
        assert_eq!(opt_msg.content, "Hello there!");
        assert_eq!(opt_msg.status, "pending");
        assert_eq!(opt_msg.chat_id, "chat-123");

        // Now with markdown mode enabled
        state.is_markdown_mode = true;
        let md_opt_msg = state.create_optimistic_message("Hello *world*", "me").unwrap();
        assert!(md_opt_msg.content.starts_with("{{md:"));
        assert!(md_opt_msg.content.ends_with("}}"));
        assert_eq!(MessageBubbleHelper::decode_content(&md_opt_msg.content), "Hello *world*");
    }
}

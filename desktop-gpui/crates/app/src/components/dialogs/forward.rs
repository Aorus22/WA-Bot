use wabot_backend_client::dto::{Chat, Message};

#[derive(Clone, Default)]
pub struct ForwardDialog {
    pub is_open: bool,
    pub message: Option<Message>,
    pub selected_chat_ids: Vec<String>,
    pub search_query: String,
}

impl ForwardDialog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(&mut self, message: Message) {
        self.is_open = true;
        self.message = Some(message);
        self.selected_chat_ids.clear();
        self.search_query.clear();
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.message = None;
        self.selected_chat_ids.clear();
    }

    pub fn toggle_chat(&mut self, chat_id: &str) {
        if let Some(pos) = self.selected_chat_ids.iter().position(|id| id == chat_id) {
            self.selected_chat_ids.remove(pos);
        } else {
            self.selected_chat_ids.push(chat_id.to_string());
        }
    }

    pub fn is_selected(&self, chat_id: &str) -> bool {
        self.selected_chat_ids.iter().any(|id| id == chat_id)
    }

    pub fn filtered_chats<'a>(&self, all_chats: &'a [Chat]) -> Vec<&'a Chat> {
        let q = self.search_query.trim().to_lowercase();
        all_chats
            .iter()
            .filter(|c| {
                if q.is_empty() {
                    true
                } else {
                    c.name.to_lowercase().contains(&q) || c.id.to_lowercase().contains(&q)
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forward_dialog_selection() {
        let mut dlg = ForwardDialog::new();
        dlg.toggle_chat("chat-1");
        assert!(dlg.is_selected("chat-1"));
        assert_eq!(dlg.selected_chat_ids.len(), 1);

        dlg.toggle_chat("chat-2");
        assert_eq!(dlg.selected_chat_ids.len(), 2);

        dlg.toggle_chat("chat-1");
        assert!(!dlg.is_selected("chat-1"));
        assert_eq!(dlg.selected_chat_ids.len(), 1);
    }
}

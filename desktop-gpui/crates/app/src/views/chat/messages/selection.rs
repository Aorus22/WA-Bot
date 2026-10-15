use crate::views::chat::*;

impl ChatView {
    /// Start select-messages mode, optionally with one message already ticked.
    pub fn enter_select_mode(&mut self, first: Option<String>, cx: &mut Context<Self>) {
        self.header_menu = None;
        self.context_menu = None;
        self.select_mode = true;
        self.select_confirm_delete = false;
        self.selected_messages.clear();
        if let Some(id) = first {
            self.selected_messages.insert(id);
        }
        self.remeasure_messages();
        cx.notify();
    }

    pub fn exit_select_mode(&mut self, cx: &mut Context<Self>) {
        if !self.select_mode {
            return;
        }
        self.select_mode = false;
        self.select_confirm_delete = false;
        self.selected_messages.clear();
        self.remeasure_messages();
        cx.notify();
    }

    pub fn toggle_select(&mut self, id: &str, cx: &mut Context<Self>) {
        if !self.selected_messages.remove(id) {
            self.selected_messages.insert(id.to_string());
        }
        self.select_confirm_delete = false;
        cx.notify();
    }

    /// Rows gain or lose the checkbox gutter, so their heights change.
    fn remeasure_messages(&mut self) {
        let count = self.messages_list_state.item_count();
        if count > 0 {
            self.messages_list_state.remeasure_items(0..count);
        }
    }

    /// The ticked messages, oldest first.
    pub(crate) fn selected_in_order(&self) -> Vec<Message> {
        self.cached_render_messages
            .iter()
            .filter(|m| self.selected_messages.contains(&m.msg.id))
            .map(|m| m.msg.clone())
            .collect()
    }

    /// Star them all, or unstar them all when they already are.
    pub fn select_star(&mut self, cx: &mut Context<Self>) {
        let msgs = self.selected_in_order();
        if msgs.is_empty() {
            return;
        }
        let all_starred = msgs.iter().all(|m| m.starred);
        for msg in msgs.into_iter().filter(|m| m.starred == all_starred) {
            self.toggle_star(msg, cx);
        }
        self.exit_select_mode(cx);
    }

    pub fn select_copy(&mut self, cx: &mut Context<Self>) {
        let msgs = self.selected_in_order();
        if msgs.is_empty() {
            return;
        }
        let text = super::super::chat_actions::message_lines(&msgs);
        cx.write_to_clipboard(ClipboardItem::new_string(text.trim_end().to_string()));
        toast::success("Pesan disalin ke clipboard", cx);
        self.exit_select_mode(cx);
    }

    pub fn select_forward(&mut self, cx: &mut Context<Self>) {
        let msgs = self.selected_in_order();
        let Some(chat_id) = self.selected_chat_id.clone() else { return };
        if msgs.is_empty() {
            return;
        }
        self.open_forward_many(chat_id, msgs, cx);
    }

    /// Whether every ticked message is ours and still standing.
    pub(crate) fn selection_deletable(&self) -> bool {
        let msgs = self.selected_in_order();
        !msgs.is_empty() && msgs.iter().all(|m| MessageBubbleHelper::is_from_me(m, "me") && m.revoked_at.is_none())
    }

    /// Delete for everyone; the first click arms it.
    pub fn select_delete(&mut self, cx: &mut Context<Self>) {
        if !self.selection_deletable() {
            return;
        }
        if !self.select_confirm_delete {
            self.select_confirm_delete = true;
            cx.notify();
            return;
        }
        let Some(chat_id) = self.selected_chat_id.clone() else { return };
        for msg in self.selected_in_order() {
            self.delete_message(chat_id.clone(), msg.id, cx);
        }
        self.exit_select_mode(cx);
    }
}

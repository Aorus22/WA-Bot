use crate::views::chat::*;

impl ChatView {
    /// Start select-messages mode, optionally with one message already ticked.
    pub fn enter_select_mode(&mut self, first: Option<String>, cx: &mut Context<Self>) {
        self.header_menu = None;
        self.context_menu = None;
        self.select_mode = true;
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
        self.selected_messages.clear();
        self.remeasure_messages();
        cx.notify();
    }

    pub fn toggle_select(&mut self, id: &str, cx: &mut Context<Self>) {
        if !self.selected_messages.remove(id) {
            self.selected_messages.insert(id.to_string());
        }
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

    /// Whether at least one message is selected for deletion.
    pub(crate) fn selection_deletable(&self) -> bool {
        !self.selected_in_order().is_empty()
    }

    /// Open the delete choice dialog for selected messages.
    pub fn select_delete(&mut self, cx: &mut Context<Self>) {
        if !self.selection_deletable() {
            return;
        }
        let Some(chat_id) = self.selected_chat_id.clone() else { return };
        self.open_delete_message_dialog(chat_id, self.selected_in_order(), cx);
    }
}

use crate::views::chat::*;

impl ChatView {
    pub fn enter_chat_select_mode(&mut self, cx: &mut Context<Self>) {
        self.header_menu = None;
        self.chat_context_menu = None;
        self.chat_select_mode = true;
        self.selected_chats.clear();
        cx.notify();
    }

    pub fn exit_chat_select_mode(&mut self, cx: &mut Context<Self>) {
        self.chat_select_mode = false;
        self.selected_chats.clear();
        cx.notify();
    }

    pub fn toggle_chat_select(&mut self, id: &str, cx: &mut Context<Self>) {
        if !self.selected_chats.remove(id) {
            self.selected_chats.insert(id.to_string());
        }
        cx.notify();
    }

    fn selected_chat_rows(&self, cx: &App) -> Vec<Chat> {
        if !cx.has_global::<ChatStore>() {
            return Vec::new();
        }
        ChatStore::global(cx)
            .chats
            .iter()
            .filter(|c| self.selected_chats.contains(&c.id))
            .cloned()
            .collect()
    }

    pub fn chats_mark_read(&mut self, cx: &mut Context<Self>) {
        for chat in self.selected_chat_rows(cx).into_iter().filter(|c| c.unread > 0) {
            self.mark_chat_read_action(chat.id, cx);
        }
        self.exit_chat_select_mode(cx);
    }

    /// Archive them all, or unarchive them all when they already are.
    pub fn chats_archive(&mut self, cx: &mut Context<Self>) {
        let chats = self.selected_chat_rows(cx);
        let all_archived = !chats.is_empty() && chats.iter().all(|c| c.archived);
        for chat in chats.into_iter().filter(|c| c.archived == all_archived) {
            self.toggle_archive(chat.id, chat.archived, cx);
        }
        self.exit_chat_select_mode(cx);
    }

    pub fn chats_pin(&mut self, cx: &mut Context<Self>) {
        let chats = self.selected_chat_rows(cx);
        let all_pinned = !chats.is_empty() && chats.iter().all(|c| c.pinned_at.is_some());
        for chat in chats.into_iter().filter(|c| c.pinned_at.is_some() == all_pinned) {
            let pinned = chat.pinned_at.is_some();
            self.toggle_pin(chat.id, pinned, cx);
        }
        self.exit_chat_select_mode(cx);
    }

    /// Mute them for good, or unmute them all when they already are muted.
    pub fn chats_mute(&mut self, cx: &mut Context<Self>) {
        let chats = self.selected_chat_rows(cx);
        let muted = |c: &Chat| !matches!(c.mute_mode.as_str(), "" | "off");
        let all_muted = !chats.is_empty() && chats.iter().all(|c| muted(c));
        for chat in chats {
            self.set_mute(chat.id, if all_muted { "off" } else { "forever" }.to_string(), cx);
        }
        self.exit_chat_select_mode(cx);
    }

    /// Replaces the list header while chats are being selected.
    pub(crate) fn render_chat_selection_bar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = *cx.app_theme();
        let muted_text = theme.muted_foreground;
        let count = self.selected_chats.len();
        let action = |id: &'static str, icon: &'static [u8], tip: &'static str| {
            div()
                .id(id)
                .p_2()
                .rounded_full()
                .when(count > 0, |el| {
                    el.cursor_pointer().hover({
                        let hover_bg = theme.muted.opacity(0.65);
                        move |s| s.bg(hover_bg)
                    })
                })
                .when(count == 0, |el| el.opacity(0.35))
                .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
                .child(svg().data(icon).size(px(18.0)).text_color(muted_text))
        };
        h_flex()
            .px_4()
            .pt_4()
            .pb_2()
            .gap_1()
            .items_center()
            .child(
                div()
                    .id("btn-chat-select-cancel")
                    .p_2()
                    .rounded_full()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.muted.opacity(0.65)))
                    .tooltip(move |window, cx| Tooltip::new("Cancel").build(window, cx))
                    .child(svg().data(X_SVG).size(px(18.0)).text_color(muted_text))
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.exit_chat_select_mode(cx))),
            )
            .child(
                div()
                    .flex_1()
                    .text_base()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.foreground)
                    .child(format!("{count} selected")),
            )
            .child(action("btn-chats-read", CHECK_CHECK_SVG, "Mark as read").on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.chats_mark_read(cx)),
            ))
            .child(action("btn-chats-archive", ARCHIVE_SVG, "Archive").on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.chats_archive(cx)),
            ))
            .child(action("btn-chats-pin", PIN_SVG, "Pin").on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.chats_pin(cx)),
            ))
            .child(action("btn-chats-mute", VOLUME_X_SVG, "Mute").on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.chats_mute(cx)),
            ))
            .into_any_element()
    }
}

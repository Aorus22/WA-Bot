use crate::views::chat::*;

impl ChatView {
    /// Right pane: the open conversation (header, messages, composer) or the empty state.
    pub(crate) fn render_conversation_pane(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        selected_chat: &Option<Chat>,
        pages: bool,
        show_right: bool,
    ) -> impl IntoElement {
        let theme = *cx.app_theme();
        let bg_color = theme.background;
        let framed = !crate::popover::is_popover(cx)
            && window_material::is_framed_window(window);
        let window_transparency = ThemeManager::global(cx).window_transparency;
        v_flex()
            .flex_1()
            .min_w(px(0.0))
            .h_full()
            .overflow_hidden()
            .when(!show_right, |el| el.hidden())
            .bg(window_material::glass_fill(
                theme.chat.map(|c| c.chat_bg).unwrap_or(bg_color),
                window_transparency,
            ))
            .when(framed, |s| s.rounded_br(px(window_material::WINDOW_CORNER_RADIUS)))
            .children(if let Some(ref active_chat) = selected_chat {
                let chat_jid = active_chat.id.clone();

                Some(
                    v_flex()
                        .relative()
                        .size_full()
                        // Chat Header (Clicking opens Info Sheet)
                        .child(self.render_chat_header(cx, pages, active_chat))
                        // Messages History View (Optimized for 60fps scrolling) with visual scrollbar
                        .child(self.render_message_list(cx))
                        .children(self.render_typing_indicator(cx))
                        // Bottom Compose Container (Banner + Input Field + Integrated Send Icon)
                        .child(self.render_composer(window, cx))
                        // Opening a conversation slides it in (a push, in pages
                        // layout) while fading; keyed by chat so reselecting the
                        // same one doesn't replay it.
                        .with_animation(
                            SharedString::from(format!("chat-pane-{chat_jid}")),
                            crate::components::motion::enter(260),
                            move |el, d| {
                                let travel = if pages { 32.0 } else { 14.0 };
                                el.opacity(d).left(px(travel * (1.0 - d)))
                            },
                        )
                        .into_any_element(),
                )
            } else {
                // Empty state when no chat selected
                Some(
                    self.render_empty_state(cx)
                )
            })
    }
}

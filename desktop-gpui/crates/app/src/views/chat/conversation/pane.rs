use crate::views::chat::*;
use crate::theme::surface::SurfaceExt;

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
        let surface = cx.surface();
        let framed = !crate::popover::is_popover(cx)
            && window_material::is_framed_window(window);
        v_flex()
            .flex_1()
            .min_w(px(0.0))
            .h_full()
            .overflow_hidden()
            .when(!show_right, |el| el.hidden())
            .bg(surface.conversation_background)
            .when(framed, |s| {
                s.rounded_tr(px(window_material::corner_radius(cx)))
                    .rounded_br(px(window_material::corner_radius(cx)))
            })
            .children(if let Some(ref active_chat) = selected_chat {
                let chat_jid = active_chat.id.clone();

                Some(
                    v_flex()
                        .relative()
                        .size_full()
                        // Chat Header (Clicking opens Info Sheet)
                        .child(if self.select_mode {
                            self.render_selection_bar(cx)
                        } else {
                            self.render_chat_header(window, cx, pages, active_chat).into_any_element()
                        })
                        // Messages pinned in this chat
                        .children(self.render_pinned_banner(cx))
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

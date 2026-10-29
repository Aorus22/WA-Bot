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
        let theme = *cx.app_theme();
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
                let chat_name = if active_chat.name.trim().is_empty() {
                    active_chat.id.clone()
                } else {
                    active_chat.name.clone()
                };

                Some(
                    v_flex()
                        .relative()
                        .size_full()
                        .on_drop(cx.listener(
                            |this, paths: &ExternalPaths, _, cx| {
                                this.handle_dropped_files(paths, cx);
                            },
                        ))
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
                        // Attachment review covers only the conversation area, like WhatsApp.
                        .children(self.render_attachment_preview(window, active_chat, cx))
                        // Show an explicit target state while an OS file drag is over this chat.
                        .child(
                            div()
                                .id("chat-file-drop-overlay")
                                .absolute()
                                .inset_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(surface.conversation_background.opacity(0.82))
                                .opacity(0.0)
                                .drag_over::<ExternalPaths>(|style, _, _, _| style.opacity(1.0))
                                .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                                    this.handle_dropped_files(paths, cx);
                                }))
                                .child(
                                    v_flex()
                                        .items_center()
                                        .gap_3()
                                        .px_8()
                                        .py_7()
                                        .rounded_2xl()
                                        .border_2()
                                        .border_color(theme.primary.opacity(0.65))
                                        .bg(surface.headerbar)
                                        .child(
                                            svg()
                                                .data(PAPERCLIP_SVG)
                                                .size(px(32.0))
                                                .text_color(theme.primary),
                                        )
                                        .child(
                                            div()
                                                .text_lg()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(theme.foreground)
                                                .child("Drop files to attach"),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(theme.muted_foreground)
                                                .child(format!("Send to {chat_name}")),
                                        ),
                                ),
                        )
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

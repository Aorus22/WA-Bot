use crate::views::chat::*;

impl ChatView {
    /// Virtualised message history with day separators and the scroll-to-bottom button.
    pub(crate) fn render_message_list(
        &mut self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = *cx.app_theme();
        let border_color = theme.border;
        let text_color = theme.foreground;
        let muted_text = theme.muted_foreground;
        let is_loading_messages = self.is_loading_messages;
        // Row heights are cached; a text size change makes them stale, so
        // re-measure every row (the splice keeps the scroll anchor).
        let scales = {
            let mgr = ThemeManager::global(cx);
            (mgr.ui_font_scale, mgr.chat_font_scale)
        };
        if scales != self.measured_text_scale {
            self.measured_text_scale = scales;
            let count = self.messages_list_state.item_count();
            if count > 0 {
                self.messages_list_state.splice(0..count, count);
            }
        }
        div()
            .flex_1()
            .relative()
            .overflow_hidden()
            .child(if is_loading_messages && self.cached_render_messages.is_empty() {
                v_flex()
                    .size_full()
                    .p_6()
                    .child(
                        v_flex()
                            .py_16()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .child(crate::components::motion::spinning(Icon::new(IconName::LoaderCircle).size(px(24.))))
                            .child(div().text_xs().text_color(muted_text).child("Loading messages..."))
                    )
                    .into_any_element()
            } else if self.cached_render_messages.is_empty() {
                v_flex()
                    .size_full()
                    .p_6()
                    .child(
                        v_flex()
                            .py_16()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .child(svg().data(MESSAGE_SQUARE_SVG).size(px(32.0)).text_color(muted_text))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(text_color)
                                    .child("No messages in this chat yet"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted_text)
                                    .child("Type a message below to start the conversation."),
                            )
                    )
                    .into_any_element()
            } else {
                list(
                    self.messages_list_state.clone(),
                    cx.processor(|this, index: usize, window, cx| {
                        Self::render_message_row(this, index, window, cx)
                    }),
                )
                .size_full()
                .into_any_element()
            })
            .child(
                Scrollbar::vertical(&self.messages_list_state)
                    .mode(ScrollbarMode::Always)
                    .styles(|s| {
                        s.track(|t| t.bg(transparent_black()))
                            .thumb(|th| th.bg(theme.muted_foreground.opacity(0.35)).radius(px(3.0)).width(px(6.0)))
                            .thumb_hover(|th| th.bg(theme.muted_foreground.opacity(0.65)).radius(px(4.0)).width(px(8.0)))
                            .thumb_active(|th| th.bg(theme.primary.opacity(0.8)).radius(px(4.0)).width(px(8.0)))
                    }),
            )
            // Floating day header, mirroring the web's sticky date pill.
            .children(self.sticky_day_label().map(|label| {
                div()
                    .absolute()
                    .top_2()
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    // The strip spans the width but only the pill should catch clicks.
                    .child(div().block_mouse_except_scroll().child(Self::day_pill(
                        label,
                        theme.muted.opacity(0.9),
                        border_color.opacity(0.4),
                        muted_text,
                    )))
                    .into_any_element()
            }))
            // Floating scroll-to-bottom chevron button when scrolled up
            .children(if self.should_show_scroll_to_bottom() {
                let btn = div()
                    .id("btn-scroll-to-bottom")
                    .cursor_pointer()
                    // Without this the click also reaches the message under the button.
                    .block_mouse_except_scroll()
                    .absolute()
                    .bottom(px(16.0))
                    .right(px(16.0))
                    .w(px(40.0))
                    .h(px(40.0))
                    .rounded_full()
                    .bg(theme.card)
                    .border_1()
                    .border_color(border_color.opacity(0.7))
                    .shadow_md()
                    .flex()
                    .items_center()
                    .justify_center()
                    .hover(|s| s.bg(theme.muted.opacity(0.8)))
                    .active(|s| s.bg(theme.muted.opacity(0.95)))
                    .tooltip(move |window, cx| {
                        Tooltip::new("Scroll to newest").build(window, cx)
                    })
                    .child(
                        Icon::new(IconName::ChevronDown)
                            .size(px(20.0))
                            .text_color(muted_text),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.scroll_to_bottom(cx);
                        }),
                    );

                Some(
                    crate::components::motion::fade_in(
                        btn,
                        "scroll-bottom-fade",
                        160,
                    )
                    .into_any_element(),
                )
            } else {
                None
            })
    }
}

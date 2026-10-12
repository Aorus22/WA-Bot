use crate::views::chat::*;

impl ChatView {
    /// Forward message modal.
    pub(crate) fn render_forward_dialog(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let theme = *cx.app_theme();
        let border_color = theme.border;
        let card_bg = theme.card;
        let primary_color = theme.primary;
        let text_color = theme.foreground;
        let muted_text = theme.muted_foreground;
        if self.forward_msg.is_some() {
            let targets: Vec<Chat> = if cx.has_global::<ChatStore>() {
                ChatStore::global(cx).chats.iter().filter(|c| !c.archived).cloned().collect()
            } else {
                Vec::new()
            };
            let selected_count = self.forward_selected.len();
            let can_send = selected_count > 0 && !self.is_forwarding;
            Some(crate::components::motion::modal_in(
                div()
                    .id("forward-backdrop")
                    .absolute()
                    .occlude()
                    .inset_0()
                    .bg(rgba(0x00000088))
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_mouse_down(MouseButton::Left, |_, _, _| {})
                    .child(
                        v_flex()
                            .w(px(400.0))
                            .p_5()
                            .rounded_2xl()
                            .bg(card_bg)
                            .border_1()
                            .border_color(border_color)
                            .shadow_2xl()
                            .gap_4()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(div().text_lg().font_weight(FontWeight::BOLD).text_color(text_color).child("Forward message to"))
                                    .child(
                                        div()
                                            .cursor_pointer()
                                            .p_1()
                                            .rounded_full()
                                            .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                            .child(svg().data(X_SVG).size(px(16.0)).text_color(muted_text))
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                this.forward_msg = None;
                                                this.forward_selected.clear();
                                                cx.notify();
                                            })),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .id("forward-chats-scroll")
                                    .max_h(px(280.0))
                                    .overflow_y_scroll()
                                    .gap_1()
                                    .children(targets.into_iter().map(|c| {
                                        let is_checked = self.forward_selected.contains(&c.id);
                                        let cid = c.id.clone();
                                        h_flex()
                                            .cursor_pointer()
                                            .p_2()
                                            .rounded_lg()
                                            .items_center()
                                            .justify_between()
                                            .bg(if is_checked { theme.primary.opacity(0.12) } else { rgba(0x00000000).into() })
                                            .hover(|s| s.bg(theme.muted.opacity(0.4)))
                                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                if !this.forward_selected.remove(&cid) {
                                                    this.forward_selected.insert(cid.clone());
                                                }
                                                cx.notify();
                                            }))
                                            .child(div().text_sm().text_color(text_color).child(c.name.clone()))
                                            .child(
                                                div()
                                                    .w(px(20.0))
                                                    .h(px(20.0))
                                                    .rounded_md()
                                                    .border_1()
                                                    .border_color(primary_color)
                                                    .bg(if is_checked { primary_color } else { rgba(0x00000000).into() })
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .children(if is_checked {
                                                        Some(svg().data(CHECK_SVG).size(px(12.0)).text_color(rgb(0xffffff)))
                                                    } else {
                                                        None
                                                    }),
                                            )
                                            .into_any_element()
                                    })),
                            )
                            .child(
                                h_flex()
                                    .justify_end()
                                    .gap_2()
                                    .child(
                                        div()
                                            .cursor_pointer()
                                            .px_4()
                                            .py_2()
                                            .rounded_xl()
                                            .border_1()
                                            .border_color(border_color)
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(text_color)
                                            .child("Cancel")
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                this.forward_msg = None;
                                                this.forward_selected.clear();
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        div()
                                            .px_4()
                                            .py_2()
                                            .rounded_xl()
                                            .bg(primary_color)
                                            .opacity(if can_send { 1.0 } else { 0.5 })
                                            .cursor_pointer()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(0xffffff))
                                            .child(if self.is_forwarding {
                                                "Sending...".to_string()
                                            } else if selected_count > 0 {
                                                format!("Forward ({selected_count})")
                                            } else {
                                                "Forward".to_string()
                                            })
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                this.submit_forward(cx);
                                            })),
                                    ),
                            ),
                    ),
                    "forward-in",
                ))
        } else {
            None
        }
    }
}

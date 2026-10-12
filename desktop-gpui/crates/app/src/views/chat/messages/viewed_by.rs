use crate::views::chat::*;

impl ChatView {
    /// "Viewed by" read receipts modal.
    pub(crate) fn render_viewed_by_dialog(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let theme = *cx.app_theme();
        let border_color = theme.border;
        let card_bg = theme.card;
        let primary_color = theme.primary;
        let text_color = theme.foreground;
        let muted_text = theme.muted_foreground;
        if self.viewed_by_message.is_some() {
            let receipts = self.viewed_by_receipts.clone();
            let group = self.group_info.clone();
            let base_url = Self::compose_base_url(cx);
            Some(crate::components::motion::modal_in(
                div()
                    .id("viewed-by-backdrop")
                    .absolute()
                    .occlude()
                    .inset_0()
                    .bg(rgba(0x00000099))
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.close_viewed_by(cx)))
                    .child(
                        v_flex()
                            .w(px(380.0))
                            .max_h(px(520.0))
                            .p_4()
                            .gap_3()
                            .rounded_2xl()
                            .bg(card_bg)
                            .border_1()
                            .border_color(border_color)
                            .shadow_xl()
                            .on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, cx| cx.stop_propagation()))
                            .child(
                                h_flex()
                                    .items_center()
                                    .justify_between()
                                    .child(div().text_base().font_weight(FontWeight::BOLD).text_color(text_color).child("Viewed by"))
                                    .child(
                                        div()
                                            .cursor_pointer()
                                            .p_1()
                                            .rounded_full()
                                            .hover(|style| style.bg(theme.muted.opacity(0.5)))
                                            .child(svg().data(X_SVG).size(px(16.0)).text_color(muted_text))
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.close_viewed_by(cx))),
                                    ),
                            )
                            .child(if self.is_loading_viewed_by {
                                v_flex()
                                    .py_8()
                                    .items_center()
                                    .gap_2()
                                    .child(Spinner::new().color(primary_color))
                                    .child(div().text_sm().text_color(muted_text).child("Loading receipts…"))
                                    .into_any_element()
                            } else if receipts.is_empty() {
                                div()
                                    .py_8()
                                    .text_sm()
                                    .text_color(muted_text)
                                    .child("No read receipts reported yet.")
                                    .into_any_element()
                            } else {
                                v_flex()
                                    .max_h(px(400.0))
                                    .overflow_y_scrollbar()
                                    .gap_1()
                                    .children(receipts.into_iter().map(|receipt| {
                                        let participant = group.as_ref().and_then(|group| group.participants.iter().find(|member| {
                                            member.jid == receipt.jid || member.jid.split('@').next() == receipt.jid.split('@').next()
                                        }));
                                        let name = participant
                                            .and_then(|member| member.name.as_deref())
                                            .filter(|name| !name.trim().is_empty())
                                            .map(str::to_string)
                                            .unwrap_or_else(|| receipt.jid.split('@').next().unwrap_or(&receipt.jid).to_string());
                                        h_flex()
                                            .items_center()
                                            .justify_between()
                                            .gap_3()
                                            .px_2()
                                            .py_2()
                                            .rounded_lg()
                                            .child(
                                                h_flex()
                                                    .min_w_0()
                                                    .items_center()
                                                    .gap_3()
                                                    .child(render_avatar(&receipt.jid, &name, "", false, 36.0, &base_url, &theme))
                                                    .child(div().text_sm().text_color(text_color).child(name)),
                                            )
                                            .child(div().text_xs().text_color(muted_text).child(Self::format_chat_time(receipt.read_at)))
                                            .into_any_element()
                                    }))
                                    .into_any_element()
                            }),
                    ),
                "viewed-by-in",
            ))
        } else {
            None
        }
    }
}

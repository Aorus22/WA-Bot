use crate::views::chat::*;

impl ChatView {
    /// Join group via invite link modal.
    pub(crate) fn render_join_group_dialog(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let theme = *cx.app_theme();
        let border_color = theme.border;
        let card_bg = theme.card;
        let primary_color = theme.primary;
        let text_color = theme.foreground;
        let muted_text = theme.muted_foreground;
        if self.is_join_group_open {
            Some(crate::components::motion::modal_in(
                div()
                    .absolute()
                    .occlude()
                    .inset_0()
                    .bg(rgba(0x00000088))
                    .flex()
                    .items_center()
                    .justify_center()
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
                            // Header
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(div().text_lg().font_weight(FontWeight::BOLD).text_color(text_color).child("Join group"))
                                    .child(
                                        div()
                                            .cursor_pointer()
                                            .p_1()
                                            .rounded_full()
                                            .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                            .child(svg().data(X_SVG).size(px(16.0)).text_color(muted_text))
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                this.is_join_group_open = false;
                                                cx.notify();
                                            })),
                                    ),
                            )
                            .child(
                                div().text_xs().text_color(muted_text).child("Paste an invite link (chat.whatsapp.com/...)"),
                            )
                            // Invite Link Input
                            .children(self.join_link_field.as_ref().map(|field| field.element(&theme, Some(LINK_SVG))))
                            // Actions
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
                                                this.is_join_group_open = false;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        div()
                                            .cursor_pointer()
                                            .px_4()
                                            .py_2()
                                            .rounded_xl()
                                            .bg(primary_color)
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(0xffffff))
                                            .child(if self.is_joining_group { "Joining..." } else { "Join" })
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                this.submit_join_group(cx);
                                            })),
                                    ),
                            ),
                    ),
                    "join-group-in",
                ))
        } else {
            None
        }
    }
}

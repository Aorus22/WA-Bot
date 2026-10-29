use crate::views::chat::*;

impl ChatView {
    /// Replaces the conversation header while messages are being selected.
    pub(crate) fn render_selection_bar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = *cx.app_theme();
        let border_color = theme.border;
        let text_color = theme.foreground;
        let muted_text = theme.muted_foreground;
        let count = self.selected_messages.len();
        let deletable = self.selection_deletable();
        let danger: Hsla = rgb(0xef4444).into();
        let radius_button = px(cx.shape().radius_button);

        let action = |id: &'static str, icon: &'static [u8], tip: &'static str, enabled: bool, color: Hsla| {
            let button = div()
                .id(id)
                .p_2()
                .rounded(radius_button)
                .when(enabled, |el| {
                    el.cursor_pointer().hover({
                        let hover_bg = theme.muted.opacity(0.65);
                        move |s| s.bg(hover_bg)
                    })
                })
                .when(!enabled, |el| el.opacity(0.35))
                .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
                .child(svg().data(icon).size(px(18.0)).text_color(color));
            button
        };

        let controls_room = if self.controls_over_conversation() { crate::components::titlebar::controls_room(cx) } else { 12.0 };
        h_flex()
            .h(px(cx.shape().header_height))
            .flex_shrink_0()
            .pl_4()
            .pr(px(controls_room))
            .gap_2()
            .items_center()
            .border_b_1()
            .border_color(border_color.opacity(0.4))
            .bg(theme.muted.opacity(0.2))
            .child(
                action("btn-select-cancel", X_SVG, "Cancel", true, muted_text)
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.exit_select_mode(cx))),
            )
            .child(
                div()
                    .flex_1()
                    .text_base()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(text_color)
                    .child(format!("{count} selected")),
            )
            .child(
                action("btn-select-star", STAR_SVG, "Star", count > 0, muted_text)
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.select_star(cx))),
            )
            .child(
                action("btn-select-copy", COPY_SVG, "Copy", count > 0, muted_text)
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.select_copy(cx))),
            )
            .child(
                action("btn-select-forward", FORWARD_SVG, "Forward", count > 0, muted_text)
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.select_forward(cx))),
            )
            .child(
                action(
                    "btn-select-delete",
                    TRASH_SVG,
                    "Delete message",
                    deletable,
                    danger,
                )
                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.select_delete(cx))),
            )
            .into_any_element()
    }
}

use crate::views::chat::*;

impl ChatView {
    /// Placeholder shown while no conversation is selected.
    pub(crate) fn render_empty_state(
        &mut self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = *cx.app_theme();
        let primary_color = theme.primary;
        let text_color = theme.foreground;
        let muted_text = theme.muted_foreground;
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_3()
            .child(
                div()
                    .w(px(72.0))
                    .h(px(72.0))
                    .rounded_3xl()
                    .bg(theme.primary.opacity(0.1))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(svg().data(MESSAGE_SQUARE_SVG).size(px(32.0)).text_color(primary_color)),
            )
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_color)
                    .child("WA Bot Desktop"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(muted_text)
                    .child("Select a conversation from the left to start chatting."),
            )
            .into_any_element()
    }
}

use crate::views::chat::*;

impl ChatView {
    /// Title row, new/join group actions, search box and filter chips.
    pub(crate) fn render_sidebar_header(
        &mut self,
        cx: &mut Context<Self>,
        _pages: bool,
        _has_selection: bool,
    ) -> AnyElement {
        let theme = *cx.app_theme();
        let text_color = theme.foreground;
        let muted_text = theme.muted_foreground;
        if self.archived_mode {
            return v_flex()
                .px_4()
                .pt_4()
                .pb_2()
                .gap_3()
                .child(
                    h_flex()
                        .items_center()
                        .gap_4()
                        .child(
                            div()
                                .id("btn-back-archived")
                                .flex()
                                .items_center()
                                .justify_center()
                                .w(px(32.0))
                                .h(px(32.0))
                                .rounded_full()
                                .cursor_pointer()
                                .hover(|s| s.bg(theme.muted.opacity(0.65)))
                                .tooltip(move |window, cx| Tooltip::new("Back").build(window, cx))
                                .child(svg().data(ARROW_LEFT_SVG).size(px(20.0)).text_color(text_color))
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                    this.archived_mode = false;
                                    cx.notify();
                                })),
                        )
                        .child(
                            div()
                                .text_base()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(text_color)
                                .child("Archived"),
                        ),
                )
                .child(
                    div()
                        .text_sm()
                        .line_height(relative(1.4))
                        .text_color(muted_text)
                        .child(
                            "These chats stay archived when new messages are received. To change this experience, go to Settings > Chats on your phone.",
                        ),
                )
                .child(div().h(px(1.0)).w_full().bg(text_color.opacity(0.85)))
                .into_any_element();
        }
        v_flex()
            .p_4()
            .gap_3()
            // Top Row: Title + Action Icons
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_2xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(text_color)
                                    .child("Messages"),
                            ),
                    )
                    // Action buttons: New Group, Join Group (matching Web)
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1()
                            // New Group Button
                            .child(
                                div()
                                    .id("btn-new-group")
                                    .cursor_pointer()
                                    .p_2()
                                    .rounded_full()
                                    .hover(|s| s.bg(theme.muted.opacity(0.65)))
                                    .tooltip(move |window, cx| {
                                        Tooltip::new("New Group").build(window, cx)
                                    })
                                    .child(svg().data(USERS_SVG).size(px(18.0)).text_color(muted_text))
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                        this.is_new_group_open = true;
                                        if let Some(field) = this.new_group_field.clone() {
                                            field.focus(window, cx);
                                        }
                                        cx.notify();
                                    })),
                            )
                            // Join Group via Link Button
                            .child(
                                div()
                                    .id("btn-join-group")
                                    .cursor_pointer()
                                    .p_2()
                                    .rounded_full()
                                    .hover(|s| s.bg(theme.muted.opacity(0.65)))
                                    .tooltip(move |window, cx| {
                                        Tooltip::new("Join Group via Link").build(window, cx)
                                    })
                                    .child(svg().data(LINK_SVG).size(px(18.0)).text_color(muted_text))
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                        this.is_join_group_open = true;
                                        if let Some(field) = this.join_link_field.clone() {
                                            field.focus(window, cx);
                                        }
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            // Search Box
            .children(
                self.search_field
                    .as_ref()
                    .map(|field| field.element(&theme, Some(SEARCH_SVG))),
            )
            .into_any_element()
    }
}
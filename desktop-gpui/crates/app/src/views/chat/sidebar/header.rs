use crate::views::chat::*;

impl ChatView {
    /// Title row, new/join group actions, search box and filter chips.
    pub(crate) fn render_sidebar_header(
        &mut self,
        cx: &mut Context<Self>,
        pages: bool,
        has_selection: bool,
    ) -> impl IntoElement {
        let theme = *cx.app_theme();
        let border_color = theme.border;
        let primary_color = theme.primary;
        let text_color = theme.foreground;
        let muted_text = theme.muted_foreground;
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
                            .children(if self.archived_mode {
                                Some(
                                    h_flex()
                                        .id("btn-back-archived")
                                        .cursor_pointer()
                                        .items_center()
                                        .gap_1()
                                        .px_2()
                                        .py_1()
                                        .rounded_lg()
                                        .hover(|s| s.bg(theme.muted.opacity(0.6)))
                                        .text_xs()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(primary_color)
                                        .child(svg().data(CHEVRON_LEFT_SVG).size(px(15.0)).text_color(primary_color))
                                        .child("Back")
                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                            this.archived_mode = false;
                                            cx.notify();
                                        })),
                                )
                            } else {
                                None
                            })
                            .child(
                                div()
                                    .text_2xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(text_color)
                                    .child(if self.archived_mode { "Archived" } else { "Messages" }),
                            ),
                    )
                    // Action buttons: New Group, Join Group (matching Web)
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1()
                            // Sidebar layout: collapse the chat list.
                            .children((!pages && has_selection).then(|| {
                                div()
                                    .id("btn-collapse-sidebar")
                                    .cursor_pointer()
                                    .p_2()
                                    .rounded_full()
                                    .hover(|s| s.bg(theme.muted.opacity(0.65)))
                                    .tooltip(move |window, cx| {
                                        Tooltip::new("Hide chats").build(window, cx)
                                    })
                                    .child(svg().data(SIDEBAR_SVG).size(px(18.0)).text_color(muted_text))
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                        this.toggle_sidebar_collapsed(cx);
                                    }))
                            }))
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
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                        this.is_new_group_open = true;
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
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                        this.is_join_group_open = true;
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            // Search Box
            .child(
                h_flex()
                    .px_3()
                    .py_2()
                    .bg(theme.muted.opacity(0.5))
                    .rounded_xl()
                    .border_1()
                    .border_color(border_color.opacity(0.3))
                    .items_center()
                    .gap_2()
                    .cursor_text()
                    .track_focus(&self.search_focus_handle)
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                        this.search_focus_handle.focus(window, cx);
                    }))
                    .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                        match ev.keystroke.key.as_str() {
                            "backspace" => {
                                this.search_query.pop();
                                cx.notify();
                            }
                            "escape" => {
                                this.search_query.clear();
                                cx.notify();
                            }
                            "space" => {
                                this.search_query.push(' ');
                                cx.notify();
                            }
                            k if k.len() == 1 => {
                                this.search_query.push_str(k);
                                cx.notify();
                            }
                            _ => {}
                        }
                    }))
                    .child(
                        svg()
                            .data(SEARCH_SVG)
                            .size(px(14.0))
                            .text_color(muted_text),
                    )
                    .child(
                        div()
                            .flex_1()
                            .cursor_text()
                            .text_xs()
                            .text_color(if self.search_query.is_empty() { muted_text } else { text_color })
                            .child(if self.search_query.is_empty() {
                                "Search conversations...".to_string()
                            } else {
                                self.search_query.clone()
                            }),
                    )
                    .children(if !self.search_query.is_empty() {
                        Some(
                            div()
                                .id("btn-clear-search")
                                .cursor_pointer()
                                .p_1()
                                .rounded_full()
                                .hover(|s| s.bg(theme.muted.opacity(0.65)))
                                .tooltip(move |window, cx| {
                                    Tooltip::new("Clear search").build(window, cx)
                                })
                                .child(svg().data(X_SVG).size(px(12.0)).text_color(muted_text))
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                    this.search_query.clear();
                                    cx.notify();
                                })),
                        )
                    } else {
                        None
                    }),
            )
    }
}

use crate::views::chat::*;

impl ChatView {
    /// Conversation header: back/collapse, avatar + title (opens the info sheet), call/search/more actions.
    pub(crate) fn render_chat_header(
        &mut self,
        cx: &mut Context<Self>,
        pages: bool,
        active_chat: &Chat,
    ) -> impl IntoElement {
        let theme = *cx.app_theme();
        let border_color = theme.border;
        let bg_color = theme.background;
        let text_color = theme.foreground;
        let muted_text = theme.muted_foreground;
        let window_transparency = ThemeManager::global(cx).window_transparency;
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };
        let collapsed = self.sidebar_collapsed;
        let chat_name = active_chat.name.clone();
        let chat_jid = active_chat.id.clone();
        let disappearing_secs = active_chat.disappearing_secs;
        let controls_room = if self.controls_over_conversation() { crate::components::titlebar::controls_room(cx) } else { 12.0 };
        h_flex()
            .h(px(crate::components::titlebar::HEADER_HEIGHT))
            .flex_shrink_0()
            .pl_5()
            .pr(px(controls_room))
            .border_b_1()
            .border_color(border_color.opacity(0.4))
            .bg(window_material::glass_fill(
                theme.chat.map(|c| c.header).unwrap_or(bg_color),
                window_transparency,
            ))
            .items_center()
            .justify_between()
            .gap_1()
            // Pages: back to the chat list. Sidebar: collapse / expand it.
            .child(
                div()
                    .id("btn-header-nav")
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_shrink_0()
                    .w(px(34.0))
                    .h(px(34.0))
                    .rounded_full()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.muted.opacity(0.65)))
                    .tooltip(move |window, cx| {
                        Tooltip::new(if pages {
                            "Back to chats"
                        } else if collapsed {
                            "Show chats"
                        } else {
                            "Hide chats"
                        })
                        .build(window, cx)
                    })
                    .child(
                        svg()
                            .data(if pages { CHEVRON_LEFT_SVG } else { SIDEBAR_SVG })
                            .size(px(18.0))
                            .text_color(muted_text),
                    )
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                        if pages {
                            this.back_to_list(cx);
                        } else {
                            this.toggle_sidebar_collapsed(cx);
                        }
                    })),
            )
            // Left Profile Area (Clicking opens Info Sheet)
            .child(
                h_flex()
                    .id("chat-header-profile")
                    .flex_shrink(1.0)
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .items_center()
                    .gap_3()
                    .cursor_pointer()
                    .px_2()
                    .py_0p5()
                    .rounded_xl()
                    .hover(|s| s.bg(theme.muted.opacity(0.6)))
                    .tooltip(move |window, cx| {
                        Tooltip::new("Click for contact info").build(window, cx)
                    })
                    // Press and release opens the info; press and drag moves the window,
                    // so a narrow window with no empty header space can still be moved.
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, ev: &MouseDownEvent, _, _| {
                        this.header_press = Some(ev.position);
                    }))
                    .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, window, cx| {
                        let Some(start) = this.header_press else { return };
                        if ev.pressed_button != Some(MouseButton::Left) {
                            this.header_press = None;
                            return;
                        }
                        let (dx, dy) = (f32::from(ev.position.x - start.x), f32::from(ev.position.y - start.y));
                        if dx * dx + dy * dy > 36.0 {
                            this.header_press = None;
                            gpui_base::GlobalState::suppress_text_selection(cx);
                            window.defer(cx, |window, cx| gpui_base::TextSelection::clear(window, cx));
                            window.start_window_move();
                        }
                    }))
                    .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| {
                        if this.header_press.take().is_some() {
                            this.toggle_info_sheet(cx);
                        }
                    }))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _, _, _| {
                        this.header_press = None;
                    }))
                    .child(render_avatar(&active_chat.id, &active_chat.name, &active_chat.avatar, active_chat.is_group, 40.0, &base_url, &theme))
                    .child(
                        v_flex()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_1p5()
                                    .child(
                                        div()
                                            .text_base()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(text_color)
                                            .child(chat_name),
                                    )
                                    // Disappearing messages are on in this chat.
                                    .when(disappearing_secs > 0, |row| {
                                        let tip = SharedString::from(format!(
                                            "Disappearing messages: {}",
                                            messages::disappearing::disappearing_label(disappearing_secs)
                                        ));
                                        row.child(
                                            div()
                                                .id("chat-header-disappearing")
                                                .flex()
                                                .items_center()
                                                .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
                                                .child(svg().data(HOURGLASS_SVG).size(px(14.0)).text_color(muted_text)),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted_text)
                                    .child(chat_jid.clone()),
                            ),
                    ),
            )
            // Free space: drag the window from here.
            .child(crate::components::titlebar::drag_area())
            // Header Action Icons matching Web: Phone, Video, Search, More
            .child({
                let is_group_chat = active_chat.is_group;
                h_flex()
                    .items_center()
                    .gap_1()
                    .child(
                        h_flex()
                            .id("btn-header-video")
                            .cursor_pointer()
                            .items_center()
                            .gap_0p5()
                            .px_2()
                            .py_2()
                            .rounded_full()
                            .hover(|s| s.bg(theme.muted.opacity(0.65)))
                            .tooltip(move |window, cx| {
                                Tooltip::new(if is_group_chat { "Group call" } else { "Call" }).build(window, cx)
                            })
                            .child(svg().data(VIDEO_SVG).size(px(18.0)).text_color(muted_text))
                            .child(svg().data(CHEVRON_RIGHT_SVG).size(px(11.0)).text_color(muted_text))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, ev: &MouseDownEvent, _, cx| {
                                cx.stop_propagation();
                                this.open_header_menu(HeaderMenuKind::Call, ev.position, cx);
                            })),
                    )
                    .child(
                        div()
                            .id("btn-header-search")
                            .cursor_pointer()
                            .p_2()
                            .rounded_full()
                            .hover(|s| s.bg(theme.muted.opacity(0.65)))
                            .tooltip(move |window, cx| {
                                Tooltip::new("Search in conversation").build(window, cx)
                            })
                            .child(svg().data(SEARCH_SVG).size(px(18.0)).text_color(muted_text))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                this.toggle_search_sheet(window, cx);
                            })),
                    )
                    .child(
                        div()
                            .id("btn-header-more")
                            .cursor_pointer()
                            .p_2()
                            .rounded_full()
                            .hover(|s| s.bg(theme.muted.opacity(0.65)))
                            .tooltip(move |window, cx| {
                                Tooltip::new("Menu").build(window, cx)
                            })
                            .child(svg().data(MORE_VERTICAL_SVG).size(px(18.0)).text_color(muted_text))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, ev: &MouseDownEvent, _, cx| {
                                cx.stop_propagation();
                                this.open_header_menu(HeaderMenuKind::Chat, ev.position, cx);
                            })),
                    )
            })
    }
}

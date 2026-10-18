use crate::views::chat::*;

impl ChatView {
    /// Right-click menu on a conversation row.
    pub(crate) fn render_chat_context_menu(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let theme = *cx.app_theme();
        let border_color = theme.border;
        let card_bg = theme.card;
        let primary_color = theme.primary;
        let text_color = theme.foreground;
        let muted_text = theme.muted_foreground;
        if let Some(ref ctx) = self.chat_context_menu {
            let chat_id = ctx.chat_id.clone();
            let is_pinned = ctx.is_pinned;
            let is_archived = ctx.is_archived;
            let is_muted = ctx.is_muted;
            let is_group = ctx.is_group;
            let unread = ctx.unread;
            let confirm_leave = ctx.confirm_leave;
            let show_mute_submenu = ctx.show_mute_submenu;
            let menu_pos = ctx.position;
            let hover_bg: Hsla = theme.foreground.opacity(0.12);
            let danger: Hsla = rgb(0xef4444).into();

            let mut menu = menu_surface(232.0, card_bg, border_color)
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.chat_context_menu = None;
                    cx.notify();
                }));

            // Mark as read (only while there is something unread)
            if unread > 0 {
                let cid = chat_id.clone();
                menu = menu.child(
                    menu_row(window, cx, "cm-read", CHECK_CHECK_SVG, "Mark as read", primary_color, text_color, hover_bg)
                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                            this.mark_chat_read_action(cid.clone(), cx);
                            this.chat_context_menu = None;
                            cx.notify();
                        })),
                );
            }

            // Archive / Unarchive
            {
                let cid = chat_id.clone();
                menu = menu.child(
                    menu_row(
                            window,
                            cx,
                        "cm-archive",
                        ARCHIVE_SVG,
                        if is_archived { "Unarchive chat" } else { "Archive chat" },
                        muted_text,
                        text_color,
                        hover_bg,
                    )
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                        this.toggle_archive(cid.clone(), is_archived, cx);
                        this.chat_context_menu = None;
                        cx.notify();
                    })),
                );
            }

            // Mute notifications (durations expand inline) / Unmute
            if is_muted {
                let cid = chat_id.clone();
                menu = menu.child(
                    menu_row(window, cx, "cm-unmute", VOLUME_X_SVG, "Unmute notifications", muted_text, text_color, hover_bg)
                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                            this.set_mute(cid.clone(), "off".to_string(), cx);
                            this.chat_context_menu = None;
                            cx.notify();
                        })),
                );
            } else {
                menu = menu.child(
                    menu_row(window, cx, "cm-mute", VOLUME_X_SVG, "Mute notifications", muted_text, text_color, hover_bg)
                        .child(div().flex_1())
                        .child(svg().data(CHEVRON_RIGHT_SVG).size(px(13.0)).text_color(muted_text))
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                            if let Some(m) = this.chat_context_menu.as_mut() {
                                m.show_mute_submenu = !m.show_mute_submenu;
                                cx.notify();
                            }
                        })),
                );
                if show_mute_submenu {
                    for (id, label, mode) in [
                        ("cm-mute-8h", "8 hours", "8h"),
                        ("cm-mute-1w", "1 week", "1w"),
                        ("cm-mute-all", "Always", "forever"),
                    ] {
                        let cid = chat_id.clone();
                        menu = menu.child(
                            with_hover_fade(
                                h_flex().id(id).cursor_pointer().pl_12().pr_5().py_2().items_center(),
                                id,
                                hover_bg,
                                px(0.0),
                                window,
                                cx,
                            )
                                .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(text_color).child(label))
                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                    this.set_mute(cid.clone(), mode.to_string(), cx);
                                    this.chat_context_menu = None;
                                    cx.notify();
                                })),
                        );
                    }
                }
            }

            // Pin / Unpin
            {
                let cid = chat_id.clone();
                menu = menu.child(
                    menu_row(
                            window,
                            cx,
                        "cm-pin",
                        PIN_SVG,
                        if is_pinned { "Unpin chat" } else { "Pin chat" },
                        muted_text,
                        text_color,
                        hover_bg,
                    )
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                        this.toggle_pin(cid.clone(), is_pinned, cx);
                        this.chat_context_menu = None;
                        cx.notify();
                    })),
                );
            }

            // Exit group (destructive: needs a second click)
            if is_group {
                let cid = chat_id.clone();
                menu = menu
                    .child(menu_separator(border_color))
                    .child(
                        menu_row(
                            window,
                            cx,
                            "cm-exit",
                            USER_MINUS_SVG,
                            if confirm_leave { "Click again to exit group" } else { "Exit group" },
                            danger,
                            danger,
                            hover_bg,
                        )
                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                            let armed = this
                                .chat_context_menu
                                .as_ref()
                                .map(|m| m.confirm_leave)
                                .unwrap_or(false);
                            if armed {
                                this.leave_group_action(cid.clone(), cx);
                                this.chat_context_menu = None;
                            } else if let Some(m) = this.chat_context_menu.as_mut() {
                                m.confirm_leave = true;
                            }
                            cx.notify();
                        })),
                    );
            }

            Some(
                deferred(anchored().child(
                    div()
                        .id("chat-context-menu-backdrop")
                        .w(window.bounds().size.width)
                        .h(window.bounds().size.height)
                        .child(
                            gpui_base::Positioner::corner(Anchor::TopLeft, menu_pos)
                                .margin(px(8.0))
                                .occlude()
                                .child(menu.with_animation(
                                    "chat-context-menu-in",
                                    Animation::new(std::time::Duration::from_millis(140))
                                        .with_easing(|d| 1.0 - (1.0 - d).powi(5)),
                                    move |el, delta| el.opacity(delta),
                                )),
                        ),
                ))
                .with_priority(gpui_base::POPUP_PRIORITY),
            )
        } else {
            None
        }
    }
}

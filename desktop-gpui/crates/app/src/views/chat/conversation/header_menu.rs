use crate::views::chat::*;

impl ChatView {
    /// Open a header dropdown just below the button that was pressed.
    pub(crate) fn open_header_menu(&mut self, kind: HeaderMenuKind, position: Point<Pixels>, cx: &mut Context<Self>) {
        // Pressing the same button again closes it.
        if self.header_menu.as_ref().is_some_and(|m| m.kind == kind) {
            self.header_menu = None;
        } else {
            self.header_menu = Some(HeaderMenu { kind, position, show_mute: false, confirm_exit: false });
        }
        cx.notify();
    }

    /// Dropdowns of the header buttons: call choice, the conversation's
    /// three-dot menu and the chat list's three-dot menu.
    pub(crate) fn render_header_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let menu_state = self.header_menu.clone()?;
        let theme = *cx.app_theme();
        let border_color = theme.border;
        let primary_color = theme.primary;
        let text_color = theme.foreground;
        let muted_text = theme.muted_foreground;
        let hover_bg: Hsla = theme.foreground.opacity(0.12);
        let danger: Hsla = rgb(0xef4444).into();

        let chat: Option<Chat> = self.selected_chat_id.as_ref().and_then(|id| {
            cx.has_global::<ChatStore>()
                .then(|| ChatStore::global(cx).chats.iter().find(|c| c.id == *id).cloned())
                .flatten()
        });

        let mut menu = menu_surface(252.0, theme.card, border_color)
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.header_menu = None;
                cx.notify();
            }));

        match menu_state.kind {
            HeaderMenuKind::Call => {
                menu = menu
                    .child(
                        menu_row(window, cx, "hm-voice", PHONE_SVG, "Voice call", muted_text, text_color, hover_bg)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.header_menu = None;
                                this.start_call(false, cx);
                            })),
                    )
                    .child(
                        menu_row(window, cx, "hm-video", VIDEO_SVG, "Video call", muted_text, text_color, hover_bg)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.header_menu = None;
                                this.start_call(true, cx);
                            })),
                    );
            }
            HeaderMenuKind::Chat => {
                let chat = chat?;
                let chat_id = chat.id.clone();
                let is_group = chat.is_group;
                let muted = !matches!(chat.mute_mode.as_str(), "" | "off");

                if is_group {
                    let cid = chat_id.clone();
                    menu = menu.child(
                        menu_row(window, cx, "hm-add", PLUS_SVG, "Add member", muted_text, text_color, hover_bg)
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                this.open_add_members(cid.clone(), cx);
                            })),
                    );
                }
                menu = menu
                    .child(
                        menu_row(
                            window,
                            cx,
                            "hm-info",
                            USER_SVG,
                            if is_group { "Group info" } else { "Contact info" },
                            muted_text,
                            text_color,
                            hover_bg,
                        )
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                            this.header_menu = None;
                            if !this.is_info_sheet_open || this.info_sheet_closing {
                                this.toggle_info_sheet(cx);
                            }
                        })),
                    )
                    .child(
                        menu_row(window, cx, "hm-search", SEARCH_SVG, "Search", muted_text, text_color, hover_bg)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                this.header_menu = None;
                                this.toggle_search_sheet(window, cx);
                            })),
                    )
                    .child(
                        menu_row(window, cx, "hm-select", CHECK_SVG, "Select messages", muted_text, text_color, hover_bg)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.enter_select_mode(None, cx);
                            })),
                    );

                // Mute notifications (durations expand inline) / Unmute
                if muted {
                    let cid = chat_id.clone();
                    menu = menu.child(
                        menu_row(window, cx, "hm-unmute", VOLUME_X_SVG, "Unmute notifications", muted_text, text_color, hover_bg)
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                this.set_mute(cid.clone(), "off".to_string(), cx);
                                this.header_menu = None;
                                cx.notify();
                            })),
                    );
                } else {
                    menu = menu.child(
                        menu_row(window, cx, "hm-mute", VOLUME_X_SVG, "Mute notifications", muted_text, text_color, hover_bg)
                            .child(div().flex_1())
                            .child(svg().data(CHEVRON_RIGHT_SVG).size(px(13.0)).text_color(muted_text))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                if let Some(m) = this.header_menu.as_mut() {
                                    m.show_mute = !m.show_mute;
                                    cx.notify();
                                }
                            })),
                    );
                    if menu_state.show_mute {
                        for (id, label, mode) in [
                            ("hm-mute-8h", "8 hours", "8h"),
                            ("hm-mute-1w", "1 week", "1w"),
                            ("hm-mute-all", "Always", "forever"),
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
                                        this.header_menu = None;
                                        cx.notify();
                                    })),
                            );
                        }
                    }
                }

                menu = menu.child(
                    menu_row(window, cx, "hm-disappearing", HOURGLASS_SVG, "Disappearing messages", muted_text, text_color, hover_bg)
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                            this.header_menu = None;
                            if !this.is_info_sheet_open || this.info_sheet_closing {
                                this.toggle_info_sheet(cx);
                            }
                        })),
                );

                {
                    let (cid, name) = (chat_id.clone(), chat.name.clone());
                    menu = menu
                        .child(
                            menu_row(window, cx, "hm-export", DOWNLOAD_SVG, "Export chat", muted_text, text_color, hover_bg)
                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                    this.export_chat(cid.clone(), name.clone(), cx);
                                })),
                        )
                        .child(
                            menu_row(window, cx, "hm-close", X_SVG, "Close chat", muted_text, text_color, hover_bg)
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                    this.header_menu = None;
                                    this.back_to_list(cx);
                                })),
                        );
                }

                if is_group {
                    let cid = chat_id.clone();
                    let confirming = menu_state.confirm_exit;
                    menu = menu.child(menu_separator(border_color)).child(
                        menu_row(
                            window,
                            cx,
                            "hm-exit",
                            USER_MINUS_SVG,
                            if confirming { "Click again to exit group" } else { "Exit group" },
                            danger,
                            danger,
                            hover_bg,
                        )
                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                            let armed = this.header_menu.as_ref().is_some_and(|m| m.confirm_exit);
                            if armed {
                                this.leave_group_action(cid.clone(), cx);
                                this.header_menu = None;
                            } else if let Some(m) = this.header_menu.as_mut() {
                                m.confirm_exit = true;
                            }
                            cx.notify();
                        })),
                    );
                }
            }
            HeaderMenuKind::Sidebar => {
                menu = menu
                    .child(
                        menu_row(window, cx, "sm-new-group", USERS_SVG, "New group", muted_text, text_color, hover_bg)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                this.header_menu = None;
                                this.add_members_for = None;
                                this.is_new_group_open = true;
                                if let Some(field) = this.new_group_field.clone() {
                                    field.focus(window, cx);
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        menu_row(window, cx, "sm-join", LINK_SVG, "Join group via link", muted_text, text_color, hover_bg)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                this.header_menu = None;
                                this.is_join_group_open = true;
                                if let Some(field) = this.join_link_field.clone() {
                                    field.focus(window, cx);
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        menu_row(window, cx, "sm-starred", STAR_SVG, "Starred messages", muted_text, text_color, hover_bg)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.header_menu = None;
                                this.toggle_starred_sheet(cx);
                            })),
                    )
                    .child(
                        menu_row(window, cx, "sm-select", CHECK_SVG, "Select chats", muted_text, text_color, hover_bg)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.enter_chat_select_mode(cx);
                            })),
                    )
                    .child(
                        menu_row(window, cx, "sm-read-all", CHECK_CHECK_SVG, "Mark all as read", primary_color, text_color, hover_bg)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.mark_all_read(cx);
                            })),
                    )
                    .child(menu_separator(border_color))
                    .child(
                        menu_row(window, cx, "sm-logout", ARROW_LEFT_SVG, "Log out", danger, danger, hover_bg)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                this.open_logout(window, cx);
                            })),
                    );
            }
        }

        Some(
            deferred(anchored().child(
                div()
                    .id("header-menu-backdrop")
                    .w(window.bounds().size.width)
                    .h(window.bounds().size.height)
                    .child(
                        gpui_base::Positioner::corner(Anchor::TopRight, menu_state.position)
                            .margin(px(8.0))
                            .occlude()
                            .child(menu.with_animation(
                                "header-menu-in",
                                Animation::new(std::time::Duration::from_millis(140))
                                    .with_easing(|d| 1.0 - (1.0 - d).powi(5)),
                                move |el, delta| el.opacity(delta),
                            )),
                    ),
            ))
            .with_priority(gpui_base::POPUP_PRIORITY),
        )
    }
}

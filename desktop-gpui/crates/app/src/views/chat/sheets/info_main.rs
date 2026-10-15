use crate::views::chat::*;

/// Thumbnails in the inline media strip; four fit the drawer's width.
const MEDIA_STRIP: usize = 4;
/// How many items each list is fetched with; a full list means "at least".
const INFO_PAGE: usize = 30;

impl ChatView {
    /// A tappable row of the info page: icon, label, optional detail text.
    fn info_row(
        id: &'static str,
        icon: &'static [u8],
        label: String,
        detail: Option<String>,
        danger: bool,
        theme: &ActiveTokens,
    ) -> Stateful<Div> {
        let color = if danger { rgb(0xe5484d).into() } else { theme.foreground };
        h_flex()
            .id(id)
            .w_full()
            .items_center()
            .gap_3()
            .px_2()
            .py_2p5()
            .rounded_lg()
            .cursor_pointer()
            .hover({
                let hover_bg = theme.muted.opacity(0.45);
                move |s| s.bg(hover_bg)
            })
            .child(svg().data(icon).size(px(18.0)).text_color(if danger { color } else { theme.muted_foreground }))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().text_sm().text_color(color).child(label))
                    .children(detail.map(|d| div().text_xs().text_color(theme.muted_foreground).child(d))),
            )
    }

    /// Segmented selector used by the notification and timer settings.
    fn info_choice_bar(theme: &ActiveTokens) -> Div {
        h_flex()
            .p_1()
            .bg(theme.muted.opacity(0.5))
            .rounded_xl()
            .border_1()
            .border_color(theme.border.opacity(0.3))
            .justify_around()
    }

    /// The Contact Info / Group Info page: profile, quick actions, a media
    /// strip, settings, members and the leave action, like WhatsApp Web.
    pub(crate) fn render_info_main(&mut self, chat: &Chat, cx: &mut Context<Self>) -> AnyElement {
        let theme = *cx.app_theme();
        let border_color = theme.border;
        let primary_color = theme.primary;
        let text_color = theme.foreground;
        let muted_text = theme.muted_foreground;
        let base_url = Self::compose_base_url(cx);
        let is_group = chat.is_group;

        let member_count = self
            .group_info
            .as_ref()
            .filter(|g| g.jid == chat.id)
            .map(|g| g.participants.len().max(g.participant_count));
        let subtitle = if is_group {
            match member_count {
                Some(n) => format!("Group · {n} members"),
                None => "Group".to_string(),
            }
        } else {
            format!("+{}", chat.id.split('@').next().unwrap_or(&chat.id))
        };
        let description = self
            .group_info
            .as_ref()
            .filter(|g| g.jid == chat.id)
            .and_then(|g| g.description.clone())
            .filter(|d| !d.trim().is_empty());

        // ---- quick actions ------------------------------------------------
        let action = |id: &'static str, icon: &'static [u8], label: &'static str| {
            v_flex()
                .id(id)
                .items_center()
                .gap_1()
                .cursor_pointer()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .w(px(48.0))
                        .h(px(48.0))
                        .rounded_full()
                        .bg(theme.muted.opacity(0.5))
                        .hover({
                            let hover_bg = theme.muted.opacity(0.85);
                            move |s| s.bg(hover_bg)
                        })
                        .child(svg().data(icon).size(px(20.0)).text_color(text_color)),
                )
                .child(div().text_xs().text_color(muted_text).child(label))
        };
        let actions = h_flex()
            .justify_center()
            .gap_4()
            .child(action("info-act-voice", PHONE_SVG, "Voice").on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.start_call(false, cx)),
            ))
            .child(action("info-act-video", VIDEO_SVG, "Video").on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.start_call(true, cx)),
            ))
            .children(is_group.then(|| {
                let cid = chat.id.clone();
                action("info-act-add", PLUS_SVG, "Add").on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| this.open_add_members(cid.clone(), cx)),
                )
            }))
            .child(action("info-act-search", SEARCH_SVG, "Search").on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.toggle_search_sheet(window, cx)),
            ));

        // ---- media, links and docs ----------------------------------------
        let total = self.info_media.len() + self.info_docs.len() + self.info_links.len();
        let more = self.info_media.len() >= INFO_PAGE
            || self.info_docs.len() >= INFO_PAGE
            || self.info_links.len() >= INFO_PAGE;
        let count_label = if self.is_loading_info && total == 0 {
            String::new()
        } else if more {
            format!("{total}+")
        } else {
            total.to_string()
        };
        let strip: Vec<(bool, String)> = self
            .info_media
            .iter()
            .take(MEDIA_STRIP)
            .map(|m| {
                let url = m.media_url.as_deref().map(|u| resolve_media_url(u, &base_url)).unwrap_or_default();
                (m.message_type == "image", url)
            })
            .collect();
        let media_section = v_flex()
            .gap_2()
            .child(
                h_flex()
                    .id("info-media-open")
                    .items_center()
                    .gap_3()
                    .px_2()
                    .py_2()
                    .rounded_lg()
                    .cursor_pointer()
                    .hover({
                        let hover_bg = theme.muted.opacity(0.45);
                        move |s| s.bg(hover_bg)
                    })
                    .child(svg().data(IMAGE_SVG).size(px(18.0)).text_color(muted_text))
                    .child(div().flex_1().text_sm().text_color(text_color).child("Media, links and docs"))
                    .child(div().text_xs().text_color(muted_text).child(count_label))
                    .child(svg().data(CHEVRON_RIGHT_SVG).size(px(14.0)).text_color(muted_text))
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                        this.info_media_open = true;
                        cx.notify();
                    })),
            )
            .children((!strip.is_empty()).then(|| {
                h_flex().gap_2().px_2().children(strip.into_iter().map(|(is_image, url)| {
                    let cell = div()
                        .w(px(62.0))
                        .h(px(62.0))
                        .rounded_lg()
                        .overflow_hidden()
                        .border_1()
                        .border_color(border_color.opacity(0.4))
                        .bg(theme.muted.opacity(0.2))
                        .cursor_pointer();
                    if is_image && !url.is_empty() {
                        cell.on_mouse_down(MouseButton::Left, cx.listener({
                            let u = url.clone();
                            move |this, _, _, cx| this.open_preview(u.clone(), None, cx)
                        }))
                        .child(
                            img(url.clone())
                                .id(format!("info-strip-{url}"))
                                .w_full()
                                .h_full()
                                .object_fit(ObjectFit::Cover),
                        )
                        .into_any_element()
                    } else {
                        cell.flex()
                            .items_center()
                            .justify_center()
                            .on_mouse_down(MouseButton::Left, cx.listener({
                                let u = url.clone();
                                move |_, _, _, cx| {
                                    if !u.is_empty() {
                                        cx.open_url(&u);
                                    }
                                }
                            }))
                            .child(svg().data(VIDEO_SVG).size(px(20.0)).text_color(muted_text))
                            .into_any_element()
                    }
                }))
            }));

        // ---- settings -----------------------------------------------------
        let chat_id = chat.id.clone();
        let muted = !matches!(chat.mute_mode.as_str(), "" | "off");
        let chat_mute_mode = chat.mute_mode.clone();
        let mute_detail = if muted { "Muted" } else { "On" };
        let mute_card = v_flex()
            .gap_2()
            .px_2()
            .py_2()
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(svg().data(VOLUME_X_SVG).size(px(18.0)).text_color(muted_text))
                    .child(
                        v_flex()
                            .child(div().text_sm().text_color(text_color).child("Notifications"))
                            .child(div().text_xs().text_color(muted_text).child(mute_detail)),
                    ),
            )
            .child(
                Self::info_choice_bar(&theme).children(
                    [("off", "On"), ("8h", "8 hours"), ("1w", "1 week"), ("forever", "Always off")].into_iter().map(
                        |(mode, label)| {
                            // 8 hours / 1 week are both "until"; only On and Always off can be told apart.
                            let selected = match mode {
                                "off" => !muted,
                                "forever" => muted && chat_mute_mode != "until",
                                _ => false,
                            };
                            let cid = chat_id.clone();
                            div()
                                .id(SharedString::from(format!("info-mute-{mode}")))
                                .cursor_pointer()
                                .px_2()
                                .py_1p5()
                                .rounded_lg()
                                .text_xs()
                                .font_weight(if selected { FontWeight::BOLD } else { FontWeight::NORMAL })
                                .bg(if selected { theme.primary.opacity(0.18) } else { rgba(0x00000000).into() })
                                .text_color(if selected { primary_color } else { muted_text })
                                .child(label)
                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                    this.set_mute(cid.clone(), mode.to_string(), cx);
                                }))
                        },
                    ),
                ),
            );

        let disappearing_card = {
            let current = chat.disappearing_secs;
            let chat_id = chat.id.clone();
            v_flex()
                .gap_2()
                .px_2()
                .py_2()
                .child(
                    h_flex()
                        .items_center()
                        .gap_3()
                        .child(svg().data(HOURGLASS_SVG).size(px(18.0)).text_color(muted_text))
                        .child(
                            v_flex()
                                .child(div().text_sm().text_color(text_color).child("Disappearing messages"))
                                .child(div().text_xs().text_color(muted_text).child(if current == 0 {
                                    "Off".to_string()
                                } else {
                                    messages::disappearing::disappearing_label(current)
                                })),
                        ),
                )
                .child(
                    Self::info_choice_bar(&theme).children(
                        messages::disappearing::DISAPPEARING_OPTIONS.into_iter().map(|(secs, label)| {
                            let selected = current == secs;
                            let chat_id = chat_id.clone();
                            div()
                                .id(SharedString::from(format!("disappearing-{secs}")))
                                .cursor_pointer()
                                .px_2p5()
                                .py_1p5()
                                .rounded_lg()
                                .text_xs()
                                .font_weight(if selected { FontWeight::BOLD } else { FontWeight::NORMAL })
                                .bg(if selected { theme.primary.opacity(0.18) } else { rgba(0x00000000).into() })
                                .text_color(if selected { primary_color } else { muted_text })
                                .child(label)
                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                    this.set_disappearing_timer(chat_id.clone(), secs, cx);
                                }))
                        }),
                    ),
                )
                .child(
                    div()
                        .text_size(px(10.5))
                        .text_color(muted_text.opacity(0.8))
                        .child("Messages stay in this app either way; only phones remove them."),
                )
        };

        let settings = v_flex()
            .child(
                Self::info_row("info-row-starred", STAR_SVG, "Starred messages".into(), None, false, &theme)
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.toggle_starred_sheet(cx))),
            )
            .child(mute_card)
            .child(disappearing_card)
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .px_2()
                    .py_2p5()
                    .child(svg().data(CHECK_CHECK_SVG).size(px(18.0)).text_color(muted_text))
                    .child(
                        v_flex()
                            .child(div().text_sm().text_color(text_color).child("Encryption"))
                            .child(div().text_xs().text_color(muted_text).child("Messages are end-to-end encrypted.")),
                    ),
            );

        // ---- members (groups) ---------------------------------------------
        let members = is_group.then(|| {
            let header = div()
                .px_2()
                .text_xs()
                .text_color(muted_text)
                .child(match member_count {
                    Some(n) => format!("{n} members"),
                    None => "Members".to_string(),
                });
            let body: AnyElement = if self.is_loading_info_members {
                v_flex()
                    .py_6()
                    .items_center()
                    .gap_2()
                    .child(Spinner::new().color(primary_color))
                    .child(div().text_sm().text_color(muted_text).child("Loading members…"))
                    .into_any_element()
            } else if let Some(group) = self.group_info.as_ref().filter(|g| g.jid == chat.id) {
                v_flex()
                    .children(group.participants.iter().map(|participant| {
                        let name = participant
                            .name
                            .as_deref()
                            .filter(|name| !name.trim().is_empty())
                            .unwrap_or_else(|| participant.jid.split('@').next().unwrap_or(&participant.jid));
                        h_flex()
                            .w_full()
                            .items_center()
                            .gap_3()
                            .px_2()
                            .py_2()
                            .rounded_lg()
                            .child(render_avatar(&participant.jid, name, "", false, 36.0, &base_url, &theme))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .child(div().text_sm().text_color(text_color).child(name.to_string()))
                                    .child(div().text_xs().text_color(muted_text).child(participant.jid.clone())),
                            )
                            .children(participant.is_admin.then(|| {
                                div()
                                    .px_2()
                                    .py(px(2.0))
                                    .rounded_md()
                                    .text_size(px(10.5))
                                    .text_color(primary_color)
                                    .bg(primary_color.opacity(0.15))
                                    .child("Group admin")
                            }))
                    }))
                    .into_any_element()
            } else {
                div().py_4().px_2().text_sm().text_color(muted_text).child("Group members unavailable").into_any_element()
            };
            v_flex().gap_1().child(header).child(body)
        });

        // ---- leaving ------------------------------------------------------
        let exit = is_group.then(|| {
            let confirming = self.info_confirm_exit;
            let cid = chat.id.clone();
            Self::info_row(
                "info-row-exit",
                USER_MINUS_SVG,
                if confirming { "Click again to leave this group".into() } else { "Exit group".into() },
                None,
                true,
                &theme,
            )
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                if this.info_confirm_exit {
                    this.info_confirm_exit = false;
                    this.leave_group_action(cid.clone(), cx);
                } else {
                    this.info_confirm_exit = true;
                    cx.notify();
                }
            }))
        });

        let divider = || div().h(px(1.0)).w_full().bg(border_color.opacity(0.3));

        v_flex()
            .gap_3()
            .child(
                v_flex()
                    .items_center()
                    .gap_1p5()
                    .py_2()
                    .child(render_avatar(&chat.id, &chat.name, &chat.avatar, chat.is_group, 96.0, &base_url, &theme))
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(text_color)
                            .max_w_full()
                            .overflow_hidden()
                            .child(chat.name.clone()),
                    )
                    .child(div().text_sm().text_color(muted_text).max_w_full().overflow_hidden().child(subtitle)),
            )
            .child(actions)
            .children(description.map(|text| {
                v_flex()
                    .gap_1()
                    .px_2()
                    .child(div().text_xs().text_color(muted_text).child("Description"))
                    .child(div().text_sm().text_color(text_color).child(text))
            }))
            .child(divider())
            .child(media_section)
            .child(divider())
            .child(settings)
            .children(members.map(|m| v_flex().gap_3().child(divider()).child(m)))
            .children(exit.map(|row| v_flex().gap_3().child(divider()).child(row)))
            .into_any_element()
    }
}

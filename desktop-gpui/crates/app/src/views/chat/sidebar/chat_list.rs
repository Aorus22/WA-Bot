use crate::views::chat::*;
use crate::theme::surface::SurfaceExt;

impl ChatView {
    /// Archived entry shown first in the conversation list.
    pub(crate) fn render_archived_row(
        &mut self,
        cx: &mut Context<Self>,
        archived_count: usize,
    ) -> Option<impl IntoElement> {
        let theme = *cx.app_theme();
        let shape = cx.shape();
        let surface = cx.surface();
        let radius_card = px(shape.radius_row);
        let text_color = theme.foreground;
        if !self.archived_mode && self.search_query.is_empty() {
            Some(
                h_flex()
                    .id("btn-archived-row")
                    .mx_2()
                    .mt_2()
                    .px_3()
                    .py_2p5()
                    .rounded(radius_card)
                    .items_center()
                    .gap_3()
                    .cursor_pointer()
                    .hover(|s| s.bg(surface.archived_row_hover))
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                        this.archived_mode = true;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .w(px(32.0))
                            .h(px(32.0))
                            .rounded_full()
                            .bg(rgba(0x00a88420))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(svg().data(ARCHIVE_SVG).size(px(16.0)).text_color(rgb(0x00a884))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(text_color)
                            .child("Archived"),
                    )
                    .children(if archived_count > 0 {
                        Some(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(0x00a884))
                                .child(format!("{}", archived_count)),
                        )
                    } else {
                        None
                    }),
            )
        } else {
            None
        }
    }

    /// "Pinned" section label above the pinned chats.
    pub(crate) fn render_pinned_label(
        &mut self,
        cx: &mut Context<Self>,
        pinned_count: usize,
    ) -> Option<impl IntoElement> {
        let theme = *cx.app_theme();
        let muted_text = theme.muted_foreground;
        if !self.archived_mode && self.search_query.is_empty() && pinned_count > 0 {
            Some(
                h_flex()
                    .mx_4()
                    .mt_2()
                    .mb_1()
                    .items_center()
                    .gap_2()
                    .child(svg().data(PIN_SVG).size(px(13.0)).text_color(muted_text))
                    .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(muted_text).child("Pinned"))
                    .child(div().text_xs().text_color(muted_text.opacity(0.7)).child(format!("{pinned_count}"))),
            )
        } else {
            None
        }
    }

    /// Virtualised conversation list with its scrollbar.
    pub(crate) fn render_chat_list(
        &mut self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = *cx.app_theme();
        let text_color = theme.foreground;
        let muted_text = theme.muted_foreground;
        let is_loading_chats = self.is_loading_chats;
        div()
            .flex_1()
            .relative()
            .overflow_hidden()
            .child(if is_loading_chats && self.cached_filtered_chats.is_empty() {
                v_flex()
                    .size_full()
                    .p_2()
                    .child(
                        v_flex()
                            .py_12()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .child(crate::components::motion::spinning(Icon::new(IconName::LoaderCircle).size(px(24.))))
                            .child(div().text_xs().text_color(muted_text).child("Loading conversations..."))
                    )
                    .into_any_element()
            } else if self.cached_filtered_chats.is_empty() {
                v_flex()
                    .size_full()
                    .p_2()
                    .child(
                        v_flex()
                            .py_12()
                            .px_4()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .child(svg().data(MESSAGE_SQUARE_SVG).size(px(32.0)).text_color(muted_text))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(text_color)
                                    .child(if self.search_query.is_empty() {
                                        "No messages yet"
                                    } else {
                                        "No results found"
                                    }),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted_text)
                                    .child(if self.search_query.is_empty() {
                                        "Incoming messages will appear here."
                                    } else {
                                        "Try another search keyword."
                                    }),
                            )
                    )
                    .into_any_element()
            } else {
                uniform_list(
                    "chat-list-scroll",
                    self.cached_filtered_chats.len(),
                    cx.processor(|this, range: Range<usize>, _window, cx| {
                        let theme = cx.app_theme();
                        let shape = cx.shape();
                        let surface = cx.surface();
                        let muted_text = theme.muted_foreground;
                        let text_color = theme.foreground;
                        let selected_chat_id = this.selected_chat_id.clone();
                        let base_url = if cx.has_global::<AuthState>() {
                            AuthState::global(cx).base_url.clone()
                        } else {
                            "http://127.0.0.1:3000/api".to_string()
                        };

                        let mut items = Vec::with_capacity(range.len());
                        for ix in range {
                            if let Some(chat) = this.cached_filtered_chats.get(ix) {
                                let select_mode = this.chat_select_mode;
                                let ticked = select_mode && this.selected_chats.contains(&chat.id);
                                let is_selected = if select_mode { ticked } else { selected_chat_id.as_deref() == Some(&chat.id) };
                                let chat_id = chat.id.clone();
                                let is_pinned = chat.pinned_at.is_some();
                                let is_muted = chat.mute_mode != "off" && !chat.mute_mode.is_empty();

                                let first_line = chat.last_msg.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
                                let last_msg_snippet = if first_line.is_empty() {
                                    "Tap to chat".to_string()
                                } else {
                                    MessageBubbleHelper::decode_content(first_line)
                                };

                                // Someone typing replaces the preview, in WhatsApp's green.
                                let typing_label = cx
                                    .has_global::<ChatStore>()
                                    .then(|| ChatStore::global(cx).typing_label(&chat.id, chat.is_group))
                                    .flatten();
                                let formatted_time = Self::format_chat_time(chat.last_time);
                                let unread = chat.unread;

                                let radius_card = px(shape.radius_row);
                                let card = h_flex()
                                    .id(SharedString::from(chat.id.clone()))
                                    .w_full()
                                    .h(px(72.0))
                                    .overflow_hidden()
                                    .px_3()
                                    .py_2p5()
                                    .rounded(radius_card)
                                    .gap_3()
                                    .items_center()
                                    .cursor_pointer()
                                    .relative()
                                    .bg(if is_selected { surface.chat_list_selected } else { rgba(0x00000000).into() })
                                    .hover(|s| s.bg(if is_selected { surface.chat_list_selected_hover } else { surface.chat_list_hover }))
                                    .when(shape.winui && is_selected, |s| {
                                        // Fluent's selection pill on the list item's leading edge.
                                        s.child(
                                            div()
                                                .absolute()
                                                .left(px(0.0))
                                                .top(px(28.0))
                                                .w(px(3.0))
                                                .h(px(16.0))
                                                .rounded(px(1.5))
                                                .bg(theme.primary),
                                        )
                                    })
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener({
                                            let cid = chat_id.clone();
                                            move |this, _, window, cx| {
                                                this.chat_context_menu = None;
                                                if this.chat_select_mode {
                                                    this.toggle_chat_select(&cid, cx);
                                                } else {
                                                    this.select_chat(cid.clone(), window, cx);
                                                }
                                            }
                                        }),
                                    )
                                    .on_mouse_down(
                                        MouseButton::Right,
                                        cx.listener({
                                            let c_id = chat_id.clone();
                                            let c_name = chat.name.clone();
                                            let pinned = is_pinned;
                                            let archived = chat.archived;
                                            let muted = is_muted;
                                            let group = chat.is_group;
                                            let unread_count = chat.unread as u32;
                                            move |this, ev: &MouseDownEvent, _, cx| {
                                                this.context_menu = None;
                                                this.chat_context_menu = Some(ChatContextMenu {
                                                    chat_id: c_id.clone(),
                                                    chat_name: c_name.clone(),
                                                    is_pinned: pinned,
                                                    is_archived: archived,
                                                    is_muted: muted,
                                                    position: ev.position,
                                                    show_mute_submenu: false,
                                                    is_group: group,
                                                    unread: unread_count,
                                                    confirm_leave: false,
                                                });
                                                cx.notify();
                                            }
                                        }),
                                    )
                                    // Tick circle while selecting chats
                                    .children(select_mode.then(|| {
                                        div()
                                            .w(px(20.0))
                                            .h(px(20.0))
                                            .flex_shrink_0()
                                            .rounded_full()
                                            .border_1()
                                            .border_color(theme.primary)
                                            .bg(if ticked { theme.primary } else { rgba(0x00000000).into() })
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .children(ticked.then(|| svg().data(CHECK_SVG).size(px(12.0)).text_color(rgb(0xffffff))))
                                    }))
                                    // Contact Avatar
                                    .child(
                                        div()
                                            .w(px(48.0))
                                            .h(px(48.0))
                                            .rounded_full()
                                            .flex_shrink_0()
                                            .overflow_hidden()
                                            .relative()
                                            .bg(avatar_color_for(&chat_id))
                                            .child(
                                                div()
                                                    .size_full()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .text_color(rgb(0xffffff))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_sm()
                                                    .child(
                                                        chat.name
                                                            .chars()
                                                            .next()
                                                            .map(|c| c.to_uppercase().to_string())
                                                            .unwrap_or_else(|| "#".to_string()),
                                                    ),
                                            )
                                            .children(if !chat.avatar.is_empty() {
                                                let a_url = resolve_avatar_url(&chat.avatar, &chat.id, &base_url);
                                                Some(
                                                    img(a_url)
                                                        .absolute()
                                                        .inset_0()
                                                        .size_full()
                                                        .rounded_full()
                                                        .object_fit(ObjectFit::Cover),
                                                )
                                            } else {
                                                None
                                            }),
                                    )
                                    // Chat Title & Last Message
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .min_w(px(0.0))
                                            .justify_center()
                                            .gap_0p5()
                                            .child(
                                                h_flex()
                                                    .items_center()
                                                    .gap_1p5()
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .truncate()
                                                            .text_sm()
                                                            .font_weight(if unread > 0 { FontWeight::BOLD } else { FontWeight::MEDIUM })
                                                            .text_color(text_color)
                                                            .child(chat.name.clone()),
                                                    )
                                                    .children(if is_muted {
                                                        Some(
                                                            svg()
                                                                .data(VOLUME_X_SVG)
                                                                .size(px(13.0))
                                                                .text_color(muted_text)
                                                                .flex_shrink_0(),
                                                        )
                                                    } else {
                                                        None
                                                    })
                                                    .children(if is_pinned {
                                                        Some(
                                                            svg()
                                                                .data(PIN_SVG)
                                                                .size(px(12.0))
                                                                .text_color(muted_text)
                                                                .flex_shrink_0(),
                                                        )
                                                    } else {
                                                        None
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .truncate()
                                                    .text_xs()
                                                    .text_color(if typing_label.is_some() {
                                                        Hsla::from(if theme.background.l < 0.5 { rgb(0x25d366) } else { rgb(0x008069) })
                                                    } else if unread > 0 {
                                                        text_color
                                                    } else {
                                                        muted_text
                                                    })
                                                    .when(typing_label.is_some(), |d| d.font_weight(FontWeight::MEDIUM))
                                                    .child(typing_label.unwrap_or(last_msg_snippet)),
                                            ),
                                    )
                                    // Timestamp & Unread Badge
                                    .child(
                                        v_flex()
                                            .items_end()
                                            .justify_center()
                                            .gap_1()
                                            .h_full()
                                            .flex_shrink_0()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(if unread > 0 { rgb(0x00a884).into() } else { muted_text })
                                                    .font_weight(if unread > 0 { FontWeight::BOLD } else { FontWeight::NORMAL })
                                                    .child(formatted_time),
                                            )
                                            .children(if unread > 0 {
                                                Some(
                                                    h_flex()
                                                        .gap_1p5()
                                                        .items_center()
                                                        .children(if chat.has_unread_mention {
                                                            Some(
                                                                div()
                                                                    .id(SharedString::from(format!("unread-mention-{}", chat.id)))
                                                                    .size(px(UNREAD_BADGE_SIZE))
                                                                    .flex_shrink_0()
                                                                    .rounded_full()
                                                                    .bg(rgb(0x00a884))
                                                                    .text_xs()
                                                                    .font_weight(FontWeight::BOLD)
                                                                    .text_color(rgb(0x111b21))
                                                                    .flex()
                                                                    .items_center()
                                                                    .justify_center()
                                                                    .tooltip(|window, cx| Tooltip::new("Unread mention").build(window, cx))
                                                                    .child("@"),
                                                            )
                                                        } else { None })
                                                        .child(
                                                            div()
                                                                .px_1()
                                                                .h(px(UNREAD_BADGE_SIZE))
                                                                .min_w(px(UNREAD_BADGE_SIZE))
                                                                .rounded_full()
                                                                .bg(rgb(0x00a884))
                                                                .text_xs()
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(rgb(0x111b21))
                                                                .flex()
                                                                .items_center()
                                                                .justify_center()
                                                                .child(format!("{}", unread)),
                                                        ),
                                                )
                                            } else {
                                                None
                                            }),
                                    )
                                    .into_any_element();
                                items.push(card);
                            }
                        }
                        items
                    }),
                )
                .size_full()
                .p_2()
                .track_scroll(&self.chat_list_scroll_handle)
                .into_any_element()
            })
            .child(
                Scrollbar::vertical(&self.chat_list_scroll_handle)
                    .mode(ScrollbarMode::Hover)
                    .styles(|s| {
                        s.track(|t| t.bg(transparent_black()))
                            .thumb(|th| th.bg(theme.muted_foreground.opacity(0.35)).radius(px(3.0)).width(px(6.0)))
                            .thumb_hover(|th| th.bg(theme.muted_foreground.opacity(0.65)).radius(px(4.0)).width(px(8.0)))
                            .thumb_active(|th| th.bg(theme.primary.opacity(0.8)).radius(px(4.0)).width(px(8.0)))
                    }),
            )
    }
}

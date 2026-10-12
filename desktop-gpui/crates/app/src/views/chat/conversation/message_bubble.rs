use crate::views::chat::*;

impl ChatView {
    /// One message bubble row (avatar/sender in groups, quoted reply, media, text, ticks, reactions).
    pub(crate) fn render_message_bubble(
        this: &mut ChatView,
        index: usize,
        window: &mut Window,
        cx: &mut Context<ChatView>,
        is_group_chat: bool,
    ) -> AnyElement {
        let theme = *cx.app_theme();
        let primary_color = theme.primary;
        let muted_text = theme.muted_foreground;
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };
        if let Some(r_msg) = this.cached_render_messages.get(index - 1) {
            let msg_idx = index - 1;
            let is_from_me = r_msg.is_from_me;
            let is_first_in_sequence = if msg_idx > 0 {
                if let Some(prev) = this.cached_render_messages.get(msg_idx - 1) {
                    prev.msg.from != r_msg.msg.from
                } else {
                    true
                }
            } else {
                true
            };

            let sender_jid = r_msg.msg.from.clone();
            let sender_name = r_msg.msg.sender_name.clone().unwrap_or_else(|| {
                sender_jid.split('@').next().unwrap_or(&sender_jid).to_string()
            });
            let sender_color = avatar_color_for(&sender_jid);

            let (content, mention_spans) = render_message_mentions_with_spans(&r_msg.content, &r_msg.msg, this.group_info.as_ref());
            let time_str = r_msg.time_str.clone();
            let ticks = r_msg.ticks;
            let reply_to_id = r_msg.msg.reply_to_id.clone();
            let quoted = r_msg.quoted.clone().or_else(|| reply_to_id.as_ref().map(|_| {
                ("Message".to_string(), "Open original message".to_string())
            }));
            // The quoted line is cached as raw text; resolve the original's
            // mentions here so it shows names instead of numeric IDs.
            let quoted = quoted.map(|(sender, line)| {
                let original = reply_to_id.as_deref().and_then(|rid| {
                    this.cached_render_messages.iter().find(|m| m.msg.id == rid)
                });
                let (line, spans) = match original {
                    Some(original) => render_message_mentions_with_spans(&line, &original.msg, this.group_info.as_ref()),
                    None => (line, Vec::new()),
                };
                (sender, line, spans)
            });
            let msg_clone = r_msg.msg.clone();

            let msg_type = r_msg.msg.message_type.as_str();
            let is_image = msg_type == "image" || (r_msg.msg.media_url.is_some() && (content == "[Image]" || content.is_empty()));
            let is_sticker = msg_type == "sticker" || (r_msg.msg.media_url.is_some() && content == "[Sticker]");
            let is_video = msg_type == "video" || (r_msg.msg.media_url.is_some() && content == "[Video]");
            // Documents keep their file name as content, so the type
            // (or a file extension) is the only reliable signal.
            let doc_file_name = content.clone();
            let is_document = msg_type == "document"
                || (r_msg.msg.media_url.is_some()
                    && looks_like_document_name(&doc_file_name));
            let image_has_caption = is_image
                && !content.is_empty()
                && content != "[Image]";
            let show_image_timestamp =
                is_image && r_msg.msg.media_url.is_some() && !image_has_caption;

            let is_dark_bg = theme.background.l < 0.5;
            // WhatsApp Web's mention colour.
            let mention_color: Hsla = if is_dark_bg { rgb(0x06cf9c).into() } else { rgb(0x008069).into() };
            let (bubble_bg, bubble_text) = if is_sticker {
                (rgba(0x00000000), if is_dark_bg { rgb(0xe9edef) } else { rgb(0x303030) })
            } else if let Some(c) = theme.chat {
                // Preset-defined bubbles (WhatsApp theme).
                if is_from_me {
                    (c.bubble_out.into(), c.bubble_out_text.into())
                } else {
                    (c.bubble_in.into(), c.bubble_in_text.into())
                }
            } else if is_dark_bg {
                if is_from_me {
                    (rgb(0x005c4b), rgb(0xe9edef))
                } else {
                    (rgb(0x202c33), rgb(0xe9edef))
                }
            } else {
                if is_from_me {
                    (rgb(0xdcf8c6), rgb(0x303030))
                } else {
                    (rgb(0xffffff), rgb(0x303030))
                }
            };

            let is_highlighted = this.highlighted_message_id.as_deref() == Some(&r_msg.msg.id);
            let base_bubble_bg: Hsla = bubble_bg.into();
            let final_bubble_bg = if is_highlighted { theme.primary.opacity(0.35) } else { base_bubble_bg };
            let mut bubble = if is_image {
                // Media bubbles sit close to the thumbnail; text bubbles keep the
                // roomier text padding.
                v_flex()
                    .max_w(px(520.0))
                    .px_1()
                    .py_1()
            } else {
                v_flex()
                    .max_w(px(520.0))
                    .px_3p5()
                    .py_2()
            }
                .rounded_2xl()
                .bg(final_bubble_bg)
                .shadow_sm()
                .gap_1();
            if is_highlighted {
                bubble = bubble.border_2().border_color(primary_color);
            }
            let bubble = bubble
                // Right-click opens Context Menu popover!
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener({
                        let m = msg_clone.clone();
                        let me = is_from_me;
                        move |this, ev: &MouseDownEvent, _, cx| {
                            this.context_menu = Some(MessageContextMenu {
                                msg: m.clone(),
                                is_from_me: me,
                                position: ev.position,
                            });
                            cx.notify();
                        }
                    }),
                )
                // Quoted reply box matching Web
                .children(if let Some((q_sender, q_line, q_spans)) = quoted {
                    Some(
                        v_flex()
                            .cursor_pointer()
                            .mb_1()
                            .px_2p5()
                            .py_1p5()
                            .bg(theme.muted.opacity(0.35))
                            .border_l_4()
                            .border_color(primary_color)
                            .rounded_r_lg()
                            .overflow_hidden()
                            .on_mouse_down(MouseButton::Left, cx.listener({
                                let target = reply_to_id.clone();
                                move |this, _, _, cx| {
                                    if let Some(target) = target.as_deref() {
                                        this.navigate_to_message(target, cx);
                                    }
                                }
                            }))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(primary_color)
                                    .child(q_sender),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted_text)
                                    .overflow_hidden()
                                    .child(
                                        StyledText::new(q_line).with_highlights(q_spans.into_iter().map(|range| {
                                            (range, HighlightStyle {
                                                color: Some(mention_color),
                                                font_weight: Some(FontWeight::MEDIUM),
                                                ..Default::default()
                                            })
                                        })),
                                    ),
                            ),
                    )
                } else {
                    None
                })
                // Media attachment: Image or Sticker
                .children(if is_image {
                    if let Some(ref m_url) = r_msg.msg.media_url {
                        let full_url = resolve_media_url(m_url, &base_url);
                        let click_url = full_url.clone();
                        // WhatsApp gives media a bounded thumbnail frame. Clamp each
                        // dimension independently, then cover-crop from the center when
                        // the image's aspect ratio does not match that frame.
                        let (image_w, image_h) =
                            match preview_load_state(&full_url, window, cx) {
                                PreviewLoad::Ready(nw, nh)
                                    if nw > 0.0 && nh > 0.0 =>
                                {
                                    let scale = (MESSAGE_IMAGE_MAX_WIDTH / nw)
                                        .min(MESSAGE_IMAGE_MAX_HEIGHT / nh)
                                        .min(1.0);
                                    (
                                        (nw * scale).clamp(
                                            MESSAGE_IMAGE_MIN_WIDTH,
                                            MESSAGE_IMAGE_MAX_WIDTH,
                                        ),
                                        (nh * scale).clamp(
                                            MESSAGE_IMAGE_MIN_HEIGHT,
                                            MESSAGE_IMAGE_MAX_HEIGHT,
                                        ),
                                    )
                                }
                                _ => (
                                    MESSAGE_IMAGE_MIN_WIDTH,
                                    MESSAGE_IMAGE_MIN_HEIGHT,
                                ),
                            };
                        let frame = div()
                                .w(px(image_w))
                                .h(px(image_h))
                                .rounded_xl()
                                .overflow_hidden()
                                .relative()
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, cx.listener({
                                    let u = click_url.clone();
                                    let open_msg = r_msg.msg.clone();
                                    let open_chat = r_msg.msg.chat_id.clone();
                                    move |this, _, _, cx| {
                                        this.open_preview(u.clone(), Some((open_chat.clone(), open_msg.clone())), cx);
                                    }
                                }))
                                .child(
                                    // The id is what lets gpui key frame state for
                                    // multi-frame GIF/WebP; without it only the first
                                    // frame ever renders.
                                    img(full_url)
                                        .id(format!("msg-image-{}", r_msg.msg.id))
                                        .w(px(image_w))
                                        .h(px(image_h))
                                        .rounded_xl()
                                        .object_fit(ObjectFit::Cover)
                                );
                        let frame = if show_image_timestamp {
                            frame.child(
                                h_flex()
                                    .absolute()
                                    .right(px(6.0))
                                    .bottom(px(6.0))
                                    .items_center()
                                    .gap_1()
                                    .px_1p5()
                                    .py(px(2.0))
                                    .rounded_lg()
                                    .bg(rgba(0x00000099))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(0xffffff))
                                            .child(time_str.clone()),
                                    )
                                    .children(if is_from_me {
                                        match ticks {
                                            MessageTicks::Read => Some(
                                                svg()
                                                    .data(CHECK_CHECK_SVG)
                                                    .size(px(14.0))
                                                    .text_color(rgb(0x53bdeb)),
                                            ),
                                            MessageTicks::Delivered => Some(
                                                svg()
                                                    .data(CHECK_CHECK_SVG)
                                                    .size(px(14.0))
                                                    .text_color(rgb(0xffffff).opacity(0.8)),
                                            ),
                                            MessageTicks::Sent => Some(
                                                svg()
                                                    .data(CHECK_SVG)
                                                    .size(px(14.0))
                                                    .text_color(rgb(0xffffff).opacity(0.8)),
                                            ),
                                            MessageTicks::Failed => Some(
                                                svg()
                                                    .data(X_SVG)
                                                    .size(px(12.0))
                                                    .text_color(rgb(0xef4444)),
                                            ),
                                            MessageTicks::None => None,
                                        }
                                    } else {
                                        None
                                    }),
                            )
                        } else {
                            frame
                        };
                        Some(frame.into_any_element())
                    } else {
                        None
                    }
                } else if is_sticker {
                    if let Some(ref m_url) = r_msg.msg.media_url {
                        let full_url = resolve_media_url(m_url, &base_url);
                        let click_url = full_url.clone();
                        Some(
                            div()
                                .w(px(160.0))
                                .h(px(160.0))
                                .cursor_pointer()
                                // Web parity: tapping a sticker opens it in the
                                // full-screen viewer, animated.
                                .on_mouse_down(MouseButton::Left, cx.listener({
                                    let open_msg = r_msg.msg.clone();
                                    let open_chat = r_msg.msg.chat_id.clone();
                                    move |this, _, _, cx| {
                                        this.open_preview(click_url.clone(), Some((open_chat.clone(), open_msg.clone())), cx);
                                    }
                                }))
                                .child(
                                    img(full_url)
                                        .id(format!("msg-sticker-{}", r_msg.msg.id))
                                        .w(px(160.0))
                                        .h(px(160.0))
                                        .object_fit(ObjectFit::Contain)
                                )
                                .into_any_element()
                        )
                    } else {
                        None
                    }
                } else if is_document {
                    // Web's document card: icon tile, file name,
                    // extension, then Open / Save As actions.
                    Some(Self::render_document_card(
                        &doc_file_name,
                        r_msg.msg.media_url.as_ref().map(|u| resolve_media_url(u, &base_url)),
                        is_dark_bg,
                        primary_color,
                        bubble_text,
                        cx,
                    ))
                } else {
                    None
                })
                // Message Body Text (hide placeholder if it's media without caption)
                .children({
                    let is_media_placeholder = (is_image && (content == "[Image]" || content.is_empty()))
                        || (is_sticker && (content == "[Sticker]" || content.is_empty()))
                        || (is_video && (content == "[Video]" || content.is_empty()))
                        || is_document;
                    if !content.is_empty() && !is_media_placeholder {
                        Some(
                            div()
                                .text_sm()
                                .text_color(bubble_text)
                                .child(MentionText::new(
                                    format!("message-body-{}", r_msg.msg.id),
                                    content.clone(),
                                    &mention_spans,
                                    mention_color,
                                ))
                        )
                    } else {
                        None
                    }
                })
                // Bottom Right Timestamp & Checkmarks
                .children((!show_image_timestamp).then(||
                    h_flex()
                        .w_full()
                        .justify_end()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(bubble_text.opacity(0.6))
                                .child(time_str),
                        )
                        .children(if is_from_me {
                            match ticks {
                                MessageTicks::Read => Some(
                                    svg()
                                        .data(CHECK_CHECK_SVG)
                                        .size(px(14.0))
                                        .text_color(rgb(0x53bdeb)),
                                ),
                                MessageTicks::Delivered => Some(
                                    svg()
                                        .data(CHECK_CHECK_SVG)
                                        .size(px(14.0))
                                        .text_color(bubble_text.opacity(0.6)),
                                ),
                                MessageTicks::Sent => Some(
                                    svg()
                                        .data(CHECK_SVG)
                                        .size(px(14.0))
                                        .text_color(bubble_text.opacity(0.6)),
                                ),
                                MessageTicks::Failed => Some(
                                    svg()
                                        .data(X_SVG)
                                        .size(px(12.0))
                                        .text_color(rgb(0xef4444)),
                                ),
                                MessageTicks::None => None,
                            }
                        } else {
                            None
                        }),
                ));

            // Reaction chips row (like web's ReactionChips):
            // floats above the bubble, tap toggles own reaction.
            let reactions_chips = {
                let reactions_list = r_msg.msg.reactions.clone().unwrap_or_default();
                if reactions_list.is_empty() {
                    None
                } else {
                    let cid_for_chips = this.selected_chat_id.clone().unwrap_or_default();
                    Some(
                        h_flex()
                            .flex_wrap()
                            .gap_1()
                            .mb(px(-2.0))
                            .relative()
                            .children(reactions_list.iter().map(|r| {
                                let em = r.emoji.clone();
                                let cid = cid_for_chips.clone();
                                let mid = r_msg.msg.id.clone();
                                let count_label = if r.senders.len() > 1 {
                                    format!("{}", r.senders.len())
                                } else {
                                    String::new()
                                };
                                div()
                                    .cursor_pointer()
                                    .px_1p5()
                                    .py(px(2.0))
                                    .rounded_full()
                                    .border_1()
                                    .border_color(theme.border.opacity(0.5))
                                    .bg(if is_dark_bg { rgba(0xffffff1a) } else { rgba(0x0000000d) })
                                    .text_size(px(12.0))
                                    .hover(|s| s.bg(if is_dark_bg { rgba(0xffffff33) } else { rgba(0x0000001a) }))
                                    .child(h_flex()
                                        .items_center()
                                        .gap(px(2.0))
                                        .child(r.emoji.clone())
                                        .children(if count_label.is_empty() { None } else { Some(
                                            div()
                                                .text_size(px(10.0))
                                                .font_weight(FontWeight::BOLD)
                                                .opacity(0.7)
                                                .child(count_label),
                                        ) }))
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                        this.react_message(cid.clone(), mid.clone(), em.clone(), cx);
                                    }))
                                    .into_any_element()
                            })),
                    )
                }
            };

            if is_from_me {
                v_flex()
                    .w_full()
                    .items_end()
                    .px_6()
                    .pb_2()
                    .children(reactions_chips)
                    .child(bubble)
                    .into_any_element()
            } else if is_group_chat {
                let avatar_slot = div()
                    .w(px(32.0))
                    .h(px(32.0))
                    .flex_shrink_0()
                    .children(if is_first_in_sequence {
                        Some(render_avatar(
                            &sender_jid,
                            &sender_name,
                            "",
                            false,
                            32.0,
                            &base_url,
                            &theme,
                        ))
                    } else {
                        None
                    });

                let msg_column = v_flex()
                    .gap_1()
                    .children(if is_first_in_sequence {
                        Some(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(sender_color)
                                .ml_1()
                                .child(sender_name),
                        )
                    } else {
                        None
                    })
                    .children(reactions_chips)
                    .child(bubble);

                h_flex()
                    .w_full()
                    .justify_start()
                    .items_start()
                    .gap_2p5()
                    .px_6()
                    .pb_2()
                    .child(avatar_slot)
                    .child(msg_column)
                    .into_any_element()
            } else {
                h_flex()
                    .w_full()
                    .justify_start()
                    .px_6()
                    .pb_2()
                    .child(
                        v_flex()
                            .items_start()
                            .children(reactions_chips)
                            .child(bubble),
                    )
                    .into_any_element()
            }
        } else {
            div().into_any_element()
        }
    }
}

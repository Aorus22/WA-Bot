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
        let chat_scale = ThemeManager::global(cx).chat_font_scale;
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
            // A reply to a status quotes the status (its own card), not a message.
            let status_reply = r_msg.msg.extra.as_ref().and_then(|extra| extra.status_reply.clone());
            let quoted = r_msg.quoted.clone().or_else(|| reply_to_id.as_ref().map(|_| {
                ("Message".to_string(), "Open original message".to_string())
            })).filter(|_| status_reply.is_none());
            let link_preview = r_msg.msg.extra.as_ref().and_then(|extra| extra.link_preview.clone());
            let is_pinned = this.is_pinned(&r_msg.msg.id);
            // The quoted line is cached as raw text; resolve the original's
            // mentions here so it shows names instead of numeric IDs.
            let quoted_original = reply_to_id
                .as_deref()
                .and_then(|rid| this.cached_render_messages.iter().find(|m| m.msg.id == rid))
                .map(|m| m.msg.clone());
            let quoted = quoted.map(|(sender, line)| {
                let (line, spans) = match quoted_original.as_ref() {
                    Some(original) => render_message_mentions_with_spans(&line, original, this.group_info.as_ref()),
                    None => (line, Vec::new()),
                };
                // A picture, voice note or file has no text to quote: say what it is.
                let is_placeholder = line.trim().is_empty() || (line.starts_with('[') && line.ends_with(']'));
                match quoted_original.as_ref() {
                    Some(original) if is_placeholder => (sender, quote_media_label(original), Vec::new()),
                    _ => (sender, line, spans),
                }
            });
            // Pictures and stickers show a small thumbnail at the end of the quote.
            let quoted_thumb = quoted_original
                .as_ref()
                .filter(|m| matches!(m.message_type.as_str(), "image" | "sticker" | "gif"))
                .and_then(|m| m.media_url.as_deref())
                .map(|url| resolve_media_url(url, &base_url));
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
            let selection_color: Hsla = if is_dark_bg { rgba(0xffffff40).into() } else { rgba(0x00000026).into() };
            let mention_color: Hsla = if is_dark_bg { rgb(0x06cf9c).into() } else { rgb(0x008069).into() };
            let link_color: Hsla = if is_dark_bg { rgb(0x53bdeb).into() } else { rgb(0x027eb5).into() };
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

            let rich_card: Option<AnyElement> = match (msg_type, r_msg.msg.extra.as_ref()) {
                ("poll", Some(extra)) => extra.poll.as_ref().map(|poll| {
                    let chat_id = this.selected_chat_id.clone().unwrap_or_default();
                    Self::render_poll_card(&chat_id, &r_msg.msg.id, poll, primary_color, bubble_text, cx)
                }),
                ("location", Some(extra)) => extra.location.as_ref().map(|location| {
                    Self::render_location_card(&r_msg.msg.id, location, &base_url, primary_color, cx)
                }),
                ("contact", Some(extra)) => extra.contact.as_ref().map(|contact| {
                    Self::render_contact_card(&r_msg.msg.id, contact, primary_color, cx)
                }),
                _ => None,
            };
            let has_rich_card = rich_card.is_some();
            let has_edits = r_msg.msg.extra.as_ref().is_some_and(|extra| !extra.edits.is_empty());

            // Pin / star / disappearing icons that sit beside the time.
            let footer_marks = (!show_image_timestamp)
                .then(|| Self::footer_marks(&r_msg.msg, is_pinned, Hsla::from(bubble_text)));
            let overlay_marks = if show_image_timestamp {
                Self::footer_marks(&r_msg.msg, is_pinned, rgb(0xffffff).into())
            } else {
                Vec::new()
            };

            let is_highlighted = this.highlighted_message_id.as_deref() == Some(&r_msg.msg.id);
            let base_bubble_bg: Hsla = bubble_bg.into();
            let final_bubble_bg = if is_highlighted { theme.primary.opacity(0.35) } else { base_bubble_bg };
            // Like WhatsApp: fit the picture inside the bounded frame keeping its
            // aspect ratio, enlarging small ones to fill it. Only extreme aspect
            // ratios hit the minimum size and get cover-cropped; everything else
            // shows in full. The bubble is exactly as wide as the picture, so a
            // caption wraps to the picture's width instead of widening the bubble.
            let image_dims: Option<(f32, f32)> = if is_image {
                r_msg.msg.media_url.as_ref().map(|m_url| {
                    let full_url = resolve_media_url(m_url, &base_url);
                    match preview_load_state(&full_url, window, cx) {
                        PreviewLoad::Ready(nw, nh) if nw > 0.0 && nh > 0.0 => {
                            let scale = (MESSAGE_IMAGE_MAX_WIDTH / nw)
                                .min(MESSAGE_IMAGE_MAX_HEIGHT / nh);
                            (
                                (nw * scale).clamp(MESSAGE_IMAGE_MIN_WIDTH, MESSAGE_IMAGE_MAX_WIDTH),
                                (nh * scale).clamp(MESSAGE_IMAGE_MIN_HEIGHT, MESSAGE_IMAGE_MAX_HEIGHT),
                            )
                        }
                        _ => (MESSAGE_IMAGE_MIN_WIDTH, MESSAGE_IMAGE_MIN_HEIGHT),
                    }
                })
            } else {
                None
            };
            let mut bubble = if is_image {
                // Media bubbles sit close to the thumbnail; text bubbles keep the
                // roomier text padding.
                // `px_1` is 4px a side, hence the 8px added to the picture's width.
                let media_bubble = v_flex().max_w(px(520.0)).px_1().py_1();
                match image_dims {
                    Some((image_w, _)) => media_bubble.w(px(image_w + 8.0)),
                    None => media_bubble,
                }
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
                                reaction_picker: false,
                                pin_picker: false,
                            });
                            cx.notify();
                        }
                    }),
                )
                // Quoted reply box matching Web
                .children(if let Some((q_sender, q_line, q_spans)) = quoted {
                    let quote_hover_bg = theme.muted.opacity(0.6);
                    Some(
                        h_flex()
                            .id(format!("quote-{}", r_msg.msg.id))
                            .cursor_pointer()
                            .mb_1()
                            .items_center()
                            .gap_2()
                            .px_2p5()
                            .py_1p5()
                            .bg(theme.muted.opacity(0.35))
                            .hover(move |s| s.bg(quote_hover_bg))
                            .border_l_4()
                            .border_color(primary_color)
                            .rounded_r_lg()
                            .overflow_hidden()
                            .on_mouse_down(MouseButton::Left, cx.listener({
                                let target = reply_to_id.clone();
                                move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    if let Some(target) = target.as_deref() {
                                        this.navigate_to_message(target, cx);
                                    }
                                }
                            }))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .chat_text(0.75, 1.0, chat_scale)
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(primary_color)
                                            .child(q_sender),
                                    )
                                    .child(
                                        div()
                                            .chat_text(0.75, 1.0, chat_scale)
                                            .text_color(muted_text)
                                            .overflow_hidden()
                                            .child(
                                                StyledText::new(q_line).with_highlights(q_spans.into_iter().map(|span| {
                                                    (span.range, HighlightStyle {
                                                        color: Some(mention_color),
                                                        font_weight: Some(FontWeight::MEDIUM),
                                                        ..Default::default()
                                                    })
                                                })),
                                            ),
                                    ),
                            )
                            .children(quoted_thumb.map(|url| {
                                img(url)
                                    .id(format!("quote-thumb-{}", r_msg.msg.id))
                                    .w(px(40.0))
                                    .h(px(40.0))
                                    .flex_shrink_0()
                                    .rounded_md()
                                    .object_fit(ObjectFit::Cover)
                            })),
                    )
                } else {
                    None
                })
                // Deleted for everyone by the sender; we keep it and say so.
                .children(r_msg.msg.revoked_at.map(|_| {
                    div()
                        .mb_1()
                        .text_xs()
                        .italic()
                        .text_color(rgb(0xe5484d))
                        .child("🚫 Deleted for everyone · kept here")
                }))
                // Quote of the status this message replies to
                .children(status_reply.as_ref().map(|reply| {
                    Self::render_status_reply_card(reply, &base_url, primary_color, muted_text, theme.muted, chat_scale)
                }))
                // Media attachment: Image or Sticker
                .children(if is_image {
                    if let Some(ref m_url) = r_msg.msg.media_url {
                        let full_url = resolve_media_url(m_url, &base_url);
                        let click_url = full_url.clone();
                        let (image_w, image_h) = image_dims.unwrap_or((
                            MESSAGE_IMAGE_MIN_WIDTH,
                            MESSAGE_IMAGE_MIN_HEIGHT,
                        ));
                        let frame = div()
                                .w(px(image_w))
                                .h(px(image_h))
                                .rounded_xl()
                                .overflow_hidden()
                                .relative()
                                .cursor_pointer()
                                .when(r_msg.msg.status == "pending", |f| f.opacity(0.7))
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
                                    img(match m_url.strip_prefix(LOCAL_MEDIA_PREFIX) {
                                        Some(path) => ImageSource::from(std::path::PathBuf::from(path)),
                                        None => ImageSource::from(full_url),
                                    })
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
                                    .children(overlay_marks)
                                    .child(
                                        div()
                                            .chat_text(0.75, 1.0, chat_scale)
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
                // Poll, location and contact cards (the text is only a placeholder for them)
                .children(rich_card.map(|card| div().text_color(bubble_text).child(card)))
                // Preview of the link in the text, the backend's own scrape of it
                .children(link_preview.as_ref().map(|preview| {
                    Self::render_link_preview_card(&r_msg.msg.id, preview, &base_url, primary_color, bubble_text, chat_scale, cx)
                }))
                // Message Body Text (hide placeholder if it's media without caption)
                .children({
                    let is_media_placeholder = (is_image && (content == "[Image]" || content.is_empty()))
                        || (is_sticker && (content == "[Sticker]" || content.is_empty()))
                        || (is_video && (content == "[Video]" || content.is_empty()))
                        || is_document
                        || has_rich_card;
                    if !content.is_empty() && !is_media_placeholder {
                        Some(
                            div()
                                .chat_text(0.875, 1.25, chat_scale)
                                .text_color(bubble_text)
                                .child({
                                    let view = cx.entity();
                                    // WhatsApp formatting (not for markdown-mode messages,
                                    // whose `*`/`_` mean something else).
                                    let formatted = (crate::text_format::enabled(cx)
                                        && !MessageBubbleHelper::is_markdown(&r_msg.msg.content))
                                    .then(|| crate::text_format::parse(&content));
                                    let (body_text, body_mentions, body_formats) = match &formatted {
                                        Some(f) => (
                                            f.text.clone(),
                                            mention_spans
                                                .iter()
                                                .map(|m| (f.map_range(&m.range), SharedString::from(m.jid.clone())))
                                                .collect::<Vec<_>>(),
                                            f.spans.clone(),
                                        ),
                                        None => (
                                            content.clone(),
                                            mention_spans
                                                .iter()
                                                .map(|m| (m.range.clone(), SharedString::from(m.jid.clone())))
                                                .collect(),
                                            Vec::new(),
                                        ),
                                    };
                                    MentionText::new(
                                        format!("message-body-{}", r_msg.msg.id),
                                        body_text,
                                        body_mentions,
                                        mention_color,
                                    )
                                    .formats(body_formats)
                                    .link_color(link_color)
                                    .on_text_menu({
                                        let view = view.clone();
                                        move |request, _, cx| {
                                            view.update(cx, |this, cx| {
                                                this.context_menu = None;
                                                this.text_menu = Some(request);
                                                cx.notify();
                                            });
                                        }
                                    })
                                    .selection_color(selection_color)
                                    .on_click({
                                        let content = content.clone();
                                        let spans = mention_spans.clone();
                                        move |jid, window, cx| {
                                            let name = spans.iter().find(|m| m.jid == jid)
                                                .map(|m| content[m.range.clone()].trim_start_matches('@').to_string())
                                                .unwrap_or_default();
                                            view.update(cx, |this, cx| this.open_direct_chat(jid, &name, window, cx));
                                        }
                                    })
                                })
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
                        .children(footer_marks.unwrap_or_default())
                        .children(has_edits.then(|| {
                            let msg_id = r_msg.msg.id.clone();
                            div()
                                .id(SharedString::from(format!("edited-{msg_id}")))
                                .chat_text(0.75, 1.0, chat_scale)
                                .cursor_pointer()
                                .text_color(bubble_text.opacity(0.6))
                                .hover(|s| s.text_color(bubble_text.opacity(0.9)))
                                .child("Edited")
                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                                    cx.stop_propagation();
                                    this.context_menu = None;
                                    this.text_menu = None;
                                    this.edit_history = Some(EditHistoryPopover {
                                        msg_id: msg_id.clone(),
                                        position: ev.position,
                                    });
                                    cx.notify();
                                }))
                        }))
                        .child(
                            div()
                                .chat_text(0.75, 1.0, chat_scale)
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
                                    .chat_text(0.75, 1.0, chat_scale)
                                    .hover(|s| s.bg(if is_dark_bg { rgba(0xffffff33) } else { rgba(0x0000001a) }))
                                    .child(h_flex()
                                        .items_center()
                                        .gap(px(2.0))
                                        .child(r.emoji.clone())
                                        .children(if count_label.is_empty() { None } else { Some(
                                            div()
                                                .chat_text(0.625, 0.875, chat_scale)
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
                                .chat_text(0.75, 1.0, chat_scale)
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

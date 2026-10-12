use crate::views::chat::*;

impl ChatView {
    /// Bottom compose area: reply/edit banner, attach/emoji panels and the input row.
    pub(crate) fn render_composer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = *cx.app_theme();
        let border_color = theme.border;
        let bg_color = theme.background;
        let card_bg = theme.card;
        let primary_color = theme.primary;
        let text_color = theme.foreground;
        let muted_text = theme.muted_foreground;
        let framed = !crate::popover::is_popover(cx)
            && window_material::is_framed_window(window);
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };
        let compose_rows = self.compose_text.split('\n').count().clamp(1, 6);
        let compose_textarea_height = px(compose_rows as f32 * 20.0 + 12.0);
        v_flex()
            .relative()
            .border_t_1()
            .border_color(border_color.opacity(0.4))
            .bg(bg_color)
            .when(framed, |s| s.rounded_br(px(window_material::WINDOW_CORNER_RADIUS)))
            // Reply Preview Banner
            .children(if let Some(ref r_msg) = self.reply_to {
                let r_sender = if r_msg.from == "me" { "You" } else { r_msg.sender_name.as_deref().unwrap_or("Sender") };
                let r_content = MessageBubbleHelper::decode_content(&r_msg.content);
                let r_first = r_content.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
                let (r_first, r_spans) = render_message_mentions_with_spans(r_first, r_msg, self.group_info.as_ref());
                Some(
                    h_flex()
                        .mx_3()
                        .mt_2p5()
                        .px_3()
                        .py_2()
                        .bg(theme.muted.opacity(0.4))
                        .border_l_4()
                        .border_color(primary_color)
                        .rounded_r_xl()
                        .items_center()
                        .justify_between()
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w(px(0.0))
                                .overflow_hidden()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(primary_color)
                                        .child(format!("Replying to {}", r_sender)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(muted_text)
                                        .overflow_hidden()
                                        .child(StyledText::new(r_first).with_highlights(r_spans.into_iter().map(|span| {
                                            (span.range, HighlightStyle {
                                                color: Some(if theme.background.l < 0.5 { rgb(0x06cf9c).into() } else { rgb(0x008069).into() }),
                                                font_weight: Some(FontWeight::MEDIUM),
                                                ..Default::default()
                                            })
                                        }))),
                                ),
                        )
                        .child(
                            div()
                                .cursor_pointer()
                                .p_1()
                                .rounded_full()
                                .hover(|s| s.bg(theme.muted.opacity(0.6)))
                                .child(svg().data(X_SVG).size(px(14.0)).text_color(muted_text))
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                    this.cancel_reply(window, cx);
                                })),
                        ),
                )
            } else if let Some(ref e_msg) = self.editing_message {
                let e_content = MessageBubbleHelper::decode_content(&e_msg.content);
                let e_first = e_content.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
                Some(
                    h_flex()
                        .mx_3()
                        .mt_2p5()
                        .px_3()
                        .py_2()
                        .bg(theme.muted.opacity(0.4))
                        .border_l_4()
                        .border_color(rgb(0xf97316))
                        .rounded_r_xl()
                        .items_center()
                        .justify_between()
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w(px(0.0))
                                .overflow_hidden()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(0xf97316))
                                        .child("Editing message"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(muted_text)
                                        .overflow_hidden()
                                        .child(e_first.to_string()),
                                ),
                        )
                        .child(
                            div()
                                .cursor_pointer()
                                .p_1()
                                .rounded_full()
                                .hover(|s| s.bg(theme.muted.opacity(0.6)))
                                .child(svg().data(X_SVG).size(px(14.0)).text_color(muted_text))
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                    this.cancel_edit(window, cx);
                                })),
                        ),
                )
            } else {
                None
            })
            // Pasted image waiting to be sent (the typed text becomes its caption)
            .children(self.pending_attachment.as_ref().map(|att| {
                h_flex()
                    .mx_3()
                    .mt_2p5()
                    .px_3()
                    .py_2()
                    .gap_3()
                    .bg(theme.muted.opacity(0.4))
                    .border_l_4()
                    .border_color(primary_color)
                    .rounded_r_xl()
                    .items_center()
                    .justify_between()
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w(px(0.0))
                            .gap_3()
                            .items_center()
                            .child(
                                img(ImageSource::Image(att.image.clone()))
                                    .w(px(56.0))
                                    .h(px(56.0))
                                    .flex_shrink_0()
                                    .rounded_lg()
                                    .object_fit(ObjectFit::Cover),
                            )
                            .child(
                                v_flex()
                                    .min_w(px(0.0))
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(primary_color)
                                            .child("Image ready to send"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(muted_text)
                                            .overflow_hidden()
                                            .child(att.file_name.clone()),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .id("btn-remove-pending-attachment")
                            .cursor_pointer()
                            .p_1()
                            .rounded_full()
                            .hover(|s| s.bg(theme.muted.opacity(0.6)))
                            .child(svg().data(X_SVG).size(px(14.0)).text_color(muted_text))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.clear_pending_attachment(cx);
                            })),
                    )
            }))
            // Attach (+) menu, emoji and sticker panels
            .children(self.render_attach_panel(
                theme,
                border_color,
                card_bg,
                primary_color,
                text_color,
                muted_text,
                cx,
            ))
            .children(self.mention_query.as_ref().and_then(|query| {
                self.group_info.as_ref().map(|group| {
                    let needle = query.to_lowercase();
                    let members: Vec<(String, String, bool)> = group.participants.iter()
                        .filter_map(|participant| {
                            let name = participant.name.as_deref()
                                .filter(|name| !name.trim().is_empty())
                                .unwrap_or_else(|| participant.jid.split('@').next().unwrap_or(&participant.jid));
                            if !needle.is_empty()
                                && !name.to_lowercase().contains(&needle)
                                && !participant.jid.to_lowercase().contains(&needle)
                            {
                                return None;
                            }
                            Some((participant.jid.clone(), name.to_string(), participant.is_admin))
                        })
                        .take(8)
                        .collect();
                    if members.is_empty() {
                        return None;
                    }
                    Some(
                        v_flex()
                            .occlude()
                            .mx_3()
                            .mb_2()
                            .max_h(px(240.0))
                            .overflow_y_scrollbar()
                            .p_1()
                            .rounded_xl()
                            .bg(card_bg)
                            .border_1()
                            .border_color(border_color)
                            .shadow_xl()
                            .children(members.into_iter().map(|(jid, name, is_admin)| {
                                h_flex()
                                    .id(SharedString::from(format!("mention-{jid}")))
                                    .w_full()
                                    .cursor_pointer()
                                    .items_center()
                                    .gap_3()
                                    .px_3()
                                    .py_2()
                                    .rounded_lg()
                                    .hover(|style| style.bg(theme.muted.opacity(0.55)))
                                    .child(render_avatar(&jid, &name, "", false, 32.0, &base_url, &theme))
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .child(div().text_sm().text_color(text_color).child(name))
                                            .child(div().text_xs().text_color(muted_text).child(jid.clone())),
                                    )
                                    .children(if is_admin { Some(div().text_xs().text_color(primary_color).child("admin")) } else { None })
                                    .on_mouse_down(MouseButton::Left, cx.listener({
                                        let target_jid = jid.clone();
                                        move |this, _, window, cx| {
                                            this.insert_group_mention(&target_jid, window, cx);
                                        }
                                    }))
                                    .into_any_element()
                            }))
                            .with_animation(
                                "mention-list-in",
                                crate::components::motion::enter(crate::components::motion::FAST_MS),
                                |el, d| el.opacity(d),
                            )
                            .into_any_element(),
                    )
                })
            }).flatten())
            // Input Row matching Web
            .child(
                h_flex()
                    .p_3()
                    .items_center()
                    .gap_2()
                    // Plus / Attachment icon
                    .child(
                        div()
                            .id("btn-attach")
                            .p_2()
                            .rounded_full()
                            .cursor_pointer()
                            .bg(if self.attach_panel.is_open() {
                                primary_color.opacity(0.15)
                            } else {
                                rgba(0x00000000).into()
                            })
                            .hover(|s| s.bg(theme.muted.opacity(0.65)))
                            .tooltip(move |window, cx| {
                                Tooltip::new("Attach").build(window, cx)
                            })
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.toggle_attach_panel(cx);
                            }))
                            .child(svg().data(PLUS_SVG).size(px(18.0)).text_color(
                                if self.attach_panel.is_open() { primary_color } else { muted_text }
                            )),
                    )
                    // Mic icon
                    .child(
                        div()
                            .id("btn-mic")
                            .p_2()
                            .rounded_full()
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.muted.opacity(0.65)))
                            .tooltip(move |window, cx| {
                                Tooltip::new("Voice message").build(window, cx)
                            })
                            .child(svg().data(MIC_SVG).size(px(18.0)).text_color(muted_text)),
                    )
                    // Text Input Field with Integrated Send Button
                    .child(
                        h_flex()
                            .flex_1()
                            .px_4()
                            .py_2()
                            .rounded_2xl()
                            .border_1()
                            .border_color(border_color.opacity(0.2))
                            .bg(theme.muted.opacity(0.5))
                            .items_center()
                            .gap_2()
                            .cursor_text()
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                this.focus_composer(window, cx);
                            }))
                            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                                if ev.keystroke.key.as_str() == "escape" {
                                    if this.reply_to.is_some() {
                                        cx.stop_propagation();
                                        this.cancel_reply(window, cx);
                                    } else if this.editing_message.is_some() {
                                        cx.stop_propagation();
                                        this.cancel_edit(window, cx);
                                    }
                                }
                            }))
                            .child({
                                let state = self.compose_textarea.as_ref().expect("composer textarea initialized");
                                let view = cx.entity();
                                Textarea::new(state)
                                    .on_paste(move |item, _window, cx| {
                                        view.update(cx, |this, cx| this.stage_clipboard_image(item, cx))
                                    })
                                    .appearance(false)
                                    .bordered(false)
                                    .w_full()
                                    .h(compose_textarea_height)
                                    .max_h(px(150.0))
                                    .text_sm()
                                    .text_color(text_color)
                                    .into_any_element()
                            })
                            // Send Button: Integrated inside input bar on the right
                            .child({
                                let has_text = !self.compose_text.trim().is_empty() || self.pending_attachment.is_some();
                                div()
                                    .id("btn-send")
                                    .cursor_pointer()
                                    .p_1p5()
                                    .rounded_xl()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .flex_shrink_0()
                                    .bg(if has_text { primary_color } else { rgba(0x00000000).into() })
                                    .hover(|h| {
                                        if has_text {
                                            h.opacity(0.85)
                                        } else {
                                            h.bg(theme.muted.opacity(0.5))
                                        }
                                    })
                                    .tooltip(move |window, cx| {
                                        Tooltip::new("Send message").build(window, cx)
                                    })
                                    .child(
                                        svg()
                                            .data(SEND_SVG)
                                            .size(px(16.0))
                                            .text_color(if has_text {
                                                theme.primary_foreground
                                            } else {
                                                muted_text.opacity(0.6)
                                            }),
                                    )
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                        this.send_current_message(window, cx);
                                    }))
                            }),
                    ),
            )
    }
}

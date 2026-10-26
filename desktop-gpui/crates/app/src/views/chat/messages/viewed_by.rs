use crate::components::ui::boxed_list::boxed_list;
use crate::theme::surface::SurfaceExt;
use crate::views::chat::*;

/// "Today at 22:03", "Yesterday at 22:03" or "7 September 2026 at 22:03".
fn format_receipt_time(timestamp: i64) -> String {
    let (hours, mins) = crate::localtime::hour_minute(timestamp);
    format!("{} at {:02}:{:02}", MessageBubbleHelper::format_date_label(timestamp), hours, mins)
}

impl ChatView {
    /// Right-hand "Message info" drawer for an outgoing group message: who has
    /// read it and who only has it delivered, like WhatsApp's own sheet.
    pub(crate) fn render_message_info_sheet(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = *cx.app_theme();
        let text_color = theme.foreground;
        let muted_text = theme.muted_foreground;
        let primary_color = theme.primary;
        let surface = cx.surface();
        let sheet_border = surface.sheet_border;
        let framed = !crate::popover::is_popover(cx) && window_material::is_framed_window(window);
        let viewport_width = f32::from(window.viewport_size().width);
        let sheet_width = (viewport_width * 0.70).min(SHEET_WIDTH).max(240.0);
        let media_width = (sheet_width - 72.0).clamp(180.0, 240.0);
        let media_height = (media_width * 0.82).clamp(150.0, 205.0);
        let base_url = Self::compose_base_url(cx);
        let media_base = base_url.clone();
        let group = self.group_info.clone();
        let message = self.viewed_by_message.clone();

        let is_group = message.as_ref().map(|m| m.chat_id.ends_with("@g.us")).unwrap_or(false);
        let status = message.as_ref().map(|m| m.status.clone()).unwrap_or_default();
        let user_of = |jid: &str| jid.split('@').next().unwrap_or(jid).to_string();

        // Members who read it, then members who only have it delivered. A read
        // implies a delivery, so each member shows up once.
        let mut read: Vec<MessageReceipt> =
            self.viewed_by_receipts.iter().filter(|r| r.read_at > 0).cloned().collect();
        read.sort_by_key(|r| r.read_at);
        let mut delivered: Vec<MessageReceipt> = self
            .viewed_by_receipts
            .iter()
            .filter(|r| r.read_at == 0 && r.delivered_at > 0)
            .cloned()
            .collect();
        delivered.sort_by_key(|r| r.delivered_at);
        // Members WhatsApp has not reported on yet.
        let remaining: Vec<String> = self
            .viewed_by_recipients
            .iter()
            .filter(|jid| {
                !self.viewed_by_receipts.iter().any(|r| {
                    user_of(&r.jid) == user_of(jid) && (r.read_at > 0 || r.delivered_at > 0)
                })
            })
            .cloned()
            .collect();
        let loading = self.is_loading_viewed_by;

        let name_of = {
            let group = group.clone();
            move |jid: &str| -> String {
                group
                    .as_ref()
                    .and_then(|group| {
                        group.participants.iter().find(|member| {
                            member.jid == jid || member.jid.split('@').next() == jid.split('@').next()
                        })
                    })
                    .and_then(|member| member.name.as_deref())
                    .filter(|name| !name.trim().is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| jid.split('@').next().unwrap_or(jid).to_string())
            }
        };
        let member_row = {
            let name_of = name_of.clone();
            move |jid: &str, at: Option<i64>| {
                let name = name_of(jid);
                h_flex()
                    .items_center()
                    .gap_3()
                    .px_4()
                    .py_2()
                    .child(render_avatar(jid, &name, "", false, 40.0, &base_url, &theme))
                    .child(
                        v_flex()
                            .min_w_0()
                            .child(div().text_sm().text_color(text_color).child(name))
                            .children(at.map(|at| {
                                div().text_xs().text_color(muted_text).child(format_receipt_time(at))
                            })),
                    )
            }
        };

        let section_header = |label: &'static str, tick: &'static [u8], tick_color: Hsla| {
            h_flex()
                .items_center()
                .gap_3()
                .px_4()
                .pt_4()
                .pb_1()
                .child(svg().data(tick).size(px(16.0)).text_color(tick_color))
                .child(div().text_sm().text_color(muted_text).child(label))
        };
        // A private chat has one recipient, so each stage is one line with its time.
        let stage_row = |label: &'static str, tick_color: Hsla, at: i64, known: bool| {
            v_flex()
                .px_4()
                .py_2()
                .gap_0p5()
                .child(
                    h_flex()
                        .items_center()
                        .gap_3()
                        .child(svg().data(CHECK_CHECK_SVG).size(px(16.0)).text_color(tick_color))
                        .child(div().text_sm().text_color(text_color).child(label)),
                )
                .child(div().pl(px(28.0)).text_xs().text_color(muted_text).child(if at > 0 {
                    format_receipt_time(at)
                } else if known {
                    "Time not recorded".to_string()
                } else {
                    "—".to_string()
                }))
        };

        let mut body = v_flex().id("message-info-scroll").flex_1().overflow_y_scroll().pb_4();

        if let Some(message) = message.as_ref() {
            let text = MessageBubbleHelper::decode_content(&message.content);
            // Media shows itself; a placeholder such as "[Sticker]" is not a caption.
            let is_placeholder = text.trim().is_empty()
                || (text.starts_with('[') && text.ends_with(']') && !text.contains(' '));
            let media_url = message.media_url.as_ref().map(|url| resolve_media_url(url, &media_base));
            let media: Option<AnyElement> = match message.message_type.as_str() {
                "sticker" => media_url.map(|url| {
                    img(url).id("info-sticker").w(px(140.0)).h(px(140.0)).object_fit(ObjectFit::Contain).into_any_element()
                }),
                "image" | "gif" => media_url.map(|url| {
                    img(url)
                        .id("info-image")
                        .w(px(media_width))
                        .h(px(media_height))
                        .rounded_lg()
                        .object_fit(ObjectFit::Contain)
                        .into_any_element()
                }),
                "video" => Some(
                    video_preview(
                        "info-video",
                        message.extra.as_ref()
                            .and_then(|extra| extra.thumbnail.as_deref())
                            .map(|url| resolve_media_url(url, &media_base)),
                        media_width,
                        media_height * 0.72,
                    )
                    .into_any_element(),
                ),
                _ => None,
            };
            if let Some(media) = media {
                let sticker = message.message_type == "sticker";
                body = body.child(
                    h_flex().w_full().justify_end().px_3().pt_3().child(
                        v_flex()
                            .max_w(px(media_width + 16.0))
                            .gap_1()
                            .when(!sticker, |b| b.p_1().rounded_lg().bg(primary_color.opacity(0.22)))
                            .child(media)
                            .when(!is_placeholder, |b| {
                                b.child(div().px_2().pb_1().text_sm().text_color(text_color).child(text.clone()))
                            }),
                    ),
                );
            } else {
            body = body.child(
                h_flex().w_full().justify_end().px_3().py_3().child(
                    div()
                        .max_w(px(media_width + 24.0))
                        .px_3()
                        .py_2()
                        .rounded_lg()
                        .bg(primary_color.opacity(0.22))
                        .text_sm()
                        .text_color(text_color)
                        .child(if is_placeholder { format!("[{}]", message.message_type) } else { text }),
                ),
            );
            }
        }

        if loading {
            body = body.child(
                v_flex()
                    .py_8()
                    .items_center()
                    .gap_2()
                    .child(Spinner::new().color(primary_color))
                    .child(div().text_sm().text_color(muted_text).child("Loading…")),
            );
        } else if !is_group {
            // The stored tick covers messages that predate per-stage times.
            let read_at = read.first().map(|r| r.read_at).unwrap_or(0);
            let delivered_at = self
                .viewed_by_receipts
                .iter()
                .map(|r| if r.delivered_at > 0 { r.delivered_at } else { r.read_at })
                .find(|at| *at > 0)
                .unwrap_or(0);
            let is_read = read_at > 0 || status == "read";
            let is_delivered = is_read || delivered_at > 0 || status == "delivered";
            if is_read {
                body = body.child(stage_row("Read", rgb(0x53bdeb).into(), read_at, true));
            }
            if is_delivered {
                body = body.child(stage_row("Delivered", muted_text, delivered_at, true));
            }
            if !is_delivered {
                body = body.child(
                    div().px_4().py_8().text_sm().text_color(muted_text).child("Not delivered yet."),
                );
            }
        } else if read.is_empty() && delivered.is_empty() && remaining.is_empty() {
            body = body.child(
                div()
                    .px_4()
                    .py_4()
                    .text_sm()
                    .text_color(muted_text)
                    .child("No receipts reported yet."),
            );
        } else {
            if !read.is_empty() {
                body = body
                    .child(section_header("Read by", CHECK_CHECK_SVG, rgb(0x53bdeb).into()))
                    .child(boxed_list(cx, read.iter().map(|receipt| member_row(&receipt.jid, Some(receipt.read_at)))));
            }
            if !delivered.is_empty() {
                body = body
                    .child(section_header("Delivered to", CHECK_CHECK_SVG, muted_text))
                    .child(boxed_list(cx, delivered.iter().map(|receipt| member_row(&receipt.jid, Some(receipt.delivered_at)))));
            }
            if !remaining.is_empty() {
                body = body
                    .child(section_header("Remaining", CHECK_SVG, muted_text))
                    .child(boxed_list(cx, remaining.iter().map(|jid| member_row(jid, None))));
            }
        }

        v_flex()
            .w(px(sheet_width))
            .min_w(px(sheet_width))
            .max_w(px(sheet_width))
            .h_full()
            .overflow_hidden()
            .border_l_1()
            .border_color(sheet_border)
            .bg(surface.view)
            .when(framed, |s| s.rounded_br(px(window_material::corner_radius(cx))))
            .flex_shrink_0()
            .child(
                h_flex()
                    .h(px(cx.shape().header_height))
                    .pl_4()
                    .pr(px(crate::components::titlebar::controls_room(cx) + 8.0))
                    .gap_3()
                    .items_center()
                    .border_b_1()
                    .border_color(sheet_border)
                    .bg(surface.headerbar)
                    .child(
                        div()
                            .id("btn-close-message-info")
                            .cursor_pointer()
                            .p_1()
                            .rounded(px(cx.shape().radius_button))
                            .hover(|s| s.bg(theme.muted.opacity(0.65)))
                            .tooltip(move |window, cx| Tooltip::new("Close").build(window, cx))
                            .child(svg().data(X_SVG).size(px(16.0)).text_color(muted_text))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.close_viewed_by(cx))),
                    )
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::BOLD)
                            .text_color(text_color)
                            .child("Message info"),
                    ),
            )
            .child(body)
            .into_any_element()
    }
}

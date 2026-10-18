use crate::views::chat::*;
use super::star_pin::{DEFAULT_PIN_SECS, PIN_DURATIONS};

impl ChatView {
    /// Right-click menu on a message bubble.
    pub(crate) fn render_message_context_menu(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let theme = *cx.app_theme();
        let border_color = theme.border;
        let card_bg = theme.card;
        let text_color = theme.foreground;
        let muted_text = theme.muted_foreground;
        let selected_chat_id = self.selected_chat_id.clone();
        if let Some(ref ctx) = self.context_menu {
            let msg_id = ctx.msg.id.clone();
            let chat_id = selected_chat_id.clone().unwrap_or_default();
            let is_from_me = ctx.is_from_me;
            let msg_content = MessageBubbleHelper::decode_content(&ctx.msg.content);
            let msg_obj = ctx.msg.clone();
            let menu_pos = ctx.position;
            let has_media = ctx.msg.media_url.is_some();
            let ctx_is_image = ctx.msg.message_type == "image";
            let media_full_url = ctx
                .msg
                .media_url
                .as_deref()
                .map(|m| resolve_media_url(m, &Self::compose_base_url(cx)))
                .unwrap_or_default();
            let is_sticker = ctx.msg.message_type == "sticker" || msg_content == "[Sticker]";
            let copyable = !msg_content.trim().is_empty()
                && !matches!(
                    msg_content.as_str(),
                    "[Image]" | "[Sticker]" | "[Video]" | "[Audio]" | "[Document]"
                );
            let hover_bg: Hsla = theme.foreground.opacity(0.12);
            let picker_open = ctx.reaction_picker;
            let pin_picker_open = ctx.pin_picker;
            // A message still on its way has no id the backend knows yet.
            let settled = !msg_id.starts_with("temp-") && ctx.msg.status != "pending";
            let is_starred = ctx.msg.starred;
            let is_pinned = self.is_pinned(&msg_id);

            macro_rules! react {
                ($emoji:expr, $cx:expr) => {{
                    let (cid, mid) = (chat_id.clone(), msg_id.clone());
                    $cx.listener(move |this, _, _, cx| {
                        this.react_message(cid.clone(), mid.clone(), $emoji.to_string(), cx);
                    })
                }};
            }

            let mut card = menu_surface(if picker_open { 308.0 } else { 224.0 }, card_bg, border_color);
            let mut pill = None;
            if picker_open {
                // Full emoji list, opened from the pill's "+" or the React row.
                card = card.px_2().child(
                    h_flex()
                        .id("reaction-grid")
                        .flex_wrap()
                        .max_h(px(236.0))
                        .overflow_y_scroll()
                        .children(
                            EMOJI_CATEGORIES.iter().flat_map(|c| c.emojis.iter().copied()).map(|emoji| {
                                div()
                                    .cursor_pointer()
                                    .w(px(34.0))
                                    .h(px(34.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_lg()
                                    .hover(|s| s.bg(theme.muted.opacity(0.6)))
                                    .text_lg()
                                    .child(emoji)
                                    .on_mouse_down(MouseButton::Left, react!(emoji, cx))
                            }),
                        ),
                );
            } else if pin_picker_open {
                // How long the pin lasts; a week unless told otherwise.
                card = card.child(
                    div()
                        .px_5()
                        .py_1()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(muted_text)
                        .child("Choose how long your pin lasts"),
                );
                for (id, (secs, label)) in ["mm-pin-1d", "mm-pin-7d", "mm-pin-30d"].into_iter().zip(PIN_DURATIONS) {
                    card = card.child(
                        menu_row(window, cx, id, CLOCK_SVG, label, muted_text, text_color, hover_bg)
                            .when(secs == DEFAULT_PIN_SECS, |row| row.bg(theme.primary.opacity(0.12)))
                            .on_mouse_down(MouseButton::Left, cx.listener({
                                let m = msg_obj.clone();
                                move |this, _, _, cx| this.set_message_pinned(m.clone(), true, secs, cx)
                            })),
                    );
                }
            } else {
                // Quick reactions float above the menu in their own pill.
                pill = Some(
                    h_flex()
                        .occlude()
                        .px_2()
                        .py_1()
                        .gap_0p5()
                        .items_center()
                        .rounded_full()
                        .bg(card_bg)
                        .border_1()
                        .border_color(border_color.opacity(0.35))
                        .shadow_xl()
                        .children(["👍", "❤️", "😂", "😮", "😢", "🙏"].into_iter().map(|emoji| {
                            let key = SharedString::from(format!("mm-react-{emoji}"));
                            with_hover_fade(
                                div().id(key.clone()).cursor_pointer().w(px(34.0)).h(px(34.0)).flex().items_center().justify_center().rounded_full(),
                                key,
                                theme.foreground.opacity(0.12),
                                px(17.0),
                                window,
                                cx,
                            )
                                .text_xl()
                                .child(emoji)
                                .on_mouse_down(MouseButton::Left, react!(emoji, cx))
                        }))
                        .child(
                            with_hover_fade(
                                div().id("mm-more-reactions").cursor_pointer().w(px(34.0)).h(px(34.0)).flex().items_center().justify_center().rounded_full(),
                                "mm-more-reactions",
                                theme.foreground.opacity(0.12),
                                px(17.0),
                                window,
                                cx,
                            )
                                .child(svg().data(PLUS_SVG).size(px(16.0)).text_color(text_color))
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                    if let Some(menu) = this.context_menu.as_mut() {
                                        menu.reaction_picker = true;
                                    }
                                    cx.notify();
                                })),
                        ),
                );

                // Reply
                card = card.child(
                    menu_row(window, cx, "mm-reply", REPLY_SVG, "Reply", muted_text, text_color, hover_bg)
                        .on_mouse_down(MouseButton::Left, cx.listener({
                            let m = msg_obj.clone();
                            move |this, _, window, cx| this.start_reply(m.clone(), window, cx)
                        })),
                );

                // React (full emoji list)
                card = card.child(
                    menu_row(window, cx, "mm-react", SMILE_SVG, "React", muted_text, text_color, hover_bg)
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                            if let Some(menu) = this.context_menu.as_mut() {
                                menu.reaction_picker = true;
                            }
                            cx.notify();
                        })),
                );

                if is_from_me {
                    card = card.child(
                        menu_row(window, cx, "mm-viewed-by", EYE_SVG, "Message info", muted_text, text_color, hover_bg)
                            .on_mouse_down(MouseButton::Left, cx.listener({
                                let message = msg_obj.clone();
                                move |this, _, _, cx| this.open_viewed_by(message.clone(), cx)
                            })),
                    );
                }

                // Select (starts select-messages mode with this one ticked)
                card = card.child(
                    menu_row(window, cx, "mm-select", CHECK_SVG, "Select", muted_text, text_color, hover_bg)
                        .on_mouse_down(MouseButton::Left, cx.listener({
                            let mid = msg_obj.id.clone();
                            move |this, _, _, cx| this.enter_select_mode(Some(mid.clone()), cx)
                        })),
                );

                // Forward
                card = card.child(
                    menu_row(window, cx, "mm-forward", FORWARD_SVG, "Forward", muted_text, text_color, hover_bg)
                        .on_mouse_down(MouseButton::Left, cx.listener({
                            let m = msg_obj.clone();
                            let cid = chat_id.clone();
                            move |this, _, _, cx| this.open_forward(cid.clone(), m.clone(), cx)
                        })),
                );

                // Pin / Star
                if settled {
                    card = card.child(if is_pinned {
                        menu_row(window, cx, "mm-pin", PIN_SVG, "Unpin", muted_text, text_color, hover_bg)
                            .on_mouse_down(MouseButton::Left, cx.listener({
                                let m = msg_obj.clone();
                                move |this, _, _, cx| this.set_message_pinned(m.clone(), false, 0, cx)
                            }))
                    } else {
                        menu_row(window, cx, "mm-pin", PIN_SVG, "Pin", muted_text, text_color, hover_bg)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                if let Some(menu) = this.context_menu.as_mut() {
                                    menu.pin_picker = true;
                                }
                                cx.notify();
                            }))
                    });
                    card = card.child(
                        menu_row(
                            window,
                            cx,
                            "mm-star",
                            STAR_SVG,
                            if is_starred { "Unstar" } else { "Star" },
                            muted_text,
                            text_color,
                            hover_bg,
                        )
                        .on_mouse_down(MouseButton::Left, cx.listener({
                            let m = msg_obj.clone();
                            move |this, _, _, cx| this.toggle_star(m.clone(), cx)
                        })),
                    );
                }

                // Copy (text only)
                if copyable {
                    card = card.child(
                        menu_row(window, cx, "mm-copy", COPY_SVG, "Copy", muted_text, text_color, hover_bg)
                            .on_mouse_down(MouseButton::Left, cx.listener({
                                let text = msg_content.clone();
                                move |this, _, _, cx| this.copy_message_text(text.clone(), cx)
                            })),
                    );
                }

                // Copy the picture itself (a still-uploading local preview has nothing to fetch)
                if settled && has_media && ctx_is_image {
                    card = card.child(
                        menu_row(window, cx, "mm-copy-image", COPY_SVG, "Copy image", muted_text, text_color, hover_bg)
                            .on_mouse_down(MouseButton::Left, cx.listener({
                                let url = media_full_url.clone();
                                move |this, _, _, cx| {
                                    this.context_menu = None;
                                    Self::copy_image(url.clone(), cx);
                                    cx.notify();
                                }
                            })),
                    );
                }

                // Save media / favourite sticker
                if has_media && settled {
                    card = card.child(
                        menu_row(window, cx, "mm-save", DOWNLOAD_SVG, "Save as…", muted_text, text_color, hover_bg)
                            .on_mouse_down(MouseButton::Left, cx.listener({
                                let m = msg_obj.clone();
                                move |this, _, _, cx| this.save_message_media(&m, cx)
                            })),
                    );
                }
                if is_sticker && has_media {
                    card = card.child(
                        menu_row(window, cx, "mm-fav", STICKER_SVG, "Add to favourites", muted_text, text_color, hover_bg)
                            .on_mouse_down(MouseButton::Left, cx.listener({
                                let m = msg_obj.clone();
                                move |this, _, _, cx| this.favorite_message_sticker(&m, cx)
                            })),
                    );
                }

                // Edit (own text messages)
                if is_from_me && !has_media && msg_obj.revoked_at.is_none() {
                    card = card.child(
                        menu_row(window, cx, "mm-edit", EDIT_SVG, "Edit", muted_text, text_color, hover_bg)
                            .on_mouse_down(MouseButton::Left, cx.listener({
                                let m = msg_obj.clone();
                                move |this, _, window, cx| this.start_edit(m.clone(), window, cx)
                            })),
                    );
                }

                // Delete (own messages that are not already deleted)
                if is_from_me && msg_obj.revoked_at.is_none() {
                    card = card.child(menu_separator(border_color)).child(
                        menu_row(window, cx, "mm-delete", TRASH_SVG, "Delete", muted_text, text_color, hover_bg)
                            .on_mouse_down(MouseButton::Left, cx.listener({
                                let cid = chat_id.clone();
                                let mid = msg_id.clone();
                                move |this, _, _, cx| this.delete_message(cid.clone(), mid.clone(), cx)
                            })),
                    );
                }
            }

            let menu = v_flex()
                .gap_2()
                .items_start()
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.context_menu = None;
                    cx.notify();
                }))
                .children(pill)
                .child(card);

            Some(
                deferred(anchored().child(
                    div()
                        .id("context-menu-backdrop")
                        .w(window.bounds().size.width)
                        .h(window.bounds().size.height)
                        .child(
                            gpui_base::Positioner::corner(Anchor::TopLeft, menu_pos)
                                .margin(px(8.0))
                                .occlude()
                                .child(menu.with_animation(
                                    "message-context-menu-in",
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

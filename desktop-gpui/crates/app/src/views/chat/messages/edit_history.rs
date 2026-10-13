use crate::views::chat::*;

impl ChatView {
    /// Popover under "Edited": the message as it is now, then every earlier
    /// version, newest first. Scrolls when the list is long.
    pub(crate) fn render_edit_history(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let popover = self.edit_history.clone()?;
        let Some(message) = self.cached_render_messages.iter().find(|m| m.msg.id == popover.msg_id) else {
            self.edit_history = None;
            return None;
        };
        let extra = message.msg.extra.as_ref()?;
        let theme = *cx.app_theme();
        let (text_color, muted_text) = (theme.foreground, theme.muted_foreground);

        // (label, time, text): the current version first, then the earlier ones.
        let mut versions = vec![(
            "Current",
            extra.edited_at.unwrap_or(message.msg.timestamp),
            message.content.clone(),
        )];
        versions.extend(extra.edits.iter().rev().map(|edit| {
            ("Earlier", edit.timestamp, MessageBubbleHelper::decode_content(&edit.content))
        }));

        let entries = versions.into_iter().enumerate().map(|(index, (label, at, text))| {
            v_flex()
                .gap_1()
                .px_4()
                .py_2p5()
                .when(index > 0, |entry| entry.border_t_1().border_color(theme.border.opacity(0.35)))
                .child(
                    h_flex()
                        .justify_between()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(if index == 0 { theme.primary } else { muted_text })
                                .child(label),
                        )
                        .child(div().text_xs().text_color(muted_text).child(Self::format_chat_time(at))),
                )
                .child(div().text_sm().text_color(text_color).child(text))
        });

        let card = v_flex()
            .occlude()
            .w(px(320.0))
            .max_h(px(300.0))
            .rounded_2xl()
            .bg(theme.card)
            .border_1()
            .border_color(theme.border.opacity(0.35))
            .shadow_xl()
            .overflow_hidden()
            .child(
                div()
                    .px_4()
                    .py_2p5()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_color)
                    .child("Edit history"),
            )
            .child(
                v_flex()
                    .id("edit-history-list")
                    .overflow_y_scroll()
                    .border_t_1()
                    .border_color(theme.border.opacity(0.35))
                    .children(entries),
            );

        Some(
            deferred(anchored().child(
                div()
                    .id("edit-history-backdrop")
                    .w(window.bounds().size.width)
                    .h(window.bounds().size.height)
                    .child(
                        gpui_base::Positioner::corner(Anchor::BottomRight, popover.position)
                            .margin(px(8.0))
                            .occlude()
                            .child(
                                v_flex()
                                    .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                        this.edit_history = None;
                                        cx.notify();
                                    }))
                                    .child(card)
                                    .with_animation(
                                        "edit-history-in",
                                        Animation::new(std::time::Duration::from_millis(140))
                                            .with_easing(|d| 1.0 - (1.0 - d).powi(5)),
                                        move |el, delta| el.opacity(delta),
                                    ),
                            ),
                    ),
            ))
            .with_priority(gpui_base::POPUP_PRIORITY),
        )
    }
}

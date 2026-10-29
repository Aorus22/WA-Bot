use crate::theme::surface::SurfaceExt;
use crate::views::chat::*;

fn attachment_thumbnail(attachment: &PendingAttachment, width: f32, height: f32) -> AnyElement {
    if attachment.media_type == "image" {
        if let Some(image) = attachment.image.clone() {
            return img(ImageSource::Image(image))
                .w(px(width))
                .h(px(height))
                .object_fit(ObjectFit::Contain)
                .into_any_element();
        }
        if let Some(path) = attachment.path.clone() {
            return img(ImageSource::from(path))
                .w(px(width))
                .h(px(height))
                .object_fit(ObjectFit::Contain)
                .into_any_element();
        }
    }

    v_flex()
        .w(px(width))
        .h(px(height))
        .items_center()
        .justify_center()
        .gap_2()
        .child(
            svg()
                .data(PAPERCLIP_SVG)
                .size(px(28.0))
                .text_color(rgb(0x9aa4a8)),
        )
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0x9aa4a8))
                .child(attachment.media_type.to_uppercase()),
        )
        .into_any_element()
}

impl ChatView {
    /// WhatsApp-style media review that covers only the active conversation.
    pub(crate) fn render_attachment_preview(
        &mut self,
        window: &mut Window,
        chat: &Chat,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if self.pending_attachments.is_empty() {
            return None;
        }

        let theme = *cx.app_theme();
        let shape = cx.shape();
        let surface = cx.surface();
        let viewport = window.bounds().size;
        let sidebar_space = if self.sidebar_collapsed {
            0.0
        } else {
            self.sidebar_width
        };
        let controls_room = crate::components::titlebar::controls_room(cx) + 8.0;
        let preview_width = (f32::from(viewport.width) - sidebar_space - 48.0).clamp(240.0, 1400.0);
        let preview_height = (f32::from(viewport.height) - 144.0).clamp(180.0, 760.0);
        let attachments = self.pending_attachments.clone();
        let count = attachments.len();
        let preview_area: AnyElement = if count == 1 {
            let attachment = &attachments[0];
            v_flex()
                .id("attachment-preview-single")
                .w_full()
                .h(px(preview_height))
                .items_center()
                .justify_center()
                .overflow_hidden()
                .child(attachment_thumbnail(attachment, preview_width, preview_height))
                .into_any_element()
        } else {
            v_flex()
                .id("attachment-preview-list")
                .w_full()
                .max_h(px(preview_height))
                .gap_2()
                .overflow_y_scroll()
                .children(attachments.iter().enumerate().map(|(index, attachment)| {
                    h_flex()
                        .id(SharedString::from(format!("attachment-preview-row-{index}")))
                        .w_full()
                        .min_h(px(96.0))
                        .gap_3()
                        .items_center()
                        .px_3()
                        .py_2()
                        .rounded_xl()
                        .bg(theme.muted.opacity(0.28))
                        .child(
                            div()
                                .w(px(112.0))
                                .h(px(80.0))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .overflow_hidden()
                                .rounded_lg()
                                .bg(rgb(0x0b141a))
                                .child(attachment_thumbnail(attachment, 108.0, 76.0)),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w(px(0.0))
                                .gap_1()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme.foreground)
                                        .overflow_hidden()
                                        .child(attachment.file_name.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child(format!("{} attachment", attachment.media_type)),
                                ),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!("remove-attachment-{index}")))
                                .cursor_pointer()
                                .p_2()
                                .rounded(px(shape.radius_button))
                                .hover(|el| el.bg(theme.muted.opacity(0.55)))
                                .child(
                                    svg()
                                        .data(X_SVG)
                                        .size(px(15.0))
                                        .text_color(theme.muted_foreground),
                                )
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        this.remove_pending_attachment(index, cx);
                                    }),
                                ),
                        )
                        .into_any_element()
                }))
                .into_any_element()
        };

        let caption_state = self.compose_textarea.as_ref()?.clone();
        let chat_name = if chat.name.trim().is_empty() {
            chat.id.clone()
        } else {
            chat.name.clone()
        };

        Some(
            crate::components::motion::modal_in(
                div()
                    .id("attachment-preview-overlay")
                    .absolute()
                    .occlude()
                    .inset_0()
                    .bg(surface.conversation_background)
                    .flex()
                    .flex_col()
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        if event.keystroke.key.as_str() == "escape" {
                            this.clear_pending_attachment(cx);
                        }
                    }))
                    .child(
                        h_flex()
                            .w_full()
                            .h(px(shape.header_height))
                            .flex_shrink_0()
                            .pl_4()
                            // Keep the file count clear of the window controls
                            // floating over the top-right edge of the chat pane.
                            .pr(px(controls_room))
                            .gap_3()
                            .items_center()
                            .bg(surface.headerbar)
                            .border_b_1()
                            .border_color(surface.border.opacity(0.45))
                            .child(
                                div()
                                    .id("btn-close-attachment-preview")
                                    .cursor_pointer()
                                    .p_2()
                                    .rounded(px(shape.radius_button))
                                    .hover(|el| el.bg(theme.muted.opacity(0.5)))
                                    .child(
                                        svg()
                                            .data(X_SVG)
                                            .size(px(17.0))
                                            .text_color(theme.muted_foreground),
                                    )
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                        this.clear_pending_attachment(cx);
                                    })),
                            )
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child("Send to"),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(theme.foreground)
                                            .child(chat_name),
                                    ),
                            )
                            .child(div().flex_1())
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(format!(
                                        "{count} file{}",
                                        if count == 1 { "" } else { "s" }
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .id("attachment-preview-stage")
                            .w_full()
                            .flex_1()
                            .min_h(px(0.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .overflow_hidden()
                            .bg(rgb(0x0b141a))
                            .child(preview_area),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .flex_shrink_0()
                            .items_center()
                            .gap_3()
                            .px_4()
                            .py_3()
                            .bg(surface.composer_background)
                            .border_t_1()
                            .border_color(surface.border.opacity(0.45))
                            .child(
                                h_flex()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .min_h(px(54.0))
                                    .px_3()
                                    .py_1()
                                    .rounded_xl()
                                    .bg(surface.composer_field_fill)
                                    .when(surface.composer_field_border.a > 0.0, |el| {
                                        el.border_1().border_color(surface.composer_field_border)
                                    })
                                    .child(
                                        Textarea::new(&caption_state)
                                            .appearance(false)
                                            .bordered(false)
                                            .w_full()
                                            .h(px(46.0))
                                            .max_h(px(100.0))
                                            .text_sm()
                                            .text_color(theme.foreground),
                                    ),
                            )
                            .child(
                                div()
                                    .id("btn-send-pending-attachments")
                                    .w(px(48.0))
                                    .h(px(48.0))
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_full()
                                    .cursor_pointer()
                                    .bg(theme.primary)
                                    .hover(|el| el.opacity(0.88))
                                    .child(
                                        svg()
                                            .data(SEND_SVG)
                                            .size(px(20.0))
                                            .text_color(theme.primary_foreground),
                                    )
                                    .tooltip(move |window, cx| Tooltip::new("Send").build(window, cx))
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                        this.send_pending_attachments(window, cx);
                                    })),
                            ),
                    ),
                "attachment-preview-in",
            )
            .into_any_element(),
        )
    }
}

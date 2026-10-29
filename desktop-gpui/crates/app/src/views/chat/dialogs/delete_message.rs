use crate::theme::surface::SurfaceExt;
use crate::views::chat::*;

impl ChatView {
    pub(crate) fn render_delete_message_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let dialog = self.delete_message_dialog.clone()?;
        let theme = *cx.app_theme();
        let shape = cx.shape();
        let surface = cx.surface();
        let count = dialog.messages.len();
        let can_delete_everyone = dialog.messages.iter().all(|message| {
            MessageBubbleHelper::is_from_me(message, "me") && message.revoked_at.is_none()
        });
        let danger: Hsla = rgb(0xef4444).into();

        Some(
            crate::components::motion::modal_in(
                div()
                    .id("delete-message-backdrop")
                    .absolute()
                    .occlude()
                    .inset_0()
                    .bg(surface.scrim)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        v_flex()
                            .w(px(420.0))
                            .max_w(px((f32::from(window.bounds().size.width) - 32.0).max(280.0)))
                            .p_5()
                            .gap_4()
                            .rounded_2xl()
                            .when(shape.styled, |el| el.rounded(px(shape.radius_popover)))
                            .bg(surface.dialog)
                            .border_1()
                            .border_color(surface.border)
                            .when(!shape.styled, |el| el.shadow_2xl())
                            .when(shape.styled && !shape.winui, |el| el.shadow_md())
                            .when(shape.winui, |el| el.shadow(crate::theme::shape::dialog_shadow()))
                            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                                if event.keystroke.key.as_str() == "escape" {
                                    this.delete_message_dialog = None;
                                    cx.notify();
                                }
                            }))
                            .child(
                                h_flex()
                                    .w_full()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_lg()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(theme.foreground)
                                            .child(if count == 1 {
                                                "Delete message?".to_string()
                                            } else {
                                                format!("Delete {count} messages?")
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("btn-close-delete-message")
                                            .cursor_pointer()
                                            .p_1()
                                            .rounded(px(shape.radius_button))
                                            .hover(|el| el.bg(theme.muted.opacity(0.5)))
                                            .child(svg().data(X_SVG).size(px(15.0)).text_color(theme.muted_foreground))
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                this.delete_message_dialog = None;
                                                cx.notify();
                                            })),
                                    ),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child("Choose whether to remove the message only for yourself or for everyone."),
                            )
                            .child(
                                h_flex()
                                    .w_full()
                                    .justify_end()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("btn-cancel-delete-message")
                                            .cursor_pointer()
                                            .px_3()
                                            .py_2()
                                            .rounded(px(shape.radius_button))
                                            .border_1()
                                            .border_color(surface.border)
                                            .text_sm()
                                            .text_color(theme.foreground)
                                            .child("Cancel")
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                this.delete_message_dialog = None;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        div()
                                            .id("btn-delete-for-me")
                                            .cursor_pointer()
                                            .px_3()
                                            .py_2()
                                            .rounded(px(shape.radius_button))
                                            .border_1()
                                            .border_color(surface.border)
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(theme.foreground)
                                            .child("Delete for me")
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                this.delete_dialog_for_me(cx);
                                            })),
                                    )
                                    .when(can_delete_everyone, |row| {
                                        row.child(
                                            div()
                                                .id("btn-delete-for-everyone")
                                                .cursor_pointer()
                                                .px_3()
                                                .py_2()
                                                .rounded(px(shape.radius_button))
                                                .bg(danger)
                                                .text_sm()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(rgb(0xffffff))
                                                .child("Delete for everyone")
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.delete_dialog_for_everyone(cx);
                                                })),
                                        )
                                    }),
                            ),
                    ),
                "delete-message-in",
            )
            .into_any_element(),
        )
    }
}

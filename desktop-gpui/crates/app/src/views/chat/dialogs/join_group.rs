use crate::theme::surface::SurfaceExt;
use crate::views::chat::*;

impl ChatView {
    /// Join group via invite link modal.
    pub(crate) fn render_join_group_dialog(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let theme = *cx.app_theme();
        let primary_color = theme.primary;
        let text_color = theme.foreground;
        let muted_text = theme.muted_foreground;
        let shape = cx.shape();
        let surface = cx.surface();
        let scrim = surface.scrim;
        let dialog_bg = surface.dialog;
        let dialog_border = surface.border;
        if self.is_join_group_open {
            Some(crate::components::motion::modal_in(
                div()
                    .absolute()
                    .occlude()
                    .inset_0()
                    .bg(scrim)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        v_flex()
                            .w(px(400.0))
                            .p_5()
                            .rounded_2xl()
                            .when(shape.styled, |dialog| dialog.rounded(px(shape.radius_popover)))
                            .bg(dialog_bg)
                            .border_1()
                            .border_color(dialog_border)
                            .when(!shape.styled, |dialog| dialog.shadow_2xl())
                            .when(shape.styled && !shape.winui, |dialog| dialog.shadow_md())
                            .when(shape.winui, |dialog| dialog.shadow(crate::theme::shape::dialog_shadow()))
                            .gap_4()
                            // Header
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .when(shape.styled, |header| header.h(px(shape.header_height)).bg(surface.headerbar))
                                    .child(div().text_lg().font_weight(FontWeight::BOLD).text_color(text_color).child("Join group"))
                                    .child(
                                        div()
                                            .cursor_pointer()
                                            .p_1()
                                            .rounded(px(cx.shape().radius_button))
                                            .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                            .child(svg().data(X_SVG).size(px(16.0)).text_color(muted_text))
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                this.is_join_group_open = false;
                                                cx.notify();
                                            })),
                                    ),
                            )
                            .child(
                                div().text_xs().text_color(muted_text).child("Paste an invite link (chat.whatsapp.com/...)"),
                            )
                            // Invite Link Input
                            .children(self.join_link_field.as_ref().map(|field| field.element(&theme, cx.shape(), Some(LINK_SVG))))
                            // Actions
                            .child(
                                h_flex()
                                    .justify_end()
                                    .gap_2()
                                    .child(
                                        div()
                                            .cursor_pointer()
                                            .px_4()
                                            .py_2()
                                            .rounded_xl()
                                            .when(shape.styled, |button| button.rounded(px(shape.radius_button)))
                                            .border_1()
                                            .border_color(dialog_border)
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(text_color)
                                            .child("Cancel")
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                this.is_join_group_open = false;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        div()
                                            .cursor_pointer()
                                            .px_4()
                                            .py_2()
                                            .rounded_xl()
                                            .when(shape.styled, |button| button.rounded(px(shape.radius_button)))
                                            .bg(primary_color)
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(0xffffff))
                                            .child(if self.is_joining_group { "Joining..." } else { "Join" })
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                this.submit_join_group(cx);
                                            })),
                                    ),
                            ),
                    ),
                    "join-group-in",
                ))
        } else {
            None
        }
    }
}

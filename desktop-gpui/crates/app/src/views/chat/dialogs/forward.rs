use crate::components::ui::boxed_list::boxed_list;
use crate::theme::surface::SurfaceExt;
use crate::views::chat::*;

impl ChatView {
    /// Forward message modal.
    pub(crate) fn render_forward_dialog(
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
        if self.forward_msg.is_some() {
            let targets: Vec<Chat> = if cx.has_global::<ChatStore>() {
                ChatStore::global(cx).chats.iter().filter(|c| !c.archived).cloned().collect()
            } else {
                Vec::new()
            };
            let selected_count = self.forward_selected.len();
            let can_send = selected_count > 0 && !self.is_forwarding;
            Some(crate::components::motion::modal_in(
                div()
                    .id("forward-backdrop")
                    .absolute()
                    .occlude()
                    .inset_0()
                    .bg(scrim)
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_mouse_down(MouseButton::Left, |_, _, _| {})
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
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .when(shape.styled, |header| header.h(px(shape.header_height)).bg(surface.headerbar))
                                    .child(div().text_lg().font_weight(FontWeight::BOLD).text_color(text_color).child("Forward message to"))
                                    .child(
                                        div()
                                            .cursor_pointer()
                                            .p_1()
                                            .rounded(px(cx.shape().radius_button))
                                            .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                            .child(svg().data(X_SVG).size(px(16.0)).text_color(muted_text))
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                this.forward_msg = None;
                                                this.forward_selected.clear();
                                                cx.notify();
                                            })),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .id("forward-chats-scroll")
                                    .max_h(px(280.0))
                                    .overflow_y_scroll()
                                    .child(boxed_list(cx, targets.into_iter().map(|c| {
                                        let is_checked = self.forward_selected.contains(&c.id);
                                        let cid = c.id.clone();
                                        h_flex()
                                            .cursor_pointer()
                                            .when(!shape.styled, |row| row.p_2().rounded_lg())
                                            .items_center()
                                            .justify_between()
                                            .bg(if is_checked {
                                                surface.card_shade
                                            } else {
                                                rgba(0x00000000).into()
                                            })
                                            .hover(|s| s.bg(surface.dialog_row_hover))
                                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                if !this.forward_selected.remove(&cid) {
                                                    this.forward_selected.insert(cid.clone());
                                                }
                                                cx.notify();
                                            }))
                                            .child(div().text_sm().text_color(text_color).child(c.name.clone()))
                                            .child(
                                                div()
                                                    .w(px(20.0))
                                                    .h(px(20.0))
                                                    .rounded_md()
                                                    .border_1()
                                                    .border_color(primary_color)
                                                    .bg(if is_checked { primary_color } else { rgba(0x00000000).into() })
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .children(if is_checked {
                                                        Some(svg().data(CHECK_SVG).size(px(12.0)).text_color(rgb(0xffffff)))
                                                    } else {
                                                        None
                                                    }),
                                            )
                                    })))
                                    .when(!shape.styled, |list| list.gap_1()),
                            )
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
                                                this.forward_msg = None;
                                                this.forward_selected.clear();
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        div()
                                            .px_4()
                                            .py_2()
                                            .rounded_xl()
                                            .when(shape.styled, |button| button.rounded(px(shape.radius_button)))
                                            .bg(primary_color)
                                            .opacity(if can_send { 1.0 } else { 0.5 })
                                            .cursor_pointer()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(0xffffff))
                                            .child(if self.is_forwarding {
                                                "Sending...".to_string()
                                            } else if selected_count > 0 {
                                                format!("Forward ({selected_count})")
                                            } else {
                                                "Forward".to_string()
                                            })
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                this.submit_forward(cx);
                                            })),
                                    ),
                            ),
                    ),
                    "forward-in",
                ))
        } else {
            None
        }
    }
}

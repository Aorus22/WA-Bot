use crate::views::chat::*;

impl ChatView {
    /// New group modal.
    pub(crate) fn render_new_group_dialog(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let theme = *cx.app_theme();
        let border_color = theme.border;
        let bg_color = theme.background;
        let card_bg = theme.card;
        let primary_color = theme.primary;
        let text_color = theme.foreground;
        let muted_text = theme.muted_foreground;
        if self.is_new_group_open {
            let contacts: Vec<Chat> = if cx.has_global::<ChatStore>() {
                ChatStore::global(cx).chats.iter().filter(|c| !c.is_group && !c.archived).cloned().collect()
            } else {
                Vec::new()
            };
            Some(crate::components::motion::modal_in(
                div()
                    .absolute()
                    .inset_0()
                    .bg(rgba(0x00000088))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        v_flex()
                            .w(px(400.0))
                            .p_5()
                            .rounded_2xl()
                            .bg(card_bg)
                            .border_1()
                            .border_color(border_color)
                            .shadow_2xl()
                            .gap_4()
                            // Header
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(div().text_lg().font_weight(FontWeight::BOLD).text_color(text_color).child("New group"))
                                    .child(
                                        div()
                                            .cursor_pointer()
                                            .p_1()
                                            .rounded_full()
                                            .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                            .child(svg().data(X_SVG).size(px(16.0)).text_color(muted_text))
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                this.is_new_group_open = false;
                                                cx.notify();
                                            })),
                                    ),
                            )
                            // Group Name Input
                            .child(
                                h_flex()
                                    .px_3()
                                    .py_2()
                                    .rounded_xl()
                                    .border_1()
                                    .border_color(border_color)
                                    .bg(bg_color)
                                    .track_focus(&self.new_group_focus)
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                        this.new_group_focus.focus(window, cx);
                                    }))
                                    .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                                        match ev.keystroke.key.as_str() {
                                            "backspace" => { this.new_group_name.pop(); cx.notify(); }
                                            "space" => { this.new_group_name.push(' '); cx.notify(); }
                                            k if k.len() == 1 => { this.new_group_name.push_str(k); cx.notify(); }
                                            _ => {}
                                        }
                                    }))
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(if self.new_group_name.is_empty() { muted_text } else { text_color })
                                            .child(if self.new_group_name.is_empty() { "Group name...".to_string() } else { self.new_group_name.clone() }),
                                    ),
                            )
                            // Contact Picker Scroll
                            .child(
                                v_flex()
                                    .id("new-group-contacts-scroll")
                                    .max_h(px(220.0))
                                    .overflow_y_scroll()
                                    .gap_1()
                                    .children(contacts.into_iter().map(|c| {
                                        let is_checked = self.new_group_selected.contains(&c.id);
                                        let cid = c.id.clone();
                                        h_flex()
                                            .cursor_pointer()
                                            .p_2()
                                            .rounded_lg()
                                            .items_center()
                                            .justify_between()
                                            .bg(if is_checked { theme.primary.opacity(0.12) } else { rgba(0x00000000).into() })
                                            .hover(|s| s.bg(theme.muted.opacity(0.4)))
                                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                if this.new_group_selected.contains(&cid) {
                                                    this.new_group_selected.remove(&cid);
                                                } else {
                                                    this.new_group_selected.insert(cid.clone());
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
                                            .into_any_element()
                                    }))
                            )
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
                                            .border_1()
                                            .border_color(border_color)
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(text_color)
                                            .child("Cancel")
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                this.is_new_group_open = false;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        div()
                                            .cursor_pointer()
                                            .px_4()
                                            .py_2()
                                            .rounded_xl()
                                            .bg(primary_color)
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(0xffffff))
                                            .child(if self.is_creating_group { "Creating..." } else { "Create Group" })
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                this.submit_create_group(cx);
                                            })),
                                    ),
                            ),
                    ),
                    "new-group-in",
                ))
        } else {
            None
        }
    }
}

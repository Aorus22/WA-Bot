use crate::theme::surface::SurfaceExt;
use crate::views::chat::*;

impl ChatView {
    /// Forward message modal.
    pub(crate) fn render_forward_dialog(
        &mut self,
        window: &mut Window,
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

        if self.forward_msg.is_none() {
            return None;
        }

        let mut targets: Vec<Chat> = if cx.has_global::<ChatStore>() {
            ChatStore::global(cx)
                .chats
                .iter()
                .filter(|chat| !chat.archived)
                .cloned()
                .collect()
        } else {
            Vec::new()
        };
        targets.sort_by(|a, b| match (a.pinned_at, b.pinned_at) {
            (Some(pa), Some(pb)) => pb.cmp(&pa),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => b.last_time.cmp(&a.last_time),
        });

        let query = self.forward_search_query.trim().to_lowercase();
        let targets: Vec<Chat> = targets
            .into_iter()
            .filter(|chat| {
                query.is_empty()
                    || chat.name.to_lowercase().contains(&query)
                    || chat.id.to_lowercase().contains(&query)
            })
            .collect();

        let selected_count = self.forward_selected.len();
        let can_send = selected_count > 0 && !self.is_forwarding;
        let viewport = window.viewport_size();
        let dialog_width = (f32::from(viewport.width) - 32.0).clamp(300.0, 480.0);
        let max_list_height = (f32::from(viewport.height) - 330.0).clamp(140.0, 400.0);
        let list_count = targets.len();
        let list_height = ((list_count as f32 * 64.0).max(96.0)).min(max_list_height);
        self.forward_filtered_chats = targets;

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
                        .w(px(dialog_width))
                        .max_h(px((f32::from(viewport.height) - 32.0).max(300.0)))
                        .p_5()
                        .rounded_2xl()
                        .when(shape.styled, |dialog| {
                            dialog.rounded(px(shape.radius_popover))
                        })
                        .bg(dialog_bg)
                        .border_1()
                        .border_color(dialog_border)
                        .when(!shape.styled, |dialog| dialog.shadow_2xl())
                        .when(shape.styled && !shape.winui, |dialog| dialog.shadow_md())
                        .when(shape.winui, |dialog| {
                            dialog.shadow(crate::theme::shape::dialog_shadow())
                        })
                        .gap_4()
                        .child(
                            h_flex()
                                .justify_between()
                                .items_center()
                                .when(shape.styled, |header| {
                                    header.h(px(shape.header_height)).bg(surface.headerbar)
                                })
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(text_color)
                                        .child("Forward message to"),
                                )
                                .child(
                                    div()
                                        .cursor_pointer()
                                        .p_1()
                                        .rounded(px(cx.shape().radius_button))
                                        .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                        .child(
                                            svg().data(X_SVG).size(px(16.0)).text_color(muted_text),
                                        )
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _, cx| {
                                                this.forward_msg = None;
                                                this.forward_selected.clear();
                                                cx.notify();
                                            }),
                                        ),
                                ),
                        )
                        .children(
                            self.forward_search_field
                                .as_ref()
                                .map(|field| field.element(&theme, shape, Some(SEARCH_SVG))),
                        )
                        .child(if self.forward_filtered_chats.is_empty() {
                            div()
                                .h(px(list_height))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_xl()
                                .border_1()
                                .border_color(dialog_border.opacity(0.55))
                                .text_sm()
                                .text_color(muted_text)
                                .child(if query.is_empty() {
                                    "No chats available"
                                } else {
                                    "No chats found"
                                })
                        } else {
                            h_flex()
                                .h(px(list_height))
                                .min_h(px(96.0))
                                .items_start()
                                .child(
                                    uniform_list(
                                        "forward-chats-scroll",
                                        self.forward_filtered_chats.len(),
                                        cx.processor(|this, range: Range<usize>, _window, cx| {
                                            let theme = *cx.app_theme();
                                            let shape = cx.shape();
                                            let surface = cx.surface();
                                            let primary_color = theme.primary;
                                            let text_color = theme.foreground;
                                            let base_url = Self::compose_base_url(cx);
                                            let mut items = Vec::with_capacity(range.len());

                                            for index in range {
                                                let Some(chat) = this.forward_filtered_chats.get(index) else {
                                                    continue;
                                                };
                                                let chat = chat.clone();
                                                let checked = this.forward_selected.contains(&chat.id);
                                                let chat_id = chat.id.clone();
                                                let name = if chat.name.trim().is_empty() {
                                                    chat.id.split('@').next().unwrap_or(&chat.id).to_string()
                                                } else {
                                                    chat.name.clone()
                                                };
                                                let selected_fill = if checked {
                                                    primary_color
                                                } else {
                                                    rgba(0x00000000).into()
                                                };

                                                items.push(
                                                    h_flex()
                                                        .w_full()
                                                        .h(px(64.0))
                                                        .items_center()
                                                        .gap_3()
                                                        .px_3()
                                                        .cursor_pointer()
                                                        .border_b_1()
                                                        .border_color(surface.border.opacity(0.45))
                                                        .bg(if checked {
                                                            surface.card_shade
                                                        } else {
                                                            surface.card
                                                        })
                                                        .hover(|s| s.bg(surface.dialog_row_hover))
                                                        .on_mouse_down(
                                                            MouseButton::Left,
                                                            cx.listener(move |this, _, _, cx| {
                                                                if !this.forward_selected.remove(&chat_id) {
                                                                    this.forward_selected.insert(chat_id.clone());
                                                                }
                                                                cx.notify();
                                                            }),
                                                        )
                                                        .child(render_avatar(
                                                            &chat.id,
                                                            &name,
                                                            &chat.avatar,
                                                            chat.is_group,
                                                            40.0,
                                                            &base_url,
                                                            &theme,
                                                        ))
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .min_w_0()
                                                                .text_sm()
                                                                .text_color(text_color)
                                                                .overflow_hidden()
                                                                .line_clamp(1)
                                                                .child(name),
                                                        )
                                                        .child(
                                                            div()
                                                                .w(px(20.0))
                                                                .h(px(20.0))
                                                                .flex_shrink_0()
                                                                .rounded_full()
                                                                .border_1()
                                                                .border_color(primary_color)
                                                                .bg(selected_fill)
                                                                .flex()
                                                                .items_center()
                                                                .justify_center()
                                                                .children(if checked {
                                                                    Some(
                                                                        svg()
                                                                            .data(CHECK_SVG)
                                                                            .size(px(12.0))
                                                                            .text_color(rgb(0xffffff)),
                                                                    )
                                                                } else {
                                                                    None
                                                                }),
                                                        )
                                                        .into_any_element(),
                                                );
                                            }
                                            items
                                        }),
                                    )
                                    .size_full()
                                    .track_scroll(&self.forward_chat_scroll),
                                )
                                .child(
                                    Scrollbar::vertical(&self.forward_chat_scroll)
                                        .mode(ScrollbarMode::Always)
                                        .styles(|s| {
                                            s.track(|t| t.bg(rgba(0x00000000).into()))
                                                .thumb(|th| {
                                                    th.bg(muted_text.opacity(0.35))
                                                        .radius(px(3.0))
                                                        .width(px(6.0))
                                                })
                                                .thumb_hover(|th| {
                                                    th.bg(muted_text.opacity(0.65))
                                                        .radius(px(4.0))
                                                        .width(px(8.0))
                                                })
                                                .thumb_active(|th| {
                                                    th.bg(primary_color.opacity(0.8))
                                                        .radius(px(4.0))
                                                        .width(px(8.0))
                                                })
                                        }),
                                )
                        })
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
                                        .when(shape.styled, |button| {
                                            button.rounded(px(shape.radius_button))
                                        })
                                        .border_1()
                                        .border_color(dialog_border)
                                        .text_xs()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(text_color)
                                        .child("Cancel")
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _, cx| {
                                                this.forward_msg = None;
                                                this.forward_selected.clear();
                                                cx.notify();
                                            }),
                                        ),
                                )
                                .child(
                                    div()
                                        .px_4()
                                        .py_2()
                                        .rounded_xl()
                                        .when(shape.styled, |button| {
                                            button.rounded(px(shape.radius_button))
                                        })
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
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _, cx| {
                                                this.submit_forward(cx);
                                            }),
                                        ),
                                ),
                        ),
                ),
            "forward-in",
        ))
    }
}

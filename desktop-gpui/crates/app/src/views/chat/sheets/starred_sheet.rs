use crate::components::ui::boxed_list::boxed_list;
use crate::theme::surface::SurfaceExt;
use crate::views::chat::*;
use messages::star_pin::{sender_label, snippet_of};

/// Starred messages fetched per request.
const STARRED_PAGE: usize = 100;

impl ChatView {
    /// Play the Starred drawer's exit animation, then unmount it.
    pub fn close_starred_sheet(&mut self, cx: &mut Context<Self>) {
        if !self.is_starred_sheet_open || self.starred_sheet_closing {
            return;
        }
        self.starred_sheet_closing = true;
        Self::after_sheet_exit(cx, |this| {
            // Re-opening during the exit cancels the pending unmount.
            if this.starred_sheet_closing {
                this.is_starred_sheet_open = false;
                this.starred_sheet_closing = false;
            }
        });
        cx.notify();
    }

    /// Open (or close) the drawer listing every starred message, in all chats.
    pub fn toggle_starred_sheet(&mut self, cx: &mut Context<Self>) {
        if self.is_starred_sheet_open && !self.starred_sheet_closing {
            self.close_starred_sheet(cx);
            return;
        }
        self.starred_sheet_closing = false;
        self.is_starred_sheet_open = true;
        // The drawers share one slot.
        self.is_info_sheet_open = false;
        self.info_sheet_closing = false;
        self.is_search_sheet_open = false;
        self.search_sheet_closing = false;
        self.viewed_by_message = None;
        self.viewed_by_closing = false;
        self.load_starred(false, cx);
        cx.notify();
    }

    /// Fetch the first page of starred messages, or with `more` the page after
    /// what is already listed.
    pub(crate) fn load_starred(&mut self, more: bool, cx: &mut Context<Self>) {
        if self.is_loading_starred {
            return;
        }
        let before = if more { self.starred_messages.last().and_then(|m| m.starred_at) } else { None };
        if more && before.is_none() {
            return;
        }
        self.is_loading_starred = true;
        let base_url = Self::compose_base_url(cx);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move { client.get_starred(None, Some(STARRED_PAGE), before).await })
                    .await;
                let _ = cx_handle.update(|cx: &mut App| {
                    let Some(view) = this.upgrade() else { return };
                    view.update(cx, |this, cx| {
                        this.is_loading_starred = false;
                        match res {
                            Ok(Ok(page)) => {
                                this.starred_has_more = page.len() >= STARRED_PAGE;
                                if more {
                                    let known: HashSet<String> = this.starred_messages.iter().map(|m| m.id.clone()).collect();
                                    this.starred_messages.extend(page.into_iter().filter(|m| !known.contains(&m.id)));
                                } else {
                                    this.starred_messages = page;
                                }
                            }
                            _ => toast::error("Gagal memuat pesan berbintang", cx),
                        }
                        cx.notify();
                    });
                });
            }
        })
        .detach();
        cx.notify();
    }

    /// Jump to a starred message, opening its chat first when it is another one.
    pub(crate) fn open_starred_message(
        &mut self,
        chat_id: String,
        message_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_chat_id.as_deref() == Some(chat_id.as_str()) {
            self.navigate_to_message(&message_id, cx);
        } else {
            // The chat's first page is still loading when select_chat returns;
            // the jump runs once it has landed.
            self.pending_jump = Some((chat_id.clone(), message_id));
            self.select_chat(chat_id, window, cx);
        }
    }

    /// Starred messages drawer. `full_width` fills the screen instead of the
    /// usual right-hand column (the chat list page has no column to spare).
    pub(crate) fn render_starred_sheet(&self, full_width: bool, cx: &mut Context<Self>) -> AnyElement {
        let theme = *cx.app_theme();
        let shape = cx.shape();
        let surface = cx.surface();
        let border_color = theme.border;
        let sheet_border = surface.sheet_border;
        let header_bg = surface.headerbar;
        let primary_color = theme.primary;
        let text_color = theme.foreground;
        let muted_text = theme.muted_foreground;
        let chats: Vec<(String, String)> = if cx.has_global::<ChatStore>() {
            ChatStore::global(cx).chats.iter().map(|c| (c.id.clone(), c.name.clone())).collect()
        } else {
            Vec::new()
        };

        v_flex()
            .when(!full_width, |el| el.w(px(SHEET_WIDTH)).min_w(px(SHEET_WIDTH)).max_w(px(SHEET_WIDTH)))
            .when(full_width, |el| el.size_full())
            .h_full()
            .overflow_hidden()
            .border_l_1()
            .border_color(sheet_border)
            .bg(surface.view)
            .flex_shrink_0()
            .child(
                h_flex()
                    .h(px(shape.header_height))
                    .pl_4()
                    .pr(px(crate::components::titlebar::controls_room(cx) + 8.0))
                    .border_b_1()
                    .border_color(sheet_border)
                    .bg(header_bg)
                    .justify_between()
                    .items_center()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(svg().data(STAR_SVG).size(px(18.0)).text_color(primary_color))
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(text_color)
                                    .child("Starred messages"),
                            ),
                    )
                    .child(
                        div()
                            .id("btn-close-starred-sheet")
                            .cursor_pointer()
                            .p_1()
                            .when(shape.styled, |s| {
                                s.w(px(shape.control_size))
                                    .h(px(shape.control_size))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                            })
                            .rounded(px(shape.radius_button))
                            .hover(|s| s.bg(theme.muted.opacity(0.65)))
                            .tooltip(move |window, cx| Tooltip::new("Close").build(window, cx))
                            .child(svg().data(X_SVG).size(px(16.0)).text_color(muted_text))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.close_starred_sheet(cx);
                            })),
                    ),
            )
            .child(if self.starred_messages.is_empty() {
                v_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .p_6()
                    .gap_3()
                    .child(if self.is_loading_starred {
                        crate::components::motion::spinning(Icon::new(IconName::LoaderCircle).size(px(24.0)))
                            .into_any_element()
                    } else {
                        div()
                            .w(px(56.0))
                            .h(px(56.0))
                            .rounded_2xl()
                            .bg(theme.muted.opacity(0.3))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(svg().data(STAR_SVG).size(px(26.0)).text_color(muted_text.opacity(0.7)))
                            .into_any_element()
                    })
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(text_color)
                            .child(if self.is_loading_starred { "Loading..." } else { "No starred messages" }),
                    )
                    .children((!self.is_loading_starred).then(|| {
                        div()
                            .text_xs()
                            .text_color(muted_text)
                            .text_center()
                            .child("Right-click a message and choose Star to keep it here.")
                    }))
                    .into_any_element()
            } else {
                v_flex()
                    .id("starred-sheet-list")
                    .flex_1()
                    .overflow_y_scroll()
                    .p_2()
                    .gap_1()
                    .child(boxed_list(cx, self.starred_messages.iter().map(|m| {
                        let (chat_id, msg_id) = (m.chat_id.clone(), m.id.clone());
                        let chat_name = m
                            .chat_name
                            .clone()
                            .filter(|name| !name.trim().is_empty())
                            .or_else(|| chats.iter().find(|(id, _)| *id == m.chat_id).map(|(_, name)| name.clone()))
                            .unwrap_or_else(|| m.chat_id.split('@').next().unwrap_or(&m.chat_id).to_string());
                        let date = Self::format_search_date(m.timestamp);
                        let time = Self::format_search_time(m.timestamp);
                        v_flex()
                            .id(SharedString::from(format!("starred-{}", m.id)))
                            .cursor_pointer()
                            .p_3()
                            .rounded_xl()
                            .hover(|s| s.bg(surface.row_hover))
                            .when(!shape.styled, |row| row.border_b_1().border_color(border_color.opacity(0.15)))
                            .gap_1()
                            .tooltip(move |window, cx| Tooltip::new("Click to jump to message").build(window, cx))
                            .on_mouse_down(MouseButton::Left, cx.listener({
                                let (chat_id, msg_id) = (chat_id.clone(), msg_id.clone());
                                move |this, _, window, cx| {
                                    this.open_starred_message(chat_id.clone(), msg_id.clone(), window, cx);
                                }
                            }))
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .min_w(px(0.0))
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(primary_color)
                                            .overflow_hidden()
                                            .line_clamp(1)
                                            .child(chat_name),
                                    )
                                    .child(
                                        div()
                                            .flex_shrink_0()
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(muted_text)
                                            .child(date),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(muted_text)
                                    .child(sender_label(m)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(text_color)
                                    .max_h(px(36.0))
                                    .overflow_hidden()
                                    .child(snippet_of(m)),
                            )
                            .child(
                                h_flex()
                                    .justify_end()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(muted_text.opacity(0.7))
                                            .child(time),
                                    )
                                    .child(
                                        div()
                                            .id(SharedString::from(format!("unstar-{}", m.id)))
                                            .p_1()
                                            .when(shape.styled, |s| {
                                                s.w(px(shape.control_size))
                                                    .h(px(shape.control_size))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                            })
                                            .rounded(px(shape.radius_button))
                                            .hover(|s| s.bg(theme.muted.opacity(0.65)))
                                            .tooltip(move |window, cx| Tooltip::new("Unstar").build(window, cx))
                                            .child(svg().data(STAR_FILLED_SVG).size(px(13.0)).text_color(primary_color))
                                            .on_mouse_down(MouseButton::Left, cx.listener({
                                                let m = m.clone();
                                                move |this, _, _, cx| {
                                                    cx.stop_propagation();
                                                    this.toggle_star(m.clone(), cx);
                                                }
                                            })),
                                    ),
                            )
                    })).when(!shape.styled, |list| list.gap_1()))
                    .when(self.starred_has_more, |list| {
                        list.child(
                            div()
                                .id("starred-load-more")
                                .cursor_pointer()
                                .py_2()
                                .rounded_xl()
                                .text_xs()
                                .text_center()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(primary_color)
                                .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                .child(if self.is_loading_starred { "Loading..." } else { "Load more" })
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                    this.load_starred(true, cx);
                                })),
                        )
                    })
                    .into_any_element()
            })
            .into_any_element()
    }
}

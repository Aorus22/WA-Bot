use crate::components::ui::boxed_list::boxed_list;
use crate::theme::surface::SurfaceExt;
use crate::views::chat::*;

impl ChatView {
    /// Play the Chat Info drawer's exit animation, then unmount it.
    pub fn close_info_sheet(&mut self, cx: &mut Context<Self>) {
        if !self.is_info_sheet_open || self.info_sheet_closing {
            return;
        }
        self.info_sheet_closing = true;
        Self::after_sheet_exit(cx, |this| {
            // Re-opening during the exit cancels the pending unmount.
            if this.info_sheet_closing {
                this.is_info_sheet_open = false;
                this.info_sheet_closing = false;
            }
        });
        cx.notify();
    }

    /// Run `done` once a drawer's exit animation has finished playing.
    pub(crate) fn after_sheet_exit(cx: &mut Context<Self>, done: impl FnOnce(&mut Self) + 'static) {
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(SHEET_ANIM_MS + 40))
                .await;
            this.update(cx, |this, cx| {
                done(this);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Toggle Chat Info Sheet drawer
    pub fn toggle_info_sheet(&mut self, cx: &mut Context<Self>) {
        if self.is_info_sheet_open && !self.info_sheet_closing {
            self.close_info_sheet(cx);
            return;
        }
        self.info_sheet_closing = false;
        self.is_info_sheet_open = true;
        self.info_media_open = false;
        self.info_confirm_exit = false;
        self.is_starred_sheet_open = false;
        self.starred_sheet_closing = false;
        self.is_search_sheet_open = false;
        self.search_sheet_closing = false;
        self.viewed_by_message = None;
        self.viewed_by_closing = false;
        self.load_info_media(cx);
        self.load_info_docs(cx);
        self.load_info_links(cx);
        if let Some(chat_id) = self.selected_chat_id.clone().filter(|id| id.ends_with("@g.us")) {
            self.load_group_info(&chat_id, cx);
        }
        cx.notify();
    }

    /// Load media for active chat info sheet
    pub fn load_info_media(&mut self, cx: &mut Context<Self>) {
        let chat_id = match &self.selected_chat_id {
            Some(id) => id.clone(),
            None => return,
        };
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        self.is_loading_info = true;

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT.spawn(async move { client.get_chat_media(&chat_id, Some(30), None).await }).await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            if let Ok(Ok(media)) = res {
                                this.info_media = media;
                            }
                            this.is_loading_info = false;
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Load docs for active chat info sheet
    pub fn load_info_docs(&mut self, cx: &mut Context<Self>) {
        let chat_id = match &self.selected_chat_id {
            Some(id) => id.clone(),
            None => return,
        };
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT.spawn(async move { client.get_chat_docs(&chat_id, Some(30), None).await }).await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            if let Ok(Ok(docs)) = res {
                                this.info_docs = docs;
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Load links for active chat info sheet
    pub fn load_info_links(&mut self, cx: &mut Context<Self>) {
        let chat_id = match &self.selected_chat_id {
            Some(id) => id.clone(),
            None => return,
        };
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT.spawn(async move { client.get_chat_links(&chat_id, Some(30), None).await }).await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            if let Ok(Ok(links)) = res {
                                this.info_links = links;
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Wraps a right-hand drawer so it plays the web's sheet motion in both
    /// directions: the slot's width animates while the drawer keeps its full
    /// size and is clipped, so the conversation beside it narrows smoothly
    /// instead of snapping and leaving an empty gutter.
    ///
    /// The open and close ids differ so mounting the closing element restarts
    /// the animation at 0, which the exit reads as "still fully open" before
    /// shrinking it away.
    pub(crate) fn animated_sheet(
        open_id: &'static str,
        close_id: &'static str,
        closing: bool,
        sheet: AnyElement,
    ) -> AnyElement {
        let animation = Animation::new(std::time::Duration::from_millis(SHEET_ANIM_MS))
            .with_easing(|d| 1.0 - (1.0 - d).powi(5));
        let slot = div().h_full().flex_shrink_0().overflow_hidden().child(sheet);
        if closing {
            slot.with_animation(close_id, animation, |el, delta| {
                el.w(px(SHEET_WIDTH * (1.0 - delta).clamp(0.0, 1.0)))
            })
            .into_any_element()
        } else {
            slot.with_animation(open_id, animation, |el, delta| {
                el.w(px(SHEET_WIDTH * delta.clamp(0.0, 1.0)))
            })
            .into_any_element()
        }
    }


    /// Right-hand chat info drawer (profile, media/docs/links/members tabs).
    pub(crate) fn render_info_sheet(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        chat: &Chat,
        full_width: bool,
    ) -> AnyElement {
        let theme = *cx.app_theme();
        let shape = cx.shape();
        let surface = cx.surface();
        let border_color = theme.border;
        let bg_color = if shape.adwaita { surface.view } else { theme.background };
        let sheet_border = if shape.adwaita { surface.border } else { border_color.opacity(0.4) };
        let header_bg = if shape.adwaita { surface.headerbar } else { theme.muted.opacity(0.2) };
        let primary_color = theme.primary;
        let text_color = theme.foreground;
        let muted_text = theme.muted_foreground;
        let framed = !crate::popover::is_popover(cx)
            && window_material::is_framed_window(window);
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };
        let media_page = self.info_media_open;
        let sheet = v_flex()
            .when(!full_width, |el| el.w(px(SHEET_WIDTH)).min_w(px(SHEET_WIDTH)).max_w(px(SHEET_WIDTH)))
            .when(full_width, |el| el.size_full())
                .h_full()
                .overflow_hidden()
                .border_l_1()
                .border_color(sheet_border)
                .bg(bg_color)
                .when(framed, |s| s.rounded_br(px(window_material::corner_radius(cx))))
                .flex_shrink_0()
                // Header
                .child(
                    h_flex()
                        .h(px(cx.shape().header_height))
                        .pl_4()
                        .pr(px(crate::components::titlebar::controls_room(cx) + 8.0))
                        .border_b_1()
                        .border_color(sheet_border)
                        .bg(header_bg)
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .text_base()
                                .font_weight(FontWeight::BOLD)
                                .text_color(text_color)
                                .child(if media_page {
                                    "Media, links and docs"
                                } else if chat.is_group {
                                    "Group info"
                                } else {
                                    "Contact info"
                                }),
                        )
                        .child(
                            div()
                                .id("btn-close-info-sheet")
                                .cursor_pointer()
                                .p_1()
                                .when(shape.adwaita, |s| {
                                    s.w(px(shape.control_size))
                                        .h(px(shape.control_size))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                })
                                .rounded(px(shape.radius_button))
                                .hover(|s| s.bg(theme.muted.opacity(0.65)))
                                .tooltip(move |window, cx| {
                                    Tooltip::new(if media_page { "Back" } else { "Close" }).build(window, cx)
                                })
                                .child(svg().data(if media_page { ARROW_LEFT_SVG } else { X_SVG }).size(px(16.0)).text_color(muted_text))
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                    if this.info_media_open {
                                        this.info_media_open = false;
                                        cx.notify();
                                    } else {
                                        this.info_confirm_exit = false;
                                        this.close_info_sheet(cx);
                                    }
                                })),
                        ),
                )
                // Content Scroll
                .child(
                    v_flex()
                        .id("info-sheet-scroll")
                        .flex_1()
                        .overflow_y_scroll()
                        .p_4()
                        .gap_4()
                        .children((!media_page).then(|| self.render_info_main(chat, cx)))
                        .when(media_page, |s| s
                        // Tabs Row: [Media] [Docs] [Links]
                        .child(
                            h_flex()
                                .p_1()
                                .bg(theme.muted.opacity(0.5))
                                .rounded_xl()
                                .border_1()
                                .border_color(border_color.opacity(0.3))
                                .justify_around()
                                .child(
                                    div()
                                        .cursor_pointer()
                                        .px_3()
                                        .py_1p5()
                                        .rounded_lg()
                                        .text_xs()
                                        .font_weight(if self.info_sheet_tab == InfoSheetTab::Media { FontWeight::BOLD } else { FontWeight::NORMAL })
                                        .bg(if self.info_sheet_tab == InfoSheetTab::Media { theme.primary.opacity(0.18) } else { rgba(0x00000000).into() })
                                        .text_color(if self.info_sheet_tab == InfoSheetTab::Media { primary_color } else { muted_text })
                                        .child("Media")
                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                            this.info_sheet_tab = InfoSheetTab::Media;
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    div()
                                        .cursor_pointer()
                                        .px_3()
                                        .py_1p5()
                                        .rounded_lg()
                                        .text_xs()
                                        .font_weight(if self.info_sheet_tab == InfoSheetTab::Docs { FontWeight::BOLD } else { FontWeight::NORMAL })
                                        .bg(if self.info_sheet_tab == InfoSheetTab::Docs { theme.primary.opacity(0.18) } else { rgba(0x00000000).into() })
                                        .text_color(if self.info_sheet_tab == InfoSheetTab::Docs { primary_color } else { muted_text })
                                        .child("Docs")
                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                            this.info_sheet_tab = InfoSheetTab::Docs;
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    div()
                                        .cursor_pointer()
                                        .px_3()
                                        .py_1p5()
                                        .rounded_lg()
                                        .text_xs()
                                        .font_weight(if self.info_sheet_tab == InfoSheetTab::Links { FontWeight::BOLD } else { FontWeight::NORMAL })
                                        .bg(if self.info_sheet_tab == InfoSheetTab::Links { theme.primary.opacity(0.18) } else { rgba(0x00000000).into() })
                                        .text_color(if self.info_sheet_tab == InfoSheetTab::Links { primary_color } else { muted_text })
                                        .child("Links")
                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                            this.info_sheet_tab = InfoSheetTab::Links;
                                            cx.notify();
                                        })),
                                )
                        )
                        // Tab Content
                        .child(
                            match self.info_sheet_tab {
                                InfoSheetTab::Media => {
                                    if self.info_media.is_empty() {
                                        if self.is_loading_info {
                                            v_flex()
                                                .py_8()
                                                .items_center()
                                                .gap_2()
                                                .child(Spinner::new().color(primary_color))
                                                .child(div().text_sm().text_color(muted_text).child("Loading media..."))
                                                .into_any_element()
                                        } else {
                                            v_flex()
                                                .py_8()
                                                .items_center()
                                                .gap_2()
                                                .opacity(0.4)
                                                .child(svg().data(IMAGE_SVG).size(px(48.0)).text_color(muted_text))
                                                .child(div().text_sm().text_color(muted_text).child("No media shared yet"))
                                                .into_any_element()
                                        }
                                    } else {
                                        // Web parity: 3-column square thumbnail grid.
                                        // Images open the full-screen preview, videos open externally.
                                        let rows: Vec<Vec<(bool, String)>> = self
                                            .info_media
                                            .chunks(3)
                                            .map(|chunk| {
                                                chunk
                                                    .iter()
                                                    .map(|m| {
                                                        let is_image = m.message_type == "image";
                                                        let url = m
                                                            .media_url
                                                            .as_deref()
                                                            .map(|u| resolve_media_url(u, &base_url))
                                                            .unwrap_or_default();
                                                        (is_image, url)
                                                    })
                                                    .collect()
                                            })
                                            .collect();

                                        v_flex()
                                            .gap_2()
                                            .children(rows.into_iter().map(|row| {
                                                h_flex()
                                                    .gap_2()
                                                    .children(row.into_iter().map(|(is_image, url)| {
                                                        let cell = div()
                                                            .w(px(88.0))
                                                            .h(px(88.0))
                                                            .rounded_lg()
                                                            .overflow_hidden()
                                                            .border_1()
                                                            .border_color(border_color.opacity(0.4))
                                                            .bg(theme.muted.opacity(0.2))
                                                            .cursor_pointer();

                                                        if is_image && !url.is_empty() {
                                                            cell.on_mouse_down(MouseButton::Left, cx.listener({
                                                                let u = url.clone();
                                                                move |this, _, _, cx| {
                                                                    this.open_preview(u.clone(), None, cx);
                                                                }
                                                            }))
                                                            .child(
                                                                img(url.clone())
                                                                    .id(format!("info-media-{url}"))
                                                                    .w_full()
                                                                    .h_full()
                                                                    .object_fit(ObjectFit::Cover),
                                                            )
                                                            .into_any_element()
                                                        } else if !url.is_empty() {
                                                            cell.on_mouse_down(MouseButton::Left, cx.listener({
                                                                let u = url.clone();
                                                                move |_, _, _, cx| {
                                                                    cx.open_url(&u);
                                                                }
                                                            }))
                                                            .child(
                                                                div()
                                                                    .w_full()
                                                                    .h_full()
                                                                    .flex()
                                                                    .items_center()
                                                                    .justify_center()
                                                                    .child(svg().data(VIDEO_SVG).size(px(28.0)).text_color(primary_color.opacity(0.4))),
                                                            )
                                                            .into_any_element()
                                                        } else {
                                                            cell.into_any_element()
                                                        }
                                                    }))
                                            }))
                                            .into_any_element()
                                    }
                                }
                                InfoSheetTab::Docs => {
                                    if self.info_docs.is_empty() {
                                        v_flex()
                                            .py_8()
                                            .items_center()
                                            .gap_2()
                                            .opacity(0.4)
                                            .child(svg().data(FILE_TEXT_SVG).size(px(48.0)).text_color(muted_text))
                                            .child(div().text_sm().text_color(muted_text).child("No documents shared yet"))
                                            .into_any_element()
                                    } else {
                                        // Web parity: tappable document rows that open the file.
                                        let docs: Vec<(String, String)> = self
                                            .info_docs
                                            .iter()
                                            .map(|d| {
                                                let url = d
                                                    .media_url
                                                    .as_deref()
                                                    .map(|u| resolve_media_url(u, &base_url))
                                                    .unwrap_or_default();
                                                (url, Self::format_search_date(d.timestamp))
                                            })
                                            .collect();

                                        boxed_list(cx, docs.into_iter().map(|(url, date)| {
                                                h_flex()
                                                    .gap_3()
                                                    .p_3()
                                                    .when(!shape.adwaita, |row| row.rounded_xl().border_1().border_color(border_color.opacity(0.4)))
                                                    .hover(|s| s.bg(if shape.adwaita { theme.foreground.opacity(0.03) } else { theme.muted.opacity(0.5) }))
                                                    .cursor_pointer()
                                                    .on_mouse_down(MouseButton::Left, cx.listener({
                                                        let u = url.clone();
                                                        move |_, _, _, cx| {
                                                            if !u.is_empty() {
                                                                cx.open_url(&u);
                                                            }
                                                        }
                                                    }))
                                                    .child(
                                                        div()
                                                            .w(px(40.0))
                                                            .h(px(40.0))
                                                            .rounded_lg()
                                                            .bg(rgb(0x3b82f6).opacity(0.1))
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .child(svg().data(FILE_TEXT_SVG).size(px(20.0)).text_color(rgb(0x2563eb))),
                                                    )
                                                    .child(
                                                        v_flex()
                                                            .flex_1()
                                                            .overflow_hidden()
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .font_weight(FontWeight::BOLD)
                                                                    .text_color(text_color)
                                                                    .overflow_hidden()
                                                                    .child("Document"),
                                                            )
                                                            .child(div().text_xs().text_color(muted_text).child(date)),
                                                    )
                                            }))
                                            .when(!shape.adwaita, |list| list.gap_3())
                                            .into_any_element()
                                    }
                                }
                                InfoSheetTab::Links => {
                                    if self.info_links.is_empty() {
                                        v_flex()
                                            .py_8()
                                            .items_center()
                                            .gap_2()
                                            .opacity(0.4)
                                            .child(svg().data(LINK_SVG).size(px(48.0)).text_color(muted_text))
                                            .child(div().text_sm().text_color(muted_text).child("No links shared yet"))
                                            .into_any_element()
                                    } else {
                                        // Web parity: extract every http(s) URL from the message body.
                                        let links: Vec<(String, String)> = self
                                            .info_links
                                            .iter()
                                            .flat_map(|l| {
                                                let ts = l.timestamp;
                                                l.content
                                                    .split_whitespace()
                                                    .filter(|t| t.starts_with("http://") || t.starts_with("https://"))
                                                    .map(move |t| (t.to_string(), Self::format_search_date(ts)))
                                                    .collect::<Vec<_>>()
                                            })
                                            .collect();

                                        boxed_list(cx, links.into_iter().map(|(url, date)| {
                                                h_flex()
                                                    .gap_3()
                                                    .p_3()
                                                    .when(!shape.adwaita, |row| row.rounded_xl().border_1().border_color(border_color.opacity(0.4)))
                                                    .hover(|s| s.bg(if shape.adwaita { theme.foreground.opacity(0.03) } else { theme.muted.opacity(0.5) }))
                                                    .hover(|s| s.bg(if shape.adwaita { theme.foreground.opacity(0.03) } else { theme.muted.opacity(0.5) }))
                                                    .cursor_pointer()
                                                    .on_mouse_down(MouseButton::Left, cx.listener({
                                                        let u = url.clone();
                                                        move |_, _, _, cx| {
                                                            cx.open_url(&u);
                                                        }
                                                    }))
                                                    .child(
                                                        div()
                                                            .w(px(40.0))
                                                            .h(px(40.0))
                                                            .rounded_lg()
                                                            .bg(rgb(0x22c55e).opacity(0.1))
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .child(svg().data(LINK_SVG).size(px(20.0)).text_color(rgb(0x16a34a))),
                                                    )
                                                    .child(
                                                        v_flex()
                                                            .flex_1()
                                                            .overflow_hidden()
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .text_color(rgb(0x3b82f6))
                                                                    .overflow_hidden()
                                                                    .child(url),
                                                            )
                                                            .child(div().text_xs().text_color(muted_text).child(date)),
                                                    )
                                            }))
                                            .when(!shape.adwaita, |list| list.gap_3())
                                            .into_any_element()
                                    }
                                }
                                // Members live on the main page now.
                                InfoSheetTab::Members => div().into_any_element()
                            }
                        )),
                )
                ;
        if full_width {
            sheet.into_any_element()
        } else {
            Self::animated_sheet("chat-info-sheet-in", "chat-info-sheet-out", self.info_sheet_closing, sheet.into_any_element())
        }
    }
}

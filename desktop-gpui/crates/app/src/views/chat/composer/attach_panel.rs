use crate::views::chat::*;
use crate::theme::surface::SurfaceExt;

impl ChatView {
    /// One row of the attach (+) menu: tinted lucide icon + label, matching
    /// the web's plus popover.
    pub(crate) fn attach_menu_row(
        label: String,
        icon: &'static [u8],
        accent: Hsla,
        text_color: Hsla,
        hover_bg: Hsla,
        handler: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let key = SharedString::from(format!("am-{}", label.to_lowercase().replace(' ', "-")));
        with_hover_fade(
            h_flex().id(key.clone()).w_full().cursor_pointer().items_center().gap_3().px_3().py_2().rounded_lg(),
            key,
            hover_bg,
            px(8.0),
            window,
            cx,
        )
            .child(svg().data(icon).size(px(16.0)).flex_shrink_0().text_color(accent))
            .child(div().text_sm().text_color(text_color).child(label))
            .on_mouse_down(MouseButton::Left, handler)
            .into_any_element()
    }

    /// Attach (+) menu, emoji picker and sticker picker, drawn just above the
    /// compose input row (web's `ChatArea` plus popover).
    pub(crate) fn render_attach_panel(
        &self,
        theme: ActiveTokens,
        border_color: Hsla,
        card_bg: Hsla,
        primary_color: Hsla,
        text_color: Hsla,
        muted_text: Hsla,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let hover_bg = theme.foreground.opacity(0.12);
        let shape = cx.shape();
        let surface = cx.surface();
        // Floats above the compose bar: absolute so it never takes flow space
        // (an in-flow panel would stretch the compose container over the chat).
        // Eases up from the compose bar; keyed per panel so switching between
        // the menu, emoji and sticker panels replays it.
        let panel_shell = |id: &'static str, width: f32, children: Vec<AnyElement>| {
            v_flex()
                .absolute()
                .occlude()
                .left(px(12.0))
                .bottom_full()
                .mb(px(8.0))
                .w(px(width))
                .p_2()
                .gap_1()
                .rounded_2xl()
                .bg(card_bg)
                .border_1()
                .border_color(border_color)
                .shadow_2xl()
                .children(children)
                .with_animation(
                    id,
                    crate::components::motion::enter(crate::components::motion::FAST_MS + 40),
                    |el, d| el.opacity(d).mb(px(8.0 * d)),
                )
                .into_any_element()
        };

        // Emoji / sticker switch at the bottom of both pickers, as in WhatsApp Web.
        let picker_switch = |sticker_active: bool, cx: &mut Context<Self>| -> AnyElement {
            let segment = |id: &'static str, icon: &'static [u8], active: bool| {
                let active_bg = surface.active_toggle_bg;
                let hover_bg = if active { active_bg } else { surface.attachment_hover };
                div()
                    .id(id)
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(52.0))
                    .h(px(30.0))
                    .rounded(px(if active { shape.radius_toggle_selected } else { shape.radius_button }))
                    .bg(if active { active_bg } else { rgba(0x00000000).into() })
                    .when(active && shape.styled, |segment| segment.shadow_sm())
                    .hover(move |s| s.bg(hover_bg))
                    .child(svg().data(icon).size(px(17.0)).text_color(if active { text_color } else { muted_text }))
            };
            h_flex()
                .w_full()
                .justify_center()
                .pt_1()
                .child(
                    h_flex()
                        .p(px(3.0))
                        .gap_0p5()
                        .rounded(px(shape.radius_button))
                        .bg(surface.attachment_separator)
                        .child(segment("picker-emoji", SMILE_SVG, !sticker_active)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.open_emoji_panel(cx))))
                        .child(segment("picker-sticker", STICKER_SVG, sticker_active)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.open_sticker_panel(cx)))),
                )
                .into_any_element()
        };

        match self.attach_panel {
            AttachPanel::None => None,
            AttachPanel::Menu => {
                let rows: Vec<AnyElement> = vec![
                    Self::attach_menu_row(
                        "Emoji".to_string(),
                        SMILE_SVG,
                        rgb(0xeab308).into(),
                        text_color,
                        hover_bg,
                        cx.listener(|this, _, _, cx| this.open_emoji_panel(cx)),
                        window,
                        cx,
                    ),
                    Self::attach_menu_row(
                        "Sticker".to_string(),
                        STICKER_SVG,
                        rgb(0xa855f7).into(),
                        text_color,
                        hover_bg,
                        cx.listener(|this, _, _, cx| this.open_sticker_panel(cx)),
                        window,
                        cx,
                    ),
                    Self::attach_menu_row(
                        "Media".to_string(),
                        PAPERCLIP_SVG,
                        rgb(0x3b82f6).into(),
                        text_color,
                        hover_bg,
                        cx.listener(|this, _, _, cx| this.pick_attachment(MediaKind::Media, cx)),
                        window,
                        cx,
                    ),
                    Self::attach_menu_row(
                        "Document".to_string(),
                        FILE_TEXT_SVG,
                        rgb(0xf97316).into(),
                        text_color,
                        hover_bg,
                        cx.listener(|this, _, _, cx| {
                            this.pick_attachment(MediaKind::Document, cx)
                        }),
                        window,
                        cx,
                    ),
                    Self::attach_menu_row(
                        "Audio".to_string(),
                        MIC_SVG,
                        rgb(0xef4444).into(),
                        text_color,
                        hover_bg,
                        cx.listener(|this, _, _, cx| this.pick_attachment(MediaKind::Audio, cx)),
                        window,
                        cx,
                    ),
                    Self::attach_menu_row(
                        "GIF".to_string(),
                        IMAGE_PLAY_SVG,
                        rgb(0xec4899).into(),
                        text_color,
                        hover_bg,
                        cx.listener(|this, _, _, cx| this.pick_attachment(MediaKind::Gif, cx)),
                        window,
                        cx,
                    ),
                    Self::attach_menu_row(
                        "Poll".to_string(),
                        CHART_COLUMN_SVG,
                        rgb(0x06b6d4).into(),
                        text_color,
                        hover_bg,
                        cx.listener(|this, _, window, cx| {
                            this.open_compose_dialog(ComposeDialog::Poll, cx);
                            this.compose_field_focus.focus(window, cx);
                        }),
                        window,
                        cx,
                    ),
                    Self::attach_menu_row(
                        "Location".to_string(),
                        MAP_PIN_SVG,
                        rgb(0x10b981).into(),
                        text_color,
                        hover_bg,
                        cx.listener(|this, _, window, cx| {
                            this.open_compose_dialog(ComposeDialog::Location, cx);
                            this.compose_field_focus.focus(window, cx);
                        }),
                        window,
                        cx,
                    ),
                    Self::attach_menu_row(
                        "Contact".to_string(),
                        USER_SVG,
                        rgb(0x6366f1).into(),
                        text_color,
                        hover_bg,
                        cx.listener(|this, _, window, cx| {
                            this.open_compose_dialog(ComposeDialog::Contact, cx);
                            this.compose_field_focus.focus(window, cx);
                        }),
                        window,
                        cx,
                    ),
                    Self::attach_menu_row(
                        format!("Markdown {}", if self.is_md_mode { "(on)" } else { "(off)" }),
                        FILE_TEXT_SVG,
                        if self.is_md_mode { primary_color } else { muted_text },
                        if self.is_md_mode { primary_color } else { text_color },
                        hover_bg,
                        cx.listener(|this, _, window, cx| this.toggle_markdown_mode(window, cx)),
                        window,
                        cx,
                    ),
                ];
                Some(
                    v_flex()
                        .absolute()
                        .occlude()
                        .left(px(12.0))
                        .bottom_full()
                        .mb(px(8.0))
                        .w(px(216.0))
                        .p_1()
                        .gap_1()
                        .rounded_2xl()
                        .bg(card_bg)
                        .border_1()
                        .border_color(border_color)
                        .shadow_2xl()
                        .children(rows)
                        .with_animation(
                            "attach-menu-in",
                            crate::components::motion::enter(crate::components::motion::FAST_MS + 40),
                            |el, d| el.opacity(d).mb(px(8.0 * d)),
                        )
                        .into_any_element(),
                )
            }
            AttachPanel::Emoji => {
                let tabs: Vec<AnyElement> = EMOJI_CATEGORIES
                    .iter()
                    .enumerate()
                    .map(|(idx, category)| {
                        let active = idx == self.emoji_picker.selected_category_idx;
                        div()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .cursor_pointer()
                            .text_xs()
                            .bg(if active {
                                primary_color.opacity(0.15)
                            } else {
                                rgba(0x00000000).into()
                            })
                            .text_color(if active { primary_color } else { muted_text })
                            .child(category.name)
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                this.emoji_picker.selected_category_idx = idx;
                                cx.notify();
                            }))
                            .into_any_element()
                    })
                    .collect();

                let emojis: Vec<AnyElement> = self
                    .emoji_picker
                    .current_emojis()
                    .into_iter()
                    .map(|emoji| {
                        let em = emoji.to_string();
                        let em_handler = em.clone();
                        div()
                            .w(px(30.0))
                            .h(px(30.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_md()
                            .cursor_pointer()
                            .text_size(px(16.0))
                            .hover(move |s| s.bg(hover_bg))
                            .child(em)
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                                this.insert_emoji(&em_handler, window, cx);
                            }))
                            .into_any_element()
                    })
                    .collect();

                Some(panel_shell("attach-emoji-in", 320.0, vec![
                    h_flex()
                        .w_full()
                        .justify_between()
                        .items_center()
                        .px_1()
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(svg().data(SMILE_SVG).size(px(14.0)).text_color(rgb(0xeab308)))
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(text_color)
                                        .child("Emoji"),
                                ),
                        )
                        .child(
                            div()
                                .cursor_pointer()
                                .p_1()
                                .rounded(px(cx.shape().radius_button))
                                .hover(move |s| s.bg(hover_bg))
                                .child(svg().data(X_SVG).size(px(13.0)).text_color(muted_text))
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                    this.close_attach_panel(cx);
                                })),
                        )
                        .into_any_element(),
                    h_flex()
                        .w_full()
                        .gap_1()
                        .px_1()
                        .children(tabs)
                        .into_any_element(),
                    v_flex()
                        .id("attach-emoji-scroll")
                        .max_h(px(190.0))
                        .overflow_y_scroll()
                        .child(h_flex().flex_wrap().gap_1().children(emojis))
                        .into_any_element(),
                    picker_switch(false, cx),
                ]))
            }
            AttachPanel::Sticker => {
                let tab = self.sticker_picker.tab;
                let tabs = [
                    (StickerTab::Recent, "sticker-tab-recent", CLOCK_SVG, "Recent"),
                    (StickerTab::Starred, "sticker-tab-starred", STAR_SVG, "Starred"),
                    (StickerTab::Custom, "sticker-tab-custom", HEART_SVG, "My stickers"),
                ]
                .into_iter()
                .map(|(kind, id, icon, label)| {
                    let active = tab == kind;
                    div()
                        .id(id)
                        .cursor_pointer()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap_1()
                        .px_4()
                        .pt_1()
                        .tooltip(move |window, cx| Tooltip::new(label).build(window, cx))
                        .child(svg().data(icon).size(px(20.0)).text_color(if active { text_color } else { muted_text }))
                        .child(
                            div()
                                .h(px(3.0))
                                .w(px(26.0))
                                .rounded_full()
                                .bg(if active { primary_color } else { rgba(0x00000000).into() }),
                        )
                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                            this.sticker_picker.tab = kind;
                            cx.notify();
                        }))
                        .into_any_element()
                })
                .collect::<Vec<_>>();

                let visible = self.sticker_picker.visible();
                let body: AnyElement = if self.sticker_picker.is_loading {
                    h_flex()
                        .w_full()
                        .py_8()
                        .justify_center()
                        .child(Spinner::new().color(primary_color))
                        .into_any_element()
                } else if visible.is_empty() {
                    div()
                        .w_full()
                        .px_4()
                        .py_8()
                        .text_sm()
                        .text_center()
                        .text_color(muted_text)
                        .child(match tab {
                            StickerTab::Recent => "Stickers you send will show up here.",
                            StickerTab::Starred => "Star a sticker in WhatsApp on your phone and it will show up here.",
                            StickerTab::Custom => "Right-click a sticker in a chat and choose Add to favourites.",
                        })
                        .into_any_element()
                } else {
                    let base_url = Self::compose_base_url(cx);
                    let stickers: Vec<AnyElement> = visible
                        .into_iter()
                        .map(|sticker| {
                            let url = resolve_media_url(&sticker.media_url, &base_url);
                            let send_url = sticker.media_url.clone();
                            let is_animated = sticker.is_animated;
                            let key = SharedString::from(format!("sticker-tile-{}", sticker.id));
                            with_hover_fade(
                                div().id(key.clone()).w(px(96.0)).h(px(96.0)).p_1().rounded_xl().overflow_hidden().cursor_pointer().bg(theme.muted.opacity(0.3)),
                                key,
                                theme.foreground.opacity(0.14),
                                px(12.0),
                                window,
                                cx,
                            )
                                .child(
                                    img(url)
                                        .id(format!("sticker-{}-{}", sticker.id, sticker.media_url))
                                        .w_full()
                                        .h_full()
                                        .object_fit(ObjectFit::Contain),
                                )
                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                    this.send_sticker(send_url.clone(), is_animated, cx);
                                }))
                                .into_any_element()
                        })
                        .collect();
                    v_flex()
                        .id("attach-sticker-scroll")
                        .h(px(300.0))
                        .overflow_y_scroll()
                        .child(h_flex().flex_wrap().gap_2().children(stickers))
                        .into_any_element()
                };

                Some(panel_shell("attach-sticker-in", 440.0, vec![
                    h_flex().w_full().justify_center().children(tabs).into_any_element(),
                    body,
                    picker_switch(true, cx),
                ]))
            }
        }
    }

}

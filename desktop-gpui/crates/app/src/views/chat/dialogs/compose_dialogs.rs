use crate::views::chat::*;

impl ChatView {
    /// Text field row used by the compose dialogs. All rows share one focus
    /// handle; `active_compose_field` decides where keystrokes land.
    pub(crate) fn dialog_field_row(
        index: usize,
        value: &str,
        placeholder: &str,
        is_active: bool,
        theme: ActiveTokens,
        border_color: Hsla,
        text_color: Hsla,
        muted_text: Hsla,
        primary_color: Hsla,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let display = if value.is_empty() {
            placeholder.to_string()
        } else {
            value.to_string()
        };
        h_flex()
            .w_full()
            .px_3()
            .py_2()
            .rounded_xl()
            .border_1()
            .border_color(if is_active { primary_color } else { border_color })
            .bg(theme.background)
            .cursor_text()
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                this.active_compose_field = index;
                this.compose_field_focus.focus(window, cx);
                cx.notify();
            }))
            .child(
                div()
                    .text_sm()
                    .text_color(if value.is_empty() { muted_text } else { text_color })
                    .child(display),
            )
            .into_any_element()
    }

    /// Checkbox row used by the compose dialogs (multi-select, live location).
    pub(crate) fn dialog_toggle_row(
        label: &'static str,
        checked: bool,
        border_color: Hsla,
        text_color: Hsla,
        primary_color: Hsla,
        handler: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> AnyElement {
        h_flex()
            .w_full()
            .cursor_pointer()
            .items_center()
            .gap_2()
            .on_mouse_down(MouseButton::Left, handler)
            .child(
                div()
                    .w(px(18.0))
                    .h(px(18.0))
                    .rounded_md()
                    .border_1()
                    .border_color(if checked { primary_color } else { border_color })
                    .bg(if checked { primary_color } else { rgba(0x00000000).into() })
                    .flex()
                    .items_center()
                    .justify_center()
                    .children(if checked {
                        Some(svg().data(CHECK_SVG).size(px(11.0)).text_color(rgb(0xffffff)))
                    } else {
                        None
                    }),
            )
            .child(div().text_sm().text_color(text_color).child(label))
            .into_any_element()
    }

    /// Footer button used by the compose dialogs.
    pub(crate) fn dialog_button(
        label: String,
        primary: bool,
        border_color: Hsla,
        text_color: Hsla,
        primary_color: Hsla,
        handler: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> AnyElement {
        div()
            .cursor_pointer()
            .px_4()
            .py_2()
            .rounded_xl()
            .border_1()
            .border_color(if primary { primary_color } else { border_color })
            .bg(if primary { primary_color } else { rgba(0x00000000).into() })
            .text_xs()
            .font_weight(if primary { FontWeight::BOLD } else { FontWeight::MEDIUM })
            .text_color(if primary { rgb(0xffffff).into() } else { text_color })
            .child(label)
            .on_mouse_down(MouseButton::Left, handler)
            .into_any_element()
    }

    /// Poll / location / contact modal, mirroring the web's compose dialogs.
    pub(crate) fn render_compose_dialog(
        &self,
        theme: ActiveTokens,
        border_color: Hsla,
        card_bg: Hsla,
        primary_color: Hsla,
        text_color: Hsla,
        muted_text: Hsla,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let dialog = self.compose_dialog?;
        let active = self.active_compose_field;
        let busy = self.is_sending_attachment;

        let (title, body, action_label): (&str, Vec<AnyElement>, &str) = match dialog {
            ComposeDialog::Poll => {
                let options: Vec<AnyElement> = self
                    .poll_draft
                    .options
                    .iter()
                    .enumerate()
                    .map(|(idx, option)| {
                        let index = idx + 1;
                        let can_remove = self.poll_draft.options.len() > 2;
                        h_flex()
                            .w_full()
                            .gap_2()
                            .items_center()
                            .child(
                                div().flex_1().child(Self::dialog_field_row(
                                    index,
                                    option,
                                    &format!("Opsi {}", index),
                                    active == index,
                                    theme,
                                    border_color,
                                    text_color,
                                    muted_text,
                                    primary_color,
                                    cx,
                                )),
                            )
                            .children(if can_remove {
                                Some(
                                    div()
                                        .cursor_pointer()
                                        .p_2()
                                        .rounded_lg()
                                        .hover(move |s| s.bg(theme.muted.opacity(0.5)))
                                        .child(svg().data(X_SVG).size(px(13.0)).text_color(muted_text))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |this, _, _, cx| {
                                                this.poll_draft.remove_option(idx);
                                                if this.active_compose_field > idx + 1 {
                                                    this.active_compose_field -= 1;
                                                }
                                                cx.notify();
                                            }),
                                        )
                                        .into_any_element(),
                                )
                            } else {
                                None
                            })
                            .into_any_element()
                    })
                    .collect();

                let at_max = self.poll_draft.options.len() >= crate::components::compose::POLL_MAX_OPTIONS;
                let mut body: Vec<AnyElement> = vec![Self::dialog_field_row(
                    0,
                    &self.poll_draft.question,
                    "Pertanyaan",
                    active == 0,
                    theme,
                    border_color,
                    text_color,
                    muted_text,
                    primary_color,
                    cx,
                )];
                body.extend(options);
                body.push(
                    div()
                        .w_full()
                        .cursor_pointer()
                        .px_3()
                        .py_1p5()
                        .rounded_lg()
                        .border_1()
                        .border_color(border_color)
                        .text_xs()
                        .text_color(if at_max { muted_text } else { primary_color })
                        .child("+ Tambah opsi")
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                            this.poll_draft.add_option();
                            cx.notify();
                        }))
                        .into_any_element(),
                );
                body.push(Self::dialog_toggle_row(
                    "Izinkan jawaban ganda",
                    self.poll_draft.multi_select,
                    border_color,
                    text_color,
                    primary_color,
                    cx.listener(|this, _, _, cx| {
                        this.poll_draft.multi_select = !this.poll_draft.multi_select;
                        cx.notify();
                    }),
                ));
                ("Buat poll", body, "Kirim")
            }
            ComposeDialog::Location => {
                let body = vec![
                    h_flex()
                        .w_full()
                        .gap_2()
                        .child(
                            div().flex_1().child(Self::dialog_field_row(
                                0,
                                &self.location_draft.latitude,
                                "Latitude",
                                active == 0,
                                theme,
                                border_color,
                                text_color,
                                muted_text,
                                primary_color,
                                cx,
                            )),
                        )
                        .child(
                            div().flex_1().child(Self::dialog_field_row(
                                1,
                                &self.location_draft.longitude,
                                "Longitude",
                                active == 1,
                                theme,
                                border_color,
                                text_color,
                                muted_text,
                                primary_color,
                                cx,
                            )),
                        )
                        .into_any_element(),
                    Self::dialog_field_row(
                        2,
                        &self.location_draft.name,
                        "Nama tempat (opsional)",
                        active == 2,
                        theme,
                        border_color,
                        text_color,
                        muted_text,
                        primary_color,
                        cx,
                    ),
                    Self::dialog_field_row(
                        3,
                        &self.location_draft.address,
                        "Alamat (opsional)",
                        active == 3,
                        theme,
                        border_color,
                        text_color,
                        muted_text,
                        primary_color,
                        cx,
                    ),
                    Self::dialog_toggle_row(
                        "Bagikan lokasi live",
                        self.location_draft.live,
                        border_color,
                        text_color,
                        primary_color,
                        cx.listener(|this, _, _, cx| {
                            this.location_draft.live = !this.location_draft.live;
                            cx.notify();
                        }),
                    ),
                ];
                ("Bagikan lokasi", body, "Bagikan")
            }
            ComposeDialog::Contact => {
                let body = vec![
                    Self::dialog_field_row(
                        0,
                        &self.contact_draft.name,
                        "Nama kontak",
                        active == 0,
                        theme,
                        border_color,
                        text_color,
                        muted_text,
                        primary_color,
                        cx,
                    ),
                    Self::dialog_field_row(
                        1,
                        &self.contact_draft.phone,
                        "Nomor telepon (mis. +62812…)",
                        active == 1,
                        theme,
                        border_color,
                        text_color,
                        muted_text,
                        primary_color,
                        cx,
                    ),
                ];
                ("Bagikan kontak", body, "Bagikan")
            }
        };

        let submit = move |this: &mut ChatView, cx: &mut Context<ChatView>| match dialog {
            ComposeDialog::Poll => this.send_poll(cx),
            ComposeDialog::Location => this.send_location(cx),
            ComposeDialog::Contact => this.send_contact(cx),
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
                        .w(px(420.0))
                        .p_5()
                        .gap_3()
                        .rounded_2xl()
                        .bg(card_bg)
                        .border_1()
                        .border_color(border_color)
                        .shadow_2xl()
                        .track_focus(&self.compose_field_focus)
                        .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                            this.dialog_key_input(ev, cx);
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
                                        .text_color(text_color)
                                        .child(title),
                                )
                                .child(
                                    div()
                                        .cursor_pointer()
                                        .p_1()
                                        .rounded_full()
                                        .hover(move |s| s.bg(theme.muted.opacity(0.5)))
                                        .child(svg().data(X_SVG).size(px(15.0)).text_color(muted_text))
                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                            this.close_compose_dialog(cx);
                                        })),
                                ),
                        )
                        .children(body)
                        .child(
                            h_flex()
                                .w_full()
                                .justify_end()
                                .gap_2()
                                .child(Self::dialog_button(
                                    "Batal".to_string(),
                                    false,
                                    border_color,
                                    text_color,
                                    primary_color,
                                    cx.listener(|this, _, _, cx| this.close_compose_dialog(cx)),
                                ))
                                .child(Self::dialog_button(
                                    if busy { "Mengirim…".to_string() } else { action_label.to_string() },
                                    true,
                                    border_color,
                                    text_color,
                                    primary_color,
                                    cx.listener(move |this, _, _, cx| submit(this, cx)),
                                )),
                        ),
                ),
                "compose-dialog-in",
            )
            .into_any_element())
    }

}

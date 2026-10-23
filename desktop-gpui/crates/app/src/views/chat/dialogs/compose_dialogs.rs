use crate::theme::shape::ShapeExt;
use crate::theme::surface::SurfaceExt;
use crate::views::chat::*;

impl ChatView {
    /// Text input used by the compose dialogs. Inputs are created on demand,
    /// keyed by `key`, seeded from the matching draft and mirror edits back to it.
    pub(crate) fn dialog_field_row(
        &mut self,
        key: &str,
        placeholder: &str,
        theme: &ActiveTokens,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if !self.dialog_fields.contains_key(key) {
            let field_key = key.to_string();
            let field = TextField::new(placeholder.to_string(), window, cx, move |this, event, _, cx| {
                if let TextFieldEvent::Change(value) = event {
                    this.set_dialog_value(&field_key, value);
                    cx.notify();
                }
            });
            let initial = self.dialog_value(key);
            if !initial.is_empty() {
                field.set_value(initial, window, cx);
            }
            if matches!(key, "poll-question" | "location-latitude" | "contact-name") {
                field.focus(window, cx);
            }
            self.dialog_fields.insert(key.to_string(), field);
        }
        self.dialog_fields[key]
                    .element(theme, cx.shape(), None)
                    .into_any_element()
    }

    fn dialog_value(&self, key: &str) -> String {
        match key {
            "poll-question" => self.poll_draft.question.clone(),
            "location-latitude" => self.location_draft.latitude.clone(),
            "location-longitude" => self.location_draft.longitude.clone(),
            "location-name" => self.location_draft.name.clone(),
            "location-address" => self.location_draft.address.clone(),
            "contact-name" => self.contact_draft.name.clone(),
            "contact-phone" => self.contact_draft.phone.clone(),
            _ => key
                .strip_prefix("poll-option-")
                .and_then(|i| i.parse::<usize>().ok())
                .and_then(|i| self.poll_draft.options.get(i).cloned())
                .unwrap_or_default(),
        }
    }

    fn set_dialog_value(&mut self, key: &str, value: String) {
        match key {
            "poll-question" => self.poll_draft.question = value,
            "location-latitude" => self.location_draft.latitude = value,
            "location-longitude" => self.location_draft.longitude = value,
            "location-name" => self.location_draft.name = value,
            "location-address" => self.location_draft.address = value,
            "contact-name" => self.contact_draft.name = value,
            "contact-phone" => self.contact_draft.phone = value,
            _ => {
                if let Some(slot) = key
                    .strip_prefix("poll-option-")
                    .and_then(|i| i.parse::<usize>().ok())
                    .and_then(|i| self.poll_draft.options.get_mut(i))
                {
                    *slot = value;
                }
            }
        }
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
        shape: crate::theme::shape::ShapeTokens,
        handler: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> AnyElement {
        div()
            .cursor_pointer()
            .px_4()
            .py_2()
            .rounded_xl()
            .when(shape.adwaita, |button| button.rounded(px(shape.radius_button)))
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
        &mut self,
        window: &mut Window,
        theme: ActiveTokens,
        border_color: Hsla,
        card_bg: Hsla,
        primary_color: Hsla,
        text_color: Hsla,
        muted_text: Hsla,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let dialog = self.compose_dialog?;
        let shape = cx.shape();
        let surface = cx.surface();
        let scrim = if shape.adwaita { surface.scrim } else { surface.scrim_default };
        let dialog_bg = if shape.adwaita { surface.dialog } else { card_bg };
        let dialog_border = if shape.adwaita { surface.border } else { border_color };
        let busy = self.is_sending_attachment;

        let (title, body, action_label): (&str, Vec<AnyElement>, &str) = match dialog {
            ComposeDialog::Poll => {
                let option_count = self.poll_draft.options.len();
                let can_remove = option_count > 2;
                let options: Vec<AnyElement> = (0..option_count)
                    .map(|idx| {
                        let field = self.dialog_field_row(
                            &format!("poll-option-{idx}"),
                            &format!("Opsi {}", idx + 1),
                            &theme,
                            window,
                            cx,
                        );
                        h_flex()
                            .w_full()
                            .gap_2()
                            .items_center()
                            .child(div().flex_1().child(field))
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
                                                // Later options shift up: rebuild their inputs.
                                                this.dialog_fields.retain(|key, _| !key.starts_with("poll-option-"));
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

                let at_max = self.poll_draft.options.len() >= POLL_MAX_OPTIONS;
                let mut body: Vec<AnyElement> = vec![self.dialog_field_row("poll-question", "Pertanyaan", &theme, window, cx)];
                body.extend(options);
                body.push(
                    div()
                        .w_full()
                        .cursor_pointer()
                        .px_3()
                        .py_1p5()
                        .rounded_lg()
                        .when(shape.adwaita, |button| button.rounded(px(shape.radius_button)))
                        .border_1()
                        .border_color(dialog_border)
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
                    dialog_border,
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
                            div().flex_1().child(self.dialog_field_row("location-latitude", "Latitude", &theme, window, cx)),
                        )
                        .child(
                            div().flex_1().child(self.dialog_field_row("location-longitude", "Longitude", &theme, window, cx)),
                        )
                        .into_any_element(),
                    self.dialog_field_row("location-name", "Nama tempat (opsional)", &theme, window, cx),
                    self.dialog_field_row("location-address", "Alamat (opsional)", &theme, window, cx),
                    Self::dialog_toggle_row(
                        "Bagikan lokasi live",
                        self.location_draft.live,
                        dialog_border,
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
                    self.dialog_field_row("contact-name", "Nama kontak", &theme, window, cx),
                    self.dialog_field_row("contact-phone", "Nomor telepon (mis. +62812…)", &theme, window, cx),
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
                .occlude()
                .inset_0()
                .bg(scrim)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    v_flex()
                        .w(px(420.0))
                        .p_5()
                        .gap_3()
                        .rounded_2xl()
                        .when(shape.adwaita, |dialog| dialog.rounded(px(shape.radius_popover)))
                        .bg(dialog_bg)
                        .border_1()
                        .border_color(dialog_border)
                        .when(!shape.adwaita, |dialog| dialog.shadow_2xl())
                        .when(shape.adwaita, |dialog| dialog.shadow_md())
                        .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                            if ev.keystroke.key.as_str() == "escape" {
                                this.close_compose_dialog(cx);
                            }
                        }))
                        .child(
                            h_flex()
                                .w_full()
                                .justify_between()
                                .items_center()
                                .when(shape.adwaita, |header| header.h(px(shape.header_height)).bg(surface.headerbar))
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
                                        .rounded(px(cx.shape().radius_button))
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
                                    dialog_border,
                                    text_color,
                                    primary_color,
                                    shape,
                                    cx.listener(|this, _, _, cx| this.close_compose_dialog(cx)),
                                ))
                                .child(Self::dialog_button(
                                    if busy { "Mengirim…".to_string() } else { action_label.to_string() },
                                    true,
                                    dialog_border,
                                    text_color,
                                    primary_color,
                                    shape,
                                    cx.listener(move |this, _, _, cx| submit(this, cx)),
                                )),
                        ),
                ),
                "compose-dialog-in",
            )
            .into_any_element())
    }

}

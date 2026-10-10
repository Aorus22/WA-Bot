use crate::views::chat::*;

impl ChatView {
    /// Toggle the attach (+) panel: closes whatever is open, else opens the menu.
    pub fn toggle_attach_panel(&mut self, cx: &mut Context<Self>) {
        self.attach_panel.toggle_menu();
        cx.notify();
    }

    /// Close the attach panel / pickers.
    pub fn close_attach_panel(&mut self, cx: &mut Context<Self>) {
        self.attach_panel = AttachPanel::None;
        cx.notify();
    }

    /// Show the emoji picker panel.
    pub fn open_emoji_panel(&mut self, cx: &mut Context<Self>) {
        self.emoji_picker.open();
        self.attach_panel = AttachPanel::Emoji;
        cx.notify();
    }

    /// Show the sticker picker, lazily loading favourites once.
    pub fn open_sticker_panel(&mut self, cx: &mut Context<Self>) {
        self.attach_panel = AttachPanel::Sticker;
        if self.sticker_picker.loaded || self.sticker_picker.is_loading {
            cx.notify();
            return;
        }
        self.sticker_picker.begin_loading();
        let base_url = Self::compose_base_url(cx);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move { client.get_favorite_stickers().await })
                    .await;
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            match res {
                                Ok(Ok(favorites)) => this.sticker_picker.set_favorites(favorites),
                                _ => {
                                    this.sticker_picker.failed();
                                    toast::error("Gagal memuat stiker", cx);
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
        cx.notify();
    }

    /// Open one of the poll / location / contact dialogs.
    pub fn open_compose_dialog(&mut self, dialog: ComposeDialog, cx: &mut Context<Self>) {
        self.attach_panel = AttachPanel::None;
        self.active_compose_field = 0;
        self.compose_dialog = Some(dialog);
        cx.notify();
    }

    /// Close the active compose dialog.
    pub fn close_compose_dialog(&mut self, cx: &mut Context<Self>) {
        self.compose_dialog = None;
        self.active_compose_field = 0;
        cx.notify();
    }

    /// Route a keystroke to the focused dialog field.
    pub(crate) fn dialog_key_input(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        match ev.keystroke.key.as_str() {
            "escape" => self.close_compose_dialog(cx),
            "backspace" => {
                if let Some(field) = self.active_dialog_field_mut() {
                    field.pop();
                }
                cx.notify();
            }
            "space" => {
                if let Some(field) = self.active_dialog_field_mut() {
                    field.push(' ');
                }
                cx.notify();
            }
            k if k.len() == 1 => {
                if let Some(field) = self.active_dialog_field_mut() {
                    field.push_str(k);
                }
                cx.notify();
            }
            _ => {}
        }
    }

    pub(crate) fn active_dialog_field_mut(&mut self) -> Option<&mut String> {
        let dialog = self.compose_dialog?;
        let index = self.active_compose_field;
        match dialog {
            ComposeDialog::Poll => {
                if index == 0 {
                    Some(&mut self.poll_draft.question)
                } else {
                    self.poll_draft.options.get_mut(index - 1)
                }
            }
            ComposeDialog::Location => match index {
                0 => Some(&mut self.location_draft.latitude),
                1 => Some(&mut self.location_draft.longitude),
                2 => Some(&mut self.location_draft.name),
                _ => Some(&mut self.location_draft.address),
            },
            ComposeDialog::Contact => match index {
                0 => Some(&mut self.contact_draft.name),
                _ => Some(&mut self.contact_draft.phone),
            },
        }
    }

}

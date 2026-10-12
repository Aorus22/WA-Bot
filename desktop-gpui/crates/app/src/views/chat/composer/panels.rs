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
                    .spawn(async move {
                        let favorites = client.get_favorite_stickers().await?;
                        // Recents are a nicety; the picker still works without them.
                        let recent = client.get_recent_stickers().await.unwrap_or_default();
                        Ok::<_, wabot_backend_client::error::ClientError>((recent, favorites))
                    })
                    .await;
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            match res {
                                Ok(Ok((recent, favorites))) => this.sticker_picker.set_stickers(recent, favorites),
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
        self.dialog_fields.clear();
        self.compose_dialog = Some(dialog);
        cx.notify();
    }

    /// Close the active compose dialog.
    pub fn close_compose_dialog(&mut self, cx: &mut Context<Self>) {
        self.compose_dialog = None;
        self.dialog_fields.clear();
        cx.notify();
    }
}

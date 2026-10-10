use crate::views::chat::*;

impl ChatView {
    /// Paste hook for the composer: an image on the clipboard is staged as a
    /// pending attachment instead of being inserted. Returns whether the paste
    /// was consumed; text (including rich sources that also offer an image
    /// rendering) falls through to the normal text paste.
    pub(crate) fn stage_clipboard_image(
        &mut self,
        item: &ClipboardItem,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.selected_chat_id.is_none() || self.editing_message.is_some() {
            return false;
        }
        if item.text().is_some_and(|text| !text.trim().is_empty()) {
            return false;
        }
        let Some(image) = item.entries().iter().find_map(|entry| match entry {
            ClipboardEntry::Image(image) => Some(image.clone()),
            _ => None,
        }) else {
            return false;
        };

        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.pending_attachment = Some(PendingAttachment {
            file_name: format!("pasted-image-{stamp}.{}", image.format.extension()),
            image: std::sync::Arc::new(image),
        });
        cx.notify();
        true
    }

    pub(crate) fn clear_pending_attachment(&mut self, cx: &mut Context<Self>) {
        self.pending_attachment = None;
        cx.notify();
    }

    /// Send the staged image to the open chat; the composer text becomes its caption.
    pub(crate) fn send_pending_attachment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(att), Some(chat_id)) = (self.pending_attachment.take(), self.selected_chat_id.clone())
        else {
            return;
        };
        let caption = self.compose_text.trim().to_string();
        self.set_compose_text(String::new(), window, cx);
        self.send_attachment_bytes(
            chat_id,
            att.file_name,
            "image".to_string(),
            att.image.bytes.clone(),
            caption,
            Some(att.image.format.mime_type().to_string()),
            MediaKind::Media,
            cx,
        );
    }
}

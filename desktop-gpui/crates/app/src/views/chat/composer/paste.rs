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
        let file_name = format!("pasted-image-{stamp}.{}", image.format.extension());
        self.pending_attachments.push(PendingAttachment {
            file_name,
            path: None,
            mime_type: Some(image.format.mime_type().to_string()),
            media_type: "image".to_string(),
            image: Some(std::sync::Arc::new(image)),
            kind: MediaKind::Media,
        });
        cx.notify();
        true
    }

    pub(crate) fn clear_pending_attachment(&mut self, cx: &mut Context<Self>) {
        self.pending_attachments.clear();
        cx.notify();
    }

    pub(crate) fn remove_pending_attachment(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.pending_attachments.len() {
            self.pending_attachments.remove(index);
            cx.notify();
        }
    }

    /// Send staged attachments to the open chat; the preview field becomes caption text.
    pub(crate) fn send_pending_attachments(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(chat_id) = self.selected_chat_id.clone() else {
            return;
        };
        if self.pending_attachments.is_empty() {
            return;
        }
        let attachments = std::mem::take(&mut self.pending_attachments);
        let caption = self.compose_text.trim().to_string();
        self.set_compose_text(String::new(), window, cx);
        for attachment in attachments {
            if let Some(path) = attachment.path {
                self.send_attachment_path(
                    chat_id.clone(),
                    path,
                    attachment.kind,
                    attachment.media_type,
                    attachment.mime_type,
                    caption.clone(),
                    cx,
                );
            } else if let Some(image) = attachment.image {
                self.send_attachment_bytes(
                    chat_id.clone(),
                    attachment.file_name,
                    attachment.media_type,
                    image.bytes.clone(),
                    caption.clone(),
                    attachment.mime_type,
                    attachment.kind,
                    cx,
                );
            }
        }
    }
}

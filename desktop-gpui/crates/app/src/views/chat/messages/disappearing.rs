use wabot_backend_client::dto::DisappearingMeta;

use crate::views::chat::*;

/// Timers WhatsApp offers for a chat: seconds and label (0 = off).
pub(crate) const DISAPPEARING_OPTIONS: [(u32, &str); 4] =
    [(0, "Off"), (86_400, "24 hours"), (604_800, "7 days"), (7_776_000, "90 days")];

/// "24 hours", "7 days", "90 days", "Off" (or a rounded figure for a timer
/// WhatsApp does not offer but the phone may have set).
pub(crate) fn disappearing_label(seconds: u32) -> String {
    if let Some((_, label)) = DISAPPEARING_OPTIONS.iter().find(|(secs, _)| *secs == seconds) {
        return (*label).to_string();
    }
    match seconds {
        s if s % 86_400 == 0 => format!("{} days", s / 86_400),
        s if s % 3_600 == 0 => format!("{} hours", s / 3_600),
        s => format!("{} min", s.div_ceil(60)),
    }
}

impl ChatView {
    /// Whether a disappearing message has already vanished on phones, and the
    /// words for its indicator's tooltip. The message itself always stays.
    pub(crate) fn disappearing_note(meta: &DisappearingMeta, now_ms: i64) -> (bool, String) {
        if meta.expires_at > 0 && meta.expires_at <= now_ms {
            return (true, "Disappeared on phone · kept here".to_string());
        }
        let when = if meta.expires_at > 0 {
            format!(
                "{}, {}",
                Self::format_search_date(meta.expires_at),
                Self::format_search_time(meta.expires_at)
            )
        } else {
            format!("in {}", disappearing_label(meta.seconds.max(0) as u32))
        };
        (false, format!("Disappears {when}"))
    }

    /// Change the open chat's disappearing timer. It shows at once; a failed
    /// request puts the old value back.
    pub fn set_disappearing_timer(&mut self, chat_id: String, seconds: u32, cx: &mut Context<Self>) {
        let previous = cx
            .has_global::<ChatStore>()
            .then(|| ChatStore::global(cx).chats.iter().find(|c| c.id == chat_id).map(|c| c.disappearing_secs))
            .flatten()
            .unwrap_or(0);
        if previous == seconds {
            return;
        }
        if cx.has_global::<ChatStore>() {
            ChatStore::global_mut(cx).set_disappearing(&chat_id, seconds);
        }
        cx.notify();

        let base_url = Self::compose_base_url(cx);
        cx.spawn(move |_this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let cid = chat_id.clone();
                let res = TOKIO_RT.spawn(async move { client.set_disappearing(&cid, seconds).await }).await;
                let _ = cx_handle.update(|cx: &mut App| match res {
                    Ok(Ok(_)) => toast::success(
                        if seconds == 0 {
                            "Pesan sementara dimatikan".to_string()
                        } else {
                            format!("Pesan sementara aktif: {}", disappearing_label(seconds))
                        },
                        cx,
                    ),
                    other => {
                        if cx.has_global::<ChatStore>() {
                            ChatStore::global_mut(cx).set_disappearing(&chat_id, previous);
                        }
                        let reason = match other {
                            Ok(Err(wabot_backend_client::error::ClientError::Api { message, .. })) => message,
                            Ok(Err(error)) => error.to_string(),
                            _ => "permintaan terputus".to_string(),
                        };
                        toast::error(format!("Gagal mengubah pesan sementara: {reason}"), cx);
                    }
                });
            }
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;

    #[test]
    fn labels_cover_the_offered_timers() {
        assert_eq!(disappearing_label(0), "Off");
        assert_eq!(disappearing_label(86_400), "24 hours");
        assert_eq!(disappearing_label(604_800), "7 days");
        assert_eq!(disappearing_label(7_776_000), "90 days");
        assert_eq!(disappearing_label(172_800), "2 days");
        assert_eq!(disappearing_label(7_200), "2 hours");
    }

    #[test]
    fn expired_messages_say_they_are_kept() {
        let meta = DisappearingMeta { seconds: 86_400, expires_at: 1_000 };
        assert_eq!(
            ChatView::disappearing_note(&meta, 2_000),
            (true, "Disappeared on phone · kept here".to_string())
        );
        let (expired, tip) = ChatView::disappearing_note(&meta, 500);
        assert!(!expired && tip.starts_with("Disappears "));
    }
}

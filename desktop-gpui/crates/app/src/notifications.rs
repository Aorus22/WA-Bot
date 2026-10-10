//! OS notifications for incoming messages.
//!
//! `plan` is the pure decision (notify or not, and with what text) so it can be
//! unit-tested; `notify_new_message` wires it to the app state and the desktop
//! notification service.

use gpui::{App, Global};
use wabot_backend_client::dto::{Chat, Message};
use wabot_settings::DesktopSettings;

use crate::components::message_bubble::MessageBubbleHelper;
use crate::state::ChatStore;

/// Messages older than this are history, not something to interrupt for.
const MAX_AGE_MS: i64 = 5 * 60 * 1000;
const MAX_BODY_CHARS: usize = 140;

/// The saved preference, as a global so the WS loop and settings view agree.
pub struct NotificationPref(pub bool);
impl Global for NotificationPref {}

pub fn enabled(cx: &App) -> bool {
    cx.try_global::<NotificationPref>().map_or(true, |p| p.0)
}

/// Change the preference: update the global and persist it.
pub fn set_enabled(cx: &mut App, enabled: bool) {
    cx.set_global(NotificationPref(enabled));
    if let Ok(mut settings) = DesktopSettings::load() {
        settings.notifications_enabled = enabled;
        let _ = settings.save();
    }
    cx.refresh_windows();
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub title: String,
    pub body: String,
}

fn is_muted(chat: &Chat, now_ms: i64) -> bool {
    match chat.mute_mode.as_str() {
        "" | "off" => false,
        "until" => chat.muted_until.map_or(true, |end| end > now_ms),
        _ => true,
    }
}

fn media_label(message_type: &str) -> &'static str {
    match message_type {
        "image" => "📷 Photo",
        "video" => "🎥 Video",
        "audio" | "ptt" | "voice" => "🎤 Voice message",
        "document" => "📄 Document",
        "sticker" => "Sticker",
        "location" => "📍 Location",
        "contact" => "👤 Contact",
        "poll" => "📊 Poll",
        _ => "New message",
    }
}

fn truncate(text: &str) -> String {
    let mut out: String = text.chars().take(MAX_BODY_CHARS).collect();
    if text.chars().count() > MAX_BODY_CHARS {
        out.push('…');
    }
    out
}

/// Decide whether `msg` deserves a notification and build its text.
/// `chat` is the stored row for `msg.chat_id`, if we know it yet.
pub fn plan(msg: &Message, chat: Option<&Chat>, now_ms: i64) -> Option<Notice> {
    if MessageBubbleHelper::is_from_me(msg, "me") || msg.is_automatic == Some(true) {
        return None;
    }
    if msg.timestamp > 0 && now_ms - msg.timestamp > MAX_AGE_MS {
        return None;
    }
    if chat.is_some_and(|c| is_muted(c, now_ms)) {
        return None;
    }

    let is_group = chat.map_or(msg.chat_id.ends_with("@g.us"), |c| c.is_group)
        || msg.chat_id.ends_with("@g.us");
    let sender = msg.sender_name.as_deref().filter(|s| !s.is_empty());
    let title = chat
        .map(|c| c.name.as_str())
        .filter(|n| !n.is_empty())
        .or(sender)
        .unwrap_or(&msg.chat_id)
        .to_string();

    let decoded = MessageBubbleHelper::decode_content(&msg.content);
    let first_line = decoded.lines().map(str::trim).find(|l| !l.is_empty());
    let text = match first_line {
        // Placeholders such as "[Image]" carry no information beyond the type.
        Some(line) if !(line.starts_with('[') && line.ends_with(']')) => truncate(line),
        _ => media_label(&msg.message_type).to_string(),
    };
    let body = match (is_group, sender) {
        (true, Some(name)) => format!("{name}: {text}"),
        _ => text,
    };
    Some(Notice { title, body })
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// Raise a notification for an incoming message, unless the user is already
/// looking at the app, has turned notifications off, or the chat is muted.
/// Call this with the event *before* it is applied to `ChatStore`.
pub fn notify_new_message(msg: &Message, cx: &App) {
    if !enabled(cx) || cx.active_window().is_some() {
        return;
    }
    let store = cx.try_global::<ChatStore>();
    // The backend can resend a message we already have; that is not new.
    let known = store
        .and_then(|s| s.messages_by_chat.get(&msg.chat_id))
        .is_some_and(|e| e.messages.iter().any(|m| m.id == msg.id));
    if known {
        return;
    }
    let chat = store.and_then(|s| s.chats.iter().find(|c| c.id == msg.chat_id));
    if let Some(notice) = plan(msg, chat, now_ms()) {
        show(notice);
    }
}

/// Fire-and-forget: talking to the notification daemon can block.
fn show(notice: Notice) {
    std::thread::spawn(move || {
        let result = notify_rust::Notification::new()
            .appname("WA Bot")
            .summary(&notice.title)
            .body(&notice.body)
            .show();
        if let Err(e) = result {
            eprintln!("[notifications] could not show notification: {e}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_700_000_000_000;

    fn msg(content: &str) -> Message {
        Message {
            id: "m1".into(),
            chat_id: "628111@s.whatsapp.net".into(),
            from: "628111@s.whatsapp.net".into(),
            to: String::new(),
            content: content.into(),
            timestamp: NOW - 1000,
            status: "received".into(),
            message_type: "text".into(),
            media_url: None,
            is_automatic: None,
            sender_name: Some("Budi".into()),
            reply_to_id: None,
            forwarded: None,
            reactions: None,
            extra: None,
        }
    }

    fn chat(name: &str) -> Chat {
        Chat {
            id: "628111@s.whatsapp.net".into(),
            name: name.into(),
            avatar: String::new(),
            last_msg: String::new(),
            last_time: 0,
            unread: 0,
            is_active: true,
            is_group: false,
            archived: false,
            pinned_at: None,
            mute_mode: "off".into(),
            muted_until: None,
        }
    }

    #[test]
    fn direct_message_uses_chat_name_and_text() {
        let n = plan(&msg("halo"), Some(&chat("Budi S")), NOW).unwrap();
        assert_eq!(n, Notice { title: "Budi S".into(), body: "halo".into() });
    }

    #[test]
    fn own_and_automatic_messages_are_silent() {
        let mut own = msg("x");
        own.from = "me".into();
        assert!(plan(&own, None, NOW).is_none());
        let mut auto = msg("x");
        auto.is_automatic = Some(true);
        assert!(plan(&auto, None, NOW).is_none());
    }

    #[test]
    fn old_messages_are_history() {
        let mut m = msg("x");
        m.timestamp = NOW - MAX_AGE_MS - 1;
        assert!(plan(&m, None, NOW).is_none());
    }

    #[test]
    fn muted_chats_are_silent_until_the_mute_ends() {
        let mut c = chat("Budi");
        c.mute_mode = "forever".into();
        assert!(plan(&msg("x"), Some(&c), NOW).is_none());
        c.mute_mode = "until".into();
        c.muted_until = Some(NOW + 1000);
        assert!(plan(&msg("x"), Some(&c), NOW).is_none());
        c.muted_until = Some(NOW - 1000);
        assert!(plan(&msg("x"), Some(&c), NOW).is_some());
    }

    #[test]
    fn group_messages_name_the_sender() {
        let mut m = msg("rapat jam 3");
        m.chat_id = "123@g.us".into();
        let mut c = chat("Tim");
        c.id = m.chat_id.clone();
        c.is_group = true;
        let n = plan(&m, Some(&c), NOW).unwrap();
        assert_eq!(n.title, "Tim");
        assert_eq!(n.body, "Budi: rapat jam 3");
    }

    #[test]
    fn media_without_caption_gets_a_label() {
        let mut m = msg("[Image]");
        m.message_type = "image".into();
        assert_eq!(plan(&m, None, NOW).unwrap().body, "📷 Photo");
        m.content = String::new();
        m.message_type = "ptt".into();
        assert_eq!(plan(&m, None, NOW).unwrap().body, "🎤 Voice message");
    }

    #[test]
    fn unknown_chat_falls_back_to_the_sender() {
        assert_eq!(plan(&msg("halo"), None, NOW).unwrap().title, "Budi");
    }

    #[test]
    fn long_text_is_truncated_to_its_first_line() {
        let n = plan(&msg(&format!("{}\nsecond", "a".repeat(300))), None, NOW).unwrap();
        assert_eq!(n.body.chars().count(), MAX_BODY_CHARS + 1);
        assert!(n.body.ends_with('…'));
    }
}

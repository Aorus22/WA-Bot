//! OS notifications for incoming messages.
//!
//! `plan` is the pure decision (notify or not, and with what text) so it can be
//! unit-tested; `notify_new_message` wires it to the app state and the desktop
//! notification service.

use gpui::{App, Global};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::Duration;
use wabot_backend_client::client::HttpClient;
use wabot_backend_client::dto::{Chat, Message};
use wabot_settings::DesktopSettings;

use crate::components::message_bubble::MessageBubbleHelper;
use crate::router::Router;
use crate::state::{AuthState, ChatStore};
use crate::TOKIO_RT;

#[cfg(target_os = "linux")]
mod linux;

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
    pub avatar: Option<String>,
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
    if MessageBubbleHelper::is_from_me(msg, "me")
        || msg.is_automatic == Some(true)
        || msg.notification_suppressed
    {
        return None;
    }
    if msg.timestamp > 0 && now_ms - msg.timestamp > MAX_AGE_MS {
        return None;
    }
    if chat.is_some_and(|c| c.archived || is_muted(c, now_ms)) {
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

    // Mentions arrive as "@<number>"; show the names the backend resolved.
    let decoded = MessageBubbleHelper::decode_content(&msg.content);
    let decoded = crate::views::chat::render_message_mentions_with_spans(&decoded, msg, None).0;
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
    let avatar = chat
        .map(|c| c.avatar.trim())
        .filter(|url| !url.is_empty())
        .map(str::to_owned);
    Some(Notice {
        title,
        body,
        avatar,
    })
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// A closed popover can leave a stale platform focus handle on Wayland.
pub fn chat_is_visible(cx: &App) -> bool {
    if crate::popover::is_popover(cx) && !crate::popover::is_open(cx) {
        return false;
    }
    cx.active_window()
        .is_some_and(|focused| cx.windows().contains(&focused))
        && cx
            .try_global::<Router>()
            .is_some_and(|router| router.current_route.is_active("/chat"))
}

fn is_reading_chat(chat_id: &str, chat_visible: bool, selected_chat: Option<&str>) -> bool {
    chat_visible && selected_chat == Some(chat_id)
}

/// Raise a notification unless this conversation is being read, notifications
/// are disabled, or the chat is muted/archived.
/// Call this with the event *before* it is applied to `ChatStore`.
pub fn notify_new_message(msg: &Message, cx: &App) {
    if !enabled(cx) {
        eprintln!("[notifications] skipped: notifications disabled");
        return;
    }
    let store = cx.try_global::<ChatStore>();
    if is_reading_chat(
        &msg.chat_id,
        chat_is_visible(cx),
        store.and_then(|s| s.active_chat_id.as_deref()),
    ) {
        eprintln!("[notifications] skipped: this conversation is being read");
        return;
    }
    // The backend can resend a message we already have; that is not new.
    let known = store
        .and_then(|s| s.messages_by_chat.get(&msg.chat_id))
        .is_some_and(|e| e.messages.iter().any(|m| m.id == msg.id));
    if known {
        eprintln!("[notifications] skipped: message already received");
        return;
    }
    let chat = store.and_then(|s| s.chats.iter().find(|c| c.id == msg.chat_id));
    if let Some(notice) = plan(msg, chat, now_ms()) {
        let Some(auth) = cx.try_global::<AuthState>() else {
            return;
        };
        // Something sent from our own number (another device, an echo) is not
        // an incoming message, whatever the backend labelled it.
        if auth.phone.as_deref().is_some_and(|phone| msg.from.split('@').next() == Some(phone)) {
            eprintln!("[notifications] skipped: sent from the linked account");
            return;
        }
        eprintln!("[notifications] dispatching incoming message");
        show(
            notice,
            HttpClient::new(&auth.base_url),
            msg.chat_id.clone(),
            msg.id.clone(),
        );
    } else {
        eprintln!("[notifications] skipped by message/chat policy: backend_silent={}, archived={}, muted={}, automatic={}, own={}, history={}",
            msg.notification_suppressed,
            chat.is_some_and(|c| c.archived),
            chat.is_some_and(|c| is_muted(c, now_ms())),
            msg.is_automatic == Some(true),
            MessageBubbleHelper::is_from_me(msg, "me"),
            msg.timestamp > 0 && now_ms() - msg.timestamp > MAX_AGE_MS);
    }
}

const AVATAR_TIMEOUT: Duration = Duration::from_secs(3);
const AVATAR_CACHE_AGE: Duration = Duration::from_secs(60 * 60);
const MAX_AVATAR_BYTES: usize = 2 * 1024 * 1024;

async fn cached_avatar(client: &HttpClient, url: &str) -> Option<PathBuf> {
    cached_avatar_in(
        client,
        url,
        &crate::backend::data_dir().join("cache/notification-avatars"),
    )
    .await
}

async fn cached_avatar_in(
    client: &HttpClient,
    url: &str,
    dir: &std::path::Path,
) -> Option<PathBuf> {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut hash);
    let path = dir.join(format!("{:016x}.img", hash.finish()));
    if std::fs::metadata(&path).ok().is_some_and(|m| {
        m.len() > 0
            && m.modified()
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age < AVATAR_CACHE_AGE)
    }) {
        return Some(path);
    }

    let bytes = tokio::time::timeout(AVATAR_TIMEOUT, async {
        let mut response = client
            .client()
            .get(url)
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?;
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.ok()? {
            if bytes.len() + chunk.len() > MAX_AVATAR_BYTES {
                return None;
            }
            bytes.extend_from_slice(&chunk);
        }
        // Reject redirects to HTML/error pages and unsupported image formats.
        let is_image = bytes.starts_with(&[0xff, 0xd8, 0xff])
            || bytes.starts_with(b"\x89PNG\r\n\x1a\n")
            || (bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"));
        is_image.then_some(bytes)
    })
    .await
    .ok()??;
    std::fs::create_dir_all(dir).ok()?;
    let mut file = tempfile::NamedTempFile::new_in(dir).ok()?;
    std::io::Write::write_all(&mut file, &bytes).ok()?;
    file.persist(&path).ok()?;
    Some(path)
}

fn claim_notice_in(dir: &std::path::Path, key: &str) -> std::io::Result<Option<PathBuf>> {
    std::fs::create_dir_all(dir)?;
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hash);
    let path = dir.join(format!("{:016x}", hash.finish()));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(_) => Ok(Some(path)),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(None),
        Err(error) => Err(error),
    }
}

/// Fire-and-forget: talking to the notification daemon can block.
fn show(notice: Notice, client: HttpClient, chat_id: String, message_id: String) {
    std::thread::spawn(move || {
        // The resident popover and a normal window can be separate processes.
        let key = format!("{}\0{chat_id}\0{message_id}", client.base_url());
        let receipt = match claim_notice_in(
            &crate::backend::data_dir().join("cache/notification-receipts"),
            &key,
        ) {
            Ok(Some(path)) => Some(path),
            Ok(None) => {
                eprintln!("[notifications] skipped: another instance already handled this message");
                return;
            }
            Err(error) => {
                eprintln!("[notifications] could not coordinate instances: {error}");
                None
            }
        };
        let proxy = format!("{}/avatar/{}", client.base_url(), chat_id);
        let avatar = TOKIO_RT.block_on(async {
            if let Some(url) = client.media_url(notice.avatar.as_deref()) {
                if let Some(path) = cached_avatar(&client, &url).await {
                    return Some(path);
                }
            }
            cached_avatar(&client, &proxy).await
        });
        #[cfg(target_os = "linux")]
        let result = linux::show(&notice, avatar.as_deref(), &chat_id).map(|_| ());
        #[cfg(not(target_os = "linux"))]
        let result = {
            let mut notification = notify_rust::Notification::new();
            notification.appname("WA Bot").summary(&notice.title).body(&notice.body).icon("wa-bot");
            if let Some(path) = avatar.as_ref().and_then(|p| p.to_str()) {
                notification.icon(path).image_path(path);
            }
            notification.show().map(|_| ())
        };
        if let Err(e) = result {
            if let Some(path) = receipt {
                let _ = std::fs::remove_file(path);
            }
            eprintln!("[notifications] could not show notification: {e}");
        } else {
            eprintln!("[notifications] notification accepted by desktop service");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_700_000_000_000;

    #[test]
    fn an_open_app_only_silences_the_conversation_being_read() {
        assert!(!is_reading_chat("incoming", true, None));
        assert!(!is_reading_chat("incoming", true, Some("another-chat")));
        assert!(is_reading_chat("incoming", true, Some("incoming")));
        assert!(!is_reading_chat("incoming", false, Some("incoming")));
    }

    #[test]
    fn separate_app_instances_only_claim_one_notification_per_message() {
        let dir = tempfile::tempdir().unwrap();
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let path = dir.path().to_path_buf();
                std::thread::spawn(move || {
                    claim_notice_in(&path, "backend\0chat\0message")
                        .unwrap()
                        .is_some()
                })
            })
            .collect();
        assert_eq!(
            threads
                .into_iter()
                .map(|thread| usize::from(thread.join().unwrap()))
                .sum::<usize>(),
            1
        );
        assert!(claim_notice_in(dir.path(), "backend\0chat\0next-message")
            .unwrap()
            .is_some());
        assert!(
            claim_notice_in(dir.path(), "another-account\0chat\0message")
                .unwrap()
                .is_some()
        );
    }

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
            notification_suppressed: false,
            sender_name: Some("Budi".into()),
            reply_to_id: None,
            forwarded: None,
            reactions: None,
            extra: None,
            ..Default::default()
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
            has_unread_mention: false,
            is_active: true,
            is_group: false,
            archived: false,
            pinned_at: None,
            mute_mode: "off".into(),
            muted_until: None,
            disappearing_secs: 0,
        }
    }

    #[test]
    fn mentions_show_names_not_numbers() {
        let mut message = msg("@11781648990305 info deployment");
        message.extra = serde_json::from_value(serde_json::json!({
            "mentions": ["11781648990305@lid"],
            "mentionNames": {"11781648990305@lid": "Bimalord Bot"}
        }))
        .unwrap();
        let n = plan(&message, Some(&chat("Budi S")), NOW).unwrap();
        assert_eq!(n.body, "@Bimalord Bot info deployment");
    }

    #[test]
    fn direct_message_uses_chat_name_and_text() {
        let n = plan(&msg("halo"), Some(&chat("Budi S")), NOW).unwrap();
        assert_eq!(
            n,
            Notice {
                title: "Budi S".into(),
                body: "halo".into(),
                avatar: None
            }
        );
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
        c.muted_until = None;
        assert!(plan(&msg("x"), Some(&c), NOW).is_none());
        c.muted_until = Some(NOW);
        assert!(plan(&msg("x"), Some(&c), NOW).is_some());
    }

    #[test]
    fn archived_chats_are_silent_even_after_a_timed_mute_expires() {
        let mut c = chat("Budi");
        c.archived = true;
        assert!(plan(&msg("x"), Some(&c), NOW).is_none());
        c.mute_mode = "until".into();
        c.muted_until = Some(NOW - 1);
        assert!(plan(&msg("x"), Some(&c), NOW).is_none());
        c.archived = false;
        assert!(plan(&msg("x"), Some(&c), NOW).is_some());
    }

    #[test]
    fn backend_suppression_works_without_chat_rows_and_after_auto_unarchive() {
        let mut m = msg("x");
        m.notification_suppressed = true;
        assert!(plan(&m, None, NOW).is_none());
        assert!(plan(&m, Some(&chat("Budi")), NOW).is_none());
    }

    #[test]
    fn notifications_use_the_chat_avatar_for_personal_and_group_chats() {
        let mut c = chat("Budi");
        c.avatar = "https://example.test/personal.jpg".into();
        assert_eq!(
            plan(&msg("x"), Some(&c), NOW).unwrap().avatar,
            Some(c.avatar.clone())
        );
        c.is_group = true;
        c.avatar = "https://example.test/group.jpg".into();
        assert_eq!(
            plan(&msg("x"), Some(&c), NOW).unwrap().avatar,
            Some(c.avatar.clone())
        );
        c.avatar = "  ".into();
        assert_eq!(plan(&msg("x"), Some(&c), NOW).unwrap().avatar, None);
    }

    fn avatar_server(status: &str, bytes: Vec<u8>) -> (String, std::thread::JoinHandle<()>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/avatar", listener.local_addr().unwrap());
        let status = status.to_owned();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let _ = socket.read(&mut [0; 4096]);
            write!(
                socket,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                bytes.len()
            )
            .unwrap();
            socket.write_all(&bytes).unwrap();
        });
        (url, server)
    }

    #[tokio::test]
    async fn avatar_is_downloaded_and_reused_without_a_server() {
        let bytes = include_bytes!("../../../../dist/icons/wa-bot-128.png").to_vec();
        let (url, server) = avatar_server("200 OK", bytes.clone());
        let client = HttpClient::new(&url);
        let dir = tempfile::tempdir().unwrap();
        let path = cached_avatar_in(&client, &url, dir.path()).await.unwrap();
        server.join().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(
            cached_avatar_in(&client, &url, dir.path()).await,
            Some(path)
        );
    }

    #[tokio::test]
    async fn missing_or_invalid_avatar_keeps_text_only_notifications() {
        let dir = tempfile::tempdir().unwrap();
        for (status, bytes) in [
            ("404 Not Found", b"missing".to_vec()),
            ("200 OK", b"<html>error</html>".to_vec()),
        ] {
            let (url, server) = avatar_server(status, bytes);
            assert!(cached_avatar_in(&HttpClient::new(&url), &url, dir.path())
                .await
                .is_none());
            server.join().unwrap();
        }
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
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

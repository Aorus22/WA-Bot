use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReactionEntry {
    pub emoji: String,
    #[serde(default)]
    pub senders: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PollOption {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PollMeta {
    pub question: String,
    #[serde(default)]
    pub options: Vec<PollOption>,
    #[serde(default)]
    pub multi_select: bool,
    #[serde(default)]
    pub votes: Option<HashMap<String, Vec<String>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LocationMeta {
    pub latitude: f64,
    pub longitude: f64,
    pub name: Option<String>,
    pub address: Option<String>,
    pub live: Option<bool>,
    pub thumbnail_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SubContact {
    pub display_name: String,
    pub vcard: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContactMeta {
    pub display_name: Option<String>,
    #[serde(default)]
    pub contacts: Vec<SubContact>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ViewOnceMeta {
    pub media_type: String,
    #[serde(default)]
    pub viewed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LinkPreviewMeta {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub thumbnail_url: Option<String>,
}

/// An earlier version of an edited message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MessageEdit {
    pub content: String,
    /// When this version was written, in milliseconds.
    #[serde(default)]
    pub timestamp: i64,
}

/// Set on a message sent inside a chat with a disappearing timer. The phones
/// drop it at `expires_at`; we keep it and only show an indicator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DisappearingMeta {
    /// The chat's timer when the message was sent, in seconds.
    #[serde(default)]
    pub seconds: i64,
    /// When WhatsApp removes the message on phones, in unix milliseconds.
    #[serde(default)]
    pub expires_at: i64,
}

/// The status (story) a message replies to.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StatusReplyMeta {
    #[serde(default)]
    pub status_id: String,
    /// JID that posted the status.
    #[serde(default)]
    pub sender: String,
    /// "text", "image", "video" or "audio".
    #[serde(rename = "type", default)]
    pub status_type: String,
    pub content: Option<String>,
    pub media_url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MessageExtra {
    /// Versions before the current content, oldest first.
    #[serde(default)]
    pub edits: Vec<MessageEdit>,
    /// When the message was last edited, in milliseconds.
    #[serde(default)]
    pub edited_at: Option<i64>,
    pub poll: Option<PollMeta>,
    pub location: Option<LocationMeta>,
    pub contact: Option<ContactMeta>,
    pub view_once: Option<ViewOnceMeta>,
    pub gif: Option<bool>,
    pub link_preview: Option<LinkPreviewMeta>,
    #[serde(default)]
    pub mentions: Vec<String>,
    #[serde(default)]
    pub mention_names: HashMap<String, String>,
    /// Mentioned JID → the chat JID to open for it.
    #[serde(default)]
    pub mention_chats: HashMap<String, String>,
    #[serde(default)]
    pub mentions_me: bool,
    pub disappearing: Option<DisappearingMeta>,
    pub status_reply: Option<StatusReplyMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MessageReceipt {
    pub message_id: String,
    pub chat_id: String,
    pub jid: String,
    /// Unix ms; 0 when the member has not been reported as having the message.
    #[serde(default)]
    pub delivered_at: i64,
    /// Unix ms; 0 when the member has not read it.
    #[serde(default)]
    pub read_at: i64,
}

/// Who has received and read an outgoing group message.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MessageReceipts {
    #[serde(default)]
    pub receipts: Vec<MessageReceipt>,
    /// Members besides us the message went to (the contact, in a private chat).
    #[serde(default)]
    pub total: usize,
    #[serde(default)]
    pub recipients: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub chat_id: String,
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub to: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub timestamp: i64,
    #[serde(default)]
    pub status: String,
    #[serde(rename = "type", default)]
    pub message_type: String,
    pub media_url: Option<String>,
    pub is_automatic: Option<bool>,
    #[serde(default)]
    pub notification_suppressed: bool,
    pub sender_name: Option<String>,
    pub reply_to_id: Option<String>,
    pub forwarded: Option<bool>,
    pub reactions: Option<Vec<ReactionEntry>>,
    pub extra: Option<MessageExtra>,
    #[serde(default)]
    pub starred: bool,
    /// When the sender deleted the message for everyone, in unix milliseconds.
    /// The message is kept and still readable here.
    #[serde(default)]
    pub revoked_at: Option<i64>,
    /// When the message was starred, in unix milliseconds; only on `/starred` rows.
    #[serde(default)]
    pub starred_at: Option<i64>,
    /// Chat title; only filled on `/starred` rows, which span chats.
    #[serde(default)]
    pub chat_name: Option<String>,
}

/// One active in-chat pin (`GET /chats/{id}/pins`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PinnedMessage {
    pub message_id: String,
    /// JID of who pinned it, or "me".
    #[serde(default)]
    pub pinned_by: String,
    #[serde(default)]
    pub pinned_at: i64,
    /// When the pin lapses, in unix milliseconds.
    #[serde(default)]
    pub expires_at: i64,
    /// Absent when the pinned message is not stored.
    pub message: Option<Message>,
}

/// What `GET /link-preview` scraped; any field may be empty.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LinkPreview {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub thumbnail_url: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starred_message_parses_with_paging_fields() {
        let raw = r#"{"id":"m1","chatId":"a@s.whatsapp.net","starred":true,"starredAt":1790000000000,"chatName":"Budi"}"#;
        let msg: Message = serde_json::from_str(raw).unwrap();
        assert!(msg.starred);
        assert_eq!(msg.starred_at, Some(1_790_000_000_000));
        assert_eq!(msg.chat_name.as_deref(), Some("Budi"));

        // Older payloads carry none of it.
        let msg: Message = serde_json::from_str(r#"{"id":"m2","chatId":"a@s.whatsapp.net"}"#).unwrap();
        assert!(!msg.starred && msg.starred_at.is_none() && msg.chat_name.is_none());
    }

    #[test]
    fn disappearing_and_status_reply_extras_parse() {
        let raw = r#"{"id":"m1","chatId":"a@s.whatsapp.net","extra":{
            "disappearing":{"seconds":604800,"expiresAt":1790604800000},
            "statusReply":{"statusId":"s1","sender":"628@s.whatsapp.net","type":"image","mediaUrl":"/media/s1.jpg"}}}"#;
        let extra = serde_json::from_str::<Message>(raw).unwrap().extra.unwrap();
        let d = extra.disappearing.expect("disappearing");
        assert_eq!((d.seconds, d.expires_at), (604_800, 1_790_604_800_000));
        let r = extra.status_reply.expect("status reply");
        assert_eq!((r.status_id.as_str(), r.status_type.as_str()), ("s1", "image"));
        assert_eq!(r.media_url.as_deref(), Some("/media/s1.jpg"));
        assert!(r.content.is_none());
    }

    #[test]
    fn pins_tolerate_a_missing_message() {
        let raw = r#"[{"messageId":"m1","pinnedBy":"me","pinnedAt":1,"expiresAt":2,"message":{"id":"m1","chatId":"c"}},
                      {"messageId":"m2","pinnedBy":"628@s.whatsapp.net","pinnedAt":3,"expiresAt":4}]"#;
        let pins: Vec<PinnedMessage> = serde_json::from_str(raw).unwrap();
        assert_eq!(pins.len(), 2);
        assert!(pins[0].message.is_some() && pins[1].message.is_none());
    }

    #[test]
    fn link_preview_fields_may_be_empty() {
        let p: LinkPreview = serde_json::from_str(r#"{"url":"https://a.b","title":"T"}"#).unwrap();
        assert_eq!((p.title.as_str(), p.description.as_str(), p.thumbnail_url.as_str()), ("T", "", ""));
    }
}

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MessageExtra {
    pub poll: Option<PollMeta>,
    pub location: Option<LocationMeta>,
    pub contact: Option<ContactMeta>,
    pub view_once: Option<ViewOnceMeta>,
    pub gif: Option<bool>,
    pub link_preview: Option<LinkPreviewMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    pub sender_name: Option<String>,
    pub reply_to_id: Option<String>,
    pub forwarded: Option<bool>,
    pub reactions: Option<Vec<ReactionEntry>>,
    pub extra: Option<MessageExtra>,
}

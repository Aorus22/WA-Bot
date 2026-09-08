use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub jid: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub subscribers: u64,
    pub invite_code: Option<String>,
    pub avatar: Option<String>,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChannelPreview {
    pub jid: String,
    pub name: String,
    #[serde(default)]
    pub subscribers: u64,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChannelMessage {
    pub id: String,
    pub server_id: i64,
    #[serde(rename = "type")]
    pub message_type: String,
    pub timestamp: i64,
    #[serde(default)]
    pub views_count: u64,
    pub reactions: Option<HashMap<String, u64>>,
    pub content: Option<String>,
    pub media_type: Option<String>,
}

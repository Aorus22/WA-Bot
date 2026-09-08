use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Chat {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub avatar: String,
    #[serde(default)]
    pub last_msg: String,
    #[serde(default)]
    pub last_time: i64,
    #[serde(default)]
    pub unread: u32,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub is_group: bool,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub pinned_at: Option<i64>,
    #[serde(default = "default_mute_mode")]
    pub mute_mode: String,
    #[serde(default)]
    pub muted_until: Option<i64>,
}

fn default_mute_mode() -> String {
    "off".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatState {
    pub chat_id: String,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub pinned_at: Option<i64>,
    #[serde(default = "default_mute_mode")]
    pub mute_mode: String,
    #[serde(default)]
    pub muted_until: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistorySyncError {
    pub chat_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistorySyncStatus {
    #[serde(default = "default_sync_state")]
    pub state: String,
    #[serde(default)]
    pub pending_chats: u64,
    #[serde(default)]
    pub pending_messages: u64,
    #[serde(default)]
    pub chats_total: u64,
    #[serde(default)]
    pub chats_processed: u64,
    #[serde(default)]
    pub messages_added: u64,
    #[serde(default)]
    pub errors: Vec<HistorySyncError>,
    #[serde(default)]
    pub started_at: Option<i64>,
    #[serde(default)]
    pub finished_at: Option<i64>,
    #[serde(default)]
    pub last_run_at: Option<i64>,
}

fn default_sync_state() -> String {
    "idle".to_string()
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StatusEntry {
    pub id: String,
    pub sender: String,
    pub sender_name: Option<String>,
    pub content: Option<String>,
    pub media_url: Option<String>,
    #[serde(rename = "type")]
    pub status_type: String,
    pub timestamp: i64,
    pub expires_at: i64,
    #[serde(default)]
    pub viewed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StatusGroup {
    pub sender: String,
    pub name: Option<String>,
    pub avatar: Option<String>,
    #[serde(default)]
    pub all_viewed: bool,
    #[serde(default)]
    pub statuses: Vec<StatusEntry>,
    #[serde(default)]
    pub latest_time: i64,
}

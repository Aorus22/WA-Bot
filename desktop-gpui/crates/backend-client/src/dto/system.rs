use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StatusResponse {
    pub is_logged_in: bool,
    /// Phone number of the linked account (empty/absent when not logged in).
    #[serde(default)]
    pub phone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QrCodeResponse {
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StatusResult {
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IdResult {
    pub status: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OptIdResult {
    pub status: String,
    pub id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GroupCreateResult {
    pub status: String,
    pub group: super::group::GroupCache,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GroupInviteLinkResult {
    pub status: String,
    pub link: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JoinGroupResult {
    pub status: String,
    pub jid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StarResult {
    pub status: String,
    #[serde(default)]
    pub starred: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PinResult {
    pub status: String,
    #[serde(default)]
    pub pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisappearingResult {
    pub status: String,
    #[serde(default)]
    pub seconds: u32,
}

/// `POST /statuses/{id}/reply`: the reply lands in the 1:1 chat with the poster.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StatusReplyResult {
    pub status: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub chat_id: String,
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StatusResponse {
    pub is_logged_in: bool,
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

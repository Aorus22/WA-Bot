use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GroupParticipantInfo {
    pub jid: String,
    pub name: Option<String>,
    #[serde(default)]
    pub is_admin: bool,
    pub is_super_admin: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GroupCache {
    pub jid: String,
    pub name: String,
    pub description: Option<String>,
    pub owner: Option<String>,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub announce: bool,
    #[serde(default)]
    pub join_approval: bool,
    pub member_add_mode: Option<String>,
    #[serde(default)]
    pub own_role: String,
    #[serde(default)]
    pub participant_count: usize,
    #[serde(default)]
    pub participants: Vec<GroupParticipantInfo>,
    #[serde(default)]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GroupPreview {
    pub jid: String,
    pub name: String,
    #[serde(default)]
    pub participant_count: usize,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateGroupChanges {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub announce: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub join_approval: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub member_add_mode: Option<String>,
}

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsResponse {
    #[serde(default)]
    pub has_gemini_key: Option<bool>,
    #[serde(default)]
    pub has_fish_key: Option<bool>,
    #[serde(default)]
    pub settings: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssistantResponse {
    pub answer: String,
}

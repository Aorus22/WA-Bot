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

impl SettingsResponse {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.settings.get(key).map(|s| s.as_str())
    }

    pub fn ai_server_url(&self) -> &str {
        self.get("ai_server_url").unwrap_or("http://localhost:8981")
    }

    pub fn call_tts_provider(&self) -> &str {
        self.get("call_tts_provider").unwrap_or("")
    }

    pub fn call_tts_default_voice(&self) -> &str {
        self.get("call_tts_default_voice").unwrap_or("")
    }

    pub fn call_tts_fish_audio_model(&self) -> &str {
        self.get("call_tts_fish_audio_model").unwrap_or("")
    }

    pub fn call_tts_fish_audio_voice_id(&self) -> &str {
        self.get("call_tts_fish_audio_voice_id").unwrap_or("")
    }

    pub fn read_receipts(&self) -> bool {
        self.get("read_receipts").map(|v| v != "false").unwrap_or(true)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssistantResponse {
    pub answer: String,
}

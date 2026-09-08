use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CallStatus {
    Preparing,
    Initiating,
    Ringing,
    Connecting,
    Connected,
    Ending,
    Ended,
    Rejected,
    Missed,
    Busy,
    Failed,
    Interrupted,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CallType {
    Audio,
    Video,
    GroupAudio,
    GroupVideo,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CallDirection {
    Incoming,
    Outgoing,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CallSource {
    Ui,
    ExternalApi,
    Incoming,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MediaMode {
    Live,
    Tts,
    AudioFile,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CallState {
    pub id: String,
    pub status: CallStatus,
    #[serde(rename = "type")]
    pub call_type: CallType,
    pub direction: CallDirection,
    pub source: CallSource,
    pub media_mode: MediaMode,
    pub target: String,
    pub group_jid: Option<String>,
    pub participants: Option<Vec<String>>,
    pub started_at: i64,
    pub answered_at: Option<i64>,
    #[serde(default)]
    pub video_enabled: bool,
    #[serde(default)]
    pub remote_video_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CallLog {
    pub id: String,
    pub meow_call_id: String,
    pub direction: CallDirection,
    pub call_type: CallType,
    pub target: String,
    pub group_jid: Option<String>,
    pub participants: Option<Vec<String>>,
    pub source: CallSource,
    pub media_mode: MediaMode,
    pub status: CallStatus,
    pub error_message: Option<String>,
    pub api_key_id: Option<String>,
    pub started_at: i64,
    pub answered_at: Option<i64>,
    pub ended_at: Option<i64>,
    pub duration_ms: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CallHistoryResponse {
    pub logs: Vec<CallLog>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CallHistoryFilter {
    pub limit: Option<usize>,
    pub before: Option<i64>,
    pub direction: Option<String>,
    #[serde(rename = "type")]
    pub call_type: Option<String>,
    pub status: Option<String>,
    pub target: Option<String>,
}

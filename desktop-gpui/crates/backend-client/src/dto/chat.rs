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
    pub has_unread_mention: bool,
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

/// How far back a history sync goes (the body of `POST /api/history-sync`).
/// All fields zero/false is "50 older messages per chat".
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistorySyncRequest {
    /// Cap on older messages added per chat; 0 = no cap.
    pub per_chat: u32,
    /// Only go back this many days; 0 = no limit.
    pub since_days: u32,
    /// Everything the phone will give, retrying chats found to be at the start.
    pub all: bool,
    /// Also ask again for chats an earlier sync found to be at their start (or
    /// that never answered).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub retry_finished: bool,
    /// Continue the stopped or interrupted sync instead of starting one; the
    /// other fields are then ignored.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub resume: bool,
}

/// What `POST /api/history-sync` did.
#[derive(Debug, Clone, PartialEq)]
pub enum HistorySyncStart {
    /// A new sync began.
    Started(HistorySyncStatus),
    /// One was already running; this is its status.
    AlreadyRunning(HistorySyncStatus),
}

/// A chat being synced right now.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistorySyncCurrent {
    pub chat_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub added: u64,
    /// "importing" or "asking-phone".
    #[serde(default)]
    pub phase: String,
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
    #[serde(default, deserialize_with = "list_or_null")]
    pub errors: Vec<HistorySyncError>,
    #[serde(default)]
    pub started_at: Option<i64>,
    #[serde(default)]
    pub finished_at: Option<i64>,
    #[serde(default)]
    pub last_run_at: Option<i64>,
    /// Chats that gained at least one message in this run.
    #[serde(default)]
    pub chats_with_new: u64,
    /// Chats whose history the phone had no more of.
    #[serde(default)]
    pub chats_exhausted: u64,
    /// Chats skipped because an earlier run already reached their start.
    #[serde(default)]
    pub chats_skipped: u64,
    /// Requests the phone never answered.
    #[serde(default)]
    pub timeouts: u64,
    #[serde(default, deserialize_with = "list_or_null")]
    pub current: Vec<HistorySyncCurrent>,
    /// Chats the next sync skips unless asked to retry them.
    #[serde(default)]
    pub chats_finished: u64,
    /// Stopped or interrupted with chats left: resuming continues where it left off.
    #[serde(default)]
    pub resumable: bool,
    /// How long the sync actually ran, not counting the time it was stopped.
    #[serde(default)]
    pub elapsed_ms: u64,
    /// Rough time left; `None` until there is enough to go on.
    #[serde(default)]
    pub eta_ms: Option<u64>,
}

impl Default for HistorySyncStatus {
    fn default() -> Self {
        Self {
            state: default_sync_state(),
            pending_chats: 0,
            pending_messages: 0,
            chats_total: 0,
            chats_processed: 0,
            messages_added: 0,
            errors: Vec::new(),
            started_at: None,
            finished_at: None,
            last_run_at: None,
            chats_with_new: 0,
            chats_exhausted: 0,
            chats_skipped: 0,
            timeouts: 0,
            current: Vec::new(),
            chats_finished: 0,
            resumable: false,
            elapsed_ms: 0,
            eta_ms: None,
        }
    }
}

/// A list that the server may send as JSON `null` when it is empty.
fn list_or_null<'de, D, T>(deserializer: D) -> std::result::Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(deserializer)?.unwrap_or_default())
}

fn default_sync_state() -> String {
    "idle".to_string()
}

#[cfg(test)]
mod history_status_tests {
    use super::*;

    #[test]
    fn a_status_with_null_lists_still_parses() {
        // What the backend sent while a sync ran with no errors yet.
        let raw = r#"{"state":"running","pendingChats":23,"pendingMessages":7502,"chatsTotal":999,
            "chatsProcessed":93,"messagesAdded":1252,"errors":null,"startedAt":1791903137642,
            "finishedAt":null,"lastRunAt":null,"options":{"perChat":50,"sinceMs":0},
            "chatsWithNew":26,"chatsExhausted":1,"chatsSkipped":0,"timeouts":0,
            "current":[{"chatId":"1@lid","name":"Budi","added":0,"phase":"asking-phone"}]}"#;
        let st: HistorySyncStatus = serde_json::from_str(raw).expect("must parse");
        assert_eq!(st.state, "running");
        assert!(st.errors.is_empty());
        assert_eq!(st.current.len(), 1);

        let raw = r#"{"state":"idle","errors":null,"current":null}"#;
        let st: HistorySyncStatus = serde_json::from_str(raw).expect("null lists");
        assert!(st.errors.is_empty() && st.current.is_empty());

        // And a bare, old-style payload without any of the new fields.
        let st: HistorySyncStatus = serde_json::from_str(r#"{"state":"completed"}"#).unwrap();
        assert_eq!(st.state, "completed");
        assert!(!st.resumable && st.elapsed_ms == 0 && st.eta_ms.is_none());

        let raw = r#"{"state":"interrupted","resumable":true,"elapsedMs":125000,"etaMs":null}"#;
        let st: HistorySyncStatus = serde_json::from_str(raw).unwrap();
        assert!(st.resumable && st.elapsed_ms == 125_000 && st.eta_ms.is_none());
    }

    #[test]
    fn resume_is_only_sent_when_asked_for() {
        let plain = serde_json::to_string(&HistorySyncRequest { per_chat: 50, ..Default::default() }).unwrap();
        assert!(!plain.contains("resume"), "{plain}");
        let resume = serde_json::to_string(&HistorySyncRequest { resume: true, ..Default::default() }).unwrap();
        assert!(resume.contains(r#""resume":true"#), "{resume}");
        assert!(!plain.contains("retryFinished"), "{plain}");
        let retry = serde_json::to_string(&HistorySyncRequest { all: true, retry_finished: true, ..Default::default() }).unwrap();
        assert!(retry.contains(r#""retryFinished":true"#), "{retry}");
    }
}

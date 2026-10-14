use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::chat::{list_or_null, HistorySyncError};

/// The body of `POST /api/history-media`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryMediaRequest {
    /// Kinds to download: "image", "sticker", "audio", "video", "document".
    pub types: Vec<String>,
    /// Also try media an earlier download found to be gone.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub retry_unavailable: bool,
    /// Ask the phone to upload media again when WhatsApp's servers no longer
    /// have it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ask_phone: bool,
    /// Continue the stopped or interrupted download instead of starting one;
    /// the other fields are then ignored.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub resume: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryMediaOptions {
    #[serde(default, deserialize_with = "list_or_null")]
    pub types: Vec<String>,
    #[serde(default)]
    pub retry_unavailable: bool,
    #[serde(default)]
    pub ask_phone: bool,
}

/// How big the account's two SQLite databases are (`GET /api/storage/usage`).
/// A size is `None` when it is not known, e.g. media kept in S3.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StorageUsage {
    /// The message database: chats, messages and staged history.
    #[serde(default)]
    pub chat_bytes: Option<u64>,
    /// The database holding the media files.
    #[serde(default)]
    pub media_bytes: Option<u64>,
}

/// What `POST /api/history-media` did.
#[derive(Debug, Clone, PartialEq)]
pub enum HistoryMediaStart {
    /// A download began (or resumed).
    Started(HistoryMediaStatus),
    /// One was already running; this is its status.
    AlreadyRunning(HistoryMediaStatus),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryMediaStatus {
    #[serde(default = "default_state")]
    pub state: String,
    /// Everything the download has to do: done + unavailable + failed + remaining.
    #[serde(default)]
    pub total: u64,
    #[serde(default)]
    pub done: u64,
    #[serde(default)]
    pub remaining: u64,
    /// Gone from WhatsApp's servers.
    #[serde(default)]
    pub unavailable: u64,
    /// Could not be downloaded; a new download tries them again.
    #[serde(default)]
    pub failed: u64,
    /// Of `done`, how many the phone uploaded again after the servers had lost them.
    #[serde(default)]
    pub recovered: u64,
    /// Gone from the servers, and the phone does not have them either.
    #[serde(default)]
    pub missing: u64,
    #[serde(default)]
    pub bytes: u64,
    #[serde(default, deserialize_with = "list_or_null")]
    pub errors: Vec<HistorySyncError>,
    #[serde(default)]
    pub started_at: Option<i64>,
    #[serde(default)]
    pub finished_at: Option<i64>,
    #[serde(default)]
    pub options: HistoryMediaOptions,
    /// Per kind, the media a new download could still fetch.
    #[serde(default)]
    pub available: HashMap<String, u64>,
    /// Per kind, media the servers no longer have, that only the phone can give back.
    #[serde(default)]
    pub expired: HashMap<String, u64>,
    /// Files being downloaded right now.
    #[serde(default)]
    pub current: u64,
    #[serde(default)]
    pub resumable: bool,
    /// How long the download actually ran, not counting the time it was stopped.
    #[serde(default)]
    pub elapsed_ms: u64,
    /// Rough time left; `None` until there is enough to go on.
    #[serde(default)]
    pub eta_ms: Option<u64>,
}

impl Default for HistoryMediaStatus {
    fn default() -> Self {
        Self {
            state: default_state(),
            total: 0,
            done: 0,
            remaining: 0,
            unavailable: 0,
            failed: 0,
            recovered: 0,
            missing: 0,
            bytes: 0,
            errors: Vec::new(),
            started_at: None,
            finished_at: None,
            options: HistoryMediaOptions::default(),
            available: HashMap::new(),
            expired: HashMap::new(),
            current: 0,
            resumable: false,
            elapsed_ms: 0,
            eta_ms: None,
        }
    }
}

fn default_state() -> String {
    "idle".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_status_from_the_backend_parses() {
        let raw = r#"{"state":"running","total":9800,"done":120,"remaining":9600,"unavailable":70,
            "failed":10,"bytes":5242880,"errors":null,"startedAt":1791905146982,"finishedAt":null,
            "options":{"types":["image","sticker"],"retryUnavailable":false,"askPhone":true},
            "available":{"image":4826,"sticker":3830,"audio":50,"video":228,"document":886},
            "expired":{"image":9000},"recovered":4,"missing":2,
            "current":3,"resumable":false,"elapsedMs":61000,"etaMs":null}"#;
        let st: HistoryMediaStatus = serde_json::from_str(raw).expect("must parse");
        assert_eq!(st.done, 120);
        assert_eq!(st.options.types, ["image", "sticker"]);
        assert_eq!(st.available["image"], 4826);
        assert_eq!(st.expired["image"], 9000);
        assert!(st.options.ask_phone && st.recovered == 4 && st.missing == 2);
        assert!(st.errors.is_empty() && st.eta_ms.is_none());

        let bare: HistoryMediaStatus = serde_json::from_str(r#"{"state":"idle","errors":null}"#).unwrap();
        assert_eq!(bare.state, "idle");
        assert!(bare.available.is_empty());
    }

    #[test]
    fn storage_usage_may_miss_a_size() {
        let u: StorageUsage = serde_json::from_str(r#"{"chatBytes":65536,"mediaBytes":null}"#).unwrap();
        assert_eq!(u.chat_bytes, Some(65536));
        assert_eq!(u.media_bytes, None);
        assert_eq!(serde_json::from_str::<StorageUsage>("{}").unwrap(), StorageUsage::default());
    }

    #[test]
    fn the_request_only_sends_what_is_set() {
        let plain = serde_json::to_string(&HistoryMediaRequest { types: vec!["image".into()], ..Default::default() }).unwrap();
        assert_eq!(plain, r#"{"types":["image"]}"#);
        let resume = serde_json::to_string(&HistoryMediaRequest { resume: true, ..Default::default() }).unwrap();
        assert!(resume.contains(r#""resume":true"#), "{resume}");
        let retry = serde_json::to_string(&HistoryMediaRequest { retry_unavailable: true, ..Default::default() }).unwrap();
        assert!(retry.contains(r#""retryUnavailable":true"#), "{retry}");
        let phone = serde_json::to_string(&HistoryMediaRequest { ask_phone: true, ..Default::default() }).unwrap();
        assert!(phone.contains(r#""askPhone":true"#), "{phone}");
        assert!(!plain.contains("askPhone"), "{plain}");
    }
}

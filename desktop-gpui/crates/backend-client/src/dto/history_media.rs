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
            bytes: 0,
            errors: Vec::new(),
            started_at: None,
            finished_at: None,
            options: HistoryMediaOptions::default(),
            available: HashMap::new(),
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
            "options":{"types":["image","sticker"],"retryUnavailable":false},
            "available":{"image":4826,"sticker":3830,"audio":50,"video":228,"document":886},
            "current":3,"resumable":false,"elapsedMs":61000,"etaMs":null}"#;
        let st: HistoryMediaStatus = serde_json::from_str(raw).expect("must parse");
        assert_eq!(st.done, 120);
        assert_eq!(st.options.types, ["image", "sticker"]);
        assert_eq!(st.available["image"], 4826);
        assert!(st.errors.is_empty() && st.eta_ms.is_none());

        let bare: HistoryMediaStatus = serde_json::from_str(r#"{"state":"idle","errors":null}"#).unwrap();
        assert_eq!(bare.state, "idle");
        assert!(bare.available.is_empty());
    }

    #[test]
    fn the_request_only_sends_what_is_set() {
        let plain = serde_json::to_string(&HistoryMediaRequest { types: vec!["image".into()], ..Default::default() }).unwrap();
        assert_eq!(plain, r#"{"types":["image"]}"#);
        let resume = serde_json::to_string(&HistoryMediaRequest { resume: true, ..Default::default() }).unwrap();
        assert!(resume.contains(r#""resume":true"#), "{resume}");
        let retry = serde_json::to_string(&HistoryMediaRequest { retry_unavailable: true, ..Default::default() }).unwrap();
        assert!(retry.contains(r#""retryUnavailable":true"#), "{retry}");
    }
}

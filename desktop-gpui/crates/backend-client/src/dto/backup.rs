use serde::{Deserialize, Serialize};

/// The body of `POST /api/backup/run`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BackupRequest {
    /// Absolute path of the backup folder.
    pub target: String,
    /// Backups to keep per account afterwards; 0 keeps everything.
    #[serde(default)]
    pub keep: u32,
}

/// The body of `POST /api/backup/restore`: merge one backup into the running app.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RestoreRequest {
    /// Absolute path of the backup folder.
    pub target: String,
    pub account: String,
    pub id: String,
}

/// Progress and outcome of the latest backup (`GET /api/backup/status`) or
/// restore (`GET /api/backup/restore/status`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BackupStatus {
    #[serde(default)]
    pub running: bool,
    #[serde(default)]
    pub account: String,
    /// "snapshot", "chat", "media" or "manifest" while running.
    #[serde(default)]
    pub account_index: i32,
    #[serde(default)]
    pub account_count: i32,
    /// Progress of the whole run, 0 to 100.
    #[serde(default)]
    pub percent: i32,
    #[serde(default)]
    pub stage: String,
    #[serde(default)]
    pub item: String,
    #[serde(default)]
    pub done: i64,
    #[serde(default)]
    pub total: i64,
    #[serde(default)]
    pub started_at: i64,
    #[serde(default)]
    pub finished_at: i64,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub error: String,
}

/// One backup found in the folder (`GET /api/backup/list`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BackupEntry {
    pub account: String,
    pub id: String,
    /// Unix milliseconds.
    pub created_at: i64,
}

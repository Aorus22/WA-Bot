use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Trigger {
    pub id: String,
    pub name: String,
    pub pattern: String,
    pub script: String,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub is_active: bool,
    pub description: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CronJob {
    pub id: String,
    pub name: String,
    pub schedule: String,
    pub script: String,
    #[serde(default)]
    pub is_active: bool,
    pub description: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Webhook {
    pub id: String,
    pub name: String,
    pub path: String,
    pub script: String,
    pub secret: Option<String>,
    #[serde(default)]
    pub is_active: bool,
    pub description: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WebhookLog {
    pub id: String,
    pub webhook_id: String,
    pub webhook_path: String,
    pub source_ip: String,
    pub method: String,
    pub headers: String,
    pub body: String,
    pub query_params: String,
    pub status_code: i32,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WebhookLogResponse {
    pub logs: Vec<WebhookLog>,
    #[serde(default)]
    pub total: usize,
}

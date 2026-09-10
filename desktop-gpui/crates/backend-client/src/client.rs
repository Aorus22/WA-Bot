use std::collections::HashMap;
use reqwest::{Client, StatusCode};
use serde_json::json;

use crate::dto::*;
use crate::error::{ClientError, Result};

#[derive(Debug, Clone)]
pub struct HttpClient {
    base_url: String,
    client: Client,
}

impl HttpClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        let mut base_url = base_url.into();
        while base_url.ends_with('/') {
            base_url.pop();
        }
        if !base_url.ends_with("/api") {
            base_url.push_str("/api");
        }
        Self {
            base_url,
            client: Client::builder().build().unwrap_or_default(),
        }
    }

    pub fn from_port(port: u16) -> Self {
        Self::new(format!("http://127.0.0.1:{}/api", port))
    }

    pub fn set_base_url(&mut self, url: impl Into<String>) {
        let mut url = url.into();
        while url.ends_with('/') {
            url.pop();
        }
        if !url.ends_with("/api") {
            url.push_str("/api");
        }
        self.base_url = url;
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn media_url(&self, value: Option<&str>) -> Option<String> {
        let value = value?;
        if value.is_empty() {
            return None;
        }
        if value.starts_with("http://") || value.starts_with("https://") {
            return Some(value.to_string());
        }
        if value == "/api" || value.starts_with("/api/") {
            let root = self.base_url.trim_end_matches("/api");
            return Some(format!("{}{}", root, value));
        }
        let slash = if value.starts_with('/') { "" } else { "/" };
        Some(format!("{}{}{}", self.base_url, slash, value))
    }

    pub fn status_media_url(&self, id: &str) -> String {
        format!("{}/statuses/{}/media", self.base_url, urlencoding(id))
    }

    async fn handle_response<T: serde::de::DeserializeOwned>(
        resp: reqwest::Response,
    ) -> Result<T> {
        let status = resp.status();
        if !status.is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            let msg = if let Ok(v) = serde_json::from_str::<serde_json::Value>(&error_text) {
                v.get("error")
                    .and_then(|e| e.as_str())
                    .unwrap_or(&error_text)
                    .to_string()
            } else {
                error_text
            };
            return Err(ClientError::Api {
                status,
                message: if msg.is_empty() {
                    status.to_string()
                } else {
                    msg
                },
            });
        }
        let parsed = resp.json::<T>().await?;
        Ok(parsed)
    }

    // --- System & Auth ---

    pub async fn get_status(&self) -> Result<StatusResponse> {
        let resp = self.client.get(format!("{}/status", self.base_url)).send().await?;
        Self::handle_response(resp).await
    }

    pub async fn get_qr_code(&self) -> Result<QrCodeResponse> {
        let resp = self.client.get(format!("{}/qr-code", self.base_url)).send().await?;
        Self::handle_response(resp).await
    }

    pub async fn logout(&self) -> Result<StatusResult> {
        let resp = self.client.post(format!("{}/logout", self.base_url)).send().await?;
        Self::handle_response(resp).await
    }

    pub async fn health_check(&self) -> Result<()> {
        let resp = self.client.get(format!("{}/health", self.base_url)).send().await?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(ClientError::Api {
                status: resp.status(),
                message: "Health check failed".to_string(),
            })
        }
    }

    // --- Chats ---

    pub async fn get_chats(&self) -> Result<Vec<Chat>> {
        let resp = self.client.get(format!("{}/chats", self.base_url)).send().await?;
        let chats: Vec<Chat> = Self::handle_response(resp).await?;
        Ok(chats.into_iter().filter(|c| !c.id.is_empty()).collect())
    }

    pub async fn mark_as_read(&self, chat_id: &str) -> Result<StatusResult> {
        let resp = self
            .client
            .post(format!("{}/chats/{}/read", self.base_url, urlencoding(chat_id)))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn pin_chat(&self, chat_id: &str, pinned: bool) -> Result<ChatState> {
        let resp = self
            .client
            .post(format!("{}/chats/{}/pin", self.base_url, urlencoding(chat_id)))
            .json(&json!({ "pinned": pinned }))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn archive_chat(&self, chat_id: &str, archived: bool) -> Result<ChatState> {
        let resp = self
            .client
            .post(format!("{}/chats/{}/archive", self.base_url, urlencoding(chat_id)))
            .json(&json!({ "archived": archived }))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn mute_chat(&self, chat_id: &str, mode: &str) -> Result<ChatState> {
        let resp = self
            .client
            .post(format!("{}/chats/{}/mute", self.base_url, urlencoding(chat_id)))
            .json(&json!({ "mode": mode }))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn subscribe_chat_presence(&self, chat_id: &str) -> Result<StatusResult> {
        let resp = self
            .client
            .post(format!(
                "{}/chats/{}/presence-subscribe",
                self.base_url,
                urlencoding(chat_id)
            ))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    // --- Messages ---

    pub async fn get_messages(
        &self,
        chat_id: &str,
        limit: Option<usize>,
        before: Option<i64>,
        after: Option<i64>,
    ) -> Result<Vec<Message>> {
        let mut url = format!(
            "{}/chats/{}/messages?limit={}",
            self.base_url,
            urlencoding(chat_id),
            limit.unwrap_or(100)
        );
        if let Some(b) = before {
            url.push_str(&format!("&before={}", b));
        }
        if let Some(a) = after {
            url.push_str(&format!("&after={}", a));
        }
        let resp = self.client.get(url).send().await?;
        Self::handle_response(resp).await
    }

    pub async fn search_messages(
        &self,
        chat_id: &str,
        query: &str,
        limit: Option<usize>,
    ) -> Result<Vec<Message>> {
        let url = format!(
            "{}/chats/{}/search?q={}&limit={}",
            self.base_url,
            urlencoding(chat_id),
            urlencoding(query),
            limit.unwrap_or(50)
        );
        let resp = self.client.get(url).send().await?;
        Self::handle_response(resp).await
    }

    pub async fn get_message_context(
        &self,
        chat_id: &str,
        message_id: &str,
        limit: Option<usize>,
    ) -> Result<Vec<Message>> {
        let url = format!(
            "{}/chats/{}/messages/{}/context?limit={}",
            self.base_url,
            urlencoding(chat_id),
            urlencoding(message_id),
            limit.unwrap_or(50)
        );
        let resp = self.client.get(url).send().await?;
        Self::handle_response(resp).await
    }

    pub async fn get_chat_media(
        &self,
        chat_id: &str,
        limit: Option<usize>,
        before: Option<i64>,
    ) -> Result<Vec<Message>> {
        let mut url = format!(
            "{}/chats/{}/media?limit={}",
            self.base_url,
            urlencoding(chat_id),
            limit.unwrap_or(30)
        );
        if let Some(b) = before {
            url.push_str(&format!("&before={}", b));
        }
        let resp = self.client.get(url).send().await?;
        Self::handle_response(resp).await
    }

    pub async fn get_chat_docs(
        &self,
        chat_id: &str,
        limit: Option<usize>,
        before: Option<i64>,
    ) -> Result<Vec<Message>> {
        let mut url = format!(
            "{}/chats/{}/docs?limit={}",
            self.base_url,
            urlencoding(chat_id),
            limit.unwrap_or(30)
        );
        if let Some(b) = before {
            url.push_str(&format!("&before={}", b));
        }
        let resp = self.client.get(url).send().await?;
        Self::handle_response(resp).await
    }

    pub async fn get_chat_links(
        &self,
        chat_id: &str,
        limit: Option<usize>,
        before: Option<i64>,
    ) -> Result<Vec<Message>> {
        let mut url = format!(
            "{}/chats/{}/links?limit={}",
            self.base_url,
            urlencoding(chat_id),
            limit.unwrap_or(30)
        );
        if let Some(b) = before {
            url.push_str(&format!("&before={}", b));
        }
        let resp = self.client.get(url).send().await?;
        Self::handle_response(resp).await
    }

    pub async fn send_message(&self, target: &str, message: &str) -> Result<IdResult> {
        let resp = self
            .client
            .post(format!("{}/send-message", self.base_url))
            .json(&json!({
                "target": target,
                "message": message,
            }))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn delete_message(&self, chat_id: &str, id: &str) -> Result<StatusResult> {
        let resp = self
            .client
            .post(format!(
                "{}/chats/{}/messages/{}/delete",
                self.base_url,
                urlencoding(chat_id),
                urlencoding(id)
            ))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn edit_message(
        &self,
        chat_id: &str,
        id: &str,
        content: &str,
    ) -> Result<StatusResult> {
        let resp = self
            .client
            .post(format!(
                "{}/chats/{}/messages/{}/edit",
                self.base_url,
                urlencoding(chat_id),
                urlencoding(id)
            ))
            .json(&json!({ "content": content }))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn reply_message(
        &self,
        chat_id: &str,
        id: &str,
        content: &str,
    ) -> Result<StatusResult> {
        let resp = self
            .client
            .post(format!(
                "{}/chats/{}/messages/{}/reply",
                self.base_url,
                urlencoding(chat_id),
                urlencoding(id)
            ))
            .json(&json!({ "content": content }))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn react_to_message(
        &self,
        chat_id: &str,
        id: &str,
        emoji: &str,
        from: Option<&str>,
    ) -> Result<StatusResult> {
        let mut body = json!({ "emoji": emoji });
        if let Some(f) = from {
            body["from"] = json!(f);
        }
        let resp = self
            .client
            .post(format!(
                "{}/chats/{}/messages/{}/react",
                self.base_url,
                urlencoding(chat_id),
                urlencoding(id)
            ))
            .json(&body)
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn forward_message(
        &self,
        chat_id: &str,
        id: &str,
        targets: &[String],
    ) -> Result<StatusResult> {
        let resp = self
            .client
            .post(format!(
                "{}/chats/{}/messages/{}/forward",
                self.base_url,
                urlencoding(chat_id),
                urlencoding(id)
            ))
            .json(&json!({ "targets": targets }))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn send_poll(
        &self,
        chat_id: &str,
        question: &str,
        options: &[String],
        multi_select: bool,
    ) -> Result<OptIdResult> {
        let resp = self
            .client
            .post(format!("{}/chats/{}/poll", self.base_url, urlencoding(chat_id)))
            .json(&json!({
                "question": question,
                "options": options,
                "multiSelect": multi_select,
            }))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn send_poll_vote(
        &self,
        chat_id: &str,
        message_id: &str,
        options: &[String],
    ) -> Result<StatusResult> {
        let resp = self
            .client
            .post(format!(
                "{}/chats/{}/messages/{}/vote",
                self.base_url,
                urlencoding(chat_id),
                urlencoding(message_id)
            ))
            .json(&json!({ "options": options }))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn send_typing(&self, chat_id: &str, typing: bool) -> Result<StatusResult> {
        let resp = self
            .client
            .post(format!("{}/chats/{}/typing", self.base_url, urlencoding(chat_id)))
            .json(&json!({ "typing": typing }))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn send_location(
        &self,
        chat_id: &str,
        latitude: f64,
        longitude: f64,
        name: Option<&str>,
        address: Option<&str>,
        live: Option<bool>,
    ) -> Result<OptIdResult> {
        let mut body = json!({
            "latitude": latitude,
            "longitude": longitude,
        });
        if let Some(n) = name {
            body["name"] = json!(n);
        }
        if let Some(a) = address {
            body["address"] = json!(a);
        }
        if let Some(l) = live {
            body["live"] = json!(l);
        }
        let resp = self
            .client
            .post(format!("{}/chats/{}/location", self.base_url, urlencoding(chat_id)))
            .json(&body)
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn send_contact(
        &self,
        chat_id: &str,
        display_name: &str,
        phone: &str,
        vcard: Option<&str>,
    ) -> Result<OptIdResult> {
        let resp = self
            .client
            .post(format!("{}/chats/{}/contact", self.base_url, urlencoding(chat_id)))
            .json(&json!({
                "displayName": display_name,
                "phone": phone,
                "vcard": vcard.unwrap_or(""),
            }))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    // --- History Sync ---

    pub async fn get_history_sync_status(&self) -> Result<HistorySyncStatus> {
        let resp = self
            .client
            .get(format!("{}/history-sync/status", self.base_url))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn start_history_sync(&self) -> Result<HistorySyncStatus> {
        let resp = self
            .client
            .post(format!("{}/history-sync", self.base_url))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    // --- Contacts & Stickers ---

    pub async fn get_contacts(&self) -> Result<Vec<Contact>> {
        let resp = self.client.get(format!("{}/contacts", self.base_url)).send().await?;
        Self::handle_response(resp).await
    }

    pub async fn get_favorite_stickers(&self) -> Result<Vec<StickerFavorite>> {
        let resp = self
            .client
            .get(format!("{}/stickers/favorites", self.base_url))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn favorite_sticker(
        &self,
        message_id: &str,
        media_url: &str,
        is_animated: bool,
    ) -> Result<StatusResult> {
        let resp = self
            .client
            .post(format!("{}/stickers/favorite", self.base_url))
            .json(&json!({
                "messageId": message_id,
                "mediaUrl": media_url,
                "isAnimated": is_animated,
            }))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn delete_favorite_sticker(&self, id: &str) -> Result<StatusResult> {
        let resp = self
            .client
            .delete(format!("{}/stickers/favorites/{}", self.base_url, urlencoding(id)))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn send_sticker(
        &self,
        target: &str,
        media_url: &str,
        is_animated: bool,
    ) -> Result<IdResult> {
        let resp = self
            .client
            .post(format!("{}/send-sticker", self.base_url))
            .json(&json!({
                "target": target,
                "mediaUrl": media_url,
                "isAnimated": is_animated,
            }))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    // --- Groups ---

    pub async fn get_group(&self, group_id: &str) -> Result<GroupCache> {
        let resp = self
            .client
            .get(format!("{}/groups/{}", self.base_url, urlencoding(group_id)))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn update_group(
        &self,
        group_id: &str,
        changes: &UpdateGroupChanges,
    ) -> Result<GroupCache> {
        let resp = self
            .client
            .patch(format!("{}/groups/{}", self.base_url, urlencoding(group_id)))
            .json(changes)
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn update_group_participants(
        &self,
        group_id: &str,
        action: &str,
        jids: &[String],
    ) -> Result<GroupCache> {
        let resp = self
            .client
            .post(format!(
                "{}/groups/{}/participants",
                self.base_url,
                urlencoding(group_id)
            ))
            .json(&json!({
                "action": action,
                "jids": jids,
            }))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn get_group_invite_link(
        &self,
        group_id: &str,
        reset: bool,
    ) -> Result<GroupInviteLinkResult> {
        let url = format!(
            "{}/groups/{}/invite-link{}",
            self.base_url,
            urlencoding(group_id),
            if reset { "?reset=true" } else { "" }
        );
        let resp = if reset {
            self.client.post(url).send().await?
        } else {
            self.client.get(url).send().await?
        };
        Self::handle_response(resp).await
    }

    pub async fn preview_group_link(&self, invite_url: &str) -> Result<GroupPreview> {
        let url = format!("{}/groups/preview?url={}", self.base_url, urlencoding(invite_url));
        let resp = self.client.get(url).send().await?;
        Self::handle_response(resp).await
    }

    pub async fn join_group_with_link(&self, invite_url: &str) -> Result<JoinGroupResult> {
        let resp = self
            .client
            .post(format!("{}/groups/join", self.base_url))
            .json(&json!({ "url": invite_url }))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn create_group(
        &self,
        name: &str,
        participants: &[String],
    ) -> Result<GroupCreateResult> {
        let resp = self
            .client
            .post(format!("{}/groups/create", self.base_url))
            .json(&json!({
                "name": name,
                "participants": participants,
            }))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn leave_group(&self, group_id: &str) -> Result<StatusResult> {
        let resp = self
            .client
            .post(format!("{}/groups/{}/leave", self.base_url, urlencoding(group_id)))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    // --- Status ---

    pub async fn list_statuses(&self) -> Result<Vec<StatusGroup>> {
        let resp = self.client.get(format!("{}/statuses", self.base_url)).send().await?;
        Self::handle_response(resp).await
    }

    pub async fn post_status_text(
        &self,
        text: &str,
        background: Option<u32>,
    ) -> Result<IdResult> {
        let resp = self
            .client
            .post(format!("{}/statuses/text", self.base_url))
            .json(&json!({
                "text": text,
                "background": background.unwrap_or(0xff075e54),
            }))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn mark_status_viewed(&self, id: &str) -> Result<StatusResult> {
        let resp = self
            .client
            .post(format!("{}/statuses/{}/viewed", self.base_url, urlencoding(id)))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    // --- Channels ---

    pub async fn list_channels(&self) -> Result<Vec<Channel>> {
        let resp = self.client.get(format!("{}/channels", self.base_url)).send().await?;
        Self::handle_response(resp).await
    }

    pub async fn preview_channel(&self, url: &str) -> Result<ChannelPreview> {
        let target_url = format!("{}/channels/preview?url={}", self.base_url, urlencoding(url));
        let resp = self.client.get(target_url).send().await?;
        Self::handle_response(resp).await
    }

    pub async fn follow_channel(&self, url: &str) -> Result<StatusResult> {
        let resp = self
            .client
            .post(format!("{}/channels", self.base_url))
            .json(&json!({ "url": url }))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn unfollow_channel(&self, jid: &str) -> Result<StatusResult> {
        let resp = self
            .client
            .delete(format!("{}/channels/{}", self.base_url, urlencoding(jid)))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn set_channel_mute(&self, jid: &str, muted: bool) -> Result<StatusResult> {
        let resp = self
            .client
            .post(format!("{}/channels/{}/mute", self.base_url, urlencoding(jid)))
            .json(&json!({ "muted": muted }))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn get_channel_messages(
        &self,
        jid: &str,
        count: Option<usize>,
        before: Option<i64>,
    ) -> Result<Vec<ChannelMessage>> {
        let mut url = format!(
            "{}/channels/{}/messages?count={}",
            self.base_url,
            urlencoding(jid),
            count.unwrap_or(30)
        );
        if let Some(b) = before {
            url.push_str(&format!("&before={}", b));
        }
        let resp = self.client.get(url).send().await?;
        Self::handle_response(resp).await
    }

    pub async fn react_channel_message(
        &self,
        jid: &str,
        server_id: i64,
        message_id: &str,
        emoji: &str,
    ) -> Result<StatusResult> {
        let resp = self
            .client
            .post(format!(
                "{}/channels/{}/messages/{}/react",
                self.base_url,
                urlencoding(jid),
                server_id
            ))
            .json(&json!({
                "messageId": message_id,
                "emoji": emoji,
            }))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    // --- Calls ---

    pub async fn get_active_call(&self) -> Result<Option<CallState>> {
        let resp = self.client.get(format!("{}/calls/active", self.base_url)).send().await?;
        if resp.status() == StatusCode::NO_CONTENT || resp.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let text = resp.text().await?;
        if text.trim().is_empty() || text.trim() == "null" {
            return Ok(None);
        }
        let call = serde_json::from_str::<CallState>(&text)?;
        Ok(Some(call))
    }

    pub async fn create_call(&self, target: &str, call_type: CallType) -> Result<CallState> {
        let resp = self
            .client
            .post(format!("{}/calls", self.base_url))
            .json(&json!({
                "target": target,
                "type": call_type,
            }))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn create_group_call(
        &self,
        group_jid: &str,
        participants: &[String],
        call_type: CallType,
    ) -> Result<CallState> {
        let resp = self
            .client
            .post(format!("{}/calls/group", self.base_url))
            .json(&json!({
                "group_jid": group_jid,
                "participants": participants,
                "type": call_type,
            }))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn add_call_participants(
        &self,
        id: &str,
        targets: &[String],
    ) -> Result<StatusResult> {
        let resp = self
            .client
            .post(format!("{}/calls/{}/participants", self.base_url, urlencoding(id)))
            .json(&json!({ "targets": targets }))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn ring_call_participant(&self, id: &str, target: &str) -> Result<StatusResult> {
        let resp = self
            .client
            .post(format!("{}/calls/{}/ring", self.base_url, urlencoding(id)))
            .json(&json!({ "target": target }))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn answer_call(&self, id: &str) -> Result<StatusResult> {
        let resp = self
            .client
            .post(format!("{}/calls/{}/answer", self.base_url, urlencoding(id)))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn reject_call(&self, id: &str) -> Result<StatusResult> {
        let resp = self
            .client
            .post(format!("{}/calls/{}/reject", self.base_url, urlencoding(id)))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn hangup_call(&self, id: &str) -> Result<StatusResult> {
        let resp = self
            .client
            .post(format!("{}/calls/{}/hangup", self.base_url, urlencoding(id)))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn get_call_history(
        &self,
        filter: Option<&CallHistoryFilter>,
    ) -> Result<CallHistoryResponse> {
        let mut url = format!("{}/calls/history", self.base_url);
        if let Some(f) = filter {
            let mut params = Vec::new();
            if let Some(l) = f.limit {
                params.push(format!("limit={}", l));
            }
            if let Some(b) = f.before {
                params.push(format!("before={}", b));
            }
            if let Some(ref d) = f.direction {
                params.push(format!("direction={}", urlencoding(d)));
            }
            if let Some(ref t) = f.call_type {
                params.push(format!("type={}", urlencoding(t)));
            }
            if let Some(ref s) = f.status {
                params.push(format!("status={}", urlencoding(s)));
            }
            if let Some(ref tg) = f.target {
                params.push(format!("target={}", urlencoding(tg)));
            }
            if !params.is_empty() {
                url.push('?');
                url.push_str(&params.join("&"));
            }
        }
        let resp = self.client.get(url).send().await?;
        Self::handle_response(resp).await
    }

    pub async fn start_video(&self, id: &str) -> Result<StatusResult> {
        let resp = self
            .client
            .post(format!("{}/calls/{}/video/start", self.base_url, urlencoding(id)))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn accept_video(&self, id: &str) -> Result<StatusResult> {
        let resp = self
            .client
            .post(format!("{}/calls/{}/video/accept", self.base_url, urlencoding(id)))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn reject_video(&self, id: &str) -> Result<StatusResult> {
        let resp = self
            .client
            .post(format!("{}/calls/{}/video/reject", self.base_url, urlencoding(id)))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn stop_video(&self, id: &str) -> Result<StatusResult> {
        let resp = self
            .client
            .post(format!("{}/calls/{}/video/stop", self.base_url, urlencoding(id)))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    // --- Bots (Triggers, Cron, Webhooks) & Docs ---

    pub async fn get_triggers(&self) -> Result<Vec<Trigger>> {
        let resp = self.client.get(format!("{}/triggers", self.base_url)).send().await?;
        Self::handle_response(resp).await
    }

    pub async fn create_trigger(&self, trigger: &serde_json::Value) -> Result<Trigger> {
        let resp = self
            .client
            .post(format!("{}/triggers", self.base_url))
            .json(trigger)
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn update_trigger(&self, id: &str, trigger: &serde_json::Value) -> Result<Trigger> {
        let resp = self
            .client
            .put(format!("{}/triggers/{}", self.base_url, urlencoding(id)))
            .json(trigger)
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn delete_trigger(&self, id: &str) -> Result<StatusResult> {
        let resp = self
            .client
            .delete(format!("{}/triggers/{}", self.base_url, urlencoding(id)))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn delete_all_triggers(&self) -> Result<StatusResult> {
        let resp = self.client.delete(format!("{}/triggers", self.base_url)).send().await?;
        Self::handle_response(resp).await
    }

    pub async fn test_trigger(&self, pattern: &str, script: &str, message: &str) -> Result<serde_json::Value> {
        let resp = self
            .client
            .post(format!("{}/triggers/test", self.base_url))
            .json(&json!({
                "pattern": pattern,
                "script": script,
                "message": message,
            }))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn get_cron_jobs(&self) -> Result<Vec<CronJob>> {
        let resp = self.client.get(format!("{}/cron", self.base_url)).send().await?;
        Self::handle_response(resp).await
    }

    pub async fn create_cron_job(&self, job: &serde_json::Value) -> Result<CronJob> {
        let resp = self
            .client
            .post(format!("{}/cron", self.base_url))
            .json(job)
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn update_cron_job(&self, id: &str, job: &serde_json::Value) -> Result<CronJob> {
        let resp = self
            .client
            .put(format!("{}/cron/{}", self.base_url, urlencoding(id)))
            .json(job)
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn delete_cron_job(&self, id: &str) -> Result<StatusResult> {
        let resp = self
            .client
            .delete(format!("{}/cron/{}", self.base_url, urlencoding(id)))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn delete_all_cron_jobs(&self) -> Result<StatusResult> {
        let resp = self.client.delete(format!("{}/cron", self.base_url)).send().await?;
        Self::handle_response(resp).await
    }

    pub async fn test_cron_job(&self, script: &str) -> Result<serde_json::Value> {
        let resp = self
            .client
            .post(format!("{}/cron/test", self.base_url))
            .json(&json!({ "script": script }))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn get_webhooks(&self) -> Result<Vec<Webhook>> {
        let resp = self.client.get(format!("{}/webhooks", self.base_url)).send().await?;
        Self::handle_response(resp).await
    }

    pub async fn create_webhook(&self, webhook: &serde_json::Value) -> Result<Webhook> {
        let resp = self
            .client
            .post(format!("{}/webhooks", self.base_url))
            .json(webhook)
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn update_webhook(&self, id: &str, webhook: &serde_json::Value) -> Result<Webhook> {
        let resp = self
            .client
            .put(format!("{}/webhooks/{}", self.base_url, urlencoding(id)))
            .json(webhook)
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn delete_webhook(&self, id: &str) -> Result<StatusResult> {
        let resp = self
            .client
            .delete(format!("{}/webhooks/{}", self.base_url, urlencoding(id)))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn delete_all_webhooks(&self) -> Result<StatusResult> {
        let resp = self.client.delete(format!("{}/webhooks", self.base_url)).send().await?;
        Self::handle_response(resp).await
    }

    pub async fn test_webhook(
        &self,
        path: &str,
        script: &str,
        method: &str,
        body: &str,
    ) -> Result<serde_json::Value> {
        let resp = self
            .client
            .post(format!("{}/webhooks/test", self.base_url))
            .json(&json!({
                "path": path,
                "script": script,
                "method": method,
                "body": body,
            }))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn get_webhook_logs(
        &self,
        webhook_id: Option<&str>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<WebhookLogResponse> {
        let mut params = Vec::new();
        if let Some(id) = webhook_id {
            params.push(format!("webhook_id={}", urlencoding(id)));
        }
        if let Some(l) = limit {
            params.push(format!("limit={}", l));
        }
        if let Some(o) = offset {
            params.push(format!("offset={}", o));
        }
        let url = if params.is_empty() {
            format!("{}/webhooks/logs", self.base_url)
        } else {
            format!("{}/webhooks/logs?{}", self.base_url, params.join("&"))
        };
        let resp = self.client.get(url).send().await?;
        Self::handle_response(resp).await
    }

    pub async fn delete_all_webhook_logs(&self) -> Result<StatusResult> {
        let resp = self
            .client
            .delete(format!("{}/webhooks/logs", self.base_url))
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    pub async fn get_docs(&self) -> Result<String> {
        let resp = self.client.get(format!("{}/docs", self.base_url)).send().await?;
        let text = resp.text().await?;
        Ok(text)
    }

    pub async fn chat_assistant(
        &self,
        prompt: &str,
        current_code: Option<&str>,
        model: Option<&str>,
    ) -> Result<AssistantResponse> {
        let mut body = json!({ "prompt": prompt });
        if let Some(c) = current_code {
            body["currentCode"] = json!(c);
        }
        if let Some(m) = model {
            body["model"] = json!(m);
        }
        let resp = self
            .client
            .post(format!("{}/ai/assistant", self.base_url))
            .json(&body)
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    // --- Settings ---

    pub async fn get_settings(&self) -> Result<SettingsResponse> {
        let resp = self.client.get(format!("{}/settings", self.base_url)).send().await?;
        Self::handle_response(resp).await
    }

    pub async fn update_settings(&self, data: &HashMap<String, String>) -> Result<SettingsResponse> {
        let resp = self
            .client
            .put(format!("{}/settings", self.base_url))
            .json(data)
            .send()
            .await?;
        Self::handle_response(resp).await
    }

    // --- Groups ---

    pub async fn join_group(&self, link: &str) -> Result<StatusResult> {
        let resp = self
            .client
            .post(format!("{}/groups/join", self.base_url))
            .json(&json!({
                "link": link,
            }))
            .send()
            .await?;
        Self::handle_response(resp).await
    }
}

fn urlencoding(s: &str) -> String {
    let mut encoded = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
            encoded.push(b as char);
        } else {
            encoded.push_str(&format!("%{:02X}", b));
        }
    }
    encoded
}

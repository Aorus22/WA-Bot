use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::time::sleep;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;

use crate::dto::{ChatState, Message};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WsMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WsEvent {
    Connected,
    Disconnected,
    NewMessage(Box<Message>),
    MessageStatus {
        chat_id: Option<String>,
        id: String,
        status: String,
    },
    MessageDeleted {
        chat_id: String,
        id: String,
    },
    MessageEdited {
        chat_id: String,
        id: String,
        content: String,
    },
    MessageReaction {
        chat_id: String,
        id: String,
        emoji: String,
        from: Option<String>,
    },
    PollUpdate {
        chat_id: String,
        message_id: String,
        payload: serde_json::Value,
    },
    ChatState(ChatState),
    ChatNameUpdate {
        chat_id: String,
        name: String,
    },
    ChatsChanged {
        reason: String,
    },
    GroupUpdated {
        jid: String,
    },
    StatusNew {
        sender: String,
        id: String,
    },
    ChannelMessage {
        jid: String,
        payload: serde_json::Value,
    },
    ChannelUpdate {
        jid: String,
    },
    ChannelsChanged {
        reason: String,
    },
    ChatPresence {
        chat_id: String,
        presence: String,
    },
    CallEvent {
        event_type: String,
        payload: serde_json::Value,
    },
    Raw(WsMessage),
}

impl WsMessage {
    pub fn to_event(&self) -> WsEvent {
        match self.msg_type.as_str() {
            "new_message" => {
                if let Ok(msg) = serde_json::from_value::<Message>(self.payload.clone()) {
                    WsEvent::NewMessage(Box::new(msg))
                } else {
                    WsEvent::Raw(self.clone())
                }
            }
            "message_status" => WsEvent::MessageStatus {
                chat_id: self
                    .payload
                    .get("chatId")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                id: self
                    .payload
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                status: self
                    .payload
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            },
            "message_deleted" => WsEvent::MessageDeleted {
                chat_id: self
                    .payload
                    .get("chatId")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                id: self
                    .payload
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            },
            "message_edited" => WsEvent::MessageEdited {
                chat_id: self
                    .payload
                    .get("chatId")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                id: self
                    .payload
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                content: self
                    .payload
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            },
            "message_reaction" => WsEvent::MessageReaction {
                chat_id: self
                    .payload
                    .get("chatId")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                id: self
                    .payload
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                emoji: self
                    .payload
                    .get("emoji")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                from: self
                    .payload
                    .get("from")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            },
            "poll_update" => WsEvent::PollUpdate {
                chat_id: self
                    .payload
                    .get("chatId")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                message_id: self
                    .payload
                    .get("messageId")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                payload: self.payload.clone(),
            },
            "chat_state" => {
                if let Ok(state) = serde_json::from_value::<ChatState>(self.payload.clone()) {
                    WsEvent::ChatState(state)
                } else {
                    WsEvent::Raw(self.clone())
                }
            }
            "chat_name_update" => WsEvent::ChatNameUpdate {
                chat_id: self
                    .payload
                    .get("chatId")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                name: self
                    .payload
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            },
            "chats_changed" => WsEvent::ChatsChanged {
                reason: self
                    .payload
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            },
            "group_updated" => WsEvent::GroupUpdated {
                jid: self
                    .payload
                    .get("jid")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            },
            "status_new" => WsEvent::StatusNew {
                sender: self
                    .payload
                    .get("sender")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                id: self
                    .payload
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            },
            "channel_message" => WsEvent::ChannelMessage {
                jid: self
                    .payload
                    .get("jid")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                payload: self.payload.clone(),
            },
            "channel_update" => WsEvent::ChannelUpdate {
                jid: self
                    .payload
                    .get("jid")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            },
            "channels_changed" => WsEvent::ChannelsChanged {
                reason: self
                    .payload
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            },
            "chat_presence" | "presence" => WsEvent::ChatPresence {
                chat_id: self
                    .payload
                    .get("chatId")
                    .or_else(|| self.payload.get("jid"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                presence: self
                    .payload
                    .get("presence")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            },
            s if s.starts_with("call.") => WsEvent::CallEvent {
                event_type: s.to_string(),
                payload: self.payload.clone(),
            },
            _ => WsEvent::Raw(self.clone()),
        }
    }
}

pub type UrlResolver = Arc<dyn Fn() -> String + Send + Sync>;

#[derive(Clone)]
pub struct WsClient {
    url_resolver: UrlResolver,
    user_id: Option<String>,
    sender: flume::Sender<WsEvent>,
    receiver: flume::Receiver<WsEvent>,
    is_running: Arc<AtomicBool>,
    is_connected: Arc<AtomicBool>,
}

impl WsClient {
    pub fn new(url_resolver: UrlResolver, user_id: Option<String>) -> Self {
        let (sender, receiver) = flume::unbounded();
        Self {
            url_resolver,
            user_id,
            sender,
            receiver,
            is_running: Arc::new(AtomicBool::new(false)),
            is_connected: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn from_static_url(url: impl Into<String>, user_id: Option<String>) -> Self {
        let static_url = url.into();
        let resolver = Arc::new(move || static_url.clone());
        Self::new(resolver, user_id)
    }

    pub fn is_connected(&self) -> bool {
        self.is_connected.load(Ordering::SeqCst)
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    pub fn subscribe(&self) -> flume::Receiver<WsEvent> {
        self.receiver.clone()
    }

    pub fn start(&self) {
        if self
            .is_running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        let is_running = Arc::clone(&self.is_running);
        let is_connected = Arc::clone(&self.is_connected);
        let url_resolver = Arc::clone(&self.url_resolver);
        let user_id = self.user_id.clone();
        let sender = self.sender.clone();

        tokio::spawn(async move {
            while is_running.load(Ordering::SeqCst) {
                let ws_url = (url_resolver)();
                if ws_url.is_empty() {
                    sleep(Duration::from_millis(500)).await;
                    continue;
                }

                match connect_async(&ws_url).await {
                    Ok((mut stream, _)) => {
                        is_connected.store(true, Ordering::SeqCst);
                        let _ = sender.send(WsEvent::Connected);

                        // 1. Send authenticate handshake if user_id is provided
                        if let Some(ref uid) = user_id {
                            let auth_payload = serde_json::json!({
                                "type": "authenticate",
                                "payload": {
                                    "userId": uid
                                }
                            });
                            let _ = stream
                                .send(TungsteniteMessage::Text(auth_payload.to_string().into()))
                                .await;
                        }

                        let mut normal_close = false;

                        while let Some(msg_res) = stream.next().await {
                            match msg_res {
                                Ok(TungsteniteMessage::Text(text)) => {
                                    if let Ok(msg) = serde_json::from_str::<WsMessage>(&text) {
                                        let _ = sender.send(msg.to_event());
                                    }
                                }
                                Ok(TungsteniteMessage::Close(frame)) => {
                                    if let Some(CloseFrame { code, .. }) = frame {
                                        if code == CloseCode::Normal {
                                            normal_close = true;
                                        }
                                    }
                                    break;
                                }
                                Ok(_) => {}
                                Err(_) => break,
                            }
                        }

                        is_connected.store(false, Ordering::SeqCst);
                        let _ = sender.send(WsEvent::Disconnected);

                        if normal_close || !is_running.load(Ordering::SeqCst) {
                            break;
                        }

                        // Reconnect backoff ~3s
                        sleep(Duration::from_millis(3000)).await;
                    }
                    Err(_) => {
                        is_connected.store(false, Ordering::SeqCst);
                        if !is_running.load(Ordering::SeqCst) {
                            break;
                        }
                        // Reconnect backoff ~3s
                        sleep(Duration::from_millis(3000)).await;
                    }
                }
            }
            is_running.store(false, Ordering::SeqCst);
            is_connected.store(false, Ordering::SeqCst);
        });
    }

    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
    }
}

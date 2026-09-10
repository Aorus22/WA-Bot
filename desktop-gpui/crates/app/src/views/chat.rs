//! Chat view matching Web ChatPage, ChatSidebar, and ChatArea with 1:1 parity.

use gpui::*;
use gpui_component::{h_flex, v_flex, Icon, IconName};
use wabot_backend_client::client::HttpClient;
use wabot_backend_client::dto::{Chat, Message};

use crate::components::message_bubble::MessageBubbleHelper;
use crate::state::auth::AuthState;
use crate::state::chat::ChatStore;
use crate::theme::manager::AppThemeExt;
use crate::TOKIO_RT;

/// Deterministic color palette for contact avatars
const AVATAR_COLORS: &[u32] = &[
    0x00a884, // WhatsApp Emerald
    0x0284c7, // Sky
    0x6366f1, // Indigo
    0x8b5cf6, // Violet
    0xec4899, // Pink
    0xf59e0b, // Amber
    0x10b981, // Green
    0x14b8a6, // Teal
];

fn avatar_color_for(id: &str) -> Hsla {
    let mut sum: usize = 0;
    for b in id.bytes() {
        sum = sum.wrapping_add(b as usize);
    }
    let hex = AVATAR_COLORS[sum % AVATAR_COLORS.len()];
    rgb(hex).into()
}

pub struct ChatView {
    pub search_query: String,
    pub search_focus_handle: FocusHandle,
    pub compose_text: String,
    pub compose_focus_handle: FocusHandle,
    pub archived_mode: bool,
    pub selected_chat_id: Option<String>,
    pub is_sending: bool,
    pub is_loading_chats: bool,
    pub is_loading_messages: bool,
    pub toast_message: Option<(String, bool)>, // (message, is_error)
}

impl ChatView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            search_query: String::new(),
            search_focus_handle: cx.focus_handle(),
            compose_text: String::new(),
            compose_focus_handle: cx.focus_handle(),
            archived_mode: false,
            selected_chat_id: None,
            is_sending: false,
            is_loading_chats: false,
            is_loading_messages: false,
            toast_message: None,
        };
        view.load_chats(cx);
        view
    }

    /// Fetch all chats from GET /api/chats and populate ChatStore
    pub fn load_chats(&mut self, cx: &mut Context<Self>) {
        if self.is_loading_chats {
            return;
        }
        self.is_loading_chats = true;

        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT.spawn(async move { client.get_chats().await }).await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_loading_chats = false;
                            if let Ok(Ok(chats)) = res {
                                if cx.has_global::<ChatStore>() {
                                    ChatStore::global_mut(cx).set_chats(chats);
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Select a chat, mark as read, and load its messages
    pub fn select_chat(&mut self, chat_id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_chat_id = Some(chat_id.clone());

        if cx.has_global::<ChatStore>() {
            let store = ChatStore::global_mut(cx);
            store.active_chat_id = Some(chat_id.clone());
            store.mark_chat_read(&chat_id);
        }

        // Mark as read in backend
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };
        let cid_clone = chat_id.clone();
        cx.spawn(move |_this: WeakEntity<Self>, _cx: &mut AsyncApp| {
            async move {
                let client = HttpClient::new(&base_url);
                let _ = TOKIO_RT
                    .spawn(async move { client.mark_as_read(&cid_clone).await })
                    .await;
            }
        })
        .detach();

        self.load_messages(&chat_id, cx);
        self.compose_focus_handle.focus(window, cx);
        cx.notify();
    }

    /// Fetch messages for a specific chat from GET /api/chats/:id/messages
    pub fn load_messages(&mut self, chat_id: &str, cx: &mut Context<Self>) {
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };
        let cid = chat_id.to_string();
        self.is_loading_messages = true;

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let cid_fetch = cid.clone();
                let res = TOKIO_RT
                    .spawn(async move { client.get_messages(&cid_fetch, Some(100), None, None).await })
                    .await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_loading_messages = false;
                            if let Ok(Ok(msgs)) = res {
                                if cx.has_global::<ChatStore>() {
                                    let has_more = msgs.len() >= 100;
                                    ChatStore::global_mut(cx).set_messages(&cid, msgs, has_more);
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Send current message in compose bar
    pub fn send_current_message(&mut self, cx: &mut Context<Self>) {
        let text = self.compose_text.trim().to_string();
        if text.is_empty() || self.is_sending {
            return;
        }
        let chat_id = match &self.selected_chat_id {
            Some(id) => id.clone(),
            None => return,
        };

        self.compose_text.clear();
        self.is_sending = true;

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let temp_id = format!("temp-{}", now_ms);

        let opt_msg = Message {
            id: temp_id.clone(),
            chat_id: chat_id.clone(),
            from: "me".to_string(),
            to: chat_id.clone(),
            content: text.clone(),
            timestamp: now_ms,
            status: "pending".to_string(),
            message_type: "text".to_string(),
            media_url: None,
            is_automatic: None,
            sender_name: Some("You".to_string()),
            reply_to_id: None,
            forwarded: None,
            reactions: None,
            extra: None,
        };

        if cx.has_global::<ChatStore>() {
            let store = ChatStore::global_mut(cx);
            store.upsert_message(&chat_id, opt_msg);
            if let Some(c) = store.chats.iter_mut().find(|c| c.id == chat_id) {
                c.last_msg = text.clone();
                c.last_time = now_ms;
            }
        }

        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        let target_chat = chat_id.clone();
        let chat_for_api = chat_id.clone();
        let sent_text = text.clone();
        let opt_id_clone = temp_id.clone();

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move { client.send_message(&chat_for_api, &sent_text).await })
                    .await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_sending = false;
                            match res {
                                Ok(Ok(id_res)) => {
                                    if cx.has_global::<ChatStore>() {
                                        ChatStore::global_mut(cx).patch_message(
                                            &target_chat,
                                            &opt_id_clone,
                                            |m| {
                                                m.id = id_res.id.clone();
                                                m.status = "sent".to_string();
                                            },
                                        );
                                    }
                                }
                                _ => {
                                    if cx.has_global::<ChatStore>() {
                                        ChatStore::global_mut(cx).patch_message(
                                            &target_chat,
                                            &opt_id_clone,
                                            |m| {
                                                m.status = "error".to_string();
                                            },
                                        );
                                    }
                                    this.toast_message = Some(("Failed to send message".into(), true));
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Toggle pin state for a chat
    pub fn toggle_pin(&mut self, chat_id: String, is_pinned: bool, cx: &mut Context<Self>) {
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        let target_chat = chat_id.clone();
        let new_state = !is_pinned;

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move { client.pin_chat(&target_chat, new_state).await })
                    .await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |_this, cx| {
                            if let Ok(Ok(cs)) = res {
                                if cx.has_global::<ChatStore>() {
                                    ChatStore::global_mut(cx).patch_chat_state(&cs);
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Toggle archive state for a chat
    pub fn toggle_archive(&mut self, chat_id: String, is_archived: bool, cx: &mut Context<Self>) {
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        let target_chat = chat_id.clone();
        let new_state = !is_archived;

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move { client.archive_chat(&target_chat, new_state).await })
                    .await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |_this, cx| {
                            if let Ok(Ok(cs)) = res {
                                if cx.has_global::<ChatStore>() {
                                    ChatStore::global_mut(cx).patch_chat_state(&cs);
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn format_chat_time(timestamp: i64) -> String {
        if timestamp == 0 {
            return String::new();
        }
        let ts_sec = if timestamp > 10_000_000_000 { timestamp / 1000 } else { timestamp };
        let hours = (ts_sec / 3600) % 24;
        let mins = (ts_sec / 60) % 60;
        format!("{:02}:{:02}", hours, mins)
    }
}

impl Render for ChatView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.app_theme();
        let border_color = theme.border;
        let bg_color = theme.background;
        let card_bg = theme.card;
        let primary_color = theme.primary;
        let text_color = theme.foreground;
        let muted_text = theme.muted_foreground;

        let all_chats: Vec<Chat> = if cx.has_global::<ChatStore>() {
            ChatStore::global(cx).chats.clone()
        } else {
            Vec::new()
        };

        // Filter and sort chats
        let query = self.search_query.trim().to_lowercase();
        let archived_mode = self.archived_mode;

        let mut filtered_chats: Vec<Chat> = all_chats
            .iter()
            .filter(|c| c.archived == archived_mode)
            .filter(|c| {
                if query.is_empty() {
                    true
                } else {
                    c.name.to_lowercase().contains(&query)
                        || c.id.to_lowercase().contains(&query)
                        || c.last_msg.to_lowercase().contains(&query)
                }
            })
            .cloned()
            .collect();

        // Sort: pinned first, then last_time descending
        filtered_chats.sort_by(|a, b| {
            match (a.pinned_at, b.pinned_at) {
                (Some(pa), Some(pb)) => pb.cmp(&pa),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => b.last_time.cmp(&a.last_time),
            }
        });

        let archived_count = all_chats.iter().filter(|c| c.archived).count();
        let selected_chat = self
            .selected_chat_id
            .as_ref()
            .and_then(|id| all_chats.iter().find(|c| c.id == *id).cloned());

        let active_messages: Vec<Message> = if let Some(ref sc) = selected_chat {
            if cx.has_global::<ChatStore>() {
                ChatStore::global(cx)
                    .messages_by_chat
                    .get(&sc.id)
                    .map(|e| e.messages.clone())
                    .unwrap_or_default()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let selected_chat_id = self.selected_chat_id.clone();
        let is_loading_chats = self.is_loading_chats;
        let is_loading_messages = self.is_loading_messages;

        h_flex()
            .size_full()
            .overflow_hidden()
            .bg(bg_color)
            // ====================================================
            // LEFT SIDEBAR: CONVERSATIONS LIST
            // ====================================================
            .child(
                v_flex()
                    .w(px(360.0))
                    .h_full()
                    .border_r_1()
                    .border_color(border_color)
                    .bg(card_bg)
                    .flex_shrink_0()
                    // Sidebar Header
                    .child(
                        v_flex()
                            .p_4()
                            .gap_3()
                            .border_b_1()
                            .border_color(border_color)
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap_2()
                                            .children(if archived_mode {
                                                Some(
                                                    div()
                                                        .cursor_pointer()
                                                        .p_1()
                                                        .rounded_full()
                                                        .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                        .text_sm()
                                                        .text_color(primary_color)
                                                        .child("← Back")
                                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                            this.archived_mode = false;
                                                            cx.notify();
                                                        })),
                                                )
                                            } else {
                                                None
                                            })
                                            .child(
                                                div()
                                                    .text_xl()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(text_color)
                                                    .child(if archived_mode { "Archived" } else { "Chats" }),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .cursor_pointer()
                                                    .px_2()
                                                    .py_1()
                                                    .rounded_md()
                                                    .text_xs()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(primary_color)
                                                    .hover(|s| s.bg(theme.primary.opacity(0.1)))
                                                    .child("Sync")
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                        this.load_chats(cx);
                                                    })),
                                            ),
                                    ),
                            )
                            // Search Box
                            .child(
                                h_flex()
                                    .px_3()
                                    .py_2()
                                    .bg(bg_color)
                                    .rounded_xl()
                                    .border_1()
                                    .border_color(border_color)
                                    .items_center()
                                    .gap_2()
                                    .track_focus(&self.search_focus_handle)
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                        this.search_focus_handle.focus(window, cx);
                                    }))
                                    .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                                        match ev.keystroke.key.as_str() {
                                            "backspace" => {
                                                this.search_query.pop();
                                                cx.notify();
                                            }
                                            "escape" => {
                                                this.search_query.clear();
                                                cx.notify();
                                            }
                                            "space" => {
                                                this.search_query.push(' ');
                                                cx.notify();
                                            }
                                            k if k.len() == 1 => {
                                                this.search_query.push_str(k);
                                                cx.notify();
                                            }
                                            _ => {}
                                        }
                                    }))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(muted_text)
                                            .child("🔍"),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_xs()
                                            .text_color(if self.search_query.is_empty() { muted_text } else { text_color })
                                            .child(if self.search_query.is_empty() {
                                                "Search conversations...".to_string()
                                            } else {
                                                self.search_query.clone()
                                            }),
                                    )
                                    .children(if !self.search_query.is_empty() {
                                        Some(
                                            div()
                                                .cursor_pointer()
                                                .text_xs()
                                                .text_color(muted_text)
                                                .child("✕")
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.search_query.clear();
                                                    cx.notify();
                                                })),
                                        )
                                    } else {
                                        None
                                    }),
                            ),
                    )
                    // Archived Button Row (when archived mode is false and archived chats exist)
                    .children(if !archived_mode && self.search_query.is_empty() && archived_count > 0 {
                        Some(
                            h_flex()
                                .mx_2()
                                .mt_2()
                                .px_3()
                                .py_2p5()
                                .rounded_xl()
                                .items_center()
                                .gap_3()
                                .cursor_pointer()
                                .hover(|s| s.bg(theme.muted.opacity(0.4)))
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                    this.archived_mode = true;
                                    cx.notify();
                                }))
                                .child(
                                    div()
                                        .w(px(32.0))
                                        .h(px(32.0))
                                        .rounded_full()
                                        .bg(rgba(0x00a88420))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_sm()
                                        .child("📦"),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(text_color)
                                        .child("Archived"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(0x00a884))
                                        .child(format!("{}", archived_count)),
                                ),
                        )
                    } else {
                        None
                    })
                    // Chat Scroll List
                    .child(
                        v_flex()
                            .id("chat-list-scroll")
                            .flex_1()
                            .overflow_y_scroll()
                            .p_2()
                            .gap_1()
                            .children(if is_loading_chats && filtered_chats.is_empty() {
                                vec![
                                    v_flex()
                                        .py_12()
                                        .items_center()
                                        .justify_center()
                                        .gap_2()
                                        .child(Icon::new(IconName::LoaderCircle).size(px(24.)))
                                        .child(div().text_xs().text_color(muted_text).child("Loading conversations..."))
                                        .into_any_element(),
                                ]
                            } else if filtered_chats.is_empty() {
                                vec![
                                    v_flex()
                                        .py_12()
                                        .px_4()
                                        .items_center()
                                        .justify_center()
                                        .gap_2()
                                        .child(div().text_2xl().child("💬"))
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(text_color)
                                                .child(if self.search_query.is_empty() {
                                                    "No messages yet"
                                                } else {
                                                    "No results found"
                                                }),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(muted_text)
                                                .child(if self.search_query.is_empty() {
                                                    "Incoming messages will appear here."
                                                } else {
                                                    "Try another search keyword."
                                                }),
                                        )
                                        .into_any_element(),
                                ]
                            } else {
                                filtered_chats
                                    .into_iter()
                                    .map(|chat| {
                                        let is_selected = selected_chat_id.as_deref() == Some(&chat.id);
                                        let chat_id = chat.id.clone();
                                        let is_pinned = chat.pinned_at.is_some();
                                        let initial = chat.name.chars().next().unwrap_or('?').to_uppercase().to_string();
                                        let avatar_bg = avatar_color_for(&chat.id);
                                        let last_msg_snippet = if chat.last_msg.is_empty() {
                                            "Tap to chat".to_string()
                                        } else {
                                            MessageBubbleHelper::decode_content(&chat.last_msg)
                                        };
                                        let formatted_time = Self::format_chat_time(chat.last_time);
                                        let unread = chat.unread;

                                        h_flex()
                                            .w_full()
                                            .px_3()
                                            .py_3()
                                            .rounded_xl()
                                            .gap_3()
                                            .items_center()
                                            .cursor_pointer()
                                            .relative()
                                            .bg(if is_selected {
                                                theme.primary.opacity(0.12)
                                            } else {
                                                rgba(0x00000000).into()
                                            })
                                            .hover(|s| s.bg(theme.muted.opacity(0.4)))
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener({
                                                    let cid = chat_id.clone();
                                                    move |this, _, window, cx| {
                                                        this.select_chat(cid.clone(), window, cx);
                                                    }
                                                }),
                                            )
                                            // Selected Indicator Bar
                                            .children(if is_selected {
                                                Some(
                                                    div()
                                                        .absolute()
                                                        .left(px(0.0))
                                                        .top(px(8.0))
                                                        .bottom(px(8.0))
                                                        .w(px(3.5))
                                                        .bg(primary_color)
                                                        .rounded_r_full(),
                                                )
                                            } else {
                                                None
                                            })
                                            // Avatar
                                            .child(
                                                div()
                                                    .w(px(46.0))
                                                    .h(px(46.0))
                                                    .rounded_full()
                                                    .bg(avatar_bg)
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .flex_shrink_0()
                                                    .text_base()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(rgb(0xffffff))
                                                    .child(initial),
                                            )
                                            // Middle: Name & Last Msg
                                            .child(
                                                v_flex()
                                                    .flex_1()
                                                    .min_w(px(0.0))
                                                    .justify_center()
                                                    .child(
                                                        h_flex()
                                                            .items_center()
                                                            .gap_1p5()
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .font_weight(FontWeight::SEMIBOLD)
                                                                    .text_color(text_color)
                                                                    .overflow_hidden()
                                                                    .child(chat.name.clone()),
                                                            )
                                                            .children(if is_pinned {
                                                                Some(div().text_xs().text_color(muted_text).child("📌"))
                                                            } else {
                                                                None
                                                            }),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(if unread > 0 { text_color } else { muted_text })
                                                            .font_weight(if unread > 0 { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                                                            .overflow_hidden()
                                                            .child(last_msg_snippet),
                                                    ),
                                            )
                                            // Right: Time & Unread Badge
                                            .child(
                                                v_flex()
                                                    .items_end()
                                                    .justify_between()
                                                    .h_full()
                                                    .flex_shrink_0()
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(if unread > 0 { rgb(0x00a884).into() } else { muted_text })
                                                            .font_weight(if unread > 0 { FontWeight::BOLD } else { FontWeight::NORMAL })
                                                            .child(formatted_time),
                                                    )
                                                    .children(if unread > 0 {
                                                        Some(
                                                            div()
                                                                .px_1p5()
                                                                .py_0p5()
                                                                .min_w(px(18.0))
                                                                .rounded_full()
                                                                .bg(rgb(0x00a884))
                                                                .text_xs()
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(rgb(0xffffff))
                                                                .flex()
                                                                .items_center()
                                                                .justify_center()
                                                                .child(format!("{}", unread)),
                                                        )
                                                    } else {
                                                        None
                                                    }),
                                            )
                                            .into_any_element()
                                    })
                                    .collect()
                            }),
                    ),
            )
            // ====================================================
            // RIGHT PANE: ACTIVE CHAT CONVERSATION OR EMPTY STATE
            // ====================================================
            .child(
                v_flex()
                    .flex_1()
                    .h_full()
                    .bg(bg_color)
                    .children(if let Some(ref active_chat) = selected_chat {
                        let chat_name = active_chat.name.clone();
                        let chat_jid = active_chat.id.clone();
                        let avatar_initial = chat_name.chars().next().unwrap_or('?').to_uppercase().to_string();
                        let avatar_bg = avatar_color_for(&active_chat.id);
                        let is_pinned = active_chat.pinned_at.is_some();
                        let is_archived = active_chat.archived;

                        Some(
                            v_flex()
                                .size_full()
                                // Chat Header
                                .child(
                                    h_flex()
                                        .px_5()
                                        .py_3()
                                        .border_b_1()
                                        .border_color(border_color)
                                        .bg(card_bg)
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .gap_3()
                                                .child(
                                                    div()
                                                        .w(px(40.0))
                                                        .h(px(40.0))
                                                        .rounded_full()
                                                        .bg(avatar_bg)
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .text_sm()
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_color(rgb(0xffffff))
                                                        .child(avatar_initial),
                                                )
                                                .child(
                                                    v_flex()
                                                        .child(
                                                            div()
                                                                .text_base()
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(text_color)
                                                                .child(chat_name),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(muted_text)
                                                                .child(chat_jid.clone()),
                                                        ),
                                                ),
                                        )
                                        // Header Actions
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .cursor_pointer()
                                                        .px_2p5()
                                                        .py_1p5()
                                                        .rounded_lg()
                                                        .border_1()
                                                        .border_color(border_color)
                                                        .bg(bg_color)
                                                        .hover(|s| s.bg(theme.muted.opacity(0.4)))
                                                        .text_xs()
                                                        .text_color(text_color)
                                                        .child(if is_pinned { "Unpin" } else { "Pin" })
                                                        .on_mouse_down(
                                                            MouseButton::Left,
                                                            cx.listener({
                                                                let cid = chat_jid.clone();
                                                                move |this, _, _, cx| {
                                                                    this.toggle_pin(cid.clone(), is_pinned, cx);
                                                                }
                                                            }),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .cursor_pointer()
                                                        .px_2p5()
                                                        .py_1p5()
                                                        .rounded_lg()
                                                        .border_1()
                                                        .border_color(border_color)
                                                        .bg(bg_color)
                                                        .hover(|s| s.bg(theme.muted.opacity(0.4)))
                                                        .text_xs()
                                                        .text_color(text_color)
                                                        .child(if is_archived { "Unarchive" } else { "Archive" })
                                                        .on_mouse_down(
                                                            MouseButton::Left,
                                                            cx.listener({
                                                                let cid = chat_jid.clone();
                                                                move |this, _, _, cx| {
                                                                    this.toggle_archive(cid.clone(), is_archived, cx);
                                                                }
                                                            }),
                                                        ),
                                                ),
                                        ),
                                )
                                // Messages History View
                                .child(
                                    v_flex()
                                        .id("messages-scroll")
                                        .flex_1()
                                        .overflow_y_scroll()
                                        .p_6()
                                        .gap_3()
                                        .children(if is_loading_messages && active_messages.is_empty() {
                                            vec![
                                                v_flex()
                                                    .py_16()
                                                    .items_center()
                                                    .justify_center()
                                                    .gap_2()
                                                    .child(Icon::new(IconName::LoaderCircle).size(px(24.)))
                                                    .child(div().text_xs().text_color(muted_text).child("Loading messages..."))
                                                    .into_any_element(),
                                            ]
                                        } else if active_messages.is_empty() {
                                            vec![
                                                v_flex()
                                                    .py_16()
                                                    .items_center()
                                                    .justify_center()
                                                    .gap_2()
                                                    .child(div().text_2xl().child("💬"))
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .text_color(text_color)
                                                            .child("No messages in this chat yet"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(muted_text)
                                                            .child("Type a message below to start the conversation."),
                                                    )
                                                    .into_any_element(),
                                            ]
                                        } else {
                                            active_messages
                                                .into_iter()
                                                .map(|msg| {
                                                    let is_from_me = MessageBubbleHelper::is_from_me(&msg, "me");
                                                    let content = MessageBubbleHelper::decode_content(&msg.content);
                                                    let time_str = Self::format_chat_time(msg.timestamp);
                                                    let ticks = MessageBubbleHelper::ticks(&msg, is_from_me);
                                                    let ticks_str = MessageBubbleHelper::ticks_display(ticks);
                                                    let is_read = ticks == crate::components::message_bubble::MessageTicks::Read;

                                                    let row = if is_from_me {
                                                        h_flex().w_full().justify_end()
                                                    } else {
                                                        h_flex().w_full().justify_start()
                                                    };

                                                    row.child(
                                                            v_flex()
                                                                .max_w(px(520.0))
                                                                .px_4()
                                                                .py_2p5()
                                                                .rounded_2xl()
                                                                .bg(if is_from_me {
                                                                    theme.primary.opacity(0.18)
                                                                } else {
                                                                    card_bg
                                                                })
                                                                .border_1()
                                                                .border_color(if is_from_me {
                                                                    theme.primary.opacity(0.35)
                                                                } else {
                                                                    border_color
                                                                })
                                                                .gap_1()
                                                                // Sender name if in group and incoming
                                                                .children(if !is_from_me && msg.sender_name.is_some() {
                                                                    Some(
                                                                        div()
                                                                            .text_xs()
                                                                            .font_weight(FontWeight::BOLD)
                                                                            .text_color(primary_color)
                                                                            .child(msg.sender_name.unwrap()),
                                                                    )
                                                                } else {
                                                                    None
                                                                })
                                                                // Message Body Text
                                                                .child(
                                                                    div()
                                                                        .text_sm()
                                                                        .text_color(text_color)
                                                                        .child(content),
                                                                )
                                                                // Footer: Timestamp + Status Ticks
                                                                .child(
                                                                    h_flex()
                                                                        .justify_end()
                                                                        .items_center()
                                                                        .gap_1()
                                                                        .child(
                                                                            div()
                                                                                .text_xs()
                                                                                .text_color(muted_text)
                                                                                .child(time_str),
                                                                        )
                                                                        .children(if is_from_me {
                                                                            Some(
                                                                                div()
                                                                                    .text_xs()
                                                                                    .font_weight(FontWeight::BOLD)
                                                                                    .text_color(if is_read {
                                                                                        rgb(0x00a884).into()
                                                                                    } else {
                                                                                        muted_text
                                                                                    })
                                                                                    .child(ticks_str),
                                                                            )
                                                                        } else {
                                                                            None
                                                                        }),
                                                                ),
                                                        )
                                                        .into_any_element()
                                                })
                                                .collect()
                                        }),
                                )
                                // Bottom Compose Bar
                                .child(
                                    h_flex()
                                        .p_3()
                                        .border_t_1()
                                        .border_color(border_color)
                                        .bg(card_bg)
                                        .items_center()
                                        .gap_3()
                                        // Text Input Field
                                        .child(
                                            h_flex()
                                                .flex_1()
                                                .px_4()
                                                .py_2p5()
                                                .rounded_xl()
                                                .border_1()
                                                .border_color(border_color)
                                                .bg(bg_color)
                                                .items_center()
                                                .track_focus(&self.compose_focus_handle)
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                                    this.compose_focus_handle.focus(window, cx);
                                                }))
                                                .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                                                    // Handle Ctrl+V / Cmd+V paste
                                                    if (ev.keystroke.modifiers.control || ev.keystroke.modifiers.platform)
                                                        && ev.keystroke.key.as_str().eq_ignore_ascii_case("v")
                                                    {
                                                        if let Some(item) = cx.read_from_clipboard() {
                                                            if let Some(text) = item.text() {
                                                                this.compose_text.push_str(&text);
                                                                cx.notify();
                                                            }
                                                        }
                                                        return;
                                                    }

                                                    match ev.keystroke.key.as_str() {
                                                        "enter" => {
                                                            this.send_current_message(cx);
                                                        }
                                                        "backspace" => {
                                                            this.compose_text.pop();
                                                            cx.notify();
                                                        }
                                                        "space" => {
                                                            this.compose_text.push(' ');
                                                            cx.notify();
                                                        }
                                                        k if k.len() == 1 => {
                                                            this.compose_text.push_str(k);
                                                            cx.notify();
                                                        }
                                                        _ => {}
                                                    }
                                                }))
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .text_sm()
                                                        .text_color(if self.compose_text.is_empty() {
                                                            muted_text
                                                        } else {
                                                            text_color
                                                        })
                                                        .child(if self.compose_text.is_empty() {
                                                            "Type a message...".to_string()
                                                        } else {
                                                            self.compose_text.clone()
                                                        }),
                                                ),
                                        )
                                        // Send Button
                                        .child(
                                            div()
                                                .cursor_pointer()
                                                .px_4()
                                                .py_2p5()
                                                .rounded_xl()
                                                .bg(primary_color)
                                                .hover(|s| s.opacity(0.85))
                                                .flex()
                                                .items_center()
                                                .gap_1p5()
                                                .text_xs()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(rgb(0xffffff))
                                                .child(if self.is_sending { "Sending..." } else { "Send" })
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.send_current_message(cx);
                                                })),
                                        ),
                                ),
                        )
                    } else {
                        // Empty state when no chat selected
                        Some(
                            v_flex()
                                .size_full()
                                .items_center()
                                .justify_center()
                                .gap_3()
                                .child(
                                    div()
                                        .w(px(72.0))
                                        .h(px(72.0))
                                        .rounded_3xl()
                                        .bg(theme.primary.opacity(0.1))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_3xl()
                                        .child("💬"),
                                )
                                .child(
                                    div()
                                        .text_xl()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(text_color)
                                        .child("WA Bot Desktop"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(muted_text)
                                        .child("Select a chat from the left pane to view messages with 1:1 web parity."),
                                ),
                        )
                    }),
            )
    }
}

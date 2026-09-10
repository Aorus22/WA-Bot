//! Chat view matching Web ChatPage, ChatSidebar, and ChatArea with 1:1 parity.

use gpui::*;
use gpui_component::{h_flex, v_flex, Icon, IconName};
use wabot_backend_client::client::HttpClient;
use wabot_backend_client::dto::{Chat, Message};

use crate::components::message_bubble::{MessageBubbleHelper, MessageTicks};
use crate::icons::*;
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
    pub reply_to: Option<Message>,
    pub editing_message: Option<Message>,
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
            reply_to: None,
            editing_message: None,
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
        self.reply_to = None;
        self.editing_message = None;

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

    /// Start replying to a message
    pub fn start_reply(&mut self, msg: Message, window: &mut Window, cx: &mut Context<Self>) {
        self.reply_to = Some(msg);
        self.editing_message = None;
        self.compose_focus_handle.focus(window, cx);
        cx.notify();
    }

    /// Cancel active reply
    pub fn cancel_reply(&mut self, cx: &mut Context<Self>) {
        self.reply_to = None;
        cx.notify();
    }

    /// Start editing an existing outgoing message
    pub fn start_edit(&mut self, msg: Message, window: &mut Window, cx: &mut Context<Self>) {
        self.compose_text = msg.content.clone();
        self.editing_message = Some(msg);
        self.reply_to = None;
        self.compose_focus_handle.focus(window, cx);
        cx.notify();
    }

    /// Cancel active edit
    pub fn cancel_edit(&mut self, cx: &mut Context<Self>) {
        self.editing_message = None;
        self.compose_text.clear();
        cx.notify();
    }

    /// Copy message text to system clipboard
    pub fn copy_message_text(&mut self, text: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.toast_message = Some(("Pesan disalin ke clipboard".into(), false));
        cx.notify();
    }

    /// Delete a message
    pub fn delete_message(&mut self, chat_id: String, msg_id: String, cx: &mut Context<Self>) {
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        if cx.has_global::<ChatStore>() {
            ChatStore::global_mut(cx).delete_message(&chat_id, &msg_id);
        }
        cx.notify();

        let cid = chat_id.clone();
        let mid = msg_id.clone();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT.spawn(async move { client.delete_message(&cid, &mid).await }).await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            match res {
                                Ok(Ok(_)) => {
                                    this.toast_message = Some(("Pesan berhasil dihapus".into(), false));
                                }
                                _ => {
                                    this.toast_message = Some(("Gagal menghapus pesan".into(), true));
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

    /// Send current message in compose bar (supports regular send, reply, and edit)
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

        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        // Case 1: Editing existing message
        if let Some(edit_msg) = self.editing_message.take() {
            let edit_id = edit_msg.id.clone();
            let sent_text = text.clone();
            let target_chat = chat_id.clone();

            if cx.has_global::<ChatStore>() {
                ChatStore::global_mut(cx).patch_message(&target_chat, &edit_id, |m| {
                    m.content = sent_text.clone();
                });
            }

            cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let view_weak = this;
                let cx_handle = cx.clone();
                async move {
                    let client = HttpClient::new(&base_url);
                    let res = TOKIO_RT
                        .spawn(async move { client.edit_message(&target_chat, &edit_id, &sent_text).await })
                        .await;

                    let _ = cx_handle.update(|cx: &mut App| {
                        if let Some(view) = view_weak.upgrade() {
                            view.update(cx, |this, cx| {
                                this.is_sending = false;
                                match res {
                                    Ok(Ok(_)) => {
                                        this.toast_message = Some(("Pesan berhasil diedit".into(), false));
                                    }
                                    _ => {
                                        this.toast_message = Some(("Gagal mengedit pesan".into(), true));
                                    }
                                }
                                cx.notify();
                            });
                        }
                    });
                }
            })
            .detach();
            return;
        }

        // Case 2: Replying to message or Case 3: Regular send
        let reply_to_id = self.reply_to.take().map(|m| m.id);
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
            reply_to_id: reply_to_id.clone(),
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

        let target_chat = chat_id.clone();
        let sent_text = text.clone();
        let opt_id_clone = temp_id.clone();
        let rep_id = reply_to_id.clone();

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let c_target = target_chat.clone();
                let s_text = sent_text.clone();

                let res = if let Some(r_id) = rep_id {
                    let rep_id_clone = r_id.clone();
                    TOKIO_RT
                        .spawn(async move {
                            client.reply_message(&c_target, &rep_id_clone, &s_text).await
                                .map(|_| wabot_backend_client::dto::IdResult {
                                    status: "success".to_string(),
                                    id: format!("rep-{}", now_ms),
                                })
                        })
                        .await
                } else {
                    TOKIO_RT
                        .spawn(async move { client.send_message(&c_target, &s_text).await })
                        .await
                };

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
            .relative()
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
                                                    h_flex()
                                                        .cursor_pointer()
                                                        .items_center()
                                                        .gap_1()
                                                        .px_2()
                                                        .py_1()
                                                        .rounded_lg()
                                                        .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                        .text_xs()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(primary_color)
                                                        .child(svg().data(CHEVRON_LEFT_SVG).size(px(15.0)).text_color(primary_color))
                                                        .child("Back")
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
                                                h_flex()
                                                    .cursor_pointer()
                                                    .px_2p5()
                                                    .py_1p5()
                                                    .rounded_lg()
                                                    .items_center()
                                                    .gap_1p5()
                                                    .hover(|s| s.bg(theme.primary.opacity(0.1)))
                                                    .child(svg().data(REFRESH_CW_SVG).size(px(13.0)).text_color(primary_color))
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(primary_color)
                                                            .child("Sync"),
                                                    )
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
                                        svg()
                                            .data(SEARCH_SVG)
                                            .size(px(14.0))
                                            .text_color(muted_text),
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
                                                .p_1()
                                                .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                .rounded_full()
                                                .child(svg().data(X_SVG).size(px(12.0)).text_color(muted_text))
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
                                        .child(svg().data(ARCHIVE_SVG).size(px(16.0)).text_color(rgb(0x00a884))),
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
                    // Chat Scroll List (Uniform 72px item height strictly enforced)
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
                                        .child(svg().data(MESSAGE_SQUARE_SVG).size(px(32.0)).text_color(muted_text))
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

                                        // Clamped single-line snippet to guarantee identical 72px row heights
                                        let first_line = chat.last_msg.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
                                        let last_msg_snippet = if first_line.is_empty() {
                                            "Tap to chat".to_string()
                                        } else {
                                            MessageBubbleHelper::decode_content(first_line)
                                        };

                                        let formatted_time = Self::format_chat_time(chat.last_time);
                                        let unread = chat.unread;

                                        h_flex()
                                            .w_full()
                                            .h(px(72.0))
                                            .overflow_hidden()
                                            .px_3()
                                            .py_2p5()
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
                                            // Middle: Name & Clamped Last Msg
                                            .child(
                                                v_flex()
                                                    .flex_1()
                                                    .min_w(px(0.0))
                                                    .overflow_hidden()
                                                    .justify_center()
                                                    .gap_0p5()
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
                                                                Some(
                                                                    svg()
                                                                        .data(PIN_SVG)
                                                                        .size(px(12.0))
                                                                        .text_color(muted_text),
                                                                )
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
                                                    .justify_center()
                                                    .gap_1()
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
                                                    h_flex()
                                                        .cursor_pointer()
                                                        .px_2p5()
                                                        .py_1p5()
                                                        .rounded_lg()
                                                        .border_1()
                                                        .border_color(border_color)
                                                        .bg(bg_color)
                                                        .hover(|s| s.bg(theme.muted.opacity(0.4)))
                                                        .items_center()
                                                        .gap_1p5()
                                                        .child(svg().data(PIN_SVG).size(px(13.0)).text_color(text_color))
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(text_color)
                                                                .child(if is_pinned { "Unpin" } else { "Pin" }),
                                                        )
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
                                                    h_flex()
                                                        .cursor_pointer()
                                                        .px_2p5()
                                                        .py_1p5()
                                                        .rounded_lg()
                                                        .border_1()
                                                        .border_color(border_color)
                                                        .bg(bg_color)
                                                        .hover(|s| s.bg(theme.muted.opacity(0.4)))
                                                        .items_center()
                                                        .gap_1p5()
                                                        .child(svg().data(ARCHIVE_SVG).size(px(13.0)).text_color(text_color))
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(text_color)
                                                                .child(if is_archived { "Unarchive" } else { "Archive" }),
                                                        )
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
                                                    .child(svg().data(MESSAGE_SQUARE_SVG).size(px(32.0)).text_color(muted_text))
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
                                            let active_messages_with_quotes: Vec<(Message, Option<(String, String)>)> = {
                                                let mut lookup = std::collections::HashMap::new();
                                                for m in &active_messages {
                                                    let sender = if m.from == "me" {
                                                        "You".to_string()
                                                    } else {
                                                        m.sender_name.clone().unwrap_or_else(|| "Sender".to_string())
                                                    };
                                                    let decoded = MessageBubbleHelper::decode_content(&m.content);
                                                    let first_line = decoded.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string();
                                                    lookup.insert(m.id.clone(), (sender, first_line));
                                                }

                                                active_messages
                                                    .into_iter()
                                                    .map(|msg| {
                                                        let quoted = msg.reply_to_id.as_ref().and_then(|rid| lookup.get(rid).cloned());
                                                        (msg, quoted)
                                                    })
                                                    .collect()
                                            };

                                            active_messages_with_quotes
                                                .into_iter()
                                                .map(|(msg, quoted_info)| {
                                                    let is_from_me = MessageBubbleHelper::is_from_me(&msg, "me");
                                                    let content = MessageBubbleHelper::decode_content(&msg.content);
                                                    let time_str = Self::format_chat_time(msg.timestamp);
                                                    let ticks = MessageBubbleHelper::ticks(&msg, is_from_me);

                                                    let row = if is_from_me {
                                                        h_flex().w_full().justify_end()
                                                    } else {
                                                        h_flex().w_full().justify_start()
                                                    };

                                                    let bubble_chat_id = chat_jid.clone();

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
                                                                        .child(msg.sender_name.clone().unwrap()),
                                                                )
                                                            } else {
                                                                None
                                                            })
                                                            // Quoted message preview box if replying to another message
                                                            .children(if let Some((q_sender, q_line)) = quoted_info {
                                                                Some(
                                                                    v_flex()
                                                                        .mb_1()
                                                                        .px_2p5()
                                                                        .py_1p5()
                                                                        .bg(theme.muted.opacity(0.35))
                                                                        .border_l_4()
                                                                        .border_color(primary_color)
                                                                        .rounded_r_lg()
                                                                        .overflow_hidden()
                                                                        .child(
                                                                            div()
                                                                                .text_xs()
                                                                                .font_weight(FontWeight::BOLD)
                                                                                .text_color(primary_color)
                                                                                .child(q_sender),
                                                                        )
                                                                        .child(
                                                                            div()
                                                                                .text_xs()
                                                                                .text_color(muted_text)
                                                                                .overflow_hidden()
                                                                                .child(q_line),
                                                                        ),
                                                                )
                                                            } else {
                                                                None
                                                            })
                                                            // Message Body Text
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .text_color(text_color)
                                                                    .child(content.clone()),
                                                            )
                                                            // Footer: Actions (Reply, Copy, Edit, Delete) + Timestamp + Status Ticks
                                                            .child(
                                                                h_flex()
                                                                    .w_full()
                                                                    .justify_between()
                                                                    .items_center()
                                                                    .mt_1()
                                                                    .gap_3()
                                                                    // Left side: Chat Action Buttons
                                                                    .child(
                                                                        h_flex()
                                                                            .items_center()
                                                                            .gap_1()
                                                                            // Reply Action
                                                                            .child(
                                                                                div()
                                                                                    .cursor_pointer()
                                                                                    .p_1()
                                                                                    .rounded_md()
                                                                                    .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                                                    .child(svg().data(REPLY_SVG).size(px(13.0)).text_color(muted_text))
                                                                                    .on_mouse_down(MouseButton::Left, cx.listener({
                                                                                        let m = msg.clone();
                                                                                        move |this, _, window, cx| {
                                                                                            this.start_reply(m.clone(), window, cx);
                                                                                        }
                                                                                    })),
                                                                            )
                                                                            // Copy Action
                                                                            .child(
                                                                                div()
                                                                                    .cursor_pointer()
                                                                                    .p_1()
                                                                                    .rounded_md()
                                                                                    .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                                                    .child(svg().data(COPY_SVG).size(px(13.0)).text_color(muted_text))
                                                                                    .on_mouse_down(MouseButton::Left, cx.listener({
                                                                                        let t = content.clone();
                                                                                        move |this, _, _, cx| {
                                                                                            this.copy_message_text(t.clone(), cx);
                                                                                        }
                                                                                    })),
                                                                            )
                                                                            // Outgoing: Edit & Delete Actions
                                                                            .children(if is_from_me {
                                                                                vec![
                                                                                    div()
                                                                                        .cursor_pointer()
                                                                                        .p_1()
                                                                                        .rounded_md()
                                                                                        .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                                                        .child(svg().data(EDIT_SVG).size(px(13.0)).text_color(rgb(0xf97316)))
                                                                                        .on_mouse_down(MouseButton::Left, cx.listener({
                                                                                            let m = msg.clone();
                                                                                            move |this, _, window, cx| {
                                                                                                this.start_edit(m.clone(), window, cx);
                                                                                            }
                                                                                        }))
                                                                                        .into_any_element(),
                                                                                    div()
                                                                                        .cursor_pointer()
                                                                                        .p_1()
                                                                                        .rounded_md()
                                                                                        .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                                                        .child(svg().data(TRASH_SVG).size(px(13.0)).text_color(rgb(0xef4444)))
                                                                                        .on_mouse_down(MouseButton::Left, cx.listener({
                                                                                            let cid = bubble_chat_id.clone();
                                                                                            let mid = msg.id.clone();
                                                                                            move |this, _, _, cx| {
                                                                                                this.delete_message(cid.clone(), mid.clone(), cx);
                                                                                            }
                                                                                        }))
                                                                                        .into_any_element(),
                                                                                ]
                                                                            } else {
                                                                                vec![]
                                                                            }),
                                                                    )
                                                                    // Right side: Timestamp & Checkmarks
                                                                    .child(
                                                                        h_flex()
                                                                            .items_center()
                                                                            .gap_1()
                                                                            .child(
                                                                                div()
                                                                                    .text_xs()
                                                                                    .text_color(muted_text)
                                                                                    .child(time_str),
                                                                            )
                                                                            .children(if is_from_me {
                                                                                match ticks {
                                                                                    MessageTicks::Read => Some(
                                                                                        svg()
                                                                                            .data(CHECK_CHECK_SVG)
                                                                                            .size(px(14.0))
                                                                                            .text_color(rgb(0x53bdeb)),
                                                                                    ),
                                                                                    MessageTicks::Delivered => Some(
                                                                                        svg()
                                                                                            .data(CHECK_CHECK_SVG)
                                                                                            .size(px(14.0))
                                                                                            .text_color(muted_text),
                                                                                    ),
                                                                                    MessageTicks::Sent => Some(
                                                                                        svg()
                                                                                            .data(CHECK_SVG)
                                                                                            .size(px(14.0))
                                                                                            .text_color(muted_text),
                                                                                    ),
                                                                                    MessageTicks::None => None,
                                                                                }
                                                                            } else {
                                                                                None
                                                                            }),
                                                                    ),
                                                            ),
                                                    )
                                                    .into_any_element()
                                                })
                                                .collect()
                                        }),
                                )
                                // Bottom Compose Container (Banner + Input Field + Buttons)
                                .child(
                                    v_flex()
                                        .border_t_1()
                                        .border_color(border_color)
                                        .bg(card_bg)
                                        // Reply Preview Banner
                                        .children(if let Some(ref r_msg) = self.reply_to {
                                            let r_sender = if r_msg.from == "me" { "You" } else { r_msg.sender_name.as_deref().unwrap_or("Sender") };
                                            let r_content = MessageBubbleHelper::decode_content(&r_msg.content);
                                            let r_first = r_content.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
                                            Some(
                                                h_flex()
                                                    .mx_3()
                                                    .mt_2p5()
                                                    .px_3()
                                                    .py_2()
                                                    .bg(theme.muted.opacity(0.4))
                                                    .border_l_4()
                                                    .border_color(primary_color)
                                                    .rounded_r_xl()
                                                    .items_center()
                                                    .justify_between()
                                                    .child(
                                                        v_flex()
                                                            .flex_1()
                                                            .min_w(px(0.0))
                                                            .overflow_hidden()
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .font_weight(FontWeight::BOLD)
                                                                    .text_color(primary_color)
                                                                    .child(format!("Replying to {}", r_sender)),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(muted_text)
                                                                    .overflow_hidden()
                                                                    .child(r_first.to_string()),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .cursor_pointer()
                                                            .p_1()
                                                            .rounded_full()
                                                            .hover(|s| s.bg(theme.muted.opacity(0.6)))
                                                            .child(svg().data(X_SVG).size(px(14.0)).text_color(muted_text))
                                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                                this.cancel_reply(cx);
                                                            })),
                                                    ),
                                            )
                                        } else if let Some(ref e_msg) = self.editing_message {
                                            let e_content = MessageBubbleHelper::decode_content(&e_msg.content);
                                            let e_first = e_content.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
                                            Some(
                                                h_flex()
                                                    .mx_3()
                                                    .mt_2p5()
                                                    .px_3()
                                                    .py_2()
                                                    .bg(theme.muted.opacity(0.4))
                                                    .border_l_4()
                                                    .border_color(rgb(0xf97316))
                                                    .rounded_r_xl()
                                                    .items_center()
                                                    .justify_between()
                                                    .child(
                                                        v_flex()
                                                            .flex_1()
                                                            .min_w(px(0.0))
                                                            .overflow_hidden()
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .font_weight(FontWeight::BOLD)
                                                                    .text_color(rgb(0xf97316))
                                                                    .child("Editing message"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(muted_text)
                                                                    .overflow_hidden()
                                                                    .child(e_first.to_string()),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .cursor_pointer()
                                                            .p_1()
                                                            .rounded_full()
                                                            .hover(|s| s.bg(theme.muted.opacity(0.6)))
                                                            .child(svg().data(X_SVG).size(px(14.0)).text_color(muted_text))
                                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                                this.cancel_edit(cx);
                                                            })),
                                                    ),
                                            )
                                        } else {
                                            None
                                        })
                                        // Input Row
                                        .child(
                                            h_flex()
                                                .p_3()
                                                .items_center()
                                                .gap_2()
                                                // Plus / Attachment icon
                                                .child(
                                                    div()
                                                        .p_2()
                                                        .rounded_full()
                                                        .cursor_pointer()
                                                        .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                        .child(svg().data(PLUS_SVG).size(px(18.0)).text_color(muted_text)),
                                                )
                                                // Smile icon
                                                .child(
                                                    div()
                                                        .p_2()
                                                        .rounded_full()
                                                        .cursor_pointer()
                                                        .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                        .child(svg().data(SMILE_SVG).size(px(18.0)).text_color(muted_text)),
                                                )
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
                                                                "escape" => {
                                                                    if this.reply_to.is_some() {
                                                                        this.cancel_reply(cx);
                                                                    } else if this.editing_message.is_some() {
                                                                        this.cancel_edit(cx);
                                                                    }
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
                                                                    if self.editing_message.is_some() {
                                                                        "Edit message...".to_string()
                                                                    } else if self.reply_to.is_some() {
                                                                        "Type reply...".to_string()
                                                                    } else {
                                                                        "Type a message...".to_string()
                                                                    }
                                                                } else {
                                                                    self.compose_text.clone()
                                                                }),
                                                        ),
                                                )
                                                // Send Button
                                                .child(
                                                    h_flex()
                                                        .cursor_pointer()
                                                        .px_4()
                                                        .py_2p5()
                                                        .rounded_xl()
                                                        .bg(primary_color)
                                                        .hover(|s| s.opacity(0.85))
                                                        .items_center()
                                                        .gap_1p5()
                                                        .child(svg().data(SEND_SVG).size(px(15.0)).text_color(rgb(0xffffff)))
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(rgb(0xffffff))
                                                                .child(if self.is_sending { "Sending..." } else { "Send" }),
                                                        )
                                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                            this.send_current_message(cx);
                                                        })),
                                                ),
                                        ),
                                ),
                        )
                    } else {
                        // Empty state when no chat selected (Zero emoji, Lucide SVG)
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
                                        .child(svg().data(MESSAGE_SQUARE_SVG).size(px(32.0)).text_color(primary_color)),
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
            // Floating Toast Notification
            .children(if let Some((ref msg, is_error)) = self.toast_message {
                Some(
                    div()
                        .absolute()
                        .bottom(px(80.0))
                        .right(px(24.0))
                        .px_4()
                        .py_2()
                        .rounded_xl()
                        .bg(if is_error { rgb(0xef4444) } else { rgb(0x00a884) })
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(0xffffff))
                        .shadow_lg()
                        .child(msg.clone()),
                )
            } else {
                None
            })
    }
}

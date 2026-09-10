//! Chat view matching Web ChatPage, ChatSidebar, and ChatArea with 1:1 parity.

use std::collections::HashSet;
use gpui::*;
use gpui_component::{h_flex, v_flex, Icon, IconName};
use wabot_backend_client::client::HttpClient;
use wabot_backend_client::dto::{Chat, Message};

use crate::components::message_bubble::{MessageBubbleHelper, MessageTicks};
use crate::icons::*;
use crate::state::auth::AuthState;
use crate::state::chat::ChatStore;
use crate::theme::manager::{ActiveTokens, AppThemeExt};
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

fn urlencode(s: &str) -> String {
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

fn resolve_avatar_url(avatar: &str, chat_id: &str, base_url: &str) -> String {
    let avatar = avatar.trim();
    if !avatar.is_empty() && !avatar.starts_with("data:") {
        if avatar.starts_with("http://") || avatar.starts_with("https://") {
            return avatar.to_string();
        }
        let clean_base = base_url.trim_end_matches('/');
        let clean_avatar = avatar.trim_start_matches('/');
        if clean_avatar.starts_with("api/") {
            let root = clean_base.strip_suffix("/api").unwrap_or(clean_base);
            return format!("{root}/{clean_avatar}");
        }
        return format!("{clean_base}/{clean_avatar}");
    }
    let clean_base = base_url.trim_end_matches('/');
    format!("{clean_base}/avatar/{}", urlencode(chat_id))
}

fn resolve_media_url(media_url: &str, base_url: &str) -> String {
    let media_url = media_url.trim();
    if media_url.starts_with("http://") || media_url.starts_with("https://") {
        return media_url.to_string();
    }
    let clean_base = base_url.trim_end_matches('/');
    let clean_url = media_url.trim_start_matches('/');
    if clean_url.starts_with("api/") {
        let root = clean_base.strip_suffix("/api").unwrap_or(clean_base);
        format!("{root}/{clean_url}")
    } else {
        format!("{clean_base}/{clean_url}")
    }
}

fn render_avatar(
    chat_id: &str,
    chat_name: &str,
    avatar_field: &str,
    is_group: bool,
    size_px: f32,
    base_url: &str,
    theme: &ActiveTokens,
) -> AnyElement {
    let initial = chat_name.chars().next().unwrap_or('?').to_uppercase().to_string();
    let avatar_url = resolve_avatar_url(avatar_field, chat_id, base_url);
    let color = avatar_color_for(chat_id);

    let fallback_init = initial.clone();
    let fallback_theme = *theme;
    let fallback_color = color;
    let fallback_group = is_group;
    let fallback_size = size_px;

    let make_fallback = move || -> AnyElement {
        if fallback_group {
            div()
                .w(px(fallback_size))
                .h(px(fallback_size))
                .rounded_full()
                .bg(fallback_theme.primary.opacity(0.15))
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .child(svg().data(USERS_SVG).size(px(fallback_size * 0.46)).text_color(fallback_theme.primary))
                .into_any_element()
        } else {
            div()
                .w(px(fallback_size))
                .h(px(fallback_size))
                .rounded_full()
                .bg(fallback_color.opacity(0.18))
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .text_size(px(fallback_size * 0.38))
                .font_weight(FontWeight::BOLD)
                .text_color(fallback_color)
                .child(fallback_init.clone())
                .into_any_element()
        }
    };

    let loading_init = initial.clone();
    let loading_theme = *theme;
    let loading_color = color;
    let loading_group = is_group;
    let loading_size = size_px;

    let make_loading = move || -> AnyElement {
        if loading_group {
            div()
                .w(px(loading_size))
                .h(px(loading_size))
                .rounded_full()
                .bg(loading_theme.primary.opacity(0.15))
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .child(svg().data(USERS_SVG).size(px(loading_size * 0.46)).text_color(loading_theme.primary))
                .into_any_element()
        } else {
            div()
                .w(px(loading_size))
                .h(px(loading_size))
                .rounded_full()
                .bg(loading_color.opacity(0.18))
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .text_size(px(loading_size * 0.38))
                .font_weight(FontWeight::BOLD)
                .text_color(loading_color)
                .child(loading_init.clone())
                .into_any_element()
        }
    };

    img(avatar_url)
        .w(px(size_px))
        .h(px(size_px))
        .rounded_full()
        .flex_shrink_0()
        .object_fit(ObjectFit::Cover)
        .with_fallback(make_fallback)
        .with_loading(make_loading)
        .into_any_element()
}

/// Filter chips below search in sidebar (matching Web / WhatsApp)
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ChatFilter {
    #[default]
    All,
    Unread,
    Groups,
    Contacts,
}

/// Tabs for Chat Info Sheet
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum InfoSheetTab {
    #[default]
    Media,
    Docs,
    Links,
    Members,
}

/// Context menu popup state when right-clicking a message bubble
#[derive(Clone, Debug)]
pub struct MessageContextMenu {
    pub msg: Message,
    pub is_from_me: bool,
    pub position: Point<Pixels>,
}

/// Context menu popup state when right-clicking a sidebar chat item
#[derive(Clone, Debug)]
pub struct ChatContextMenu {
    pub chat_id: String,
    pub chat_name: String,
    pub is_pinned: bool,
    pub is_archived: bool,
    pub is_muted: bool,
    pub position: Point<Pixels>,
    pub show_mute_submenu: bool,
}

/// Pre-computed, cached renderable message to guarantee silky smooth 60fps scrolling
#[derive(Clone, Debug)]
pub struct RenderMessage {
    pub msg: Message,
    pub is_from_me: bool,
    pub content: String,
    pub time_str: String,
    pub ticks: MessageTicks,
    pub quoted: Option<(String, String)>, // (sender_name, first_line)
}

pub struct ChatView {
    pub search_query: String,
    pub search_focus_handle: FocusHandle,
    pub filter: ChatFilter,
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

    // Cached messages for 60fps scrolling
    pub cached_chat_id: Option<String>,
    pub cached_render_messages: Vec<RenderMessage>,

    // Context menu popup on message right click
    pub context_menu: Option<MessageContextMenu>,

    // Context menu popup on sidebar chat right click
    pub chat_context_menu: Option<ChatContextMenu>,

    // Chat Info Sheet drawer (Right side)
    pub is_info_sheet_open: bool,
    pub info_sheet_tab: InfoSheetTab,
    pub info_media: Vec<Message>,
    pub info_docs: Vec<Message>,
    pub info_links: Vec<Message>,
    pub is_loading_info: bool,

    // New Group modal
    pub is_new_group_open: bool,
    pub new_group_name: String,
    pub new_group_selected: HashSet<String>,
    pub new_group_focus: FocusHandle,
    pub is_creating_group: bool,

    // Full-screen image preview
    pub preview_image_url: Option<String>,

    // Join Group modal
    pub is_join_group_open: bool,
    pub join_group_link: String,
    pub join_group_focus: FocusHandle,
    pub is_joining_group: bool,
}

impl ChatView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            search_query: String::new(),
            search_focus_handle: cx.focus_handle(),
            filter: ChatFilter::All,
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

            cached_chat_id: None,
            cached_render_messages: Vec::new(),

            context_menu: None,
            chat_context_menu: None,

            is_info_sheet_open: false,
            info_sheet_tab: InfoSheetTab::Media,
            info_media: Vec::new(),
            info_docs: Vec::new(),
            info_links: Vec::new(),
            is_loading_info: false,

            is_new_group_open: false,
            new_group_name: String::new(),
            new_group_selected: HashSet::new(),
            new_group_focus: cx.focus_handle(),
            is_creating_group: false,

            preview_image_url: None,

            is_join_group_open: false,
            join_group_link: String::new(),
            join_group_focus: cx.focus_handle(),
            is_joining_group: false,
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
        self.context_menu = None;
        self.chat_context_menu = None;
        self.is_info_sheet_open = false;

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

    /// Rebuild cached render messages for 60fps smooth scrolling
    fn rebuild_message_cache(&mut self, chat_id: &str, msgs: &[Message]) {
        let mut lookup = std::collections::HashMap::new();
        for m in msgs {
            let sender = if m.from == "me" {
                "You".to_string()
            } else {
                m.sender_name.clone().unwrap_or_else(|| "Sender".to_string())
            };
            let decoded = MessageBubbleHelper::decode_content(&m.content);
            let first_line = decoded.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string();
            lookup.insert(m.id.clone(), (sender, first_line));
        }

        self.cached_chat_id = Some(chat_id.to_string());
        self.cached_render_messages = msgs
            .iter()
            .map(|m| {
                let is_from_me = MessageBubbleHelper::is_from_me(m, "me");
                let content = MessageBubbleHelper::decode_content(&m.content);
                let time_str = Self::format_chat_time(m.timestamp);
                let ticks = MessageBubbleHelper::ticks(m, is_from_me);
                let quoted = m.reply_to_id.as_ref().and_then(|rid| lookup.get(rid).cloned());

                RenderMessage {
                    msg: m.clone(),
                    is_from_me,
                    content,
                    time_str,
                    ticks,
                    quoted,
                }
            })
            .collect();
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
                                this.rebuild_message_cache(&cid, &msgs);
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

    /// Toggle Chat Info Sheet drawer
    pub fn toggle_info_sheet(&mut self, cx: &mut Context<Self>) {
        self.is_info_sheet_open = !self.is_info_sheet_open;
        if self.is_info_sheet_open {
            self.load_info_media(cx);
            self.load_info_docs(cx);
            self.load_info_links(cx);
        }
        cx.notify();
    }

    /// Load media for active chat info sheet
    pub fn load_info_media(&mut self, cx: &mut Context<Self>) {
        let chat_id = match &self.selected_chat_id {
            Some(id) => id.clone(),
            None => return,
        };
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
                let res = TOKIO_RT.spawn(async move { client.get_chat_media(&chat_id, Some(30), None).await }).await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            if let Ok(Ok(media)) = res {
                                this.info_media = media;
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Load docs for active chat info sheet
    pub fn load_info_docs(&mut self, cx: &mut Context<Self>) {
        let chat_id = match &self.selected_chat_id {
            Some(id) => id.clone(),
            None => return,
        };
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
                let res = TOKIO_RT.spawn(async move { client.get_chat_docs(&chat_id, Some(30), None).await }).await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            if let Ok(Ok(docs)) = res {
                                this.info_docs = docs;
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Load links for active chat info sheet
    pub fn load_info_links(&mut self, cx: &mut Context<Self>) {
        let chat_id = match &self.selected_chat_id {
            Some(id) => id.clone(),
            None => return,
        };
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
                let res = TOKIO_RT.spawn(async move { client.get_chat_links(&chat_id, Some(30), None).await }).await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            if let Ok(Ok(links)) = res {
                                this.info_links = links;
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// React to message with emoji
    pub fn react_message(&mut self, chat_id: String, msg_id: String, emoji: String, cx: &mut Context<Self>) {
        self.context_menu = None;
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        let cid = chat_id.clone();
        let mid = msg_id.clone();
        let em = emoji.clone();

        cx.spawn(move |_this: WeakEntity<Self>, _cx: &mut AsyncApp| {
            async move {
                let client = HttpClient::new(&base_url);
                let _ = TOKIO_RT
                    .spawn(async move { client.react_to_message(&cid, &mid, &em, None).await })
                    .await;
            }
        })
        .detach();

        self.toast_message = Some((format!("Reaksi {} terkirim", emoji), false));
        cx.notify();
    }

    /// Start replying to a message
    pub fn start_reply(&mut self, msg: Message, window: &mut Window, cx: &mut Context<Self>) {
        self.reply_to = Some(msg);
        self.editing_message = None;
        self.context_menu = None;
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
        self.context_menu = None;
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
        self.context_menu = None;
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.toast_message = Some(("Pesan disalin ke clipboard".into(), false));
        cx.notify();
    }

    /// Delete a message
    pub fn delete_message(&mut self, chat_id: String, msg_id: String, cx: &mut Context<Self>) {
        self.context_menu = None;
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        if cx.has_global::<ChatStore>() {
            ChatStore::global_mut(cx).delete_message(&chat_id, &msg_id);
        }
        self.cached_render_messages.retain(|m| m.msg.id != msg_id);
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
            if let Some(rm) = self.cached_render_messages.iter_mut().find(|m| m.msg.id == edit_id) {
                rm.content = sent_text.clone();
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
            store.upsert_message(&chat_id, opt_msg.clone());
            if let Some(c) = store.chats.iter_mut().find(|c| c.id == chat_id) {
                c.last_msg = text.clone();
                c.last_time = now_ms;
            }
        }

        // Optimistically add to cached render messages
        self.cached_render_messages.push(RenderMessage {
            msg: opt_msg.clone(),
            is_from_me: true,
            content: text.clone(),
            time_str: Self::format_chat_time(now_ms),
            ticks: MessageTicks::Sent,
            quoted: None,
        });

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
                                    if let Some(rm) = this.cached_render_messages.iter_mut().find(|m| m.msg.id == opt_id_clone) {
                                        rm.msg.id = id_res.id.clone();
                                        rm.msg.status = "sent".to_string();
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

    /// Set mute mode for a chat ("off", "8h", "1w", "forever")
    pub fn set_mute(&mut self, chat_id: String, mode: String, cx: &mut Context<Self>) {
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        let target_chat = chat_id.clone();
        let target_mode = mode.clone();

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move { client.mute_chat(&target_chat, &target_mode).await })
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

    /// Create new group
    pub fn submit_create_group(&mut self, cx: &mut Context<Self>) {
        let name = self.new_group_name.trim().to_string();
        if name.is_empty() {
            self.toast_message = Some(("Nama grup wajib diisi".into(), true));
            cx.notify();
            return;
        }

        let participants: Vec<String> = self.new_group_selected.iter().cloned().collect();
        self.is_creating_group = true;

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
                let res = TOKIO_RT
                    .spawn(async move { client.create_group(&name, &participants).await })
                    .await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_creating_group = false;
                            match res {
                                Ok(Ok(_)) => {
                                    this.is_new_group_open = false;
                                    this.new_group_name.clear();
                                    this.new_group_selected.clear();
                                    this.toast_message = Some(("Grup berhasil dibuat".into(), false));
                                    this.load_chats(cx);
                                }
                                _ => {
                                    this.toast_message = Some(("Gagal membuat grup".into(), true));
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

    /// Join group by link
    pub fn submit_join_group(&mut self, cx: &mut Context<Self>) {
        let link = self.join_group_link.trim().to_string();
        if link.is_empty() {
            self.toast_message = Some(("Tautan undangan wajib diisi".into(), true));
            cx.notify();
            return;
        }

        self.is_joining_group = true;
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
                let res = TOKIO_RT
                    .spawn(async move { client.join_group(&link).await })
                    .await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_joining_group = false;
                            match res {
                                Ok(Ok(_)) => {
                                    this.is_join_group_open = false;
                                    this.join_group_link.clear();
                                    this.toast_message = Some(("Berhasil bergabung ke grup".into(), false));
                                    this.load_chats(cx);
                                }
                                _ => {
                                    this.toast_message = Some(("Gagal bergabung ke grup".into(), true));
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
        let period = if hours >= 12 { "PM" } else { "AM" };
        let h12 = if hours % 12 == 0 { 12 } else { hours % 12 };
        format!("{:02}:{:02} {}", h12, mins, period)
    }
}

impl Render for ChatView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.app_theme();
        let border_color = theme.border;
        let bg_color = theme.background;
        let card_bg = theme.card;
        let primary_color = theme.primary;
        let text_color = theme.foreground;
        let muted_text = theme.muted_foreground;

        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        let all_chats: Vec<Chat> = if cx.has_global::<ChatStore>() {
            ChatStore::global(cx).chats.clone()
        } else {
            Vec::new()
        };

        // Filter and sort chats
        let query = self.search_query.trim().to_lowercase();
        let archived_mode = self.archived_mode;
        let current_filter = self.filter;

        let mut filtered_chats: Vec<Chat> = all_chats
            .iter()
            .filter(|c| c.archived == archived_mode)
            .filter(|c| match current_filter {
                ChatFilter::All => true,
                ChatFilter::Unread => c.unread > 0,
                ChatFilter::Groups => c.is_group,
                ChatFilter::Contacts => !c.is_group,
            })
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

        let selected_chat_id = self.selected_chat_id.clone();
        let is_loading_chats = self.is_loading_chats;
        let is_loading_messages = self.is_loading_messages;

        // Ensure cached messages match currently selected chat
        if let Some(ref sc) = selected_chat {
            if self.cached_chat_id.as_deref() != Some(&sc.id) {
                if cx.has_global::<ChatStore>() {
                    if let Some(entry) = ChatStore::global(cx).messages_by_chat.get(&sc.id) {
                        self.rebuild_message_cache(&sc.id, &entry.messages);
                    }
                }
            }
        }

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
                    .border_color(border_color.opacity(0.4))
                    .bg(bg_color)
                    .flex_shrink_0()
                    // Sidebar Header
                    .child(
                        v_flex()
                            .p_4()
                            .gap_3()
                            // Top Row: Title + Action Icons
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
                                                    .text_2xl()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(text_color)
                                                    .child(if archived_mode { "Archived" } else { "Messages" }),
                                            ),
                                    )
                                    // Action buttons: New Group, Join Group (matching Web)
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap_1()
                                            // New Group Button
                                            .child(
                                                div()
                                                    .cursor_pointer()
                                                    .p_2()
                                                    .rounded_full()
                                                    .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                    .child(svg().data(USERS_SVG).size(px(18.0)).text_color(muted_text))
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                        this.is_new_group_open = true;
                                                        cx.notify();
                                                    })),
                                            )
                                            // Join Group via Link Button
                                            .child(
                                                div()
                                                    .cursor_pointer()
                                                    .p_2()
                                                    .rounded_full()
                                                    .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                    .child(svg().data(LINK_SVG).size(px(18.0)).text_color(muted_text))
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                        this.is_join_group_open = true;
                                                        cx.notify();
                                                    })),
                                            ),
                                    ),
                            )
                            // Search Box
                            .child(
                                h_flex()
                                    .px_3()
                                    .py_2()
                                    .bg(theme.muted.opacity(0.5))
                                    .rounded_xl()
                                    .border_1()
                                    .border_color(border_color.opacity(0.3))
                                    .items_center()
                                    .gap_2()
                                    .cursor_text()
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
                                            .cursor_text()
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
                                        let is_muted = chat.mute_mode != "off" && !chat.mute_mode.is_empty();

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
                                                        this.chat_context_menu = None;
                                                        this.select_chat(cid.clone(), window, cx);
                                                    }
                                                }),
                                            )
                                            .on_mouse_down(
                                                MouseButton::Right,
                                                cx.listener({
                                                    let c_id = chat_id.clone();
                                                    let c_name = chat.name.clone();
                                                    let pinned = is_pinned;
                                                    let archived = chat.archived;
                                                    let muted = is_muted;
                                                    move |this, ev: &MouseDownEvent, _, cx| {
                                                        this.context_menu = None;
                                                        this.chat_context_menu = Some(ChatContextMenu {
                                                            chat_id: c_id.clone(),
                                                            chat_name: c_name.clone(),
                                                            is_pinned: pinned,
                                                            is_archived: archived,
                                                            is_muted: muted,
                                                            position: ev.position,
                                                            show_mute_submenu: false,
                                                        });
                                                        cx.notify();
                                                    }
                                                }),
                                            )
                                            // Selected Indicator Bar
                                            .children(if is_selected {
                                                Some(
                                                    div()
                                                        .absolute()
                                                        .left(px(0.0))
                                                        .top(px(16.0))
                                                        .bottom(px(16.0))
                                                        .w(px(3.5))
                                                        .bg(primary_color)
                                                        .rounded_r_full(),
                                                )
                                            } else {
                                                None
                                            })
                                            // Real Profile Picture or Fallback Initial Avatar
                                            .child(render_avatar(&chat.id, &chat.name, &chat.avatar, chat.is_group, 48.0, &base_url, &theme))
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
                                                            })
                                                            .children(if is_muted {
                                                                Some(
                                                                    svg()
                                                                        .data(VOLUME_X_SVG)
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
                    .min_w(px(0.0))
                    .h_full()
                    .overflow_hidden()
                    .bg(bg_color)
                    .children(if let Some(ref active_chat) = selected_chat {
                        let chat_name = active_chat.name.clone();
                        let chat_jid = active_chat.id.clone();

                        Some(
                            v_flex()
                                .size_full()
                                // Chat Header (Clicking opens Info Sheet)
                                .child(
                                    h_flex()
                                        .px_5()
                                        .py_3()
                                        .border_b_1()
                                        .border_color(border_color.opacity(0.4))
                                        .bg(bg_color)
                                        .items_center()
                                        .justify_between()
                                        // Left Profile Area (Clicking opens Info Sheet)
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .gap_3()
                                                .cursor_pointer()
                                                .p_1()
                                                .rounded_lg()
                                                .hover(|s| s.bg(theme.muted.opacity(0.4)))
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.toggle_info_sheet(cx);
                                                }))
                                                .child(render_avatar(&active_chat.id, &active_chat.name, &active_chat.avatar, active_chat.is_group, 40.0, &base_url, &theme))
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
                                        // Header Action Icons matching Web: Phone, Video, Search, More
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .gap_1()
                                                .child(
                                                    div()
                                                        .cursor_pointer()
                                                        .p_2()
                                                        .rounded_full()
                                                        .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                        .child(svg().data(PHONE_SVG).size(px(18.0)).text_color(muted_text))
                                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                            this.toast_message = Some(("Voice call is handled via WhatsApp Web".into(), false));
                                                            cx.notify();
                                                        })),
                                                )
                                                .child(
                                                    div()
                                                        .cursor_pointer()
                                                        .p_2()
                                                        .rounded_full()
                                                        .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                        .child(svg().data(VIDEO_SVG).size(px(18.0)).text_color(muted_text))
                                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                            this.toast_message = Some(("Video call is handled via WhatsApp Web".into(), false));
                                                            cx.notify();
                                                        })),
                                                )
                                                .child(
                                                    div()
                                                        .cursor_pointer()
                                                        .p_2()
                                                        .rounded_full()
                                                        .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                        .child(svg().data(SEARCH_SVG).size(px(18.0)).text_color(muted_text))
                                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                                            this.search_focus_handle.focus(window, cx);
                                                            cx.notify();
                                                        })),
                                                )
                                                .child(
                                                    div()
                                                        .cursor_pointer()
                                                        .p_2()
                                                        .rounded_full()
                                                        .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                        .child(svg().data(MORE_VERTICAL_SVG).size(px(18.0)).text_color(muted_text))
                                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                            this.toggle_info_sheet(cx);
                                                        })),
                                                ),
                                        ),
                                )
                                // Messages History View (Optimized for 60fps scrolling)
                                .child(
                                    v_flex()
                                        .id("messages-scroll")
                                        .flex_1()
                                        .overflow_y_scroll()
                                        .p_6()
                                        .gap_3()
                                        .children(if is_loading_messages && self.cached_render_messages.is_empty() {
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
                                        } else if self.cached_render_messages.is_empty() {
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
                                            let mut elements = Vec::new();
                                            // Date separator badge matching Web: "TODAY"
                                            elements.push(
                                                h_flex()
                                                    .w_full()
                                                    .justify_center()
                                                    .my_2()
                                                    .child(
                                                        div()
                                                            .px_3()
                                                            .py_1()
                                                            .rounded_full()
                                                            .bg(theme.muted.opacity(0.4))
                                                            .text_xs()
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(muted_text)
                                                            .child("TODAY"),
                                                    )
                                                    .into_any_element(),
                                            );

                                            for r_msg in &self.cached_render_messages {
                                                let is_from_me = r_msg.is_from_me;
                                                let content = r_msg.content.clone();
                                                let time_str = r_msg.time_str.clone();
                                                let ticks = r_msg.ticks;
                                                let quoted = r_msg.quoted.clone();
                                                let msg_clone = r_msg.msg.clone();

                                                let row = if is_from_me {
                                                    h_flex().w_full().justify_end()
                                                } else {
                                                    h_flex().w_full().justify_start()
                                                };

                                                let msg_type = r_msg.msg.message_type.as_str();
                                                let is_image = msg_type == "image" || (r_msg.msg.media_url.is_some() && (content == "[Image]" || content.is_empty()));
                                                let is_sticker = msg_type == "sticker" || (r_msg.msg.media_url.is_some() && content == "[Sticker]");
                                                let is_video = msg_type == "video" || (r_msg.msg.media_url.is_some() && content == "[Video]");

                                                let is_dark_bg = theme.background.l < 0.5;
                                                let (bubble_bg, bubble_text) = if is_sticker {
                                                    (rgba(0x00000000), if is_dark_bg { rgb(0xe9edef) } else { rgb(0x303030) })
                                                } else if is_dark_bg {
                                                    if is_from_me {
                                                        (rgb(0x005c4b), rgb(0xe9edef))
                                                    } else {
                                                        (rgb(0x202c33), rgb(0xe9edef))
                                                    }
                                                } else {
                                                    if is_from_me {
                                                        (rgb(0xdcf8c6), rgb(0x303030))
                                                    } else {
                                                        (rgb(0xffffff), rgb(0x303030))
                                                    }
                                                };

                                                let bubble = v_flex()
                                                    .max_w(px(520.0))
                                                    .px_3p5()
                                                    .py_2()
                                                    .rounded_2xl()
                                                    .bg(bubble_bg)
                                                    .shadow_sm()
                                                    .gap_1()
                                                    // Right-click opens Context Menu popover!
                                                    .on_mouse_down(
                                                        MouseButton::Right,
                                                        cx.listener({
                                                            let m = msg_clone.clone();
                                                            let me = is_from_me;
                                                            move |this, ev: &MouseDownEvent, _, cx| {
                                                                this.context_menu = Some(MessageContextMenu {
                                                                    msg: m.clone(),
                                                                    is_from_me: me,
                                                                    position: ev.position,
                                                                });
                                                                cx.notify();
                                                            }
                                                        }),
                                                    )
                                                    // Sender name in group chat
                                                    .children(if !is_from_me && r_msg.msg.sender_name.is_some() {
                                                        Some(
                                                            div()
                                                                .text_xs()
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(primary_color)
                                                                .child(r_msg.msg.sender_name.clone().unwrap()),
                                                        )
                                                    } else {
                                                        None
                                                    })
                                                    // Quoted reply box matching Web
                                                    .children(if let Some((q_sender, q_line)) = quoted {
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
                                                    // Media attachment: Image or Sticker
                                                    .children(if is_image {
                                                        if let Some(ref m_url) = r_msg.msg.media_url {
                                                            let full_url = resolve_media_url(m_url, &base_url);
                                                            let click_url = full_url.clone();
                                                            Some(
                                                                div()
                                                                    .mb_1p5()
                                                                    .max_w(px(340.0))
                                                                    .max_h(px(360.0))
                                                                    .rounded_xl()
                                                                    .overflow_hidden()
                                                                    .cursor_pointer()
                                                                    .on_mouse_down(MouseButton::Left, cx.listener({
                                                                        let u = click_url.clone();
                                                                        move |this, _, _, cx| {
                                                                            this.preview_image_url = Some(u.clone());
                                                                            cx.notify();
                                                                        }
                                                                    }))
                                                                    .child(
                                                                        img(full_url)
                                                                            .max_w(px(340.0))
                                                                            .max_h(px(360.0))
                                                                            .rounded_xl()
                                                                            .object_fit(ObjectFit::Contain)
                                                                    )
                                                                    .into_any_element()
                                                            )
                                                        } else {
                                                            None
                                                        }
                                                    } else if is_sticker {
                                                        if let Some(ref m_url) = r_msg.msg.media_url {
                                                            let full_url = resolve_media_url(m_url, &base_url);
                                                            Some(
                                                                div()
                                                                    .w(px(160.0))
                                                                    .h(px(160.0))
                                                                    .cursor_pointer()
                                                                    .child(
                                                                        img(full_url)
                                                                            .w(px(160.0))
                                                                            .h(px(160.0))
                                                                            .object_fit(ObjectFit::Contain)
                                                                    )
                                                                    .into_any_element()
                                                            )
                                                        } else {
                                                            None
                                                        }
                                                    } else {
                                                        None
                                                    })
                                                    // Message Body Text (hide placeholder if it's media without caption)
                                                    .children({
                                                        let is_media_placeholder = (is_image && (content == "[Image]" || content.is_empty()))
                                                            || (is_sticker && (content == "[Sticker]" || content.is_empty()))
                                                            || (is_video && (content == "[Video]" || content.is_empty()));
                                                        if !content.is_empty() && !is_media_placeholder {
                                                            Some(
                                                                div()
                                                                    .text_sm()
                                                                    .text_color(bubble_text)
                                                                    .child(content)
                                                            )
                                                        } else {
                                                            None
                                                        }
                                                    })
                                                    // Bottom Right Timestamp & Checkmarks
                                                    .child(
                                                        h_flex()
                                                            .w_full()
                                                            .justify_end()
                                                            .items_center()
                                                            .gap_1()
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(bubble_text.opacity(0.6))
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
                                                                            .text_color(bubble_text.opacity(0.6)),
                                                                    ),
                                                                    MessageTicks::Sent => Some(
                                                                        svg()
                                                                            .data(CHECK_SVG)
                                                                            .size(px(14.0))
                                                                            .text_color(bubble_text.opacity(0.6)),
                                                                    ),
                                                                    MessageTicks::None => None,
                                                                }
                                                            } else {
                                                                None
                                                            }),
                                                    );

                                                elements.push(row.child(bubble).into_any_element());
                                            }

                                            elements
                                        }),
                                )
                                // Bottom Compose Container (Banner + Input Field + Integrated Send Icon)
                                .child(
                                    v_flex()
                                        .border_t_1()
                                        .border_color(border_color.opacity(0.4))
                                        .bg(bg_color)
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
                                        // Input Row matching Web
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
                                                // Mic icon
                                                .child(
                                                    div()
                                                        .p_2()
                                                        .rounded_full()
                                                        .cursor_pointer()
                                                        .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                        .child(svg().data(MIC_SVG).size(px(18.0)).text_color(muted_text)),
                                                )
                                                // Text Input Field with Integrated Send Button
                                                .child(
                                                    h_flex()
                                                        .flex_1()
                                                        .px_4()
                                                        .py_2()
                                                        .rounded_2xl()
                                                        .border_1()
                                                        .border_color(border_color.opacity(0.2))
                                                        .bg(theme.muted.opacity(0.5))
                                                        .items_center()
                                                        .gap_2()
                                                        .cursor_text()
                                                        .track_focus(&self.compose_focus_handle)
                                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                                            this.compose_focus_handle.focus(window, cx);
                                                        }))
                                                        .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
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
                                                                .cursor_text()
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
                                                        )
                                                        // Send Button: Integrated inside input bar on the right
                                                        .child({
                                                            let has_text = !self.compose_text.trim().is_empty();
                                                            div()
                                                                .cursor_pointer()
                                                                .p_1p5()
                                                                .rounded_xl()
                                                                .flex()
                                                                .items_center()
                                                                .justify_center()
                                                                .flex_shrink_0()
                                                                .bg(if has_text { primary_color } else { rgba(0x00000000).into() })
                                                                .hover(|h| {
                                                                    if has_text {
                                                                        h.opacity(0.85)
                                                                    } else {
                                                                        h
                                                                    }
                                                                })
                                                                .child(
                                                                    svg()
                                                                        .data(SEND_SVG)
                                                                        .size(px(16.0))
                                                                        .text_color(if has_text {
                                                                            theme.primary_foreground
                                                                        } else {
                                                                            muted_text.opacity(0.6)
                                                                        }),
                                                                )
                                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                                    this.send_current_message(cx);
                                                                }))
                                                        }),
                                                ),
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
                                        .child("Select a conversation from the left to start chatting."),
                                ),
                        )
                    }),
            )
            // ====================================================
            // RIGHT DRAWER: CHAT INFO SHEET
            // ====================================================
            .children(if self.is_info_sheet_open && selected_chat.is_some() {
                let chat = selected_chat.clone().unwrap();

                Some(
                    v_flex()
                        .w(px(320.0))
                        .min_w(px(320.0))
                        .max_w(px(320.0))
                        .h_full()
                        .overflow_hidden()
                        .border_l_1()
                        .border_color(border_color.opacity(0.4))
                        .bg(bg_color)
                        .flex_shrink_0()
                        // Header
                        .child(
                            h_flex()
                                .px_4()
                                .py_3()
                                .border_b_1()
                                .border_color(border_color.opacity(0.4))
                                .bg(theme.muted.opacity(0.2))
                                .justify_between()
                                .items_center()
                                .child(
                                    div()
                                        .text_base()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(text_color)
                                        .child(if chat.is_group { "Group Info" } else { "Contact Info" }),
                                )
                                .child(
                                    div()
                                        .cursor_pointer()
                                        .p_1()
                                        .rounded_full()
                                        .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                        .child(svg().data(X_SVG).size(px(16.0)).text_color(muted_text))
                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                            this.is_info_sheet_open = false;
                                            cx.notify();
                                        })),
                                ),
                        )
                        // Content Scroll
                        .child(
                            v_flex()
                                .id("info-sheet-scroll")
                                .flex_1()
                                .overflow_y_scroll()
                                .p_4()
                                .gap_4()
                                // Profile Section
                                .child(
                                    v_flex()
                                        .items_center()
                                        .gap_2()
                                        .py_2()
                                        .child(render_avatar(&chat.id, &chat.name, &chat.avatar, chat.is_group, 80.0, &base_url, &theme))
                                        .child(
                                            div()
                                                .text_lg()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(text_color)
                                                .max_w_full()
                                                .overflow_hidden()
                                                .child(chat.name.clone()),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(muted_text)
                                                .max_w_full()
                                                .overflow_hidden()
                                                .child(chat.id.clone()),
                                        ),
                                )
                                // Tabs Row: [Media] [Docs] [Links]
                                .child(
                                    h_flex()
                                        .p_1()
                                        .bg(theme.muted.opacity(0.5))
                                        .rounded_xl()
                                        .border_1()
                                        .border_color(border_color.opacity(0.3))
                                        .justify_around()
                                        .child(
                                            div()
                                                .cursor_pointer()
                                                .px_3()
                                                .py_1p5()
                                                .rounded_lg()
                                                .text_xs()
                                                .font_weight(if self.info_sheet_tab == InfoSheetTab::Media { FontWeight::BOLD } else { FontWeight::NORMAL })
                                                .bg(if self.info_sheet_tab == InfoSheetTab::Media { theme.primary.opacity(0.18) } else { rgba(0x00000000).into() })
                                                .text_color(if self.info_sheet_tab == InfoSheetTab::Media { primary_color } else { muted_text })
                                                .child("Media")
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.info_sheet_tab = InfoSheetTab::Media;
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            div()
                                                .cursor_pointer()
                                                .px_3()
                                                .py_1p5()
                                                .rounded_lg()
                                                .text_xs()
                                                .font_weight(if self.info_sheet_tab == InfoSheetTab::Docs { FontWeight::BOLD } else { FontWeight::NORMAL })
                                                .bg(if self.info_sheet_tab == InfoSheetTab::Docs { theme.primary.opacity(0.18) } else { rgba(0x00000000).into() })
                                                .text_color(if self.info_sheet_tab == InfoSheetTab::Docs { primary_color } else { muted_text })
                                                .child("Docs")
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.info_sheet_tab = InfoSheetTab::Docs;
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            div()
                                                .cursor_pointer()
                                                .px_3()
                                                .py_1p5()
                                                .rounded_lg()
                                                .text_xs()
                                                .font_weight(if self.info_sheet_tab == InfoSheetTab::Links { FontWeight::BOLD } else { FontWeight::NORMAL })
                                                .bg(if self.info_sheet_tab == InfoSheetTab::Links { theme.primary.opacity(0.18) } else { rgba(0x00000000).into() })
                                                .text_color(if self.info_sheet_tab == InfoSheetTab::Links { primary_color } else { muted_text })
                                                .child("Links")
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.info_sheet_tab = InfoSheetTab::Links;
                                                    cx.notify();
                                                })),
                                        ),
                                )
                                // Tab Content
                                .child(
                                    match self.info_sheet_tab {
                                        InfoSheetTab::Media => {
                                            if self.info_media.is_empty() {
                                                div().py_8().text_xs().text_color(muted_text).text_center().child("No media found").into_any_element()
                                            } else {
                                                v_flex()
                                                    .gap_2()
                                                    .children(self.info_media.iter().map(|m| {
                                                        div()
                                                            .p_2()
                                                            .rounded_lg()
                                                            .bg(bg_color)
                                                            .text_xs()
                                                            .text_color(text_color)
                                                            .child(format!("[Media] {}", m.content))
                                                            .into_any_element()
                                                    }))
                                                    .into_any_element()
                                            }
                                        }
                                        InfoSheetTab::Docs => {
                                            if self.info_docs.is_empty() {
                                                div().py_8().text_xs().text_color(muted_text).text_center().child("No documents found").into_any_element()
                                            } else {
                                                v_flex()
                                                    .gap_2()
                                                    .children(self.info_docs.iter().map(|d| {
                                                        div()
                                                            .p_2()
                                                            .rounded_lg()
                                                            .bg(bg_color)
                                                            .text_xs()
                                                            .text_color(text_color)
                                                            .child(format!("[Doc] {}", d.content))
                                                            .into_any_element()
                                                    }))
                                                    .into_any_element()
                                            }
                                        }
                                        InfoSheetTab::Links => {
                                            if self.info_links.is_empty() {
                                                div().py_8().text_xs().text_color(muted_text).text_center().child("No links found").into_any_element()
                                            } else {
                                                v_flex()
                                                    .gap_2()
                                                    .children(self.info_links.iter().map(|l| {
                                                        div()
                                                            .p_2()
                                                            .rounded_lg()
                                                            .bg(bg_color)
                                                            .text_xs()
                                                            .text_color(text_color)
                                                            .child(l.content.clone())
                                                            .into_any_element()
                                                    }))
                                                    .into_any_element()
                                            }
                                        }
                                        InfoSheetTab::Members => div().into_any_element(),
                                    }
                                ),
                        ),
                )
            } else {
                None
            })
            // ====================================================
            // CONTEXT MENU POPOVER (RIGHT-CLICK ON BUBBLE)
            // ====================================================
            .children(if let Some(ref ctx) = self.context_menu {
                let msg_id = ctx.msg.id.clone();
                let chat_id = selected_chat_id.clone().unwrap_or_default();
                let is_from_me = ctx.is_from_me;
                let msg_content = MessageBubbleHelper::decode_content(&ctx.msg.content);
                let msg_obj = ctx.msg.clone();

                let win_w = f32::from(window.viewport_size().width);
                let win_h = f32::from(window.viewport_size().height);
                let menu_w = 230.0;
                let menu_h = 160.0;

                let click_x = f32::from(ctx.position.x);
                let click_y = f32::from(ctx.position.y);

                let pos_x = if click_x + menu_w > win_w - 20.0 {
                    (click_x - menu_w).max(20.0)
                } else {
                    click_x.max(20.0)
                };

                let pos_y = if click_y + menu_h > win_h - 20.0 {
                    (click_y - menu_h).max(20.0)
                } else {
                    click_y.max(20.0)
                };

                Some(
                    div()
                        .id("context-menu-backdrop")
                        .absolute()
                        .inset_0()
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                            this.context_menu = None;
                            cx.notify();
                        }))
                        .child(
                            v_flex()
                                .absolute()
                                .left(px(pos_x))
                                .top(px(pos_y))
                                .w(px(220.0))
                                .p_2()
                                .gap_1()
                                .rounded_2xl()
                                .bg(card_bg)
                                .border_1()
                                .border_color(border_color)
                                .shadow_xl()
                                .on_mouse_down(MouseButton::Left, |_, _, _| {})
                                // Quick Reactions Row matching Web
                                .child(
                                    h_flex()
                                        .p_1()
                                        .border_b_1()
                                        .border_color(border_color)
                                        .justify_between()
                                        .items_center()
                                        .children(["👍", "❤️", "😂", "😮", "😢", "🙏", "👎"].into_iter().map(|emoji| {
                                            let em = emoji.to_string();
                                            let cid = chat_id.clone();
                                            let mid = msg_id.clone();
                                            div()
                                                .cursor_pointer()
                                                .p_1()
                                                .rounded_md()
                                                .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                .text_base()
                                                .child(emoji)
                                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                    this.react_message(cid.clone(), mid.clone(), em.clone(), cx);
                                                }))
                                                .into_any_element()
                                        }))
                                )
                                // Reply Action
                                .child(
                                    h_flex()
                                        .cursor_pointer()
                                        .px_3()
                                        .py_2()
                                        .rounded_lg()
                                        .gap_2p5()
                                        .items_center()
                                        .hover(|s| s.bg(theme.muted.opacity(0.4)))
                                        .child(svg().data(REPLY_SVG).size(px(14.0)).text_color(primary_color))
                                        .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(text_color).child("Reply"))
                                        .on_mouse_down(MouseButton::Left, cx.listener({
                                            let m = msg_obj.clone();
                                            move |this, _, window, cx| {
                                                this.start_reply(m.clone(), window, cx);
                                            }
                                        })),
                                )
                                // Copy Action
                                .child(
                                    h_flex()
                                        .cursor_pointer()
                                        .px_3()
                                        .py_2()
                                        .rounded_lg()
                                        .gap_2p5()
                                        .items_center()
                                        .hover(|s| s.bg(theme.muted.opacity(0.4)))
                                        .child(svg().data(COPY_SVG).size(px(14.0)).text_color(muted_text))
                                        .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(text_color).child("Copy text"))
                                        .on_mouse_down(MouseButton::Left, cx.listener({
                                            let text = msg_content.clone();
                                            move |this, _, _, cx| {
                                                this.copy_message_text(text.clone(), cx);
                                            }
                                        })),
                                )
                                // Edit Action (Outgoing only)
                                .children(if is_from_me {
                                    Some(
                                        h_flex()
                                            .cursor_pointer()
                                            .px_3()
                                            .py_2()
                                            .rounded_lg()
                                            .gap_2p5()
                                            .items_center()
                                            .hover(|s| s.bg(theme.muted.opacity(0.4)))
                                            .child(svg().data(EDIT_SVG).size(px(14.0)).text_color(rgb(0xf97316)))
                                            .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(text_color).child("Edit"))
                                            .on_mouse_down(MouseButton::Left, cx.listener({
                                                let m = msg_obj.clone();
                                                move |this, _, window, cx| {
                                                    this.start_edit(m.clone(), window, cx);
                                                }
                                            })),
                                    )
                                } else {
                                    None
                                })
                                // Delete Action (Outgoing only)
                                .children(if is_from_me {
                                    Some(
                                        h_flex()
                                            .cursor_pointer()
                                            .px_3()
                                            .py_2()
                                            .rounded_lg()
                                            .gap_2p5()
                                            .items_center()
                                            .hover(|s| s.bg(theme.muted.opacity(0.4)))
                                            .child(svg().data(TRASH_SVG).size(px(14.0)).text_color(rgb(0xef4444)))
                                            .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(rgb(0xef4444)).child("Delete for everyone"))
                                            .on_mouse_down(MouseButton::Left, cx.listener({
                                                let cid = chat_id.clone();
                                                let mid = msg_id.clone();
                                                move |this, _, _, cx| {
                                                    this.delete_message(cid.clone(), mid.clone(), cx);
                                                }
                                            })),
                                    )
                                } else {
                                    None
                                }),
                        ),
                )
            } else {
                None
            })
            // ====================================================
            // CHAT SIDEBAR CONTEXT MENU POPOVER (RIGHT-CLICK ON CHAT)
            // ====================================================
            .children(if let Some(ref ctx) = self.chat_context_menu {
                let chat_id = ctx.chat_id.clone();
                let is_pinned = ctx.is_pinned;
                let is_archived = ctx.is_archived;
                let is_muted = ctx.is_muted;
                let show_mute_submenu = ctx.show_mute_submenu;

                let win_w = f32::from(window.viewport_size().width);
                let win_h = f32::from(window.viewport_size().height);
                let menu_w = 175.0;
                let menu_h = 135.0;

                let click_x = f32::from(ctx.position.x);
                let click_y = f32::from(ctx.position.y);

                let pos_x = if click_x + menu_w > win_w - 20.0 {
                    (click_x - menu_w).max(20.0)
                } else {
                    click_x.max(20.0)
                };

                let pos_y = if click_y + menu_h > win_h - 20.0 {
                    (click_y - menu_h).max(20.0)
                } else {
                    click_y.max(20.0)
                };

                let sub_w = 130.0;
                let sub_h = 120.0;
                let sub_x = if pos_x + menu_w + sub_w > win_w - 15.0 {
                    (pos_x - sub_w - 4.0).max(10.0)
                } else {
                    pos_x + menu_w + 4.0
                };
                let sub_y = (pos_y + 70.0).min(win_h - sub_h - 15.0).max(10.0);

                let mute_bg = if show_mute_submenu {
                    theme.primary.opacity(0.25)
                } else {
                    rgba(0x00000000).into()
                };

                Some(
                    div()
                        .id("chat-context-menu-backdrop")
                        .absolute()
                        .inset_0()
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                            this.chat_context_menu = None;
                            cx.notify();
                        }))
                        .on_mouse_down(MouseButton::Right, cx.listener(|this, _, _, cx| {
                            this.chat_context_menu = None;
                            cx.notify();
                        }))
                        .child(
                            v_flex()
                                .absolute()
                                .left(px(pos_x))
                                .top(px(pos_y))
                                .w(px(menu_w))
                                .p_1p5()
                                .gap_0p5()
                                .rounded_2xl()
                                .bg(card_bg)
                                .border_1()
                                .border_color(border_color)
                                .shadow_xl()
                                .on_mouse_down(MouseButton::Left, |_, _, _| {})
                                // Pin / Unpin Action
                                .child({
                                    let cid = chat_id.clone();
                                    h_flex()
                                        .cursor_pointer()
                                        .px_3()
                                        .py_2()
                                        .rounded_xl()
                                        .gap_2p5()
                                        .items_center()
                                        .hover(|s| s.bg(theme.muted.opacity(0.4)))
                                        .on_mouse_move(cx.listener(|this, _, _, cx| {
                                            if let Some(m) = this.chat_context_menu.as_mut() {
                                                if m.show_mute_submenu {
                                                    m.show_mute_submenu = false;
                                                    cx.notify();
                                                }
                                            }
                                        }))
                                        .child(svg().data(PIN_SVG).size(px(15.0)).text_color(muted_text))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(text_color)
                                                .child(if is_pinned { "Unpin" } else { "Pin" }),
                                        )
                                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                            this.toggle_pin(cid.clone(), is_pinned, cx);
                                            this.chat_context_menu = None;
                                            cx.notify();
                                        }))
                                })
                                // Archive / Unarchive Action
                                .child({
                                    let cid = chat_id.clone();
                                    h_flex()
                                        .cursor_pointer()
                                        .px_3()
                                        .py_2()
                                        .rounded_xl()
                                        .gap_2p5()
                                        .items_center()
                                        .hover(|s| s.bg(theme.muted.opacity(0.4)))
                                        .on_mouse_move(cx.listener(|this, _, _, cx| {
                                            if let Some(m) = this.chat_context_menu.as_mut() {
                                                if m.show_mute_submenu {
                                                    m.show_mute_submenu = false;
                                                    cx.notify();
                                                }
                                            }
                                        }))
                                        .child(svg().data(ARCHIVE_SVG).size(px(15.0)).text_color(muted_text))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(text_color)
                                                .child(if is_archived { "Unarchive" } else { "Archive" }),
                                        )
                                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                            this.toggle_archive(cid.clone(), is_archived, cx);
                                            this.chat_context_menu = None;
                                            cx.notify();
                                        }))
                                })
                                // Mute / Unmute Action
                                .child({
                                    let cid = chat_id.clone();
                                    if is_muted {
                                        // Directly unmute if already muted
                                        h_flex()
                                            .cursor_pointer()
                                            .px_3()
                                            .py_2()
                                            .rounded_xl()
                                            .gap_2p5()
                                            .items_center()
                                            .hover(|s| s.bg(theme.muted.opacity(0.4)))
                                            .child(svg().data(VOLUME_X_SVG).size(px(15.0)).text_color(muted_text))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(text_color)
                                                    .child("Unmute"),
                                            )
                                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                this.set_mute(cid.clone(), "off".to_string(), cx);
                                                this.chat_context_menu = None;
                                                cx.notify();
                                            }))
                                    } else {
                                        // Flyout submenu on hover / click
                                        h_flex()
                                            .cursor_pointer()
                                            .px_3()
                                            .py_2()
                                            .rounded_xl()
                                            .justify_between()
                                            .items_center()
                                            .bg(mute_bg)
                                            .hover(|s| s.bg(theme.primary.opacity(0.25)))
                                            .on_mouse_move(cx.listener(|this, _, _, cx| {
                                                if let Some(m) = this.chat_context_menu.as_mut() {
                                                    if !m.show_mute_submenu {
                                                        m.show_mute_submenu = true;
                                                        cx.notify();
                                                    }
                                                }
                                            }))
                                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                if let Some(m) = this.chat_context_menu.as_mut() {
                                                    m.show_mute_submenu = !m.show_mute_submenu;
                                                    cx.notify();
                                                }
                                            }))
                                            .child(
                                                h_flex()
                                                    .gap_2p5()
                                                    .items_center()
                                                    .child(svg().data(VOLUME_X_SVG).size(px(15.0)).text_color(muted_text))
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(text_color)
                                                            .child("Mute"),
                                                    ),
                                            )
                                            .child(
                                                svg()
                                                    .data(CHEVRON_RIGHT_SVG)
                                                    .size(px(13.0))
                                                    .text_color(muted_text),
                                            )
                                    }
                                }),
                        )
                        // Flyout Submenu for Mute Durations
                        .children(if show_mute_submenu && !is_muted {
                            let cid_8h = chat_id.clone();
                            let cid_1w = chat_id.clone();
                            let cid_always = chat_id.clone();
                            Some(
                                v_flex()
                                    .absolute()
                                    .left(px(sub_x))
                                    .top(px(sub_y))
                                    .w(px(sub_w))
                                    .p_1p5()
                                    .gap_0p5()
                                    .rounded_2xl()
                                    .bg(card_bg)
                                    .border_1()
                                    .border_color(border_color)
                                    .shadow_xl()
                                    .on_mouse_down(MouseButton::Left, |_, _, _| {})
                                    .child(
                                        h_flex()
                                            .cursor_pointer()
                                            .px_3()
                                            .py_2()
                                            .rounded_xl()
                                            .items_center()
                                            .hover(|s| s.bg(theme.muted.opacity(0.4)))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(text_color)
                                                    .child("8 hours"),
                                            )
                                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                this.set_mute(cid_8h.clone(), "8h".to_string(), cx);
                                                this.chat_context_menu = None;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        h_flex()
                                            .cursor_pointer()
                                            .px_3()
                                            .py_2()
                                            .rounded_xl()
                                            .items_center()
                                            .hover(|s| s.bg(theme.muted.opacity(0.4)))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(text_color)
                                                    .child("1 week"),
                                            )
                                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                this.set_mute(cid_1w.clone(), "1w".to_string(), cx);
                                                this.chat_context_menu = None;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        h_flex()
                                            .cursor_pointer()
                                            .px_3()
                                            .py_2()
                                            .rounded_xl()
                                            .items_center()
                                            .hover(|s| s.bg(theme.muted.opacity(0.4)))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(text_color)
                                                    .child("Always"),
                                            )
                                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                this.set_mute(cid_always.clone(), "forever".to_string(), cx);
                                                this.chat_context_menu = None;
                                                cx.notify();
                                            })),
                                    ),
                            )
                        } else {
                            None
                        }),
                )
            } else {
                None
            })
            // ====================================================
            // MODAL: NEW GROUP DIALOG
            // ====================================================
            .children(if self.is_new_group_open {
                let contacts: Vec<Chat> = all_chats.iter().filter(|c| !c.is_group && !c.archived).cloned().collect();
                Some(
                    div()
                        .absolute()
                        .inset_0()
                        .bg(rgba(0x00000088))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            v_flex()
                                .w(px(400.0))
                                .p_5()
                                .rounded_2xl()
                                .bg(card_bg)
                                .border_1()
                                .border_color(border_color)
                                .shadow_2xl()
                                .gap_4()
                                // Header
                                .child(
                                    h_flex()
                                        .justify_between()
                                        .items_center()
                                        .child(div().text_lg().font_weight(FontWeight::BOLD).text_color(text_color).child("New group"))
                                        .child(
                                            div()
                                                .cursor_pointer()
                                                .p_1()
                                                .rounded_full()
                                                .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                .child(svg().data(X_SVG).size(px(16.0)).text_color(muted_text))
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.is_new_group_open = false;
                                                    cx.notify();
                                                })),
                                        ),
                                )
                                // Group Name Input
                                .child(
                                    h_flex()
                                        .px_3()
                                        .py_2()
                                        .rounded_xl()
                                        .border_1()
                                        .border_color(border_color)
                                        .bg(bg_color)
                                        .track_focus(&self.new_group_focus)
                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                            this.new_group_focus.focus(window, cx);
                                        }))
                                        .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                                            match ev.keystroke.key.as_str() {
                                                "backspace" => { this.new_group_name.pop(); cx.notify(); }
                                                "space" => { this.new_group_name.push(' '); cx.notify(); }
                                                k if k.len() == 1 => { this.new_group_name.push_str(k); cx.notify(); }
                                                _ => {}
                                            }
                                        }))
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(if self.new_group_name.is_empty() { muted_text } else { text_color })
                                                .child(if self.new_group_name.is_empty() { "Group name...".to_string() } else { self.new_group_name.clone() }),
                                        ),
                                )
                                // Contact Picker Scroll
                                .child(
                                    v_flex()
                                        .id("new-group-contacts-scroll")
                                        .max_h(px(220.0))
                                        .overflow_y_scroll()
                                        .gap_1()
                                        .children(contacts.into_iter().map(|c| {
                                            let is_checked = self.new_group_selected.contains(&c.id);
                                            let cid = c.id.clone();
                                            h_flex()
                                                .cursor_pointer()
                                                .p_2()
                                                .rounded_lg()
                                                .items_center()
                                                .justify_between()
                                                .bg(if is_checked { theme.primary.opacity(0.12) } else { rgba(0x00000000).into() })
                                                .hover(|s| s.bg(theme.muted.opacity(0.4)))
                                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                    if this.new_group_selected.contains(&cid) {
                                                        this.new_group_selected.remove(&cid);
                                                    } else {
                                                        this.new_group_selected.insert(cid.clone());
                                                    }
                                                    cx.notify();
                                                }))
                                                .child(div().text_sm().text_color(text_color).child(c.name.clone()))
                                                .child(
                                                    div()
                                                        .w(px(20.0))
                                                        .h(px(20.0))
                                                        .rounded_md()
                                                        .border_1()
                                                        .border_color(primary_color)
                                                        .bg(if is_checked { primary_color } else { rgba(0x00000000).into() })
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .children(if is_checked {
                                                            Some(svg().data(CHECK_SVG).size(px(12.0)).text_color(rgb(0xffffff)))
                                                        } else {
                                                            None
                                                        }),
                                                )
                                                .into_any_element()
                                        }))
                                )
                                // Actions
                                .child(
                                    h_flex()
                                        .justify_end()
                                        .gap_2()
                                        .child(
                                            div()
                                                .cursor_pointer()
                                                .px_4()
                                                .py_2()
                                                .rounded_xl()
                                                .border_1()
                                                .border_color(border_color)
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(text_color)
                                                .child("Cancel")
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.is_new_group_open = false;
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            div()
                                                .cursor_pointer()
                                                .px_4()
                                                .py_2()
                                                .rounded_xl()
                                                .bg(primary_color)
                                                .text_xs()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(rgb(0xffffff))
                                                .child(if self.is_creating_group { "Creating..." } else { "Create Group" })
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.submit_create_group(cx);
                                                })),
                                        ),
                                ),
                        ),
                )
            } else {
                None
            })
            // ====================================================
            // MODAL: JOIN GROUP DIALOG
            // ====================================================
            .children(if self.is_join_group_open {
                Some(
                    div()
                        .absolute()
                        .inset_0()
                        .bg(rgba(0x00000088))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            v_flex()
                                .w(px(400.0))
                                .p_5()
                                .rounded_2xl()
                                .bg(card_bg)
                                .border_1()
                                .border_color(border_color)
                                .shadow_2xl()
                                .gap_4()
                                // Header
                                .child(
                                    h_flex()
                                        .justify_between()
                                        .items_center()
                                        .child(div().text_lg().font_weight(FontWeight::BOLD).text_color(text_color).child("Join group"))
                                        .child(
                                            div()
                                                .cursor_pointer()
                                                .p_1()
                                                .rounded_full()
                                                .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                .child(svg().data(X_SVG).size(px(16.0)).text_color(muted_text))
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.is_join_group_open = false;
                                                    cx.notify();
                                                })),
                                        ),
                                )
                                .child(
                                    div().text_xs().text_color(muted_text).child("Paste an invite link (chat.whatsapp.com/...)"),
                                )
                                // Invite Link Input
                                .child(
                                    h_flex()
                                        .px_3()
                                        .py_2()
                                        .rounded_xl()
                                        .border_1()
                                        .border_color(border_color)
                                        .bg(bg_color)
                                        .track_focus(&self.join_group_focus)
                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                            this.join_group_focus.focus(window, cx);
                                        }))
                                        .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                                            if (ev.keystroke.modifiers.control || ev.keystroke.modifiers.platform)
                                                && ev.keystroke.key.as_str().eq_ignore_ascii_case("v")
                                            {
                                                if let Some(item) = cx.read_from_clipboard() {
                                                    if let Some(text) = item.text() {
                                                        this.join_group_link.push_str(&text);
                                                        cx.notify();
                                                    }
                                                }
                                                return;
                                            }
                                            match ev.keystroke.key.as_str() {
                                                "backspace" => { this.join_group_link.pop(); cx.notify(); }
                                                "space" => { this.join_group_link.push(' '); cx.notify(); }
                                                k if k.len() == 1 => { this.join_group_link.push_str(k); cx.notify(); }
                                                _ => {}
                                            }
                                        }))
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(if self.join_group_link.is_empty() { muted_text } else { text_color })
                                                .child(if self.join_group_link.is_empty() { "https://chat.whatsapp.com/...".to_string() } else { self.join_group_link.clone() }),
                                        ),
                                )
                                // Actions
                                .child(
                                    h_flex()
                                        .justify_end()
                                        .gap_2()
                                        .child(
                                            div()
                                                .cursor_pointer()
                                                .px_4()
                                                .py_2()
                                                .rounded_xl()
                                                .border_1()
                                                .border_color(border_color)
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(text_color)
                                                .child("Cancel")
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.is_join_group_open = false;
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            div()
                                                .cursor_pointer()
                                                .px_4()
                                                .py_2()
                                                .rounded_xl()
                                                .bg(primary_color)
                                                .text_xs()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(rgb(0xffffff))
                                                .child(if self.is_joining_group { "Joining..." } else { "Join" })
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.submit_join_group(cx);
                                                })),
                                        ),
                                ),
                        ),
                )
            } else {
                None
            })
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
            // Full-screen Image Preview Overlay (click anywhere to close)
            .children(if let Some(ref img_url) = self.preview_image_url {
                let u = img_url.clone();
                Some(
                    div()
                        .absolute()
                        .inset_0()
                        .bg(gpui::rgba(0x000000e0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .p_8()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                            this.preview_image_url = None;
                            cx.notify();
                        }))
                        .child(
                            img(u)
                                .max_w(px(800.0))
                                .max_h(px(700.0))
                                .rounded_xl()
                                .object_fit(ObjectFit::Contain)
                        )
                        .into_any_element()
                )
            } else {
                None
            })
    }
}

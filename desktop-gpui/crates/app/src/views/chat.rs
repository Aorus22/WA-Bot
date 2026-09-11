//! Chat view matching Web ChatPage, ChatSidebar, and ChatArea with 1:1 parity.

use std::collections::HashSet;
use std::ops::Range;
use gpui::*;
use gpui_component::scroll::{Scrollbar, ScrollbarMode};
use gpui_component::spinner::Spinner;
use gpui_component::tooltip::Tooltip;
use gpui_component::{h_flex, v_flex, Icon, IconName};
use wabot_backend_client::client::HttpClient;
use wabot_backend_client::dto::{CallType, Chat, Message, ReactionEntry};
use wabot_backend_client::multipart::SendMediaBuilder;

use crate::components::{
    compose::{
        encode_markdown, looks_like_document_name, AttachPanel, ComposeDialog, ContactDraft,
        LocationDraft, MediaKind, PollDraft, StickerPickerState,
    },
    emoji_picker::{EmojiPickerState, EMOJI_CATEGORIES},
    connection_banner::{ConnectionState, ConnectionStatus},
    toast,
};
use crate::components::message_bubble::{MessageBubbleHelper, MessageTicks};
use crate::components::nav_sidebar::SIDEBAR_WIDTH;
use crate::components::titlebar::TITLEBAR_HEIGHT;
use crate::icons::*;
use crate::state::auth::AuthState;
use crate::state::call::CallManager;
use crate::state::chat::ChatStore;
use crate::theme::manager::{ActiveTokens, AppThemeExt};
use crate::TOKIO_RT;

/// Conversation sidebar sizing: the desktop opens at 360 like before, and
/// drags within the web client's bounds (`ChatPage.tsx` clamps 280–600).
const SIDEBAR_DEFAULT_WIDTH: f32 = 360.0;
const SIDEBAR_MIN_WIDTH: f32 = 280.0;
const SIDEBAR_MAX_WIDTH: f32 = 600.0;
/// Width of the divider's grab area, straddling the sidebar's right edge.
const SIDEBAR_HANDLE_WIDTH: f32 = 6.0;
/// Unread count badge: square minimum + fixed height keeps a single digit a
/// circle while two digits grow into a pill, like WhatsApp.
const UNREAD_BADGE_SIZE: f32 = 20.0;
/// Width of the right-hand drawers (search + chat info); the entrance
/// animation slides across exactly this distance.
const SHEET_WIDTH: f32 = 320.0;
/// Entrance duration for the right-hand drawers, in ms.
const SHEET_ANIM_MS: u64 = 240;

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
    pub is_loading_more: bool,
    pub is_loading_newer: bool,
    pub has_more: bool,
    pub has_more_next: bool,

    // Cached filtered chats for zero-allocation scrolling
    pub cached_filtered_chats: Vec<Chat>,
    pub last_filter_key: (u64, String, ChatFilter, bool),
    // Last ChatStore::messages_version seen by the render cache
    pub last_messages_version: u64,

    // Cached messages for 60fps scrolling
    pub cached_chat_id: Option<String>,
    pub cached_render_messages: Vec<RenderMessage>,

    // Scroll handles for visual scrollbars (uniform_list and virtual list)
    pub chat_list_scroll_handle: UniformListScrollHandle,
    pub messages_list_state: ListState,

    // Conversation sidebar width and its live resize drag (web parity)
    pub sidebar_width: f32,
    /// `(pointer x, sidebar width)` captured when the divider drag started.
    pub sidebar_resize_drag: Option<(f32, f32)>,

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

    // Chat Search Sheet drawer (Right side)
    pub is_search_sheet_open: bool,
    pub search_sheet_query: String,
    pub search_sheet_focus: FocusHandle,
    pub search_sheet_results: Vec<Message>,
    pub is_searching_messages: bool,

    // Right-hand drawers play an exit animation before they unmount
    pub info_sheet_closing: bool,
    pub search_sheet_closing: bool,
    pub highlighted_message_id: Option<String>,

    // New Group modal
    pub is_new_group_open: bool,
    pub new_group_name: String,
    pub new_group_selected: HashSet<String>,
    pub new_group_focus: FocusHandle,
    pub is_creating_group: bool,

    // Attach (+) menu, emoji picker and compose dialogs (web parity)
    pub attach_panel: AttachPanel,
    pub emoji_picker: EmojiPickerState,
    pub is_md_mode: bool,
    pub compose_dialog: Option<ComposeDialog>,
    pub poll_draft: PollDraft,
    pub location_draft: LocationDraft,
    pub contact_draft: ContactDraft,
    pub sticker_picker: StickerPickerState,
    // Focus handle shared by every dialog input; `active_compose_field` selects
    // which field receives the keystrokes.
    pub compose_field_focus: FocusHandle,
    pub active_compose_field: usize,
    pub is_sending_attachment: bool,

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
            is_loading_more: false,
            is_loading_newer: false,
            has_more: true,
            has_more_next: false,

            cached_filtered_chats: Vec::new(),
            last_filter_key: (u64::MAX, String::new(), ChatFilter::All, false),
            last_messages_version: u64::MAX,

            cached_chat_id: None,
            cached_render_messages: Vec::new(),

            chat_list_scroll_handle: UniformListScrollHandle::new(),
            messages_list_state: ListState::new(0, ListAlignment::Top, px(200.0)),

            sidebar_width: SIDEBAR_DEFAULT_WIDTH,
            sidebar_resize_drag: None,

            context_menu: None,
            chat_context_menu: None,

            is_info_sheet_open: false,
            info_sheet_tab: InfoSheetTab::Media,
            info_media: Vec::new(),
            info_docs: Vec::new(),
            info_links: Vec::new(),
            is_loading_info: false,

            is_search_sheet_open: false,
            search_sheet_query: String::new(),
            search_sheet_focus: cx.focus_handle(),
            search_sheet_results: Vec::new(),
            is_searching_messages: false,

            info_sheet_closing: false,
            search_sheet_closing: false,
            highlighted_message_id: None,

            is_new_group_open: false,
            new_group_name: String::new(),
            new_group_selected: HashSet::new(),
            new_group_focus: cx.focus_handle(),
            is_creating_group: false,

            preview_image_url: None,

            is_join_group_open: false,

            attach_panel: AttachPanel::None,
            emoji_picker: EmojiPickerState::new(),
            is_md_mode: false,
            compose_dialog: None,
            poll_draft: PollDraft::new(),
            location_draft: LocationDraft::new(),
            contact_draft: ContactDraft::new(),
            sticker_picker: StickerPickerState::new(),
            compose_field_focus: cx.focus_handle(),
            active_compose_field: 0,
            is_sending_attachment: false,
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
        // Switching conversations cuts both drawers immediately — a lingering
        // exit animation would sit over the wrong thread.
        self.is_info_sheet_open = false;
        self.is_search_sheet_open = false;
        self.info_sheet_closing = false;
        self.search_sheet_closing = false;
        self.search_sheet_query.clear();
        self.search_sheet_results.clear();
        self.is_searching_messages = false;
        self.is_loading_more = false;
        self.is_loading_newer = false;
        self.has_more = true;
        self.has_more_next = false;
        self.highlighted_message_id = None;
        self.messages_list_state.scroll_to_end();

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

        let total_items = if self.cached_render_messages.is_empty() { 0 } else { self.cached_render_messages.len() + 1 };
        self.messages_list_state.reset(total_items);
        self.messages_list_state.scroll_to_end();
    }

    /// Rebuild cached render messages without jumping to bottom (preserves scroll position)
    fn rebuild_message_cache_preserve_scroll(&mut self, chat_id: &str, msgs: &[Message]) {
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

        let total_items = if self.cached_render_messages.is_empty() { 0 } else { self.cached_render_messages.len() + 1 };

        // Same item count (reaction, edit, status-tick update): remeasure in
        // place so the scroll position is preserved exactly. `reset` clears
        // the scroll offset, which snaps the list back to the bottom.
        if self.messages_list_state.item_count() == total_items {
            self.messages_list_state.remeasure();
        } else {
            self.messages_list_state.reset(total_items);
        }
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
                                this.has_more = msgs.len() >= 100;
                                this.has_more_next = false;
                                this.rebuild_message_cache(&cid, &msgs);
                                if cx.has_global::<ChatStore>() {
                                    ChatStore::global_mut(cx).set_messages(&cid, msgs, this.has_more);
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

    /// Load older messages when scrolling near the top (paginating backwards)
    pub fn load_older_messages(&mut self, cx: &mut Context<Self>) {
        if self.is_loading_more || !self.has_more {
            return;
        }
        let chat_id = match &self.selected_chat_id {
            Some(id) => id.clone(),
            None => return,
        };
        let oldest_ts = self.cached_render_messages.first().map(|m| m.msg.timestamp).unwrap_or(0);
        if oldest_ts == 0 {
            return;
        }
        self.is_loading_more = true;
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };
        let cid = chat_id.clone();

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            let cid_fetch = cid.clone();
            let cid_async = cid_fetch.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move { client.get_messages(&cid_async, Some(40), Some(oldest_ts), None).await })
                    .await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_loading_more = false;
                            if let Ok(Ok(older_msgs)) = res {
                                if older_msgs.is_empty() {
                                    this.has_more = false;
                                } else {
                                    this.has_more = older_msgs.len() >= 40;
                                    let mut all_msgs = older_msgs;
                                    for rm in &this.cached_render_messages {
                                        all_msgs.push(rm.msg.clone());
                                    }
                                    all_msgs.sort_by_key(|m| m.timestamp);
                                    all_msgs.dedup_by(|a, b| a.id == b.id);
                                    this.rebuild_message_cache_preserve_scroll(&cid_fetch, &all_msgs);
                                    if cx.has_global::<ChatStore>() {
                                        ChatStore::global_mut(cx).set_messages(&cid_fetch, all_msgs, this.has_more);
                                    }
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

    /// Load newer messages when scrolling near the bottom (after jumping to an older historical message)
    pub fn load_newer_messages(&mut self, cx: &mut Context<Self>) {
        if self.is_loading_newer || !self.has_more_next {
            return;
        }
        let chat_id = match &self.selected_chat_id {
            Some(id) => id.clone(),
            None => return,
        };
        let newest_ts = self.cached_render_messages.last().map(|m| m.msg.timestamp).unwrap_or(0);
        if newest_ts == 0 {
            return;
        }
        self.is_loading_newer = true;
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };
        let cid = chat_id.clone();

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            let cid_fetch = cid.clone();
            let cid_async = cid_fetch.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move { client.get_messages(&cid_async, Some(40), None, Some(newest_ts)).await })
                    .await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_loading_newer = false;
                            if let Ok(Ok(newer_msgs)) = res {
                                if newer_msgs.is_empty() {
                                    this.has_more_next = false;
                                } else {
                                    this.has_more_next = newer_msgs.len() >= 40;
                                    let mut all_msgs: Vec<Message> = this.cached_render_messages.iter().map(|rm| rm.msg.clone()).collect();
                                    all_msgs.extend(newer_msgs);
                                    all_msgs.sort_by_key(|m| m.timestamp);
                                    all_msgs.dedup_by(|a, b| a.id == b.id);
                                    this.rebuild_message_cache_preserve_scroll(&cid_fetch, &all_msgs);
                                    if cx.has_global::<ChatStore>() {
                                        ChatStore::global_mut(cx).set_messages(&cid_fetch, all_msgs, this.has_more);
                                    }
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

    /// Play the Chat Info drawer's exit animation, then unmount it.
    pub fn close_info_sheet(&mut self, cx: &mut Context<Self>) {
        if !self.is_info_sheet_open || self.info_sheet_closing {
            return;
        }
        self.info_sheet_closing = true;
        Self::after_sheet_exit(cx, |this| {
            // Re-opening during the exit cancels the pending unmount.
            if this.info_sheet_closing {
                this.is_info_sheet_open = false;
                this.info_sheet_closing = false;
            }
        });
        cx.notify();
    }

    /// Play the Search drawer's exit animation, then unmount it.
    pub fn close_search_sheet(&mut self, cx: &mut Context<Self>) {
        if !self.is_search_sheet_open || self.search_sheet_closing {
            return;
        }
        self.search_sheet_closing = true;
        Self::after_sheet_exit(cx, |this| {
            // Re-opening during the exit cancels the pending unmount.
            if this.search_sheet_closing {
                this.is_search_sheet_open = false;
                this.search_sheet_closing = false;
            }
        });
        cx.notify();
    }

    /// Run `done` once a drawer's exit animation has finished playing.
    fn after_sheet_exit(cx: &mut Context<Self>, done: impl FnOnce(&mut Self) + 'static) {
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(SHEET_ANIM_MS + 40))
                .await;
            this.update(cx, |this, cx| {
                done(this);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Toggle Chat Info Sheet drawer
    pub fn toggle_info_sheet(&mut self, cx: &mut Context<Self>) {
        if self.is_info_sheet_open && !self.info_sheet_closing {
            self.close_info_sheet(cx);
            return;
        }
        self.info_sheet_closing = false;
        self.is_info_sheet_open = true;
        self.is_search_sheet_open = false;
        self.search_sheet_closing = false;
        self.load_info_media(cx);
        self.load_info_docs(cx);
        self.load_info_links(cx);
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

        self.is_loading_info = true;

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
                            this.is_loading_info = false;
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

    /// Initiate voice or video call matching Web client parity
    pub fn start_call(&mut self, is_video: bool, cx: &mut Context<Self>) {
        let chat_id = match &self.selected_chat_id {
            Some(id) => id.clone(),
            None => return,
        };

        let is_group = chat_id.ends_with("@g.us") || if cx.has_global::<ChatStore>() {
            ChatStore::global(cx).chats.iter().find(|c| c.id == chat_id).map(|c| c.is_group).unwrap_or(false)
        } else {
            false
        };

        let chat_name = if cx.has_global::<ChatStore>() {
            ChatStore::global(cx).chats.iter().find(|c| c.id == chat_id).map(|c| c.name.clone()).unwrap_or_else(|| chat_id.clone())
        } else {
            chat_id.clone()
        };

        let call_type = match (is_group, is_video) {
            (true, true) => CallType::GroupVideo,
            (true, false) => CallType::GroupAudio,
            (false, true) => CallType::Video,
            (false, false) => CallType::Audio,
        };

        // Show the full-screen call overlay immediately (Web does the same with
        // its optimistic `setActiveCall`); the API response then replaces it
        // with the authoritative backend state.
        let group_jid = if is_group { Some(chat_id.clone()) } else { None };
        if cx.has_global::<CallManager>() {
            CallManager::global_mut(cx).initiate_call(&chat_id, call_type.clone(), group_jid);
        }
        let _ = chat_name;
        cx.notify();

        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        let cid = chat_id.clone();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT.spawn(async move {
                    if is_group {
                        client.create_group_call(&cid, &[], call_type).await
                    } else {
                        client.create_call(&cid, call_type).await
                    }
                }).await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |_this, cx| {
                            match res {
                                Ok(Ok(call_state)) => {
                                    if cx.has_global::<CallManager>() {
                                        CallManager::global_mut(cx).adopt(call_state);
                                    }
                                }
                                Ok(Err(e)) => {
                                    if cx.has_global::<CallManager>() {
                                        CallManager::global_mut(cx).end_local(None);
                                    }
                                    toast::error(format!("Failed to start call: {e}"), cx);
                                }
                                Err(_) => {
                                    if cx.has_global::<CallManager>() {
                                        CallManager::global_mut(cx).end_local(None);
                                    }
                                    toast::error("Failed to start call", cx);
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

    /// Toggle Chat Search Sheet drawer (Right side)
    pub fn toggle_search_sheet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_search_sheet_open && !self.search_sheet_closing {
            self.close_search_sheet(cx);
            return;
        }
        self.search_sheet_closing = false;
        self.is_search_sheet_open = true;
        self.is_info_sheet_open = false;
        self.info_sheet_closing = false;
        self.search_sheet_focus.focus(window, cx);
        if !self.search_sheet_query.is_empty() {
            self.run_search_messages(cx);
        }
        cx.notify();
    }

    /// Run message search both locally (immediate) and remotely (via HttpClient)
    pub fn run_search_messages(&mut self, cx: &mut Context<Self>) {
        let chat_id = match &self.selected_chat_id {
            Some(id) => id.clone(),
            None => return,
        };
        let query = self.search_sheet_query.trim().to_string();
        if query.is_empty() {
            self.search_sheet_results.clear();
            self.is_searching_messages = false;
            cx.notify();
            return;
        }

        let q_lower = query.to_lowercase();

        // 1. Instant local results from current cached render messages & ChatStore
        let mut local_results: Vec<Message> = Vec::new();
        let mut seen_ids = HashSet::new();

        for rm in &self.cached_render_messages {
            let content_lower = rm.content.to_lowercase();
            if content_lower.contains(&q_lower) && seen_ids.insert(rm.msg.id.clone()) {
                local_results.push(rm.msg.clone());
            }
        }

        if cx.has_global::<ChatStore>() {
            let store = ChatStore::global(cx);
            if let Some(history) = store.messages_by_chat.get(&chat_id) {
                for m in &history.messages {
                    let decoded = MessageBubbleHelper::decode_content(&m.content).to_lowercase();
                    if decoded.contains(&q_lower) && seen_ids.insert(m.id.clone()) {
                        local_results.push(m.clone());
                    }
                }
            }
        }

        // Sort local results newest first
        local_results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        self.search_sheet_results = local_results;
        self.is_searching_messages = true;
        cx.notify();

        // 2. Fetch server messages via HttpClient::search_messages
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        let cid = chat_id.clone();
        let q_fetch = query.clone();

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            let cid_async = cid.clone();
            let q_async = q_fetch.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move { client.search_messages(&cid_async, &q_async, Some(50)).await })
                    .await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            if this.search_sheet_query.trim() == q_fetch.trim() {
                                this.is_searching_messages = false;
                                if let Ok(Ok(server_msgs)) = res {
                                    let mut combined = this.search_sheet_results.clone();
                                    let mut seen: HashSet<String> = combined.iter().map(|m| m.id.clone()).collect();
                                    for sm in server_msgs {
                                        if seen.insert(sm.id.clone()) {
                                            combined.push(sm);
                                        }
                                    }
                                    combined.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
                                    this.search_sheet_results = combined;
                                }
                                cx.notify();
                            }
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Jump/teleport directly to a specific message in the conversation
    pub fn navigate_to_message(&mut self, message_id: &str, cx: &mut Context<Self>) {
        // Jumping away needs the drawer gone now, not sliding out of the way.
        self.is_search_sheet_open = false;
        self.search_sheet_closing = false;
        self.highlighted_message_id = Some(message_id.to_string());
        let mid_flash = message_id.to_string();

        // Clear highlight after 2.5 seconds
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            let mid_clone = mid_flash.clone();
            async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(2500)).await;
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            if this.highlighted_message_id.as_deref() == Some(&mid_clone) {
                                this.highlighted_message_id = None;
                                cx.notify();
                            }
                        });
                    }
                });
            }
        })
        .detach();

        // If message is already loaded in cached_render_messages
        if let Some(pos) = self.cached_render_messages.iter().position(|m| m.msg.id == message_id) {
            self.messages_list_state.scroll_to_reveal_item(pos + 1);
            cx.notify();
            return;
        }

        // If not in local cache, fetch message context around it from backend
        let chat_id = match &self.selected_chat_id {
            Some(id) => id.clone(),
            None => return,
        };

        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        let target_mid = message_id.to_string();
        let cid = chat_id.clone();
        self.is_loading_messages = true;
        cx.notify();

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            let cid_async = cid.clone();
            let target_mid_async = target_mid.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move { client.get_message_context(&cid_async, &target_mid_async, Some(50)).await })
                    .await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_loading_messages = false;
                            if let Ok(Ok(mut msgs)) = res {
                                if !msgs.is_empty() {
                                    msgs.sort_by_key(|m| m.timestamp);
                                    this.has_more = true;
                                    this.has_more_next = true;
                                    this.rebuild_message_cache(&cid, &msgs);
                                    if cx.has_global::<ChatStore>() {
                                        ChatStore::global_mut(cx).set_messages(&cid, msgs, true);
                                    }
                                    if let Some(pos) = this.cached_render_messages.iter().position(|m| m.msg.id == target_mid) {
                                        this.messages_list_state.scroll_to_reveal_item(pos + 1);
                                    }
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

    /// Format unix epoch timestamp to readable search date (e.g. "10 Sep 2026")
    fn format_search_date(timestamp: i64) -> String {
        if timestamp == 0 {
            return String::new();
        }
        let ts_sec = if timestamp > 10_000_000_000 { timestamp / 1000 } else { timestamp };
        let mut days = (ts_sec / 86400) as i32;
        if days < 0 {
            return String::new();
        }
        let mut year = 1970;
        loop {
            let leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
            let days_in_year = if leap { 366 } else { 365 };
            if days >= days_in_year {
                days -= days_in_year;
                year += 1;
            } else {
                break;
            }
        }
        let leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
        let days_in_months = [
            31, if leap { 29 } else { 28 }, 31, 30, 31, 30,
            31, 31, 30, 31, 30, 31,
        ];
        let month_names = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun",
            "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        let mut month = 0;
        for (m_idx, &dim) in days_in_months.iter().enumerate() {
            if days >= dim {
                days -= dim;
            } else {
                month = m_idx;
                break;
            }
        }
        let day = days + 1;
        format!("{} {} {}", day, month_names[month], year)
    }

    /// Format unix epoch timestamp to readable search time (e.g. "23:26")
    fn format_search_time(timestamp: i64) -> String {
        if timestamp == 0 {
            return String::new();
        }
        let ts_sec = if timestamp > 10_000_000_000 { timestamp / 1000 } else { timestamp };
        let hours = (ts_sec / 3600) % 24;
        let mins = (ts_sec / 60) % 60;
        format!("{:02}:{:02}", hours, mins)
    }

    /// React to message with emoji. Toggles like the web client: sending the
    /// same emoji again removes "my" reaction (backend ApplyReaction treats
    /// the sender list as authoritative).
    pub fn react_message(&mut self, chat_id: String, msg_id: String, emoji: String, cx: &mut Context<Self>) {
        self.context_menu = None;
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        // Optimistic local update, mirroring the web's applyReactionLocal:
        // drop "me" from every entry, re-add under the new emoji.
        let new_reactions = {
            let mut out: Vec<ReactionEntry> = Vec::new();
            let existing = cx
                .has_global::<ChatStore>()
                .then(|| {
                    ChatStore::global(cx)
                        .messages_by_chat
                        .get(&chat_id)
                        .and_then(|e| e.messages.iter().find(|m| m.id == msg_id))
                        .and_then(|m| m.reactions.clone())
                })
                .flatten()
                .unwrap_or_default();
            for r in existing {
                let mut kept: Vec<String> = r.senders.into_iter().filter(|s| s != "me").collect();
                if r.emoji == emoji {
                    kept.push("me".to_string());
                }
                if !kept.is_empty() {
                    out.push(ReactionEntry { emoji: r.emoji, senders: kept });
                }
            }
            if !out.iter().any(|r| r.emoji == emoji) {
                out.push(ReactionEntry { emoji: emoji.clone(), senders: vec!["me".to_string()] });
            }
            out
        };
        if cx.has_global::<ChatStore>() {
            let reactions_clone = new_reactions.clone();
            ChatStore::global_mut(cx).patch_message(&chat_id, &msg_id, move |m| {
                m.reactions = Some(reactions_clone);
            });
        }
        if let Some(rm) = self.cached_render_messages.iter_mut().find(|m| m.msg.id == msg_id) {
            rm.msg.reactions = Some(new_reactions.clone());
            // The chips row changes the item's rendered height — keep the
            // list's height summary in sync without moving the scroll.
            self.messages_list_state.remeasure();
        }

        let cid = chat_id.clone();
        let mid = msg_id.clone();
        let em = emoji.clone();

        cx.spawn(move |_this: WeakEntity<Self>, _cx: &mut AsyncApp| {
            let cx_handle = _cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move { client.react_to_message(&cid, &mid, &em, None).await })
                    .await;

                // On failure, revert is unnecessary: the authoritative
                // `message_reaction` WS broadcast (sent on success only) won't
                // arrive, and a fresh load restores server state. Surface the
                // error like the web client does.
                if !matches!(res, Ok(Ok(_))) {
                    let _ = cx_handle.update(|cx: &mut App| {
                        toast::error("Gagal mengirim reaksi", cx);
                    });
                }
            }
        })
        .detach();

        cx.notify();
    }

    // ========================================================================
    // ATTACH MENU, PICKERS AND COMPOSE DIALOGS (web parity)
    // ========================================================================

    fn compose_base_url(cx: &App) -> String {
        if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        }
    }

    /// Toggle the attach (+) panel: closes whatever is open, else opens the menu.
    pub fn toggle_attach_panel(&mut self, cx: &mut Context<Self>) {
        self.attach_panel.toggle_menu();
        cx.notify();
    }

    /// Close the attach panel / pickers.
    pub fn close_attach_panel(&mut self, cx: &mut Context<Self>) {
        self.attach_panel = AttachPanel::None;
        cx.notify();
    }

    /// Show the emoji picker panel.
    pub fn open_emoji_panel(&mut self, cx: &mut Context<Self>) {
        self.emoji_picker.open();
        self.attach_panel = AttachPanel::Emoji;
        cx.notify();
    }

    /// Append an emoji to the compose box (web's `addEmoji`).
    pub fn insert_emoji(&mut self, emoji: &str, cx: &mut Context<Self>) {
        self.compose_text.push_str(emoji);
        cx.notify();
    }

    /// Toggle markdown mode (web's "Markdown (on/off)" item).
    pub fn toggle_markdown_mode(&mut self, cx: &mut Context<Self>) {
        self.is_md_mode = !self.is_md_mode;
        self.attach_panel = AttachPanel::None;
        if self.is_md_mode {
            toast::info("Markdown aktif untuk pesan berikutnya", cx);
        } else {
            toast::info("Markdown nonaktif", cx);
        }
        cx.notify();
    }

    /// Show the sticker picker, lazily loading favourites once.
    pub fn open_sticker_panel(&mut self, cx: &mut Context<Self>) {
        self.attach_panel = AttachPanel::Sticker;
        if self.sticker_picker.loaded || self.sticker_picker.is_loading {
            cx.notify();
            return;
        }
        self.sticker_picker.begin_loading();
        let base_url = Self::compose_base_url(cx);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move { client.get_favorite_stickers().await })
                    .await;
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            match res {
                                Ok(Ok(favorites)) => this.sticker_picker.set_favorites(favorites),
                                _ => {
                                    this.sticker_picker.failed();
                                    toast::error("Gagal memuat stiker", cx);
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
        cx.notify();
    }

    /// Send a sticker by its media URL (web's `handleStickerSelect`).
    pub fn send_sticker(&mut self, media_url: String, is_animated: bool, cx: &mut Context<Self>) {
        let Some(chat_id) = self.selected_chat_id.clone() else {
            return;
        };
        self.attach_panel = AttachPanel::None;
        let base_url = Self::compose_base_url(cx);
        let temp_id = self.push_optimistic_attachment(
            cx,
            &chat_id,
            "[Sticker]".to_string(),
            "sticker",
            Some(media_url.clone()),
        );

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let target = chat_id.clone();
                let url = media_url.clone();
                let res = TOKIO_RT
                    .spawn(async move { client.send_sticker(&target, &url, is_animated).await })
                    .await;
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            match res {
                                Ok(Ok(id_res)) => {
                                    this.finish_optimistic_attachment(
                                        cx,
                                        &chat_id,
                                        &temp_id,
                                        Some(id_res.id),
                                    );
                                }
                                _ => {
                                    this.finish_optimistic_attachment(cx, &chat_id, &temp_id, None);
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

    /// Open the native file picker for `kind` and send the picked file.
    pub fn pick_attachment(&mut self, kind: MediaKind, cx: &mut Context<Self>) {
        let Some(chat_id) = self.selected_chat_id.clone() else {
            return;
        };
        self.attach_panel = AttachPanel::None;
        self.is_sending_attachment = true;
        let base_url = Self::compose_base_url(cx);

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let options = PathPromptOptions {
                    files: true,
                    directories: false,
                    multiple: false,
                    prompt: Some(kind.prompt().into()),
                };
                let receiver = cx_handle.update(|cx: &mut App| cx.prompt_for_paths(options));
                let picked = match receiver.await {
                    Ok(Ok(Some(paths))) => paths.into_iter().next(),
                    _ => None,
                };
                let Some(path) = picked else {
                    // Cancelled: nothing to send.
                    let _ = cx_handle.update(|cx: &mut App| {
                        if let Some(view) = this.upgrade() {
                            view.update(cx, |this, cx| {
                                this.is_sending_attachment = false;
                                cx.notify();
                            });
                        }
                    });
                    return;
                };

                let file_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "file".to_string());
                let media_type = kind.media_type(&file_name).to_string();

                let read_path = path.clone();
                let read = TOKIO_RT
                    .spawn_blocking(move || std::fs::read(read_path))
                    .await;
                let Ok(Ok(bytes)) = read else {
                    let _ = cx_handle.update(|cx: &mut App| {
                        if let Some(view) = this.upgrade() {
                            view.update(cx, |this, cx| {
                                this.is_sending_attachment = false;
                                toast::error("Gagal membaca file", cx);
                                cx.notify();
                            });
                        }
                    });
                    return;
                };

                // Optimistic bubble, like the web client's temp message.
                let content = kind.optimistic_content(&file_name, &media_type);
                let msg_type = kind.optimistic_type(&media_type).to_string();
                let mut temp_id = String::new();
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            temp_id = this.push_optimistic_attachment(
                                cx,
                                &chat_id,
                                content.clone(),
                                &msg_type,
                                None,
                            );
                            cx.notify();
                        });
                    }
                });
                if temp_id.is_empty() {
                    return;
                }

                let client = HttpClient::new(&base_url);
                let target = chat_id.clone();
                let file_for_send = file_name.clone();
                let media_for_send = media_type.clone();
                let res = TOKIO_RT
                    .spawn(async move {
                        if media_for_send == "audio" {
                            client
                                .send_audio(&target, bytes, &file_for_send, false, None, None)
                                .await
                        } else {
                            client
                                .send_media(SendMediaBuilder::new(
                                    &target,
                                    media_for_send,
                                    bytes,
                                    file_for_send,
                                ))
                                .await
                        }
                    })
                    .await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_sending_attachment = false;
                            match res {
                                Ok(Ok(id_res)) => {
                                    this.finish_optimistic_attachment(
                                        cx,
                                        &chat_id,
                                        &temp_id,
                                        Some(id_res.id),
                                    );
                                    toast::success("Lampiran terkirim", cx);
                                }
                                _ => {
                                    this.finish_optimistic_attachment(cx, &chat_id, &temp_id, None);
                                    toast::error("Gagal mengirim lampiran", cx);
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
        cx.notify();
    }

    /// Insert the local pending bubble for an outgoing attachment.
    fn push_optimistic_attachment(
        &mut self,
        cx: &mut Context<Self>,
        chat_id: &str,
        content: String,
        msg_type: &str,
        media_url: Option<String>,
    ) -> String {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let temp_id = format!("temp-{}-{}", msg_type, now_ms);
        let msg = Message {
            id: temp_id.clone(),
            chat_id: chat_id.to_string(),
            from: "me".to_string(),
            to: chat_id.to_string(),
            content: content.clone(),
            timestamp: now_ms,
            status: "pending".to_string(),
            message_type: msg_type.to_string(),
            media_url,
            is_automatic: None,
            sender_name: Some("You".to_string()),
            reply_to_id: self.reply_to.as_ref().map(|r| r.id.clone()),
            forwarded: None,
            reactions: None,
            extra: None,
        };

        if cx.has_global::<ChatStore>() {
            let store = ChatStore::global_mut(cx);
            store.upsert_message(chat_id, msg.clone());
            if let Some(c) = store.chats.iter_mut().find(|c| c.id == chat_id) {
                c.last_msg = content.clone();
                c.last_time = now_ms;
            }
        }

        self.cached_render_messages.push(RenderMessage {
            msg,
            is_from_me: true,
            content,
            time_str: Self::format_chat_time(now_ms),
            ticks: MessageTicks::Sent,
            quoted: None,
        });
        let total_items = self.cached_render_messages.len() + 1;
        self.messages_list_state.reset(total_items);
        self.messages_list_state.scroll_to_end();
        temp_id
    }

    /// Reconcile the optimistic attachment bubble with the server response.
    fn finish_optimistic_attachment(
        &mut self,
        cx: &mut Context<Self>,
        chat_id: &str,
        temp_id: &str,
        real_id: Option<String>,
    ) {
        let Some(real_id) = real_id else {
            // Failure: keep the bubble around marked as failed.
            if cx.has_global::<ChatStore>() {
                ChatStore::global_mut(cx).patch_message(chat_id, temp_id, |m| {
                    m.status = "failed".to_string();
                });
            }
            if let Some(rm) = self.cached_render_messages.iter_mut().find(|m| m.msg.id == temp_id) {
                rm.msg.status = "failed".to_string();
                rm.ticks = MessageTicks::Failed;
            }
            self.messages_list_state.remeasure();
            return;
        };

        // The authoritative WS broadcast may have landed first; then the temp
        // bubble is a duplicate and gets dropped (web does the same).
        let already_arrived = cx.has_global::<ChatStore>()
            && ChatStore::global(cx)
                .messages_by_chat
                .get(chat_id)
                .map(|entry| entry.messages.iter().any(|m| m.id == real_id))
                .unwrap_or(false);

        if already_arrived {
            if cx.has_global::<ChatStore>() {
                ChatStore::global_mut(cx).delete_message(chat_id, temp_id);
            }
            self.cached_render_messages.retain(|m| m.msg.id != temp_id);
        } else {
            let rid = real_id.clone();
            if cx.has_global::<ChatStore>() {
                ChatStore::global_mut(cx).patch_message(chat_id, temp_id, move |m| {
                    m.id = rid.clone();
                    m.status = "sent".to_string();
                });
            }
            if let Some(rm) = self.cached_render_messages.iter_mut().find(|m| m.msg.id == temp_id) {
                rm.msg.id = real_id;
                rm.msg.status = "sent".to_string();
            }
        }
        self.messages_list_state.remeasure();
    }

    /// Open one of the poll / location / contact dialogs.
    pub fn open_compose_dialog(&mut self, dialog: ComposeDialog, cx: &mut Context<Self>) {
        self.attach_panel = AttachPanel::None;
        self.active_compose_field = 0;
        self.compose_dialog = Some(dialog);
        cx.notify();
    }

    /// Close the active compose dialog.
    pub fn close_compose_dialog(&mut self, cx: &mut Context<Self>) {
        self.compose_dialog = None;
        self.active_compose_field = 0;
        cx.notify();
    }

    /// Route a keystroke to the focused dialog field.
    fn dialog_key_input(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        match ev.keystroke.key.as_str() {
            "escape" => self.close_compose_dialog(cx),
            "backspace" => {
                if let Some(field) = self.active_dialog_field_mut() {
                    field.pop();
                }
                cx.notify();
            }
            "space" => {
                if let Some(field) = self.active_dialog_field_mut() {
                    field.push(' ');
                }
                cx.notify();
            }
            k if k.len() == 1 => {
                if let Some(field) = self.active_dialog_field_mut() {
                    field.push_str(k);
                }
                cx.notify();
            }
            _ => {}
        }
    }

    fn active_dialog_field_mut(&mut self) -> Option<&mut String> {
        let dialog = self.compose_dialog?;
        let index = self.active_compose_field;
        match dialog {
            ComposeDialog::Poll => {
                if index == 0 {
                    Some(&mut self.poll_draft.question)
                } else {
                    self.poll_draft.options.get_mut(index - 1)
                }
            }
            ComposeDialog::Location => match index {
                0 => Some(&mut self.location_draft.latitude),
                1 => Some(&mut self.location_draft.longitude),
                2 => Some(&mut self.location_draft.name),
                _ => Some(&mut self.location_draft.address),
            },
            ComposeDialog::Contact => match index {
                0 => Some(&mut self.contact_draft.name),
                _ => Some(&mut self.contact_draft.phone),
            },
        }
    }

    /// Send the poll dialog contents (web's `PollDialog`).
    pub fn send_poll(&mut self, cx: &mut Context<Self>) {
        let Some(chat_id) = self.selected_chat_id.clone() else {
            return;
        };
        let (question, options, multi_select) = match self.poll_draft.validated() {
            Ok(parts) => parts,
            Err(msg) => {
                toast::error(msg, cx);
                return;
            }
        };
        if self.is_sending_attachment {
            return;
        }
        self.is_sending_attachment = true;
        let base_url = Self::compose_base_url(cx);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move {
                        client.send_poll(&chat_id, &question, &options, multi_select).await
                    })
                    .await;
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_sending_attachment = false;
                            match res {
                                Ok(Ok(_)) => {
                                    this.poll_draft.reset();
                                    this.close_compose_dialog(cx);
                                    toast::success("Poll terkirim", cx);
                                }
                                _ => toast::error("Gagal mengirim poll", cx),
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
        cx.notify();
    }

    /// Send the location dialog contents (web's `LocationDialog`).
    pub fn send_location(&mut self, cx: &mut Context<Self>) {
        let Some(chat_id) = self.selected_chat_id.clone() else {
            return;
        };
        let (latitude, longitude, name, address, live) = match self.location_draft.validated() {
            Ok(parts) => parts,
            Err(msg) => {
                toast::error(msg, cx);
                return;
            }
        };
        if self.is_sending_attachment {
            return;
        }
        self.is_sending_attachment = true;
        let base_url = Self::compose_base_url(cx);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move {
                        client
                            .send_location(&chat_id, latitude, longitude, Some(&name), Some(&address), Some(live))
                            .await
                    })
                    .await;
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_sending_attachment = false;
                            match res {
                                Ok(Ok(_)) => {
                                    this.location_draft.reset();
                                    this.close_compose_dialog(cx);
                                    toast::success("Lokasi terkirim", cx);
                                }
                                _ => toast::error("Gagal mengirim lokasi", cx),
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
        cx.notify();
    }

    /// Send the contact dialog contents (web's `ContactDialog`).
    pub fn send_contact(&mut self, cx: &mut Context<Self>) {
        let Some(chat_id) = self.selected_chat_id.clone() else {
            return;
        };
        let (name, phone) = match self.contact_draft.validated() {
            Ok(parts) => parts,
            Err(msg) => {
                toast::error(msg, cx);
                return;
            }
        };
        if self.is_sending_attachment {
            return;
        }
        self.is_sending_attachment = true;
        let base_url = Self::compose_base_url(cx);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move { client.send_contact(&chat_id, &name, &phone, None).await })
                    .await;
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_sending_attachment = false;
                            match res {
                                Ok(Ok(_)) => {
                                    this.contact_draft.reset();
                                    this.close_compose_dialog(cx);
                                    toast::success("Kontak terkirim", cx);
                                }
                                _ => toast::error("Gagal mengirim kontak", cx),
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
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
        toast::success("Pesan disalin ke clipboard", cx);
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
        let total_items = if self.cached_render_messages.is_empty() { 0 } else { self.cached_render_messages.len() + 1 };
        self.messages_list_state.reset(total_items);
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
                        view.update(cx, |_this, cx| {
                            match res {
                                Ok(Ok(_)) => {
                                    toast::success("Pesan berhasil dihapus", cx);
                                }
                                _ => {
                                    toast::error("Gagal menghapus pesan", cx);
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

        // Markdown mode wraps the outgoing text in the `{{md:…}}` wire format
        // the renderers understand; the optimistic bubble keeps the plain text.
        let wire_text = if self.is_md_mode {
            encode_markdown(&text)
        } else {
            text.clone()
        };

        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        // Case 1: Editing existing message
        if let Some(edit_msg) = self.editing_message.take() {
            let edit_id = edit_msg.id.clone();
            let sent_text = wire_text.clone();
            let target_chat = chat_id.clone();

            if cx.has_global::<ChatStore>() {
                ChatStore::global_mut(cx).patch_message(&target_chat, &edit_id, |m| {
                    m.content = sent_text.clone();
                });
            }
            if let Some(rm) = self.cached_render_messages.iter_mut().find(|m| m.msg.id == edit_id) {
                rm.content = text.clone();
                rm.msg.content = sent_text.clone();
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
                                        toast::success("Pesan berhasil diedit", cx);
                                    }
                                    _ => {
                                        toast::error("Gagal mengedit pesan", cx);
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
            content: wire_text.clone(),
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
        let total_items = self.cached_render_messages.len() + 1;
        self.messages_list_state.reset(total_items);
        self.messages_list_state.scroll_to_end();

        let target_chat = chat_id.clone();
        let sent_text = wire_text.clone();
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
                                    toast::error("Failed to send message", cx);
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
            toast::error("Nama grup wajib diisi", cx);
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
                                    toast::success("Grup berhasil dibuat", cx);
                                    this.load_chats(cx);
                                }
                                _ => {
                                    toast::error("Gagal membuat grup", cx);
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
            toast::error("Tautan undangan wajib diisi", cx);
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
                                    toast::success("Berhasil bergabung ke grup", cx);
                                    this.load_chats(cx);
                                }
                                _ => {
                                    toast::error("Gagal bergabung ke grup", cx);
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

    /// Render message search sheet drawer (Right side, matching Web / GTK 1:1)
    fn render_search_sheet(
        &self,
        _chat: &Chat,
        theme: ActiveTokens,
        border_color: Hsla,
        bg_color: Hsla,
        _card_bg: Hsla,
        primary_color: Hsla,
        text_color: Hsla,
        muted_text: Hsla,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        v_flex()
            .w(px(SHEET_WIDTH))
            .min_w(px(SHEET_WIDTH))
            .max_w(px(SHEET_WIDTH))
            .h_full()
            .overflow_hidden()
            .border_l_1()
            .border_color(border_color.opacity(0.4))
            .bg(bg_color)
            .flex_shrink_0()
            // Header
            .child(
                v_flex()
                    .border_b_1()
                    .border_color(border_color.opacity(0.4))
                    .bg(theme.muted.opacity(0.2))
                    // Header Title row
                    .child(
                        h_flex()
                            .px_4()
                            .py_3()
                            .justify_between()
                            .items_center()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .child(svg().data(SEARCH_SVG).size(px(18.0)).text_color(primary_color))
                                    .child(
                                        div()
                                            .text_base()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(text_color)
                                            .child("Search Messages"),
                                    ),
                            )
                            .child(
                                div()
                                    .id("btn-close-search-sheet")
                                    .cursor_pointer()
                                    .p_1()
                                    .rounded_full()
                                    .hover(|s| s.bg(theme.muted.opacity(0.65)))
                                    .tooltip(move |window, cx| {
                                        Tooltip::new("Close").build(window, cx)
                                    })
                                    .child(svg().data(X_SVG).size(px(16.0)).text_color(muted_text))                                     .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                         this.close_search_sheet(cx);
                                     })),
                            ),
                    )
                    // Search Input Box
                    .child(
                        div()
                            .px_3()
                            .pb_3()
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
                                    .track_focus(&self.search_sheet_focus)
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                        this.search_sheet_focus.focus(window, cx);
                                    }))
                                    .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                                        match ev.keystroke.key.as_str() {
                                            "backspace" => {
                                                this.search_sheet_query.pop();
                                                this.run_search_messages(cx);
                                            }
                                            "escape" => {
                                                if !this.search_sheet_query.is_empty() {
                                                    this.search_sheet_query.clear();
                                                    this.search_sheet_results.clear();
                                                    this.is_searching_messages = false;
                                                } else {
                                                    this.close_search_sheet(cx);
                                                }
                                                cx.notify();
                                            }
                                            "space" => {
                                                this.search_sheet_query.push(' ');
                                                this.run_search_messages(cx);
                                            }
                                            "enter" => {
                                                this.run_search_messages(cx);
                                            }
                                            k if k.len() == 1 => {
                                                this.search_sheet_query.push_str(k);
                                                this.run_search_messages(cx);
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
                                            .text_color(if self.search_sheet_query.is_empty() { muted_text } else { text_color })
                                            .child(if self.search_sheet_query.is_empty() {
                                                "Search messages...".to_string()
                                            } else {
                                                self.search_sheet_query.clone()
                                            }),
                                    )
                                    .children(if !self.search_sheet_query.is_empty() {
                                        Some(
                                            div()
                                                .id("btn-clear-search-sheet")
                                                .cursor_pointer()
                                                .p_1()
                                                .rounded_full()
                                                .hover(|s| s.bg(theme.muted.opacity(0.65)))
                                                .tooltip(move |window, cx| {
                                                    Tooltip::new("Clear search").build(window, cx)
                                                })
                                                .child(svg().data(X_SVG).size(px(12.0)).text_color(muted_text))
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.search_sheet_query.clear();
                                                    this.search_sheet_results.clear();
                                                    this.is_searching_messages = false;
                                                    cx.notify();
                                                })),
                                        )
                                    } else {
                                        None
                                    }),
                            ),
                    ),
            )
            // Body Content
            .child(
                if self.is_searching_messages && self.search_sheet_results.is_empty() {
                    // Loading state
                    v_flex()
                        .flex_1()
                        .items_center()
                        .justify_center()
                        .gap_3()
                        .child(Icon::new(IconName::LoaderCircle).size(px(24.0)))
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted_text)
                                .child("Searching history..."),
                        )
                        .into_any_element()
                } else if !self.search_sheet_results.is_empty() {
                    // Results list
                    v_flex()
                        .id("search-sheet-results-list")
                        .flex_1()
                        .overflow_y_scroll()
                        .p_2()
                        .gap_1()
                        .children(self.search_sheet_results.iter().map(|m| {
                            let msg_id = m.id.clone();
                            let is_me = m.from == "me" || MessageBubbleHelper::is_from_me(m, "me");
                            let sender = if is_me {
                                "You".to_string()
                            } else {
                                m.sender_name.clone().unwrap_or_else(|| {
                                    m.from.split('@').next().unwrap_or(&m.from).to_string()
                                })
                            };

                            let decoded = MessageBubbleHelper::decode_content(&m.content);
                            let snippet = if decoded.trim().is_empty() {
                                format!("[{}]", if m.message_type.is_empty() { "Message" } else { &m.message_type })
                            } else {
                                decoded
                            };

                            let date_str = Self::format_search_date(m.timestamp);
                            let time_str = Self::format_search_time(m.timestamp);

                            v_flex()
                                .id(SharedString::from(format!("search-result-{}", msg_id)))
                                .cursor_pointer()
                                .p_3()
                                .rounded_xl()
                                .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                .border_b_1()
                                .border_color(border_color.opacity(0.15))
                                .gap_1()
                                .tooltip(move |window, cx| {
                                    Tooltip::new("Click to jump to message").build(window, cx)
                                })
                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                    this.navigate_to_message(&msg_id, cx);
                                }))
                                // Sender name & Date
                                .child(
                                    h_flex()
                                        .justify_between()
                                        .items_center()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(primary_color)
                                                .max_w(px(170.0))
                                                .overflow_hidden()
                                                .child(sender),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(muted_text)
                                                .child(date_str),
                                        ),
                                )
                                // Snippet
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(text_color)
                                        .max_h(px(36.0))
                                        .overflow_hidden()
                                        .child(snippet),
                                )
                                // Time
                                .child(
                                    h_flex()
                                        .justify_end()
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(muted_text.opacity(0.7))
                                                .child(time_str),
                                        ),
                                )
                                .into_any_element()
                        }))
                        .into_any_element()
                } else if !self.search_sheet_query.trim().is_empty() {
                    // Empty results state
                    v_flex()
                        .flex_1()
                        .items_center()
                        .justify_center()
                        .p_6()
                        .gap_3()
                        .child(
                            div()
                                .w(px(56.0))
                                .h(px(56.0))
                                .rounded_2xl()
                                .bg(theme.muted.opacity(0.3))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(svg().data(MESSAGE_SQUARE_SVG).size(px(26.0)).text_color(muted_text.opacity(0.7))),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(text_color)
                                .child("No results found"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted_text)
                                .text_center()
                                .child(format!("We couldn't find any messages matching \"{}\"", self.search_sheet_query)),
                        )
                        .into_any_element()
                } else {
                    // Initial prompt state
                    v_flex()
                        .flex_1()
                        .items_center()
                        .justify_center()
                        .p_6()
                        .gap_3()
                        .child(
                            div()
                                .w(px(56.0))
                                .h(px(56.0))
                                .rounded_2xl()
                                .bg(theme.muted.opacity(0.3))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(svg().data(SEARCH_SVG).size(px(26.0)).text_color(muted_text.opacity(0.7))),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(muted_text)
                                .child("Search for keywords, dates, or phrases"),
                        )
                        .into_any_element()
                },
            )
            .into_any_element()
    }

    /// Wraps a right-hand drawer so it plays the web's sheet motion in both
    /// directions: the slot's width animates while the drawer keeps its full
    /// size and is clipped, so the conversation beside it narrows smoothly
    /// instead of snapping and leaving an empty gutter.
    ///
    /// The open and close ids differ so mounting the closing element restarts
    /// the animation at 0, which the exit reads as "still fully open" before
    /// shrinking it away.
    fn animated_sheet(
        open_id: &'static str,
        close_id: &'static str,
        closing: bool,
        sheet: AnyElement,
    ) -> AnyElement {
        let animation = Animation::new(std::time::Duration::from_millis(SHEET_ANIM_MS))
            .with_easing(|d| 1.0 - (1.0 - d).powi(5));
        let slot = div().h_full().flex_shrink_0().overflow_hidden().child(sheet);
        if closing {
            slot.with_animation(close_id, animation, |el, delta| {
                el.w(px(SHEET_WIDTH * (1.0 - delta).clamp(0.0, 1.0)))
            })
            .into_any_element()
        } else {
            slot.with_animation(open_id, animation, |el, delta| {
                el.w(px(SHEET_WIDTH * delta.clamp(0.0, 1.0)))
            })
            .into_any_element()
        }
    }

    /// One row of the attach (+) menu: tinted lucide icon + label, matching
    /// the web's plus popover.
    fn attach_menu_row(
        label: String,
        icon: &'static [u8],
        accent: Hsla,
        text_color: Hsla,
        hover_bg: Hsla,
        handler: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> AnyElement {
        h_flex()
            .w_full()
            .cursor_pointer()
            .items_center()
            .gap_3()
            .px_3()
            .py_2()
            .rounded_lg()
            .hover(move |s| s.bg(hover_bg))
            .child(svg().data(icon).size(px(16.0)).flex_shrink_0().text_color(accent))
            .child(div().text_sm().text_color(text_color).child(label))
            .on_mouse_down(MouseButton::Left, handler)
            .into_any_element()
    }

    /// Text field row used by the compose dialogs. All rows share one focus
    /// handle; `active_compose_field` decides where keystrokes land.
    fn dialog_field_row(
        index: usize,
        value: &str,
        placeholder: &str,
        is_active: bool,
        theme: ActiveTokens,
        border_color: Hsla,
        text_color: Hsla,
        muted_text: Hsla,
        primary_color: Hsla,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let display = if value.is_empty() {
            placeholder.to_string()
        } else {
            value.to_string()
        };
        h_flex()
            .w_full()
            .px_3()
            .py_2()
            .rounded_xl()
            .border_1()
            .border_color(if is_active { primary_color } else { border_color })
            .bg(theme.background)
            .cursor_text()
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                this.active_compose_field = index;
                this.compose_field_focus.focus(window, cx);
                cx.notify();
            }))
            .child(
                div()
                    .text_sm()
                    .text_color(if value.is_empty() { muted_text } else { text_color })
                    .child(display),
            )
            .into_any_element()
    }

    /// Checkbox row used by the compose dialogs (multi-select, live location).
    fn dialog_toggle_row(
        label: &'static str,
        checked: bool,
        border_color: Hsla,
        text_color: Hsla,
        primary_color: Hsla,
        handler: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> AnyElement {
        h_flex()
            .w_full()
            .cursor_pointer()
            .items_center()
            .gap_2()
            .on_mouse_down(MouseButton::Left, handler)
            .child(
                div()
                    .w(px(18.0))
                    .h(px(18.0))
                    .rounded_md()
                    .border_1()
                    .border_color(if checked { primary_color } else { border_color })
                    .bg(if checked { primary_color } else { rgba(0x00000000).into() })
                    .flex()
                    .items_center()
                    .justify_center()
                    .children(if checked {
                        Some(svg().data(CHECK_SVG).size(px(11.0)).text_color(rgb(0xffffff)))
                    } else {
                        None
                    }),
            )
            .child(div().text_sm().text_color(text_color).child(label))
            .into_any_element()
    }

    /// Footer button used by the compose dialogs.
    fn dialog_button(
        label: String,
        primary: bool,
        border_color: Hsla,
        text_color: Hsla,
        primary_color: Hsla,
        handler: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> AnyElement {
        div()
            .cursor_pointer()
            .px_4()
            .py_2()
            .rounded_xl()
            .border_1()
            .border_color(if primary { primary_color } else { border_color })
            .bg(if primary { primary_color } else { rgba(0x00000000).into() })
            .text_xs()
            .font_weight(if primary { FontWeight::BOLD } else { FontWeight::MEDIUM })
            .text_color(if primary { rgb(0xffffff).into() } else { text_color })
            .child(label)
            .on_mouse_down(MouseButton::Left, handler)
            .into_any_element()
    }

    /// Centered date pill, shared by the in-flow day separators and the
    /// sticky header at the top of the message list.
    fn day_pill(label: String, bg: Hsla, border_color: Hsla, text: Hsla) -> AnyElement {
        div()
            .px_3()
            .py_1()
            .rounded_full()
            .bg(bg)
            .border_1()
            .border_color(border_color)
            .text_xs()
            .font_weight(FontWeight::MEDIUM)
            .text_color(text)
            .child(label)
            .into_any_element()
    }

    /// Date label of the message at `index` in the cached list (clamped).
    fn date_label_at(&self, index: usize) -> Option<String> {
        let len = self.cached_render_messages.len();
        if len == 0 {
            return None;
        }
        let msg_idx = index.saturating_sub(1).min(len - 1);
        self.cached_render_messages
            .get(msg_idx)
            .map(|m| MessageBubbleHelper::format_date_label(m.msg.timestamp).to_uppercase())
    }

    /// Label for the floating day header — the web's `sticky top-0` date pill:
    /// the day group occupying the top of the message list. `None` while that
    /// group's own in-flow pill is still on screen (it *is* the header), and
    /// while the next group's pill has scrolled up to take its place.
    fn sticky_day_label(&self) -> Option<String> {
        let len = self.cached_render_messages.len();
        if len == 0 {
            return None;
        }
        let state = &self.messages_list_state;
        let msg_idx = state
            .logical_scroll_top()
            .item_ix
            .saturating_sub(1)
            .min(len - 1);

        let timestamps: Vec<i64> = self
            .cached_render_messages
            .iter()
            .map(|m| m.msg.timestamp)
            .collect();

        // The in-flow pill for this day is still visible → the real pill wins.
        // Unknown (no layout yet) counts as "not above" so the header never
        // doubles up with the pill it mirrors.
        let pill_ix = MessageBubbleHelper::day_group_pill_index(&timestamps, msg_idx);
        if !state.item_is_above_viewport(pill_ix).unwrap_or(false) {
            return None;
        }

        // The next day's pill has reached the top → it takes over from here.
        if let Some(next_ix) = MessageBubbleHelper::next_day_group_pill_index(&timestamps, msg_idx) {
            if let Some(bounds) = state.bounds_for_item(next_ix) {
                // Header inset (8px) plus the pill's own height (~24px).
                if bounds.top() <= state.viewport_bounds().top() + px(32.0) {
                    return None;
                }
            }
        }

        self.cached_render_messages
            .get(msg_idx)
            .map(|m| MessageBubbleHelper::format_date_label(m.msg.timestamp).to_uppercase())
    }

    /// Document attachment card, mirroring the web's document bubble: file
    /// icon tile, file name, uppercase extension, then Open / Save As.
    fn render_document_card(
        file_name: &str,
        media_url: Option<String>,
        is_dark_bg: bool,
        primary_color: Hsla,
        bubble_text: Rgba,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let name = if file_name.trim().is_empty() {
            "Document".to_string()
        } else {
            file_name.trim().to_string()
        };
        let extension = name
            .rsplit_once('.')
            .map(|(_, ext)| ext.to_uppercase())
            .filter(|ext| !ext.is_empty())
            .unwrap_or_else(|| "FILE".to_string());

        let actions = media_url.map(|url| {
            let open_url = url.clone();
            let download_url = url.clone();
            let save_name = name.clone();
            h_flex()
                .w_full()
                .gap_2()
                .pt_2()
                .border_t_1()
                .border_color(bubble_text.opacity(0.12))
                .child(
                    h_flex()
                        .flex_1()
                        .justify_center()
                        .items_center()
                        .gap_1p5()
                        .py_2()
                        .rounded_lg()
                        .cursor_pointer()
                        .bg(primary_color.opacity(0.12))
                        .hover(move |s| s.bg(primary_color.opacity(0.22)))
                        .child(svg().data(EXTERNAL_LINK_SVG).size(px(14.0)).text_color(primary_color))
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(primary_color)
                                .child("Open"),
                        )
                        .on_mouse_down(MouseButton::Left, cx.listener(move |_this, _, _, cx| {
                            cx.open_url(&open_url);
                        }))
                        .into_any_element(),
                )
                .child(
                    h_flex()
                        .flex_1()
                        .justify_center()
                        .items_center()
                        .gap_1p5()
                        .py_2()
                        .rounded_lg()
                        .cursor_pointer()
                        .bg(bubble_text.opacity(0.08))
                        .hover(move |s| s.bg(bubble_text.opacity(0.16)))
                        .child(svg().data(DOWNLOAD_SVG).size(px(14.0)).text_color(bubble_text))
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(bubble_text)
                                .child("Save As"),
                        )
                        .on_mouse_down(MouseButton::Left, cx.listener(move |_this, _, _, cx| {
                            Self::save_document(download_url.clone(), save_name.clone(), cx);
                        }))
                        .into_any_element(),
                )
                .into_any_element()
        });

        v_flex()
            .mb_1p5()
            .p_2()
            .gap_2()
            .min_w(px(210.0))
            .max_w(px(300.0))
            .rounded_xl()
            .bg(if is_dark_bg {
                rgba(0xffffff14)
            } else {
                rgba(0x00000010)
            })
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .w(px(40.0))
                            .h(px(40.0))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_lg()
                            .bg(primary_color.opacity(0.15))
                            .child(svg().data(FILE_TEXT_SVG).size(px(22.0)).text_color(primary_color)),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w(px(0.0))
                            .overflow_hidden()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(bubble_text)
                                    .overflow_hidden()
                                    .child(name),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(bubble_text.opacity(0.6))
                                    .child(extension),
                            ),
                    ),
            )
            .children(actions)
            .into_any_element()
    }

    /// "Save As" for a document: ask for a destination, download, write it.
    fn save_document(url: String, file_name: String, cx: &mut Context<Self>) {
        let directory = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let base_url = Self::compose_base_url(cx);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let receiver = cx_handle.update(|cx: &mut App| {
                    cx.prompt_for_new_path(&directory, Some(file_name.as_str()))
                });
                let target = match receiver.await {
                    Ok(Ok(Some(path))) => path,
                    _ => return,
                };

                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move { client.download_bytes(&url).await })
                    .await;

                // The write happens outside `update`, where awaiting is not allowed.
                let mut downloaded = false;
                let saved = match res {
                    Ok(Ok(bytes)) => {
                        downloaded = true;
                        let write_path = target.clone();
                        TOKIO_RT
                            .spawn_blocking(move || std::fs::write(&write_path, bytes).is_ok())
                            .await
                            .unwrap_or(false)
                    }
                    _ => false,
                };

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |_this, cx| {
                            if saved {
                                toast::success("Dokumen disimpan", cx);
                            } else if downloaded {
                                toast::error("Gagal menyimpan dokumen", cx);
                            } else {
                                toast::error("Gagal mengunduh dokumen", cx);
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Attach (+) menu, emoji picker and sticker picker, drawn just above the
    /// compose input row (web's `ChatArea` plus popover).
    fn render_attach_panel(
        &self,
        theme: ActiveTokens,
        border_color: Hsla,
        card_bg: Hsla,
        primary_color: Hsla,
        text_color: Hsla,
        muted_text: Hsla,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let hover_bg = theme.muted.opacity(0.5);
        // Floats above the compose bar: absolute so it never takes flow space
        // (an in-flow panel would stretch the compose container over the chat).
        let panel_shell = |children: Vec<AnyElement>| {
            v_flex()
                .absolute()
                .left(px(12.0))
                .bottom_full()
                .mb(px(8.0))
                .w(px(320.0))
                .p_2()
                .gap_1()
                .rounded_2xl()
                .bg(card_bg)
                .border_1()
                .border_color(border_color)
                .shadow_2xl()
                .children(children)
                .into_any_element()
        };

        match self.attach_panel {
            AttachPanel::None => None,
            AttachPanel::Menu => {
                let rows: Vec<AnyElement> = vec![
                    Self::attach_menu_row(
                        "Emoji".to_string(),
                        SMILE_SVG,
                        rgb(0xeab308).into(),
                        text_color,
                        hover_bg,
                        cx.listener(|this, _, _, cx| this.open_emoji_panel(cx)),
                    ),
                    Self::attach_menu_row(
                        "Sticker".to_string(),
                        STICKER_SVG,
                        rgb(0xa855f7).into(),
                        text_color,
                        hover_bg,
                        cx.listener(|this, _, _, cx| this.open_sticker_panel(cx)),
                    ),
                    Self::attach_menu_row(
                        "Media".to_string(),
                        PAPERCLIP_SVG,
                        rgb(0x3b82f6).into(),
                        text_color,
                        hover_bg,
                        cx.listener(|this, _, _, cx| this.pick_attachment(MediaKind::Media, cx)),
                    ),
                    Self::attach_menu_row(
                        "Document".to_string(),
                        FILE_TEXT_SVG,
                        rgb(0xf97316).into(),
                        text_color,
                        hover_bg,
                        cx.listener(|this, _, _, cx| {
                            this.pick_attachment(MediaKind::Document, cx)
                        }),
                    ),
                    Self::attach_menu_row(
                        "Audio".to_string(),
                        MIC_SVG,
                        rgb(0xef4444).into(),
                        text_color,
                        hover_bg,
                        cx.listener(|this, _, _, cx| this.pick_attachment(MediaKind::Audio, cx)),
                    ),
                    Self::attach_menu_row(
                        "GIF".to_string(),
                        IMAGE_PLAY_SVG,
                        rgb(0xec4899).into(),
                        text_color,
                        hover_bg,
                        cx.listener(|this, _, _, cx| this.pick_attachment(MediaKind::Gif, cx)),
                    ),
                    Self::attach_menu_row(
                        "Poll".to_string(),
                        CHART_COLUMN_SVG,
                        rgb(0x06b6d4).into(),
                        text_color,
                        hover_bg,
                        cx.listener(|this, _, window, cx| {
                            this.open_compose_dialog(ComposeDialog::Poll, cx);
                            this.compose_field_focus.focus(window, cx);
                        }),
                    ),
                    Self::attach_menu_row(
                        "Location".to_string(),
                        MAP_PIN_SVG,
                        rgb(0x10b981).into(),
                        text_color,
                        hover_bg,
                        cx.listener(|this, _, window, cx| {
                            this.open_compose_dialog(ComposeDialog::Location, cx);
                            this.compose_field_focus.focus(window, cx);
                        }),
                    ),
                    Self::attach_menu_row(
                        "Contact".to_string(),
                        USER_SVG,
                        rgb(0x6366f1).into(),
                        text_color,
                        hover_bg,
                        cx.listener(|this, _, window, cx| {
                            this.open_compose_dialog(ComposeDialog::Contact, cx);
                            this.compose_field_focus.focus(window, cx);
                        }),
                    ),
                    Self::attach_menu_row(
                        format!("Markdown {}", if self.is_md_mode { "(on)" } else { "(off)" }),
                        FILE_TEXT_SVG,
                        if self.is_md_mode { primary_color } else { muted_text },
                        if self.is_md_mode { primary_color } else { text_color },
                        hover_bg,
                        cx.listener(|this, _, _, cx| this.toggle_markdown_mode(cx)),
                    ),
                ];
                Some(
                    v_flex()
                        .absolute()
                        .left(px(12.0))
                        .bottom_full()
                        .mb(px(8.0))
                        .w(px(216.0))
                        .p_1()
                        .gap_1()
                        .rounded_2xl()
                        .bg(card_bg)
                        .border_1()
                        .border_color(border_color)
                        .shadow_2xl()
                        .children(rows)
                        .into_any_element(),
                )
            }
            AttachPanel::Emoji => {
                let tabs: Vec<AnyElement> = EMOJI_CATEGORIES
                    .iter()
                    .enumerate()
                    .map(|(idx, category)| {
                        let active = idx == self.emoji_picker.selected_category_idx;
                        div()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .cursor_pointer()
                            .text_xs()
                            .bg(if active {
                                primary_color.opacity(0.15)
                            } else {
                                rgba(0x00000000).into()
                            })
                            .text_color(if active { primary_color } else { muted_text })
                            .child(category.name)
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                this.emoji_picker.selected_category_idx = idx;
                                cx.notify();
                            }))
                            .into_any_element()
                    })
                    .collect();

                let emojis: Vec<AnyElement> = self
                    .emoji_picker
                    .current_emojis()
                    .into_iter()
                    .map(|emoji| {
                        let em = emoji.to_string();
                        let em_handler = em.clone();
                        div()
                            .w(px(30.0))
                            .h(px(30.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_md()
                            .cursor_pointer()
                            .text_size(px(16.0))
                            .hover(move |s| s.bg(hover_bg))
                            .child(em)
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                this.insert_emoji(&em_handler, cx);
                            }))
                            .into_any_element()
                    })
                    .collect();

                Some(panel_shell(vec![
                    h_flex()
                        .w_full()
                        .justify_between()
                        .items_center()
                        .px_1()
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(svg().data(SMILE_SVG).size(px(14.0)).text_color(rgb(0xeab308)))
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(text_color)
                                        .child("Emoji"),
                                ),
                        )
                        .child(
                            div()
                                .cursor_pointer()
                                .p_1()
                                .rounded_full()
                                .hover(move |s| s.bg(hover_bg))
                                .child(svg().data(X_SVG).size(px(13.0)).text_color(muted_text))
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                    this.close_attach_panel(cx);
                                })),
                        )
                        .into_any_element(),
                    h_flex()
                        .w_full()
                        .gap_1()
                        .px_1()
                        .children(tabs)
                        .into_any_element(),
                    v_flex()
                        .id("attach-emoji-scroll")
                        .max_h(px(190.0))
                        .overflow_y_scroll()
                        .child(h_flex().flex_wrap().gap_1().children(emojis))
                        .into_any_element(),
                ]))
            }
            AttachPanel::Sticker => {
                let body: AnyElement = if self.sticker_picker.is_loading {
                    h_flex()
                        .w_full()
                        .py_4()
                        .justify_center()
                        .child(Spinner::new().color(primary_color))
                        .into_any_element()
                } else if self.sticker_picker.favorites.is_empty() {
                    div()
                        .w_full()
                        .py_4()
                        .text_xs()
                        .text_color(muted_text)
                        .child("Belum ada stiker favorit")
                        .into_any_element()
                } else {
                    let base_url = Self::compose_base_url(cx);
                    let stickers: Vec<AnyElement> = self
                        .sticker_picker
                        .favorites
                        .iter()
                        .map(|fav| {
                            let url = resolve_media_url(&fav.media_url, &base_url);
                            let send_url = fav.media_url.clone();
                            let is_animated = fav.is_animated;
                            div()
                                .w(px(64.0))
                                .h(px(64.0))
                                .rounded_lg()
                                .overflow_hidden()
                                .cursor_pointer()
                                .bg(theme.muted.opacity(0.35))
                                .child(
                                    img(url)
                                        .id(format!("sticker-fav-{}", fav.media_url))
                                        .w_full()
                                        .h_full()
                                        .object_fit(ObjectFit::Contain),
                                )
                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                    this.send_sticker(send_url.clone(), is_animated, cx);
                                }))
                                .into_any_element()
                        })
                        .collect();
                    v_flex()
                        .id("attach-sticker-scroll")
                        .max_h(px(190.0))
                        .overflow_y_scroll()
                        .child(h_flex().flex_wrap().gap_2().children(stickers))
                        .into_any_element()
                };

                Some(panel_shell(vec![
                    h_flex()
                        .w_full()
                        .justify_between()
                        .items_center()
                        .px_1()
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(svg().data(STICKER_SVG).size(px(14.0)).text_color(rgb(0xa855f7)))
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(text_color)
                                        .child("Sticker"),
                                ),
                        )
                        .child(
                            div()
                                .cursor_pointer()
                                .p_1()
                                .rounded_full()
                                .hover(move |s| s.bg(hover_bg))
                                .child(svg().data(X_SVG).size(px(13.0)).text_color(muted_text))
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                    this.close_attach_panel(cx);
                                })),
                        )
                        .into_any_element(),
                    body,
                ]))
            }
        }
    }

    /// Poll / location / contact modal, mirroring the web's compose dialogs.
    fn render_compose_dialog(
        &self,
        theme: ActiveTokens,
        border_color: Hsla,
        card_bg: Hsla,
        primary_color: Hsla,
        text_color: Hsla,
        muted_text: Hsla,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let dialog = self.compose_dialog?;
        let active = self.active_compose_field;
        let busy = self.is_sending_attachment;

        let (title, body, action_label): (&str, Vec<AnyElement>, &str) = match dialog {
            ComposeDialog::Poll => {
                let options: Vec<AnyElement> = self
                    .poll_draft
                    .options
                    .iter()
                    .enumerate()
                    .map(|(idx, option)| {
                        let index = idx + 1;
                        let can_remove = self.poll_draft.options.len() > 2;
                        h_flex()
                            .w_full()
                            .gap_2()
                            .items_center()
                            .child(
                                div().flex_1().child(Self::dialog_field_row(
                                    index,
                                    option,
                                    &format!("Opsi {}", index),
                                    active == index,
                                    theme,
                                    border_color,
                                    text_color,
                                    muted_text,
                                    primary_color,
                                    cx,
                                )),
                            )
                            .children(if can_remove {
                                Some(
                                    div()
                                        .cursor_pointer()
                                        .p_2()
                                        .rounded_lg()
                                        .hover(move |s| s.bg(theme.muted.opacity(0.5)))
                                        .child(svg().data(X_SVG).size(px(13.0)).text_color(muted_text))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |this, _, _, cx| {
                                                this.poll_draft.remove_option(idx);
                                                if this.active_compose_field > idx + 1 {
                                                    this.active_compose_field -= 1;
                                                }
                                                cx.notify();
                                            }),
                                        )
                                        .into_any_element(),
                                )
                            } else {
                                None
                            })
                            .into_any_element()
                    })
                    .collect();

                let at_max = self.poll_draft.options.len() >= crate::components::compose::POLL_MAX_OPTIONS;
                let mut body: Vec<AnyElement> = vec![Self::dialog_field_row(
                    0,
                    &self.poll_draft.question,
                    "Pertanyaan",
                    active == 0,
                    theme,
                    border_color,
                    text_color,
                    muted_text,
                    primary_color,
                    cx,
                )];
                body.extend(options);
                body.push(
                    div()
                        .w_full()
                        .cursor_pointer()
                        .px_3()
                        .py_1p5()
                        .rounded_lg()
                        .border_1()
                        .border_color(border_color)
                        .text_xs()
                        .text_color(if at_max { muted_text } else { primary_color })
                        .child("+ Tambah opsi")
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                            this.poll_draft.add_option();
                            cx.notify();
                        }))
                        .into_any_element(),
                );
                body.push(Self::dialog_toggle_row(
                    "Izinkan jawaban ganda",
                    self.poll_draft.multi_select,
                    border_color,
                    text_color,
                    primary_color,
                    cx.listener(|this, _, _, cx| {
                        this.poll_draft.multi_select = !this.poll_draft.multi_select;
                        cx.notify();
                    }),
                ));
                ("Buat poll", body, "Kirim")
            }
            ComposeDialog::Location => {
                let body = vec![
                    h_flex()
                        .w_full()
                        .gap_2()
                        .child(
                            div().flex_1().child(Self::dialog_field_row(
                                0,
                                &self.location_draft.latitude,
                                "Latitude",
                                active == 0,
                                theme,
                                border_color,
                                text_color,
                                muted_text,
                                primary_color,
                                cx,
                            )),
                        )
                        .child(
                            div().flex_1().child(Self::dialog_field_row(
                                1,
                                &self.location_draft.longitude,
                                "Longitude",
                                active == 1,
                                theme,
                                border_color,
                                text_color,
                                muted_text,
                                primary_color,
                                cx,
                            )),
                        )
                        .into_any_element(),
                    Self::dialog_field_row(
                        2,
                        &self.location_draft.name,
                        "Nama tempat (opsional)",
                        active == 2,
                        theme,
                        border_color,
                        text_color,
                        muted_text,
                        primary_color,
                        cx,
                    ),
                    Self::dialog_field_row(
                        3,
                        &self.location_draft.address,
                        "Alamat (opsional)",
                        active == 3,
                        theme,
                        border_color,
                        text_color,
                        muted_text,
                        primary_color,
                        cx,
                    ),
                    Self::dialog_toggle_row(
                        "Bagikan lokasi live",
                        self.location_draft.live,
                        border_color,
                        text_color,
                        primary_color,
                        cx.listener(|this, _, _, cx| {
                            this.location_draft.live = !this.location_draft.live;
                            cx.notify();
                        }),
                    ),
                ];
                ("Bagikan lokasi", body, "Bagikan")
            }
            ComposeDialog::Contact => {
                let body = vec![
                    Self::dialog_field_row(
                        0,
                        &self.contact_draft.name,
                        "Nama kontak",
                        active == 0,
                        theme,
                        border_color,
                        text_color,
                        muted_text,
                        primary_color,
                        cx,
                    ),
                    Self::dialog_field_row(
                        1,
                        &self.contact_draft.phone,
                        "Nomor telepon (mis. +62812…)",
                        active == 1,
                        theme,
                        border_color,
                        text_color,
                        muted_text,
                        primary_color,
                        cx,
                    ),
                ];
                ("Bagikan kontak", body, "Bagikan")
            }
        };

        let submit = move |this: &mut ChatView, cx: &mut Context<ChatView>| match dialog {
            ComposeDialog::Poll => this.send_poll(cx),
            ComposeDialog::Location => this.send_location(cx),
            ComposeDialog::Contact => this.send_contact(cx),
        };

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
                        .w(px(420.0))
                        .p_5()
                        .gap_3()
                        .rounded_2xl()
                        .bg(card_bg)
                        .border_1()
                        .border_color(border_color)
                        .shadow_2xl()
                        .track_focus(&self.compose_field_focus)
                        .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                            this.dialog_key_input(ev, cx);
                        }))
                        .child(
                            h_flex()
                                .w_full()
                                .justify_between()
                                .items_center()
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(text_color)
                                        .child(title),
                                )
                                .child(
                                    div()
                                        .cursor_pointer()
                                        .p_1()
                                        .rounded_full()
                                        .hover(move |s| s.bg(theme.muted.opacity(0.5)))
                                        .child(svg().data(X_SVG).size(px(15.0)).text_color(muted_text))
                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                            this.close_compose_dialog(cx);
                                        })),
                                ),
                        )
                        .children(body)
                        .child(
                            h_flex()
                                .w_full()
                                .justify_end()
                                .gap_2()
                                .child(Self::dialog_button(
                                    "Batal".to_string(),
                                    false,
                                    border_color,
                                    text_color,
                                    primary_color,
                                    cx.listener(|this, _, _, cx| this.close_compose_dialog(cx)),
                                ))
                                .child(Self::dialog_button(
                                    if busy { "Mengirim…".to_string() } else { action_label.to_string() },
                                    true,
                                    border_color,
                                    text_color,
                                    primary_color,
                                    cx.listener(move |this, _, _, cx| submit(this, cx)),
                                )),
                        ),
                )
                .into_any_element(),
        )
    }
}

impl Render for ChatView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.app_theme();
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

        let (chats_version, archived_count) = if cx.has_global::<ChatStore>() {
            let store = ChatStore::global(cx);
            (store.chats_version, store.chats.iter().filter(|c| c.archived).count())
        } else {
            (0, 0)
        };

        // Cache filtered and sorted chats to avoid expensive allocations during 60fps scrolling
        let filter_key = (chats_version, self.search_query.clone(), self.filter, self.archived_mode);
        if self.last_filter_key != filter_key {
            let empty_chats = Vec::new();
            let all_chats = if cx.has_global::<ChatStore>() {
                &ChatStore::global(cx).chats
            } else {
                &empty_chats
            };

            let query = self.search_query.trim().to_lowercase();
            let archived_mode = self.archived_mode;
            let current_filter = self.filter;

            let mut filtered: Vec<Chat> = all_chats
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
            filtered.sort_by(|a, b| {
                match (a.pinned_at, b.pinned_at) {
                    (Some(pa), Some(pb)) => pb.cmp(&pa),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => b.last_time.cmp(&a.last_time),
                }
            });

            self.cached_filtered_chats = filtered;
            self.last_filter_key = filter_key;
        }

        let selected_chat = self
            .selected_chat_id
            .as_ref()
            .and_then(|id| {
                if cx.has_global::<ChatStore>() {
                    ChatStore::global(cx).chats.iter().find(|c| c.id == *id).cloned()
                } else {
                    None
                }
            });

        let selected_chat_id = self.selected_chat_id.clone();
        let is_loading_chats = self.is_loading_chats;
        let is_loading_messages = self.is_loading_messages;

        // Ensure cached messages match currently selected chat, and rebuild
        // when the store reports any change (new/edited/deleted messages,
        // status ticks, reactions) so live WS updates repaint the bubbles.
        let messages_version = if cx.has_global::<ChatStore>() {
            ChatStore::global(cx).messages_version
        } else {
            0
        };
        if let Some(ref sc) = selected_chat {
            if self.cached_chat_id.as_deref() != Some(&sc.id) {
                if cx.has_global::<ChatStore>() {
                    if let Some(entry) = ChatStore::global(cx).messages_by_chat.get(&sc.id) {
                        self.rebuild_message_cache(&sc.id, &entry.messages);
                    }
                }
            } else if self.last_messages_version != messages_version {
                if cx.has_global::<ChatStore>() {
                    if let Some(entry) = ChatStore::global(cx).messages_by_chat.get(&sc.id) {
                        self.rebuild_message_cache_preserve_scroll(&sc.id, &entry.messages);
                    }
                }
            }
        }
        self.last_messages_version = messages_version;

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
                    .w(px(self.sidebar_width))
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
                                            .children(if self.archived_mode {
                                                Some(
                                                    h_flex()
                                                        .id("btn-back-archived")
                                                        .cursor_pointer()
                                                        .items_center()
                                                        .gap_1()
                                                        .px_2()
                                                        .py_1()
                                                        .rounded_lg()
                                                        .hover(|s| s.bg(theme.muted.opacity(0.6)))
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
                                                    .child(if self.archived_mode { "Archived" } else { "Messages" }),
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
                                                    .id("btn-new-group")
                                                    .cursor_pointer()
                                                    .p_2()
                                                    .rounded_full()
                                                    .hover(|s| s.bg(theme.muted.opacity(0.65)))
                                                    .tooltip(move |window, cx| {
                                                        Tooltip::new("New Group").build(window, cx)
                                                    })
                                                    .child(svg().data(USERS_SVG).size(px(18.0)).text_color(muted_text))
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                        this.is_new_group_open = true;
                                                        cx.notify();
                                                    })),
                                            )
                                            // Join Group via Link Button
                                            .child(
                                                div()
                                                    .id("btn-join-group")
                                                    .cursor_pointer()
                                                    .p_2()
                                                    .rounded_full()
                                                    .hover(|s| s.bg(theme.muted.opacity(0.65)))
                                                    .tooltip(move |window, cx| {
                                                        Tooltip::new("Join Group via Link").build(window, cx)
                                                    })
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
                                                .id("btn-clear-search")
                                                .cursor_pointer()
                                                .p_1()
                                                .rounded_full()
                                                .hover(|s| s.bg(theme.muted.opacity(0.65)))
                                                .tooltip(move |window, cx| {
                                                    Tooltip::new("Clear search").build(window, cx)
                                                })
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
                    .children(if !self.archived_mode && self.search_query.is_empty() && archived_count > 0 {
                        Some(
                            h_flex()
                                .id("btn-archived-row")
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
                    // Chat Scroll List (Uniform 72px item height strictly enforced) with scrollbar
                    .child(
                        div()
                            .flex_1()
                            .relative()
                            .overflow_hidden()
                            .child(if is_loading_chats && self.cached_filtered_chats.is_empty() {
                                v_flex()
                                    .size_full()
                                    .p_2()
                                    .child(
                                        v_flex()
                                            .py_12()
                                            .items_center()
                                            .justify_center()
                                            .gap_2()
                                            .child(Icon::new(IconName::LoaderCircle).size(px(24.)))
                                            .child(div().text_xs().text_color(muted_text).child("Loading conversations..."))
                                    )
                                    .into_any_element()
                            } else if self.cached_filtered_chats.is_empty() {
                                v_flex()
                                    .size_full()
                                    .p_2()
                                    .child(
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
                                    )
                                    .into_any_element()
                            } else {
                                uniform_list(
                                    "chat-list-scroll",
                                    self.cached_filtered_chats.len(),
                                    cx.processor(|this, range: Range<usize>, _window, cx| {
                                        let theme = cx.app_theme();
                                        let muted_text = theme.muted_foreground;
                                        let text_color = theme.foreground;
                                        let selected_chat_id = this.selected_chat_id.clone();
                                        let base_url = if cx.has_global::<AuthState>() {
                                            AuthState::global(cx).base_url.clone()
                                        } else {
                                            "http://127.0.0.1:3000/api".to_string()
                                        };

                                        let mut items = Vec::with_capacity(range.len());
                                        for ix in range {
                                            if let Some(chat) = this.cached_filtered_chats.get(ix) {
                                                let is_selected = selected_chat_id.as_deref() == Some(&chat.id);
                                                let chat_id = chat.id.clone();
                                                let is_pinned = chat.pinned_at.is_some();
                                                let is_muted = chat.mute_mode != "off" && !chat.mute_mode.is_empty();

                                                let first_line = chat.last_msg.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
                                                let last_msg_snippet = if first_line.is_empty() {
                                                    "Tap to chat".to_string()
                                                } else {
                                                    MessageBubbleHelper::decode_content(first_line)
                                                };

                                                let formatted_time = Self::format_chat_time(chat.last_time);
                                                let unread = chat.unread;

                                                let card = h_flex()
                                                    .id(SharedString::from(chat.id.clone()))
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
                                                        theme.primary.opacity(0.14)
                                                    } else {
                                                        rgba(0x00000000).into()
                                                    })
                                                    .hover(|s| {
                                                        if is_selected {
                                                            s.bg(theme.primary.opacity(0.22))
                                                        } else {
                                                            s.bg(theme.muted.opacity(0.65))
                                                        }
                                                    })
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
                                                    // Contact Avatar
                                                    .child(
                                                        div()
                                                            .w(px(48.0))
                                                            .h(px(48.0))
                                                            .rounded_full()
                                                            .flex_shrink_0()
                                                            .overflow_hidden()
                                                            .relative()
                                                            .bg(avatar_color_for(&chat_id))
                                                            .child(
                                                                div()
                                                                    .size_full()
                                                                    .flex()
                                                                    .items_center()
                                                                    .justify_center()
                                                                    .text_color(rgb(0xffffff))
                                                                    .font_weight(FontWeight::BOLD)
                                                                    .text_sm()
                                                                    .child(
                                                                        chat.name
                                                                            .chars()
                                                                            .next()
                                                                            .map(|c| c.to_uppercase().to_string())
                                                                            .unwrap_or_else(|| "#".to_string()),
                                                                    ),
                                                            )
                                                            .children(if !chat.avatar.is_empty() {
                                                                let a_url = resolve_avatar_url(&chat.avatar, &chat.id, &base_url);
                                                                Some(
                                                                    img(a_url)
                                                                        .absolute()
                                                                        .inset_0()
                                                                        .size_full()
                                                                        .rounded_full()
                                                                        .object_fit(ObjectFit::Cover),
                                                                )
                                                            } else {
                                                                None
                                                            }),
                                                    )
                                                    // Chat Title & Last Message
                                                    .child(
                                                        v_flex()
                                                            .flex_1()
                                                            .min_w(px(0.0))
                                                            .justify_center()
                                                            .gap_0p5()
                                                            .child(
                                                                h_flex()
                                                                    .items_center()
                                                                    .gap_1p5()
                                                                    .child(
                                                                        div()
                                                                            .flex_1()
                                                                            .truncate()
                                                                            .text_sm()
                                                                            .font_weight(if unread > 0 { FontWeight::BOLD } else { FontWeight::MEDIUM })
                                                                            .text_color(text_color)
                                                                            .child(chat.name.clone()),
                                                                    )
                                                                    .children(if is_muted {
                                                                        Some(
                                                                            svg()
                                                                                .data(VOLUME_X_SVG)
                                                                                .size(px(13.0))
                                                                                .text_color(muted_text)
                                                                                .flex_shrink_0(),
                                                                        )
                                                                    } else {
                                                                        None
                                                                    })
                                                                    .children(if is_pinned {
                                                                        Some(
                                                                            svg()
                                                                                .data(PIN_SVG)
                                                                                .size(px(12.0))
                                                                                .text_color(muted_text)
                                                                                .flex_shrink_0(),
                                                                        )
                                                                    } else {
                                                                        None
                                                                    }),
                                                            )
                                                            .child(
                                                                div()
                                                                    .truncate()
                                                                    .text_xs()
                                                                    .text_color(if unread > 0 { text_color } else { muted_text })
                                                                    .child(last_msg_snippet),
                                                            ),
                                                    )
                                                    // Timestamp & Unread Badge
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
                                                                        .px_1()
                                                                        .h(px(UNREAD_BADGE_SIZE))
                                                                        .min_w(px(UNREAD_BADGE_SIZE))
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
                                                    .into_any_element();
                                                items.push(card);
                                            }
                                        }
                                        items
                                    }),
                                )
                                .size_full()
                                .p_2()
                                .track_scroll(&self.chat_list_scroll_handle)
                                .into_any_element()
                            })
                            .child(
                                Scrollbar::vertical(&self.chat_list_scroll_handle)
                                    .mode(ScrollbarMode::Hover)
                                    .styles(|s| {
                                        s.track(|t| t.bg(transparent_black()))
                                            .thumb(|th| th.bg(theme.muted_foreground.opacity(0.35)).radius(px(3.0)).width(px(6.0)))
                                            .thumb_hover(|th| th.bg(theme.muted_foreground.opacity(0.65)).radius(px(4.0)).width(px(8.0)))
                                            .thumb_active(|th| th.bg(theme.primary.opacity(0.8)).radius(px(4.0)).width(px(8.0)))
                                    }),
                            ),
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
                                                .id("chat-header-profile")
                                                .items_center()
                                                .gap_3()
                                                .cursor_pointer()
                                                .px_2()
                                                .py_1p5()
                                                .rounded_xl()
                                                .hover(|s| s.bg(theme.muted.opacity(0.6)))
                                                .tooltip(move |window, cx| {
                                                    Tooltip::new("Click for contact info").build(window, cx)
                                                })
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
                                        .child({
                                            let is_group_chat = active_chat.is_group;
                                            h_flex()
                                                .items_center()
                                                .gap_1()
                                                .child(
                                                    div()
                                                        .id("btn-header-phone")
                                                        .cursor_pointer()
                                                        .p_2()
                                                        .rounded_full()
                                                        .hover(|s| s.bg(theme.muted.opacity(0.65)))
                                                        .tooltip(move |window, cx| {
                                                            Tooltip::new(if is_group_chat { "Group voice call" } else { "Voice call" }).build(window, cx)
                                                        })
                                                        .child(svg().data(PHONE_SVG).size(px(18.0)).text_color(muted_text))
                                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                            this.start_call(false, cx);
                                                        })),
                                                )
                                                .child(
                                                    div()
                                                        .id("btn-header-video")
                                                        .cursor_pointer()
                                                        .p_2()
                                                        .rounded_full()
                                                        .hover(|s| s.bg(theme.muted.opacity(0.65)))
                                                        .tooltip(move |window, cx| {
                                                            Tooltip::new(if is_group_chat { "Group video call" } else { "Video call" }).build(window, cx)
                                                        })
                                                        .child(svg().data(VIDEO_SVG).size(px(18.0)).text_color(muted_text))
                                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                            this.start_call(true, cx);
                                                        })),
                                                )
                                                .child(
                                                    div()
                                                        .id("btn-header-search")
                                                        .cursor_pointer()
                                                        .p_2()
                                                        .rounded_full()
                                                        .hover(|s| s.bg(theme.muted.opacity(0.65)))
                                                        .tooltip(move |window, cx| {
                                                            Tooltip::new("Search in conversation").build(window, cx)
                                                        })
                                                        .child(svg().data(SEARCH_SVG).size(px(18.0)).text_color(muted_text))
                                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                                            this.toggle_search_sheet(window, cx);
                                                        })),
                                                )
                                                .child(
                                                    div()
                                                        .id("btn-header-more")
                                                        .cursor_pointer()
                                                        .p_2()
                                                        .rounded_full()
                                                        .hover(|s| s.bg(theme.muted.opacity(0.65)))
                                                        .tooltip(move |window, cx| {
                                                            Tooltip::new("Chat info").build(window, cx)
                                                        })
                                                        .child(svg().data(MORE_VERTICAL_SVG).size(px(18.0)).text_color(muted_text))
                                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                            this.toggle_info_sheet(cx);
                                                        })),
                                                )
                                        }),
                                )
                                // Messages History View (Optimized for 60fps scrolling) with visual scrollbar
                                .child(
                                    div()
                                        .flex_1()
                                        .relative()
                                        .overflow_hidden()
                                        .child(if is_loading_messages && self.cached_render_messages.is_empty() {
                                            v_flex()
                                                .size_full()
                                                .p_6()
                                                .child(
                                                    v_flex()
                                                        .py_16()
                                                        .items_center()
                                                        .justify_center()
                                                        .gap_2()
                                                        .child(Icon::new(IconName::LoaderCircle).size(px(24.)))
                                                        .child(div().text_xs().text_color(muted_text).child("Loading messages..."))
                                                )
                                                .into_any_element()
                                        } else if self.cached_render_messages.is_empty() {
                                            v_flex()
                                                .size_full()
                                                .p_6()
                                                .child(
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
                                                )
                                                .into_any_element()
                                        } else {
                                            list(
                                                self.messages_list_state.clone(),
                                                cx.processor(|this, index: usize, _window, cx| {
                                                    let theme = *cx.app_theme();
                                                    let primary_color = theme.primary;
                                                    let muted_text = theme.muted_foreground;
                                                    let _text_color = theme.foreground;
                                                    let base_url = if cx.has_global::<AuthState>() {
                                                        AuthState::global(cx).base_url.clone()
                                                    } else {
                                                        "http://127.0.0.1:3000/api".to_string()
                                                    };

                                                    let is_group_chat = this.selected_chat_id.as_ref().map(|cid| {
                                                        cid.ends_with("@g.us") || if cx.has_global::<ChatStore>() {
                                                            ChatStore::global(cx).chats.iter().find(|c| c.id == *cid).map(|c| c.is_group).unwrap_or(false)
                                                        } else {
                                                            false
                                                        }
                                                    }).unwrap_or(false);

                                                    // Auto-load older messages when user scrolls near top
                                                    if index <= 2 && this.has_more && !this.is_loading_more && !this.is_loading_messages {
                                                        this.load_older_messages(cx);
                                                    }
                                                    // Auto-load newer messages when user scrolls near bottom
                                                    if index + 3 >= this.cached_render_messages.len() && this.has_more_next && !this.is_loading_newer && !this.is_loading_messages {
                                                        this.load_newer_messages(cx);
                                                    }

                                                    let body: AnyElement = if index == 0 {
                                                        let first_label = this
                                                            .date_label_at(1)
                                                            .unwrap_or_else(|| "TODAY".to_string());
                                                        h_flex()
                                                            .w_full()
                                                            .justify_center()
                                                            .py_3()
                                                            .child(Self::day_pill(
                                                                first_label,
                                                                theme.muted.opacity(0.4),
                                                                rgba(0x00000000).into(),
                                                                muted_text,
                                                            ))
                                                            .into_any_element()
                                                    } else if let Some(r_msg) = this.cached_render_messages.get(index - 1) {
                                                        let msg_idx = index - 1;
                                                        let is_from_me = r_msg.is_from_me;
                                                        let is_first_in_sequence = if msg_idx > 0 {
                                                            if let Some(prev) = this.cached_render_messages.get(msg_idx - 1) {
                                                                prev.msg.from != r_msg.msg.from
                                                            } else {
                                                                true
                                                            }
                                                        } else {
                                                            true
                                                        };

                                                        let sender_jid = r_msg.msg.from.clone();
                                                        let sender_name = r_msg.msg.sender_name.clone().unwrap_or_else(|| {
                                                            sender_jid.split('@').next().unwrap_or(&sender_jid).to_string()
                                                        });
                                                        let sender_color = avatar_color_for(&sender_jid);

                                                        let content = r_msg.content.clone();
                                                        let time_str = r_msg.time_str.clone();
                                                        let ticks = r_msg.ticks;
                                                        let quoted = r_msg.quoted.clone();
                                                        let msg_clone = r_msg.msg.clone();

                                                        let msg_type = r_msg.msg.message_type.as_str();
                                                        let is_image = msg_type == "image" || (r_msg.msg.media_url.is_some() && (content == "[Image]" || content.is_empty()));
                                                        let is_sticker = msg_type == "sticker" || (r_msg.msg.media_url.is_some() && content == "[Sticker]");
                                                        let is_video = msg_type == "video" || (r_msg.msg.media_url.is_some() && content == "[Video]");
                                                        // Documents keep their file name as content, so the type
                                                        // (or a file extension) is the only reliable signal.
                                                        let doc_file_name = content.clone();
                                                        let is_document = msg_type == "document"
                                                            || (r_msg.msg.media_url.is_some()
                                                                && looks_like_document_name(&doc_file_name));

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

                                                        let is_highlighted = this.highlighted_message_id.as_deref() == Some(&r_msg.msg.id);
                                                        let base_bubble_bg: Hsla = bubble_bg.into();
                                                        let final_bubble_bg = if is_highlighted { theme.primary.opacity(0.35) } else { base_bubble_bg };
                                                        let mut bubble = v_flex()
                                                            .max_w(px(520.0))
                                                            .px_3p5()
                                                            .py_2()
                                                            .rounded_2xl()
                                                            .bg(final_bubble_bg)
                                                            .shadow_sm()
                                                            .gap_1();
                                                        if is_highlighted {
                                                            bubble = bubble.border_2().border_color(primary_color);
                                                        }
                                                        let bubble = bubble
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
                                                                                // The id is what lets gpui key frame state for
                                                                                // multi-frame GIF/WebP; without it only the first
                                                                                // frame ever renders.
                                                                                img(full_url)
                                                                                    .id(format!("msg-image-{}", r_msg.msg.id))
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
                                                                    let click_url = full_url.clone();
                                                                    Some(
                                                                        div()
                                                                            .w(px(160.0))
                                                                            .h(px(160.0))
                                                                            .cursor_pointer()
                                                                            // Web parity: tapping a sticker opens it in the
                                                                            // full-screen viewer, animated.
                                                                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                                                this.preview_image_url = Some(click_url.clone());
                                                                                cx.notify();
                                                                            }))
                                                                            .child(
                                                                                img(full_url)
                                                                                    .id(format!("msg-sticker-{}", r_msg.msg.id))
                                                                                    .w(px(160.0))
                                                                                    .h(px(160.0))
                                                                                    .object_fit(ObjectFit::Contain)
                                                                            )
                                                                            .into_any_element()
                                                                    )
                                                                } else {
                                                                    None
                                                                }
                                                            } else if is_document {
                                                                // Web's document card: icon tile, file name,
                                                                // extension, then Open / Save As actions.
                                                                Some(Self::render_document_card(
                                                                    &doc_file_name,
                                                                    r_msg.msg.media_url.as_ref().map(|u| resolve_media_url(u, &base_url)),
                                                                    is_dark_bg,
                                                                    primary_color,
                                                                    bubble_text,
                                                                    cx,
                                                                ))
                                                            } else {
                                                                None
                                                            })
                                                            // Message Body Text (hide placeholder if it's media without caption)
                                                            .children({
                                                                let is_media_placeholder = (is_image && (content == "[Image]" || content.is_empty()))
                                                                    || (is_sticker && (content == "[Sticker]" || content.is_empty()))
                                                                    || (is_video && (content == "[Video]" || content.is_empty()))
                                                                    || is_document;
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
                                                                            MessageTicks::Failed => Some(
                                                                                svg()
                                                                                    .data(X_SVG)
                                                                                    .size(px(12.0))
                                                                                    .text_color(rgb(0xef4444)),
                                                                            ),
                                                                            MessageTicks::None => None,
                                                                        }
                                                                    } else {
                                                                        None
                                                                    }),
                                                            );

                                                        // Reaction chips row (like web's ReactionChips):
                                                        // floats above the bubble, tap toggles own reaction.
                                                        let reactions_chips = {
                                                            let reactions_list = r_msg.msg.reactions.clone().unwrap_or_default();
                                                            if reactions_list.is_empty() {
                                                                None
                                                            } else {
                                                                let cid_for_chips = this.selected_chat_id.clone().unwrap_or_default();
                                                                Some(
                                                                    h_flex()
                                                                        .flex_wrap()
                                                                        .gap_1()
                                                                        .mb(px(-2.0))
                                                                        .relative()
                                                                        .children(reactions_list.iter().map(|r| {
                                                                            let em = r.emoji.clone();
                                                                            let cid = cid_for_chips.clone();
                                                                            let mid = r_msg.msg.id.clone();
                                                                            let count_label = if r.senders.len() > 1 {
                                                                                format!("{}", r.senders.len())
                                                                            } else {
                                                                                String::new()
                                                                            };
                                                                            div()
                                                                                .cursor_pointer()
                                                                                .px_1p5()
                                                                                .py(px(2.0))
                                                                                .rounded_full()
                                                                                .border_1()
                                                                                .border_color(theme.border.opacity(0.5))
                                                                                .bg(if is_dark_bg { rgba(0xffffff1a) } else { rgba(0x0000000d) })
                                                                                .text_size(px(12.0))
                                                                                .hover(|s| s.bg(if is_dark_bg { rgba(0xffffff33) } else { rgba(0x0000001a) }))
                                                                                .child(h_flex()
                                                                                    .items_center()
                                                                                    .gap(px(2.0))
                                                                                    .child(r.emoji.clone())
                                                                                    .children(if count_label.is_empty() { None } else { Some(
                                                                                        div()
                                                                                            .text_size(px(10.0))
                                                                                            .font_weight(FontWeight::BOLD)
                                                                                            .opacity(0.7)
                                                                                            .child(count_label),
                                                                                    ) }))
                                                                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                                                    this.react_message(cid.clone(), mid.clone(), em.clone(), cx);
                                                                                }))
                                                                                .into_any_element()
                                                                        })),
                                                                )
                                                            }
                                                        };

                                                        if is_from_me {
                                                            v_flex()
                                                                .w_full()
                                                                .items_end()
                                                                .px_6()
                                                                .pb_2()
                                                                .children(reactions_chips)
                                                                .child(bubble)
                                                                .into_any_element()
                                                        } else if is_group_chat {
                                                            let avatar_slot = div()
                                                                .w(px(32.0))
                                                                .h(px(32.0))
                                                                .flex_shrink_0()
                                                                .children(if is_first_in_sequence {
                                                                    Some(render_avatar(
                                                                        &sender_jid,
                                                                        &sender_name,
                                                                        "",
                                                                        false,
                                                                        32.0,
                                                                        &base_url,
                                                                        &theme,
                                                                    ))
                                                                } else {
                                                                    None
                                                                });

                                                            let msg_column = v_flex()
                                                                .gap_1()
                                                                .children(if is_first_in_sequence {
                                                                    Some(
                                                                        div()
                                                                            .text_xs()
                                                                            .font_weight(FontWeight::BOLD)
                                                                            .text_color(sender_color)
                                                                            .ml_1()
                                                                            .child(sender_name),
                                                                    )
                                                                } else {
                                                                    None
                                                                })
                                                                .children(reactions_chips)
                                                                .child(bubble);

                                                            h_flex()
                                                                .w_full()
                                                                .justify_start()
                                                                .items_start()
                                                                .gap_2p5()
                                                                .px_6()
                                                                .pb_2()
                                                                .child(avatar_slot)
                                                                .child(msg_column)
                                                                .into_any_element()
                                                        } else {
                                                            h_flex()
                                                                .w_full()
                                                                .justify_start()
                                                                .px_6()
                                                                .pb_2()
                                                                .child(
                                                                    v_flex()
                                                                        .items_start()
                                                                        .children(reactions_chips)
                                                                        .child(bubble),
                                                                )
                                                                .into_any_element()
                                                        }
                                                    } else {
                                                        div().into_any_element()
                                                    };

                                                    // Day separator: shown above the first message of
                                                    // each day, like the web's date pill.
                                                    let day_separator: Option<AnyElement> = if index > 1 {
                                                        let current = this
                                                            .cached_render_messages
                                                            .get(index - 1)
                                                            .map(|m| MessageBubbleHelper::day_index(m.msg.timestamp));
                                                        let previous = this
                                                            .cached_render_messages
                                                            .get(index - 2)
                                                            .map(|m| MessageBubbleHelper::day_index(m.msg.timestamp));
                                                        match (current, previous) {
                                                            (Some(cur), Some(prev)) if cur != prev => this
                                                                .date_label_at(index)
                                                                .map(|label| {
                                                                    h_flex()
                                                                        .w_full()
                                                                        .justify_center()
                                                                        .py_3()
                                                                        .child(Self::day_pill(
                                                                            label,
                                                                            theme.muted.opacity(0.4),
                                                                            rgba(0x00000000).into(),
                                                                            muted_text,
                                                                        ))
                                                                        .into_any_element()
                                                                }),
                                                            _ => None,
                                                        }
                                                    } else {
                                                        None
                                                    };

                                                    v_flex()
                                                        .w_full()
                                                        .children(day_separator)
                                                        .child(body)
                                                        .into_any_element()
                                                }),
                                            )
                                            .size_full()
                                            .into_any_element()
                                        })
                                        .child(
                                            Scrollbar::vertical(&self.messages_list_state)
                                                .mode(ScrollbarMode::Always)
                                                .styles(|s| {
                                                    s.track(|t| t.bg(transparent_black()))
                                                        .thumb(|th| th.bg(theme.muted_foreground.opacity(0.35)).radius(px(3.0)).width(px(6.0)))
                                                        .thumb_hover(|th| th.bg(theme.muted_foreground.opacity(0.65)).radius(px(4.0)).width(px(8.0)))
                                                        .thumb_active(|th| th.bg(theme.primary.opacity(0.8)).radius(px(4.0)).width(px(8.0)))
                                                }),
                                        )
                                        // Floating day header, mirroring the web's sticky date pill.
                                        .children(self.sticky_day_label().map(|label| {
                                            div()
                                                .absolute()
                                                .top_2()
                                                .left_0()
                                                .right_0()
                                                .flex()
                                                .justify_center()
                                                .child(Self::day_pill(
                                                    label,
                                                    theme.muted.opacity(0.9),
                                                    border_color.opacity(0.4),
                                                    muted_text,
                                                ))
                                                .into_any_element()
                                        })),

                                )
                                // Bottom Compose Container (Banner + Input Field + Integrated Send Icon)
                                .child(
                                    v_flex()
                                        .relative()
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
                                        // Attach (+) menu, emoji and sticker panels
                                        .children(self.render_attach_panel(
                                            theme,
                                            border_color,
                                            card_bg,
                                            primary_color,
                                            text_color,
                                            muted_text,
                                            cx,
                                        ))
                                        // Input Row matching Web
                                        .child(
                                            h_flex()
                                                .p_3()
                                                .items_center()
                                                .gap_2()
                                                // Plus / Attachment icon
                                                .child(
                                                    div()
                                                        .id("btn-attach")
                                                        .p_2()
                                                        .rounded_full()
                                                        .cursor_pointer()
                                                        .bg(if self.attach_panel.is_open() {
                                                            primary_color.opacity(0.15)
                                                        } else {
                                                            rgba(0x00000000).into()
                                                        })
                                                        .hover(|s| s.bg(theme.muted.opacity(0.65)))
                                                        .tooltip(move |window, cx| {
                                                            Tooltip::new("Attach").build(window, cx)
                                                        })
                                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                            this.toggle_attach_panel(cx);
                                                        }))
                                                        .child(svg().data(PLUS_SVG).size(px(18.0)).text_color(
                                                            if self.attach_panel.is_open() { primary_color } else { muted_text }
                                                        )),
                                                )
                                                // Mic icon
                                                .child(
                                                    div()
                                                        .id("btn-mic")
                                                        .p_2()
                                                        .rounded_full()
                                                        .cursor_pointer()
                                                        .hover(|s| s.bg(theme.muted.opacity(0.65)))
                                                        .tooltip(move |window, cx| {
                                                            Tooltip::new("Voice message").build(window, cx)
                                                        })
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
                                                                .id("btn-send")
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
                                                                        h.bg(theme.muted.opacity(0.5))
                                                                    }
                                                                })
                                                                .tooltip(move |window, cx| {
                                                                    Tooltip::new("Send message").build(window, cx)
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
            .children(if (self.is_info_sheet_open || self.info_sheet_closing)
                && selected_chat.is_some()
            {
                let chat = selected_chat.clone().unwrap();

                Some(Self::animated_sheet(
                    "chat-info-sheet-in",
                    "chat-info-sheet-out",
                    self.info_sheet_closing,
                    v_flex()
                        .w(px(SHEET_WIDTH))
                        .min_w(px(SHEET_WIDTH))
                        .max_w(px(SHEET_WIDTH))
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
                                        .id("btn-close-info-sheet")
                                        .cursor_pointer()
                                        .p_1()
                                        .rounded_full()
                                        .hover(|s| s.bg(theme.muted.opacity(0.65)))
                                        .tooltip(move |window, cx| {
                                            Tooltip::new("Close").build(window, cx)
                                        })
                                        .child(svg().data(X_SVG).size(px(16.0)).text_color(muted_text))
                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                            this.close_info_sheet(cx);
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
                                                if self.is_loading_info {
                                                    v_flex()
                                                        .py_8()
                                                        .items_center()
                                                        .gap_2()
                                                        .child(Spinner::new().color(primary_color))
                                                        .child(div().text_sm().text_color(muted_text).child("Loading media..."))
                                                        .into_any_element()
                                                } else {
                                                    v_flex()
                                                        .py_8()
                                                        .items_center()
                                                        .gap_2()
                                                        .opacity(0.4)
                                                        .child(svg().data(IMAGE_SVG).size(px(48.0)).text_color(muted_text))
                                                        .child(div().text_sm().text_color(muted_text).child("No media shared yet"))
                                                        .into_any_element()
                                                }
                                            } else {
                                                // Web parity: 3-column square thumbnail grid.
                                                // Images open the full-screen preview, videos open externally.
                                                let rows: Vec<Vec<(bool, String)>> = self
                                                    .info_media
                                                    .chunks(3)
                                                    .map(|chunk| {
                                                        chunk
                                                            .iter()
                                                            .map(|m| {
                                                                let is_image = m.message_type == "image";
                                                                let url = m
                                                                    .media_url
                                                                    .as_deref()
                                                                    .map(|u| resolve_media_url(u, &base_url))
                                                                    .unwrap_or_default();
                                                                (is_image, url)
                                                            })
                                                            .collect()
                                                    })
                                                    .collect();

                                                v_flex()
                                                    .gap_2()
                                                    .children(rows.into_iter().map(|row| {
                                                        h_flex()
                                                            .gap_2()
                                                            .children(row.into_iter().map(|(is_image, url)| {
                                                                let cell = div()
                                                                    .w(px(88.0))
                                                                    .h(px(88.0))
                                                                    .rounded_lg()
                                                                    .overflow_hidden()
                                                                    .border_1()
                                                                    .border_color(border_color.opacity(0.4))
                                                                    .bg(theme.muted.opacity(0.2))
                                                                    .cursor_pointer();

                                                                if is_image && !url.is_empty() {
                                                                    cell.on_mouse_down(MouseButton::Left, cx.listener({
                                                                        let u = url.clone();
                                                                        move |this, _, _, cx| {
                                                                            this.preview_image_url = Some(u.clone());
                                                                            cx.notify();
                                                                        }
                                                                    }))
                                                                    .child(
                                                                        img(url.clone())
                                                                            .id(format!("info-media-{url}"))
                                                                            .w_full()
                                                                            .h_full()
                                                                            .object_fit(ObjectFit::Cover),
                                                                    )
                                                                    .into_any_element()
                                                                } else if !url.is_empty() {
                                                                    cell.on_mouse_down(MouseButton::Left, cx.listener({
                                                                        let u = url.clone();
                                                                        move |_, _, _, cx| {
                                                                            cx.open_url(&u);
                                                                        }
                                                                    }))
                                                                    .child(
                                                                        div()
                                                                            .w_full()
                                                                            .h_full()
                                                                            .flex()
                                                                            .items_center()
                                                                            .justify_center()
                                                                            .child(svg().data(VIDEO_SVG).size(px(28.0)).text_color(primary_color.opacity(0.4))),
                                                                    )
                                                                    .into_any_element()
                                                                } else {
                                                                    cell.into_any_element()
                                                                }
                                                            }))
                                                    }))
                                                    .into_any_element()
                                            }
                                        }
                                        InfoSheetTab::Docs => {
                                            if self.info_docs.is_empty() {
                                                v_flex()
                                                    .py_8()
                                                    .items_center()
                                                    .gap_2()
                                                    .opacity(0.4)
                                                    .child(svg().data(FILE_TEXT_SVG).size(px(48.0)).text_color(muted_text))
                                                    .child(div().text_sm().text_color(muted_text).child("No documents shared yet"))
                                                    .into_any_element()
                                            } else {
                                                // Web parity: tappable document rows that open the file.
                                                let docs: Vec<(String, String)> = self
                                                    .info_docs
                                                    .iter()
                                                    .map(|d| {
                                                        let url = d
                                                            .media_url
                                                            .as_deref()
                                                            .map(|u| resolve_media_url(u, &base_url))
                                                            .unwrap_or_default();
                                                        (url, Self::format_search_date(d.timestamp))
                                                    })
                                                    .collect();

                                                v_flex()
                                                    .gap_3()
                                                    .children(docs.into_iter().map(|(url, date)| {
                                                        h_flex()
                                                            .gap_3()
                                                            .p_3()
                                                            .rounded_xl()
                                                            .border_1()
                                                            .border_color(border_color.opacity(0.4))
                                                            .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                            .cursor_pointer()
                                                            .on_mouse_down(MouseButton::Left, cx.listener({
                                                                let u = url.clone();
                                                                move |_, _, _, cx| {
                                                                    if !u.is_empty() {
                                                                        cx.open_url(&u);
                                                                    }
                                                                }
                                                            }))
                                                            .child(
                                                                div()
                                                                    .w(px(40.0))
                                                                    .h(px(40.0))
                                                                    .rounded_lg()
                                                                    .bg(rgb(0x3b82f6).opacity(0.1))
                                                                    .flex()
                                                                    .items_center()
                                                                    .justify_center()
                                                                    .child(svg().data(FILE_TEXT_SVG).size(px(20.0)).text_color(rgb(0x2563eb))),
                                                            )
                                                            .child(
                                                                v_flex()
                                                                    .flex_1()
                                                                    .overflow_hidden()
                                                                    .child(
                                                                        div()
                                                                            .text_sm()
                                                                            .font_weight(FontWeight::BOLD)
                                                                            .text_color(text_color)
                                                                            .overflow_hidden()
                                                                            .child("Document"),
                                                                    )
                                                                    .child(div().text_xs().text_color(muted_text).child(date)),
                                                            )
                                                            .into_any_element()
                                                    }))
                                                    .into_any_element()
                                            }
                                        }
                                        InfoSheetTab::Links => {
                                            if self.info_links.is_empty() {
                                                v_flex()
                                                    .py_8()
                                                    .items_center()
                                                    .gap_2()
                                                    .opacity(0.4)
                                                    .child(svg().data(LINK_SVG).size(px(48.0)).text_color(muted_text))
                                                    .child(div().text_sm().text_color(muted_text).child("No links shared yet"))
                                                    .into_any_element()
                                            } else {
                                                // Web parity: extract every http(s) URL from the message body.
                                                let links: Vec<(String, String)> = self
                                                    .info_links
                                                    .iter()
                                                    .flat_map(|l| {
                                                        let ts = l.timestamp;
                                                        l.content
                                                            .split_whitespace()
                                                            .filter(|t| t.starts_with("http://") || t.starts_with("https://"))
                                                            .map(move |t| (t.to_string(), Self::format_search_date(ts)))
                                                            .collect::<Vec<_>>()
                                                    })
                                                    .collect();

                                                v_flex()
                                                    .gap_3()
                                                    .children(links.into_iter().map(|(url, date)| {
                                                        h_flex()
                                                            .gap_3()
                                                            .p_3()
                                                            .rounded_xl()
                                                            .border_1()
                                                            .border_color(border_color.opacity(0.4))
                                                            .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                            .cursor_pointer()
                                                            .on_mouse_down(MouseButton::Left, cx.listener({
                                                                let u = url.clone();
                                                                move |_, _, _, cx| {
                                                                    cx.open_url(&u);
                                                                }
                                                            }))
                                                            .child(
                                                                div()
                                                                    .w(px(40.0))
                                                                    .h(px(40.0))
                                                                    .rounded_lg()
                                                                    .bg(rgb(0x22c55e).opacity(0.1))
                                                                    .flex()
                                                                    .items_center()
                                                                    .justify_center()
                                                                    .child(svg().data(LINK_SVG).size(px(20.0)).text_color(rgb(0x16a34a))),
                                                            )
                                                            .child(
                                                                v_flex()
                                                                    .flex_1()
                                                                    .overflow_hidden()
                                                                    .child(
                                                                        div()
                                                                            .text_sm()
                                                                            .text_color(rgb(0x3b82f6))
                                                                            .overflow_hidden()
                                                                            .child(url),
                                                                    )
                                                                    .child(div().text_xs().text_color(muted_text).child(date)),
                                                            )
                                                            .into_any_element()
                                                    }))
                                                    .into_any_element()
                                            }
                                        }
                                        InfoSheetTab::Members => div().into_any_element(),
                                    }
                                ),
                        )
                        .into_any_element(),
                ))
            } else if (self.is_search_sheet_open || self.search_sheet_closing)
                && selected_chat.is_some()
            {
                let chat = selected_chat.clone().unwrap();
                Some(Self::animated_sheet(
                    "chat-search-sheet-in",
                    "chat-search-sheet-out",
                    self.search_sheet_closing,
                    self.render_search_sheet(&chat, theme, border_color, bg_color, card_bg, primary_color, text_color, muted_text, cx),
                ))
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
                let menu_w = 248.0;
                let menu_h = 210.0;

                let banner_h = if cx.has_global::<ConnectionState>() {
                    match ConnectionState::global(cx).status {
                        ConnectionStatus::Connected => 0.0,
                        _ => 36.0,
                    }
                } else {
                    0.0
                };
                let offset_x = SIDEBAR_WIDTH;
                let offset_y = TITLEBAR_HEIGHT + banner_h;

                let view_w = (win_w - offset_x).max(100.0);
                let view_h = (win_h - offset_y).max(100.0);

                let rel_x = f32::from(ctx.position.x) - offset_x;
                let rel_y = f32::from(ctx.position.y) - offset_y;

                let pos_x = if rel_x + menu_w > view_w - 15.0 {
                    (rel_x - menu_w).max(8.0)
                } else {
                    rel_x.max(8.0)
                };

                let pos_y = if rel_y + menu_h > view_h - 15.0 {
                    (rel_y - menu_h).max(8.0)
                } else {
                    rel_y.max(8.0)
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
                                .w(px(248.0))
                                .p_2()
                                .gap_1()
                                .rounded_2xl()
                                .bg(card_bg)
                                .border_1()
                                .border_color(border_color)
                                .shadow_xl()
                                .on_mouse_down(MouseButton::Left, |_, _, _| {})
                                // Quick Reactions Row matching Web. Each emoji gets a
                                // fixed square hit area so the row can never overflow
                                // the menu and butt against its right border.
                                .child(
                                    h_flex()
                                        .px_1()
                                        .py_1()
                                        .gap_0p5()
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
                                                .w(px(28.0))
                                                .h(px(28.0))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .rounded_full()
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
                                })
                                // Subtle entrance: fade up over the last few px.
                                .with_animation(
                                    "message-context-menu-in",
                                    Animation::new(std::time::Duration::from_millis(140))
                                        .with_easing(|d| 1.0 - (1.0 - d).powi(5)),
                                    move |el, delta| {
                                        el.opacity(delta).top(px(pos_y + 6.0 * (1.0 - delta)))
                                    },
                                ),
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

                let banner_h = if cx.has_global::<ConnectionState>() {
                    match ConnectionState::global(cx).status {
                        ConnectionStatus::Connected => 0.0,
                        _ => 36.0,
                    }
                } else {
                    0.0
                };
                let offset_x = SIDEBAR_WIDTH;
                let offset_y = TITLEBAR_HEIGHT + banner_h;

                let view_w = (win_w - offset_x).max(100.0);
                let view_h = (win_h - offset_y).max(100.0);

                let rel_x = f32::from(ctx.position.x) - offset_x;
                let rel_y = f32::from(ctx.position.y) - offset_y;

                let pos_x = if rel_x + menu_w > view_w - 15.0 {
                    (rel_x - menu_w).max(8.0)
                } else {
                    rel_x.max(8.0)
                };

                let pos_y = if rel_y + menu_h > view_h - 15.0 {
                    (rel_y - menu_h).max(8.0)
                } else {
                    rel_y.max(8.0)
                };

                let sub_w = 130.0;
                let sub_h = 120.0;
                let sub_x = if pos_x + menu_w + sub_w > view_w - 15.0 {
                    (pos_x - sub_w - 4.0).max(8.0)
                } else {
                    pos_x + menu_w + 4.0
                };
                let sub_y = (pos_y + 70.0).min(view_h - sub_h - 15.0).max(8.0);

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
                        })
                        // Subtle entrance, same as the message menu.
                        .with_animation(
                            "chat-context-menu-in",
                            Animation::new(std::time::Duration::from_millis(140))
                                .with_easing(|d| 1.0 - (1.0 - d).powi(5)),
                            move |el, delta| {
                                el.opacity(delta).top(px(pos_y + 6.0 * (1.0 - delta)))
                            },
                        ),
                )
            } else {
                None
            })
            // ====================================================
            // MODAL: NEW GROUP DIALOG
            // ====================================================
            // ====================================================
            // MODAL: POLL / LOCATION / CONTACT COMPOSE DIALOGS
            // ====================================================
            .children(self.render_compose_dialog(
                theme,
                border_color,
                card_bg,
                primary_color,
                text_color,
                muted_text,
                cx,
            ))
            .children(if self.is_new_group_open {
                let contacts: Vec<Chat> = if cx.has_global::<ChatStore>() {
                    ChatStore::global(cx).chats.iter().filter(|c| !c.is_group && !c.archived).cloned().collect()
                } else {
                    Vec::new()
                };
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
            // ====================================================
            // CONVERSATION SIDEBAR RESIZE HANDLE (web parity)
            // ====================================================
            .child(
                div()
                    .id("sidebar-resize-handle")
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(px(self.sidebar_width - SIDEBAR_HANDLE_WIDTH / 2.0))
                    .w(px(SIDEBAR_HANDLE_WIDTH))
                    .cursor_col_resize()
                    .hover(|s| s.bg(primary_color.opacity(0.18)))
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, ev: &MouseDownEvent, _, cx| {
                        this.sidebar_resize_drag = Some((f32::from(ev.position.x), this.sidebar_width));
                        cx.notify();
                    })),
            )
            // While dragging, a full-screen catcher keeps the pointer glued to
            // the divider however fast it moves (and off the chat underneath).
            .children(self.sidebar_resize_drag.is_some().then(|| {
                div()
                    .id("sidebar-resize-overlay")
                    .absolute()
                    .inset_0()
                    .cursor_col_resize()
                    .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                        if let Some((start_x, start_width)) = this.sidebar_resize_drag {
                            let moved = f32::from(ev.position.x) - start_x;
                            this.sidebar_width = (start_width + moved)
                                .clamp(SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH);
                            cx.notify();
                        }
                    }))
                    .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| {
                        this.sidebar_resize_drag = None;
                        cx.notify();
                    }))
            }))
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
                                .id("preview-image-frame")
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

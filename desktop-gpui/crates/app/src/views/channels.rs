//! Channels view matching Web ChannelsPage 1:1.

use gpui::*;
use gpui_component::scroll::{Scrollbar, ScrollbarMode};
use gpui_component::{h_flex, v_flex, Icon, IconName};
use wabot_backend_client::client::HttpClient;
use wabot_backend_client::dto::{Channel, ChannelMessage, ChannelPreview};

use crate::components::toast;
use crate::icons::*;
use crate::state::auth::AuthState;
use crate::theme::manager::AppThemeExt;
use crate::TOKIO_RT;

const QUICK_REACTIONS: &[&str] = &["👍", "❤️", "😂", "😮", "😢", "🙏"];

const AVATAR_COLORS: &[u32] = &[
    0x00a884, 0x0284c7, 0x6366f1, 0x8b5cf6, 0xec4899, 0xf59e0b, 0x10b981, 0x14b8a6,
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
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", b));
            }
        }
    }
    encoded
}

fn resolve_avatar_url(avatar: &str, jid: &str, base_url: &str) -> String {
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
    format!("{clean_base}/avatar/{}", urlencode(jid))
}

fn format_post_time(ts: i64) -> String {
    if ts <= 0 {
        return String::new();
    }
    let ts_sec = if ts > 10_000_000_000 { ts / 1000 } else { ts };
    let hours = (ts_sec / 3600) % 24;
    let mins = (ts_sec / 60) % 60;
    format!("{:02}:{:02}", hours, mins)
}

fn format_post_date(ts: i64) -> String {
    let ts_sec = if ts > 10_000_000_000 { ts / 1000 } else { ts };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days_diff = (now / 86400).saturating_sub(ts_sec / 86400);
    if days_diff == 0 {
        "Today".to_string()
    } else if days_diff == 1 {
        "Yesterday".to_string()
    } else {
        "Earlier".to_string()
    }
}

pub struct ChannelsView {
    channels: Vec<Channel>,
    is_loading: bool,
    selected_channel: Option<Channel>,
    posts: Vec<ChannelMessage>,
    posts_list_state: ListState,
    is_loading_posts: bool,
    is_loading_more: bool,
    has_more: bool,
    confirm_unfollow: bool,

    // Discover Modal
    is_discover_open: bool,
    discover_link: String,
    discover_preview: Option<ChannelPreview>,
    discover_focus: FocusHandle,
    is_discovering: bool,

}

impl ChannelsView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let discover_focus = cx.focus_handle();
        let mut this = Self {
            channels: Vec::new(),
            is_loading: true,
            selected_channel: None,
            posts: Vec::new(),
            posts_list_state: ListState::new(0, ListAlignment::Top, px(120.0)),
            is_loading_posts: false,
            is_loading_more: false,
            has_more: false,
            confirm_unfollow: false,

            is_discover_open: false,
            discover_link: String::new(),
            discover_preview: None,
            discover_focus,
            is_discovering: false,

        };
        this.load_channels(cx);
        this
    }

    pub fn load_channels(&mut self, cx: &mut Context<Self>) {
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
                let res = TOKIO_RT.spawn(async move { client.list_channels().await }).await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_loading = false;
                            if let Ok(Ok(list)) = res {
                                this.channels = list;
                                if let Some(selected) = &this.selected_channel {
                                    if let Some(fresh) = this.channels.iter().find(|c| c.jid == selected.jid) {
                                        this.selected_channel = Some(fresh.clone());
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

    pub fn select_channel(&mut self, channel: Channel, cx: &mut Context<Self>) {
        self.selected_channel = Some(channel.clone());
        self.posts.clear();
        self.posts_list_state.reset(0);
        self.is_loading_posts = true;
        self.has_more = false;
        cx.notify();

        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        let jid = channel.jid.clone();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT.spawn(async move {
                    client.get_channel_messages(&jid, Some(30), None).await
                }).await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_loading_posts = false;
                            if let Ok(Ok(mut msgs)) = res {
                                msgs.sort_by_key(|m| m.server_id);
                                this.has_more = msgs.len() == 30;
                                this.posts = msgs;
                                let total_items = if this.posts.is_empty() { 0 } else { this.posts.len() + 1 };
                                this.posts_list_state.reset(total_items);
                                this.posts_list_state.scroll_to_end();
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    pub fn load_more_posts(&mut self, cx: &mut Context<Self>) {
        if self.is_loading_more || self.posts.is_empty() {
            return;
        }
        let channel_jid = match &self.selected_channel {
            Some(c) => c.jid.clone(),
            None => return,
        };
        let oldest_id = self.posts.first().map(|p| p.server_id).unwrap_or(0);
        self.is_loading_more = true;
        cx.notify();

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
                let res = TOKIO_RT.spawn(async move {
                    client.get_channel_messages(&channel_jid, Some(30), Some(oldest_id)).await
                }).await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_loading_more = false;
                            if let Ok(Ok(older)) = res {
                                this.has_more = older.len() == 30;
                                let prev_count = this.posts.len();
                                for p in older {
                                    if !this.posts.iter().any(|existing| existing.id == p.id) {
                                        this.posts.insert(0, p);
                                    }
                                }
                                this.posts.sort_by_key(|m| m.server_id);
                                let total_items = if this.posts.is_empty() { 0 } else { this.posts.len() + 1 };
                                let new_items_added = this.posts.len().saturating_sub(prev_count);
                                this.posts_list_state.reset(total_items);
                                if new_items_added > 0 {
                                    this.posts_list_state.scroll_to_reveal_item(new_items_added + 1);
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

    pub fn toggle_mute(&mut self, cx: &mut Context<Self>) {
        let channel = match &self.selected_channel {
            Some(c) => c.clone(),
            None => return,
        };
        let new_muted = !channel.muted;
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        let jid = channel.jid.clone();
        if let Some(c) = &mut self.selected_channel {
            c.muted = new_muted;
        }
        if let Some(c) = self.channels.iter_mut().find(|c| c.jid == jid) {
            c.muted = new_muted;
        }
        TOKIO_RT.spawn(async move {
            let client = HttpClient::new(&base_url);
            let _ = client.set_channel_mute(&jid, new_muted).await;
        });
    }

    pub fn unfollow_channel(&mut self, cx: &mut Context<Self>) {
        let channel = match &self.selected_channel {
            Some(c) => c.clone(),
            None => return,
        };
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        let jid = channel.jid.clone();
        self.channels.retain(|c| c.jid != jid);
        self.selected_channel = None;
        self.confirm_unfollow = false;
        self.posts.clear();
        self.posts_list_state.reset(0);
        toast::success("Unfollowed channel", cx);
        cx.notify();

        TOKIO_RT.spawn(async move {
            let client = HttpClient::new(&base_url);
            let _ = client.unfollow_channel(&jid).await;
        });
    }

    pub fn react_message(&mut self, message_id: String, server_id: i64, emoji: &str, cx: &mut Context<Self>) {
        let channel = match &self.selected_channel {
            Some(c) => c.clone(),
            None => return,
        };

        // Optimistic reaction bump
        if let Some(post) = self.posts.iter_mut().find(|p| p.id == message_id) {
            let reactions = post.reactions.get_or_insert_with(std::collections::HashMap::new);
            let count = reactions.entry(emoji.to_string()).or_insert(0);
            *count += 1;
        }
        cx.notify();

        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        let jid = channel.jid;
        let emo = emoji.to_string();
        TOKIO_RT.spawn(async move {
            let client = HttpClient::new(&base_url);
            let _ = client.react_channel_message(&jid, server_id, &message_id, &emo).await;
        });
    }

    pub fn preview_channel(&mut self, cx: &mut Context<Self>) {
        let link = self.discover_link.trim().to_string();
        if link.is_empty() || self.is_discovering {
            return;
        }
        self.is_discovering = true;
        self.discover_preview = None;
        cx.notify();

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
                let res = TOKIO_RT.spawn(async move { client.preview_channel(&link).await }).await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_discovering = false;
                            match res {
                                Ok(Ok(preview)) => {
                                    this.discover_preview = Some(preview);
                                }
                                _ => {
                                    toast::error("Invalid channel link", cx);
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

    pub fn follow_channel(&mut self, cx: &mut Context<Self>) {
        let link = self.discover_link.trim().to_string();
        if link.is_empty() || self.is_discovering {
            return;
        }
        self.is_discovering = true;
        cx.notify();

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
                let res = TOKIO_RT.spawn(async move { client.follow_channel(&link).await }).await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_discovering = false;
                            match res {
                                Ok(Ok(_)) => {
                                    this.is_discover_open = false;
                                    this.discover_link.clear();
                                    this.discover_preview = None;
                                    toast::success("Channel followed", cx);
                                    this.load_channels(cx);
                                }
                                Ok(Err(e)) => {
                                    toast::error(format!("Failed to follow: {e}"), cx);
                                }
                                _ => {
                                    toast::error("Failed to follow channel", cx);
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
}

impl Render for ChannelsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.app_theme();
        let border_color = theme.border;
        let text_color = theme.foreground;
        let muted_text = theme.muted_foreground;
        let bg_color = theme.background;
        let card_bg = theme.card;

        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        let selected_jid = self.selected_channel.as_ref().map(|c| c.jid.clone());

        h_flex()
            .id("channels-view")
            .size_full()
            .overflow_hidden()
            .bg(bg_color)
            .relative()
            // --- Left Sidebar (340px) ---
            .child(
                v_flex()
                    .w(px(340.0))
                    .h_full()
                    .flex_shrink_0()
                    .border_r_1()
                    .border_color(border_color)
                    .bg(bg_color)
                    // Header
                    .child(
                        h_flex()
                            .h(px(64.0))
                            .px_5()
                            .justify_between()
                            .items_center()
                            .border_b_1()
                            .border_color(border_color)
                            .child(
                                div()
                                    .text_2xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(text_color)
                                    .child("Channels"),
                            )
                            .child(
                                div()
                                    .cursor_pointer()
                                    .w(px(36.0))
                                    .h(px(36.0))
                                    .rounded_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                    .child(svg().data(PLUS_SVG).size(px(20.0)).text_color(text_color))
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                        this.is_discover_open = true;
                                        this.discover_focus.focus(window, cx);
                                        cx.notify();
                                    })),
                            ),
                    )
                    // Channels Scroll List
                    .child(
                        v_flex()
                            .id("channels-list-scroll")
                            .flex_1()
                            .overflow_y_scroll()
                            .px_3()
                            .py_3()
                            .gap_1()
                            .children(if self.is_loading && self.channels.is_empty() {
                                Some(
                                    div()
                                        .py_8()
                                        .text_xs()
                                        .text_color(muted_text)
                                        .flex()
                                        .justify_center()
                                        .child("Loading channels…"),
                                )
                            } else if self.channels.is_empty() {
                                Some(
                                    v_flex()
                                        .py_12()
                                        .items_center()
                                        .justify_center()
                                        .gap_2()
                                        .child(svg().data(MEGAPHONE_SVG).size(px(36.0)).text_color(muted_text.opacity(0.5)))
                                        .child(div().text_sm().text_color(muted_text).child("No channels followed")),
                                )
                            } else {
                                None
                            })
                            // Channel List items
                            .children(self.channels.iter().map(|channel| {
                                let is_active = selected_jid.as_deref() == Some(&channel.jid);
                                let ch_clone = channel.clone();
                                let a_url = resolve_avatar_url(
                                    channel.avatar.as_deref().unwrap_or(""),
                                    &channel.jid,
                                    &base_url,
                                );
                                let jid_clone = channel.jid.clone();

                                h_flex()
                                    .id(SharedString::from(format!("ch-item-{}", channel.jid)))
                                    .w_full()
                                    .p_3()
                                    .rounded_xl()
                                    .items_center()
                                    .gap_3()
                                    .cursor_pointer()
                                    .bg(if is_active {
                                        theme.primary.opacity(0.14)
                                    } else {
                                        rgba(0x00000000).into()
                                    })
                                    .hover(|s| {
                                        if is_active {
                                            s.bg(theme.primary.opacity(0.2))
                                        } else {
                                            s.bg(theme.muted.opacity(0.4))
                                        }
                                    })
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                        this.select_channel(ch_clone.clone(), cx);
                                    }))
                                    .child(
                                        div()
                                            .w(px(44.0))
                                            .h(px(44.0))
                                            .rounded_full()
                                            .overflow_hidden()
                                            .relative()
                                            .bg(avatar_color_for(&jid_clone))
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
                                                        channel.name
                                                            .chars()
                                                            .next()
                                                            .map(|c| c.to_uppercase().to_string())
                                                            .unwrap_or_else(|| "?".to_string()),
                                                    ),
                                            )
                                            .child(
                                                img(a_url)
                                                    .absolute()
                                                    .inset_0()
                                                    .size_full()
                                                    .rounded_full()
                                                    .object_fit(ObjectFit::Cover),
                                            ),
                                    )
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .overflow_hidden()
                                            .gap_0p5()
                                            .child(
                                                h_flex()
                                                    .items_center()
                                                    .gap_1()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(text_color)
                                                            .overflow_hidden()
                                                            .child(channel.name.clone()),
                                                    )
                                                    .children(if channel.verified {
                                                        Some(div().text_xs().text_color(theme.primary).font_weight(FontWeight::BOLD).child("✓"))
                                                    } else {
                                                        None
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(muted_text)
                                                    .child(format!(
                                                        "{} subscribers{}",
                                                        channel.subscribers,
                                                        if channel.muted { " · muted" } else { "" }
                                                    )),
                                            ),
                                    )
                            }))
                            // "Find channels" action card
                            .child(
                                v_flex()
                                    .w_full()
                                    .mt_4()
                                    .child(
                                        div()
                                            .px_3()
                                            .py_2()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(muted_text)
                                            .child("FIND CHANNELS"),
                                    )
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .p_3()
                                            .rounded_xl()
                                            .items_center()
                                            .gap_3()
                                            .cursor_pointer()
                                            .hover(|s| s.bg(theme.muted.opacity(0.4)))
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                                this.is_discover_open = true;
                                                this.discover_focus.focus(window, cx);
                                                cx.notify();
                                            }))
                                            .child(
                                                div()
                                                    .w(px(44.0))
                                                    .h(px(44.0))
                                                    .rounded_full()
                                                    .bg(theme.primary.opacity(0.12))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(
                                                        svg()
                                                            .data(SEARCH_SVG)
                                                            .size(px(20.0))
                                                            .text_color(theme.primary),
                                                    ),
                                            )
                                            .child(
                                                v_flex()
                                                    .flex_1()
                                                    .gap_0p5()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(text_color)
                                                            .child("Follow via link"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(muted_text)
                                                            .child("Paste a whatsapp.com/channel/… link"),
                                                    ),
                                            ),
                                    ),
                            ),
                    ),
            )
            // --- Right Channel Conversation Pane ---
            .child(
                v_flex()
                    .flex_1()
                    .h_full()
                    .overflow_hidden()
                    .bg(bg_color)
                    .children(if let Some(channel) = self.selected_channel.clone() {
                        let a_url = resolve_avatar_url(
                            channel.avatar.as_deref().unwrap_or(""),
                            &channel.jid,
                            &base_url,
                        );
                        let is_muted = channel.muted;
                        let jid_clone = channel.jid.clone();

                        Some(
                            v_flex()
                                .size_full()
                                // Channel Header (h-16 / 64px)
                                .child(
                                    h_flex()
                                        .h(px(64.0))
                                        .px_4()
                                        .justify_between()
                                        .items_center()
                                        .border_b_1()
                                        .border_color(border_color)
                                        .bg(bg_color)
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .gap_3()
                                                .child(
                                                    div()
                                                        .w(px(40.0))
                                                        .h(px(40.0))
                                                        .rounded_full()
                                                        .overflow_hidden()
                                                        .relative()
                                                        .bg(avatar_color_for(&jid_clone))
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
                                                                    channel.name
                                                                        .chars()
                                                                        .next()
                                                                        .map(|c| c.to_uppercase().to_string())
                                                                        .unwrap_or_else(|| "?".to_string()),
                                                                ),
                                                        )
                                                        .child(
                                                            img(a_url)
                                                                .absolute()
                                                                .inset_0()
                                                                .size_full()
                                                                .rounded_full()
                                                                .object_fit(ObjectFit::Cover),
                                                        ),
                                                )
                                                .child(
                                                    v_flex()
                                                        .gap_0p5()
                                                        .child(
                                                            h_flex()
                                                                .items_center()
                                                                .gap_1()
                                                                .child(
                                                                    div()
                                                                        .text_base()
                                                                        .font_weight(FontWeight::BOLD)
                                                                        .text_color(text_color)
                                                                        .child(channel.name.clone()),
                                                                )
                                                                .children(if channel.verified {
                                                                    Some(div().text_xs().text_color(theme.primary).font_weight(FontWeight::BOLD).child("✓"))
                                                                } else {
                                                                    None
                                                                }),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(muted_text)
                                                                .child(format!("{} followers", channel.subscribers)),
                                                        ),
                                                ),
                                        )
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
                                                        .child(
                                                            svg()
                                                                .data(if is_muted { VOLUME_X_SVG } else { VOLUME_2_SVG })
                                                                .size(px(18.0))
                                                                .text_color(muted_text),
                                                        )
                                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                            this.toggle_mute(cx);
                                                        })),
                                                )
                                                .child(
                                                    div()
                                                        .cursor_pointer()
                                                        .p_2()
                                                        .rounded_full()
                                                        .hover(|s| s.bg(theme.destructive.opacity(0.1)))
                                                        .child(
                                                            svg()
                                                                .data(USER_MINUS_SVG)
                                                                .size(px(18.0))
                                                                .text_color(theme.destructive),
                                                        )
                                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                            this.confirm_unfollow = true;
                                                            cx.notify();
                                                        })),
                                                ),
                                        ),
                                )
                                // Posts Stream Feed with 60fps virtualization and visual scrollbar
                                .child(
                                    div()
                                        .flex_1()
                                        .relative()
                                        .overflow_hidden()
                                        .child(if self.is_loading_posts && self.posts.is_empty() {
                                            v_flex()
                                                .size_full()
                                                .items_center()
                                                .justify_center()
                                                .gap_2()
                                                .child(Icon::new(IconName::LoaderCircle).size(px(24.0)))
                                                .child(div().text_xs().text_color(muted_text).child("Loading posts…"))
                                                .into_any_element()
                                        } else if self.posts.is_empty() {
                                            v_flex()
                                                .size_full()
                                                .items_center()
                                                .justify_center()
                                                .gap_2()
                                                .child(svg().data(MEGAPHONE_SVG).size(px(36.0)).text_color(muted_text.opacity(0.5)))
                                                .child(div().text_sm().text_color(muted_text).child("No posts yet"))
                                                .into_any_element()
                                        } else {
                                            list(
                                                self.posts_list_state.clone(),
                                                cx.processor(|this, index: usize, _window, cx| {
                                                    let theme = *cx.app_theme();
                                                    let border_color = theme.border;
                                                    let text_color = theme.foreground;
                                                    let muted_text = theme.muted_foreground;
                                                    let card_bg = theme.card;
                                                    let primary_color = theme.primary;

                                                    // Auto-load older posts when user scrolls near top
                                                    if index <= 1 && this.has_more && !this.is_loading_more && !this.is_loading_posts {
                                                        this.load_more_posts(cx);
                                                    }

                                                    if index == 0 {
                                                        if this.has_more {
                                                            h_flex()
                                                                .w_full()
                                                                .justify_center()
                                                                .py_3()
                                                                .child(
                                                                    div()
                                                                        .cursor_pointer()
                                                                        .px_4()
                                                                        .py_1p5()
                                                                        .rounded_full()
                                                                        .border_1()
                                                                        .border_color(border_color)
                                                                        .text_xs()
                                                                        .font_weight(FontWeight::MEDIUM)
                                                                        .text_color(text_color)
                                                                        .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                                        .child(if this.is_loading_more { "Loading…" } else { "Load older posts" })
                                                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                                            this.load_more_posts(cx);
                                                                        })),
                                                                )
                                                                .into_any_element()
                                                        } else {
                                                            h_flex()
                                                                .w_full()
                                                                .justify_center()
                                                                .py_3()
                                                                .child(
                                                                    div()
                                                                        .px_3()
                                                                        .py_1()
                                                                        .rounded_full()
                                                                        .bg(theme.muted.opacity(0.4))
                                                                        .text_xs()
                                                                        .font_weight(FontWeight::MEDIUM)
                                                                        .text_color(muted_text)
                                                                        .child("Beginning of channel updates"),
                                                                )
                                                                .into_any_element()
                                                        }
                                                    } else if let Some(post) = this.posts.get(index - 1) {
                                                        let time_str = format!("{} at {}", format_post_date(post.timestamp), format_post_time(post.timestamp));
                                                        let post_id = post.id.clone();
                                                        let server_id = post.server_id;
                                                        let reactions_map = post.reactions.clone().unwrap_or_default();
                                                        let content = post.content.clone().unwrap_or_default();
                                                        let channel_name = this.selected_channel.as_ref().map(|c| c.name.clone()).unwrap_or_default();

                                                        v_flex()
                                                            .w_full()
                                                            .px_6()
                                                            .pb_4()
                                                            .items_center()
                                                            .child(
                                                                v_flex()
                                                                    .w_full()
                                                                    .max_w(px(600.0))
                                                                    .p_4()
                                                                    .rounded_2xl()
                                                                    .bg(card_bg)
                                                                    .border_1()
                                                                    .border_color(border_color)
                                                                    .shadow_sm()
                                                                    .gap_2p5()
                                                                    // Post Header & Timestamp
                                                                    .child(
                                                                        h_flex()
                                                                            .justify_between()
                                                                            .items_center()
                                                                            .child(
                                                                                div()
                                                                                    .text_xs()
                                                                                    .font_weight(FontWeight::BOLD)
                                                                                    .text_color(primary_color)
                                                                                    .child(channel_name),
                                                                            )
                                                                            .child(
                                                                                div()
                                                                                    .text_xs()
                                                                                    .text_color(muted_text)
                                                                                    .child(time_str),
                                                                            ),
                                                                    )
                                                                    // Post Text Content
                                                                    .child(
                                                                        div()
                                                                            .text_sm()
                                                                            .text_color(text_color)
                                                                            .line_height(relative(1.4))
                                                                            .child(content),
                                                                    )
                                                                    // Post Meta & Reactions Row
                                                                    .child(
                                                                        h_flex()
                                                                            .justify_between()
                                                                            .items_center()
                                                                            .pt_1()
                                                                            .border_t_1()
                                                                            .border_color(border_color.opacity(0.5))
                                                                            .child(
                                                                                h_flex()
                                                                                    .items_center()
                                                                                    .gap_1()
                                                                                    .child(svg().data(EYE_SVG).size(px(14.0)).text_color(muted_text))
                                                                                    .child(
                                                                                        div()
                                                                                            .text_xs()
                                                                                            .text_color(muted_text)
                                                                                            .child(format!("{}", post.views_count)),
                                                                                    ),
                                                                            )
                                                                            // Reactions Chips
                                                                            .child(
                                                                                h_flex()
                                                                                    .items_center()
                                                                                    .gap_1p5()
                                                                                    .children(reactions_map.iter().map(|(emoji, count)| {
                                                                                        h_flex()
                                                                                            .px_2()
                                                                                            .py_0p5()
                                                                                            .rounded_full()
                                                                                            .bg(theme.muted.opacity(0.4))
                                                                                            .gap_1()
                                                                                            .items_center()
                                                                                            .child(div().text_xs().child(emoji.clone()))
                                                                                            .child(div().text_xs().text_color(muted_text).child(format!("{count}")))
                                                                                    }))
                                                                                    // Quick Reactions Buttons
                                                                                    .child(
                                                                                        h_flex()
                                                                                            .gap_1()
                                                                                            .pl_2()
                                                                                            .children(QUICK_REACTIONS.iter().map(|&emo| {
                                                                                                let pid = post_id.clone();
                                                                                                div()
                                                                                                    .cursor_pointer()
                                                                                                    .px_1p5()
                                                                                                    .py_0p5()
                                                                                                    .rounded_md()
                                                                                                    .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                                                                    .child(div().text_xs().child(emo))
                                                                                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                                                                        this.react_message(pid.clone(), server_id, emo, cx);
                                                                                                    }))
                                                                                            })),
                                                                                    ),
                                                                            ),
                                                                    ),
                                                            )
                                                            .into_any_element()
                                                    } else {
                                                        div().into_any_element()
                                                    }
                                                })
                                            )
                                            .size_full()
                                            .into_any_element()
                                        })
                                        .child(
                                            Scrollbar::vertical(&self.posts_list_state)
                                                .mode(ScrollbarMode::Always)
                                                .styles(|s| {
                                                    s.track(|t| t.bg(transparent_black()))
                                                        .thumb(|th| th.bg(theme.muted_foreground.opacity(0.35)).radius(px(3.0)).width(px(6.0)))
                                                        .thumb_hover(|th| th.bg(theme.muted_foreground.opacity(0.65)).radius(px(4.0)).width(px(8.0)))
                                                        .thumb_active(|th| th.bg(theme.primary.opacity(0.8)).radius(px(4.0)).width(px(8.0)))
                                                }),
                                        ),
                                )
                                // Bottom One-Way Channel Notice Footer
                                .child(
                                    h_flex()
                                        .h(px(48.0))
                                        .px_4()
                                        .items_center()
                                        .justify_center()
                                        .border_t_1()
                                        .border_color(border_color)
                                        .bg(card_bg.opacity(0.5))
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(muted_text)
                                                .child("Channels are one-way — you can react to posts but not reply."),
                                        ),
                                ),
                        )
                    } else {
                        // Empty State Graphic
                        Some(
                            v_flex()
                                .size_full()
                                .items_center()
                                .justify_center()
                                .gap_4()
                                .child(
                                    div()
                                        .w(px(80.0))
                                        .h(px(80.0))
                                        .rounded_full()
                                        .bg(theme.primary.opacity(0.1))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(
                                            svg()
                                                .data(MEGAPHONE_SVG)
                                                .size(px(40.0))
                                                .text_color(theme.primary.opacity(0.7)),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_2xl()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(text_color)
                                        .child("Select a channel"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(muted_text)
                                        .child("Follow channels to see their posts here."),
                                ),
                        )
                    }),
            )
            // --- Discover / Find Channel Dialog ---
            .children(if self.is_discover_open {
                let link_val = self.discover_link.clone();
                let is_busy = self.is_discovering;
                let preview = self.discover_preview.clone();

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
                                .w(px(440.0))
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
                                        .child(
                                            div()
                                                .text_lg()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(text_color)
                                                .child("Find channel"),
                                        )
                                        .child(
                                            div()
                                                .cursor_pointer()
                                                .p_1()
                                                .rounded_full()
                                                .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                .child(svg().data(X_SVG).size(px(16.0)).text_color(muted_text))
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.is_discover_open = false;
                                                    cx.notify();
                                                })),
                                        ),
                                )
                                // Link Input Field
                                .child(
                                    h_flex()
                                        .px_3()
                                        .py_2()
                                        .rounded_xl()
                                        .border_1()
                                        .border_color(border_color)
                                        .bg(bg_color)
                                        .items_center()
                                        .gap_2()
                                        .track_focus(&self.discover_focus)
                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                            this.discover_focus.focus(window, cx);
                                        }))
                                        .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                                            match ev.keystroke.key.as_str() {
                                                "backspace" => {
                                                    this.discover_link.pop();
                                                    this.discover_preview = None;
                                                    cx.notify();
                                                }
                                                "space" => {
                                                    this.discover_link.push(' ');
                                                    cx.notify();
                                                }
                                                "enter" => {
                                                    this.preview_channel(cx);
                                                }
                                                k if k.len() == 1 => {
                                                    this.discover_link.push_str(k);
                                                    this.discover_preview = None;
                                                    cx.notify();
                                                }
                                                _ => {}
                                            }
                                        }))
                                        .child(svg().data(LINK_SVG).size(px(16.0)).text_color(muted_text))
                                        .child(
                                            div()
                                                .flex_1()
                                                .text_sm()
                                                .text_color(if link_val.is_empty() { muted_text } else { text_color })
                                                .child(if link_val.is_empty() {
                                                    "https://whatsapp.com/channel/…".to_string()
                                                } else {
                                                    link_val
                                                }),
                                        ),
                                )
                                // Preview Action Button
                                .child(
                                    h_flex()
                                        .justify_start()
                                        .child(
                                            div()
                                                .cursor_pointer()
                                                .px_3()
                                                .py_1p5()
                                                .rounded_lg()
                                                .border_1()
                                                .border_color(border_color)
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(text_color)
                                                .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                .child(if is_busy { "Previewing…" } else { "Preview" })
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.preview_channel(cx);
                                                })),
                                        ),
                                )
                                // Channel Preview Card
                                .children(if let Some(p) = preview {
                                    Some(
                                        v_flex()
                                            .p_3()
                                            .rounded_xl()
                                            .bg(theme.muted.opacity(0.4))
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(text_color)
                                                    .child(p.name),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(muted_text)
                                                    .child(format!("{} subscribers", p.subscribers)),
                                            )
                                            .children(if let Some(desc) = p.description {
                                                Some(
                                                    div()
                                                        .mt_1()
                                                        .text_xs()
                                                        .text_color(text_color)
                                                        .child(desc),
                                                )
                                            } else {
                                                None
                                            }),
                                    )
                                } else {
                                    None
                                })
                                // Footer
                                .child(
                                    h_flex()
                                        .justify_end()
                                        .gap_2()
                                        .pt_2()
                                        .child(
                                            div()
                                                .cursor_pointer()
                                                .px_4()
                                                .py_2()
                                                .rounded_xl()
                                                .text_sm()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(text_color)
                                                .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                .child("Cancel")
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.is_discover_open = false;
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            div()
                                                .cursor_pointer()
                                                .px_5()
                                                .py_2()
                                                .rounded_xl()
                                                .bg(if is_busy || self.discover_link.trim().is_empty() {
                                                    theme.primary.opacity(0.5)
                                                } else {
                                                    theme.primary
                                                })
                                                .text_color(theme.primary_foreground)
                                                .text_sm()
                                                .font_weight(FontWeight::BOLD)
                                                .hover(|s| s.opacity(0.9))
                                                .child("Follow")
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.follow_channel(cx);
                                                })),
                                        ),
                                ),
                        ),
                )
            } else {
                None
            })
            // --- Unfollow Confirmation Dialog ---
            .children(if self.confirm_unfollow {
                let channel_name = self.selected_channel.as_ref().map(|c| c.name.clone()).unwrap_or_default();
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
                                .w(px(380.0))
                                .p_5()
                                .rounded_2xl()
                                .bg(card_bg)
                                .border_1()
                                .border_color(border_color)
                                .shadow_2xl()
                                .gap_4()
                                .child(
                                    div()
                                        .text_base()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(text_color)
                                        .child(format!("Unfollow {}?", channel_name)),
                                )
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
                                                .text_sm()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(text_color)
                                                .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                .child("Cancel")
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.confirm_unfollow = false;
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            div()
                                                .cursor_pointer()
                                                .px_4()
                                                .py_2()
                                                .rounded_xl()
                                                .bg(theme.destructive)
                                                .text_color(theme.destructive_foreground)
                                                .text_sm()
                                                .font_weight(FontWeight::BOLD)
                                                .hover(|s| s.opacity(0.9))
                                                .child("Unfollow")
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.unfollow_channel(cx);
                                                })),
                                        ),
                                ),
                        ),
                )
            } else {
                None
            })
    }
}

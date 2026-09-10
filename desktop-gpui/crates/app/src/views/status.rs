//! Status view matching Web StatusPage 1:1.

use gpui::*;
use gpui_component::{h_flex, v_flex};
use wabot_backend_client::client::HttpClient;
use wabot_backend_client::dto::StatusGroup;

use crate::icons::*;
use crate::state::auth::AuthState;
use crate::theme::manager::AppThemeExt;
use crate::TOKIO_RT;

const BACKGROUNDS: &[u32] = &[
    0xff075e54, 0xff128c7e, 0xff777a77, 0xff2c3e50, 0xff6a3080, 0xffc43e00, 0xffd4a017, 0xff0e5a8a,
];

fn argb_to_rgb(argb: u32) -> Rgba {
    rgb(argb & 0x00ffffff)
}

fn format_status_time(ts: i64) -> String {
    if ts <= 0 {
        return String::new();
    }
    let ts_sec = if ts > 10_000_000_000 { ts / 1000 } else { ts };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let hours = (ts_sec / 3600) % 24;
    let mins = (ts_sec / 60) % 60;
    let time_str = format!("{:02}:{:02}", hours, mins);

    let days_diff = (now / 86400).saturating_sub(ts_sec / 86400);
    if days_diff == 0 {
        format!("Today at {}", time_str)
    } else if days_diff == 1 {
        format!("Yesterday at {}", time_str)
    } else {
        format!("Recent at {}", time_str)
    }
}

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

fn resolve_avatar_url(avatar: &str, sender: &str, base_url: &str) -> String {
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
    format!("{clean_base}/avatar/{}", urlencode(sender))
}

fn resolve_media_url(media_url: &str, base_url: &str) -> String {
    let media_url = media_url.trim();
    if media_url.starts_with("http://") || media_url.starts_with("https://") {
        return media_url.to_string();
    }
    let clean_base = base_url.trim_end_matches('/');
    let clean_media = media_url.trim_start_matches('/');
    if clean_media.starts_with("api/") {
        let root = clean_base.strip_suffix("/api").unwrap_or(clean_base);
        return format!("{root}/{clean_media}");
    }
    format!("{clean_base}/{clean_media}")
}

pub struct StatusView {
    groups: Vec<StatusGroup>,
    is_loading: bool,
    viewing_group: Option<StatusGroup>,
    viewing_index: usize,
    composer_open: bool,
    composer_text: String,
    composer_bg: u32,
    composer_focus: FocusHandle,
    is_posting: bool,
    toast_message: Option<String>,
}

impl StatusView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let composer_focus = cx.focus_handle();
        let mut this = Self {
            groups: Vec::new(),
            is_loading: true,
            viewing_group: None,
            viewing_index: 0,
            composer_open: false,
            composer_text: String::new(),
            composer_bg: BACKGROUNDS[0],
            composer_focus,
            is_posting: false,
            toast_message: None,
        };
        this.load_statuses(cx);
        this
    }

    pub fn load_statuses(&mut self, cx: &mut Context<Self>) {
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
                let res = TOKIO_RT.spawn(async move { client.list_statuses().await }).await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_loading = false;
                            if let Ok(Ok(groups)) = res {
                                this.groups = groups;
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    pub fn open_group(&mut self, group: StatusGroup, cx: &mut Context<Self>) {
        if group.statuses.is_empty() {
            self.composer_open = true;
            cx.notify();
            return;
        }
        self.viewing_group = Some(group.clone());
        self.viewing_index = 0;
        self.mark_current_viewed(cx);
        cx.notify();
    }

    pub fn mark_current_viewed(&mut self, cx: &mut Context<Self>) {
        let entry_id = if let Some(group) = &self.viewing_group {
            let is_own = group.name.as_deref() == Some("Status Saya") || group.name.as_deref() == Some("My status");
            if is_own {
                return;
            }
            if let Some(entry) = group.statuses.get(self.viewing_index) {
                if !entry.viewed {
                    Some(entry.id.clone())
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some(id) = entry_id {
            let base_url = if cx.has_global::<AuthState>() {
                AuthState::global(cx).base_url.clone()
            } else {
                "http://127.0.0.1:3000/api".to_string()
            };

            TOKIO_RT.spawn(async move {
                let client = HttpClient::new(&base_url);
                let _ = client.mark_status_viewed(&id).await;
            });
        }
    }

    pub fn navigate(&mut self, delta: isize, cx: &mut Context<Self>) {
        if let Some(group) = &self.viewing_group {
            let total = group.statuses.len() as isize;
            let next_idx = self.viewing_index as isize + delta;
            if next_idx < 0 || next_idx >= total {
                self.viewing_group = None;
                self.viewing_index = 0;
                self.load_statuses(cx);
            } else {
                self.viewing_index = next_idx as usize;
                self.mark_current_viewed(cx);
            }
            cx.notify();
        }
    }

    pub fn post_status(&mut self, cx: &mut Context<Self>) {
        let text = self.composer_text.trim().to_string();
        if text.is_empty() || self.is_posting {
            return;
        }
        self.is_posting = true;
        let bg = self.composer_bg;

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
                let res = TOKIO_RT.spawn(async move { client.post_status_text(&text, Some(bg)).await }).await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_posting = false;
                            match res {
                                Ok(Ok(_)) => {
                                    this.composer_open = false;
                                    this.composer_text.clear();
                                    this.toast_message = Some("Status posted".to_string());
                                    this.load_statuses(cx);
                                }
                                Ok(Err(e)) => {
                                    this.toast_message = Some(format!("Failed to post: {e}"));
                                }
                                _ => {
                                    this.toast_message = Some("Failed to post status".to_string());
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

impl Render for StatusView {
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

        let own_group = self.groups.iter().find(|g| {
            g.name.as_deref() == Some("Status Saya") || g.name.as_deref() == Some("My status")
        }).cloned();

        let recent_groups: Vec<StatusGroup> = self.groups.iter().filter(|g| {
            !(g.name.as_deref() == Some("Status Saya") || g.name.as_deref() == Some("My status"))
        }).cloned().collect();

        h_flex()
            .id("status-view")
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
                    // Sidebar Header
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
                                    .child("Status"),
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
                                        this.composer_open = true;
                                        this.composer_focus.focus(window, cx);
                                        cx.notify();
                                    })),
                            ),
                    )
                    // Scrollable Status List
                    .child(
                        v_flex()
                            .id("status-list-scroll")
                            .flex_1()
                            .overflow_y_scroll()
                            .px_3()
                            .py_3()
                            .gap_1()
                            // Loading indicator
                            .children(if self.is_loading && self.groups.is_empty() {
                                Some(
                                    div()
                                        .py_4()
                                        .text_xs()
                                        .text_color(muted_text)
                                        .flex()
                                        .justify_center()
                                        .child("Memuat…"),
                                )
                            } else {
                                None
                            })
                            // "Status Saya" Row
                            .child({
                                let own_clone = own_group.clone();
                                let has_own = own_group.is_some() && !own_group.as_ref().unwrap().statuses.is_empty();
                                let latest_time = own_group.as_ref().map(|g| g.latest_time).unwrap_or(0);
                                h_flex()
                                    .w_full()
                                    .p_3()
                                    .rounded_xl()
                                    .items_center()
                                    .gap_3()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.muted.opacity(0.4)))
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                                        if let Some(og) = &own_clone {
                                            if !og.statuses.is_empty() {
                                                this.open_group(og.clone(), cx);
                                                return;
                                            }
                                        }
                                        this.composer_open = true;
                                        this.composer_focus.focus(window, cx);
                                        cx.notify();
                                    }))
                                    .child(
                                        div()
                                            .relative()
                                            .w(px(48.0))
                                            .h(px(48.0))
                                            .flex_shrink_0()
                                            .child(
                                                div()
                                                    .size_full()
                                                    .rounded_full()
                                                    .bg(theme.primary.opacity(0.12))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(
                                                        svg()
                                                            .data(FILE_TEXT_SVG)
                                                            .size(px(20.0))
                                                            .text_color(theme.primary),
                                                    ),
                                            )
                                            .children(if !has_own {
                                                Some(
                                                    div()
                                                        .absolute()
                                                        .bottom(px(-2.0))
                                                        .right(px(-2.0))
                                                        .w(px(20.0))
                                                        .h(px(20.0))
                                                        .rounded_full()
                                                        .bg(theme.primary)
                                                        .border_2()
                                                        .border_color(bg_color)
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .child(
                                                            svg()
                                                                .data(PLUS_SVG)
                                                                .size(px(12.0))
                                                                .text_color(theme.primary_foreground),
                                                        ),
                                                )
                                            } else {
                                                None
                                            }),
                                    )
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .overflow_hidden()
                                            .gap_0p5()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(text_color)
                                                    .child("Status Saya"),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(muted_text)
                                                    .child(if has_own {
                                                        format_status_time(latest_time)
                                                    } else {
                                                        "Klik untuk tambah status".to_string()
                                                    }),
                                            ),
                                    )
                            })
                            // Recent Statuses Section
                            .children(if !recent_groups.is_empty() {
                                Some(
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
                                                .child("RECENT"),
                                        )
                                        .children(recent_groups.into_iter().map(|group| {
                                            let unseen = !group.all_viewed;
                                            let display_name = group.name.clone().unwrap_or_else(|| {
                                                group.sender.split('@').next().unwrap_or(&group.sender).to_string()
                                            });
                                            let time_str = format_status_time(group.latest_time);
                                            let sender_clone = group.sender.clone();
                                            let avatar_url = resolve_avatar_url(
                                                group.avatar.as_deref().unwrap_or(""),
                                                &group.sender,
                                                &base_url,
                                            );
                                            let group_clone = group.clone();

                                            h_flex()
                                                .id(SharedString::from(format!("status-item-{}", group.sender)))
                                                .w_full()
                                                .p_3()
                                                .rounded_xl()
                                                .items_center()
                                                .gap_3()
                                                .cursor_pointer()
                                                .hover(|s| s.bg(theme.muted.opacity(0.4)))
                                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                    this.open_group(group_clone.clone(), cx);
                                                }))
                                                .child(
                                                    div()
                                                        .w(px(46.0))
                                                        .h(px(46.0))
                                                        .rounded_full()
                                                        .p(px(2.0))
                                                        .border_2()
                                                        .border_color(if unseen {
                                                            theme.primary
                                                        } else {
                                                            muted_text.opacity(0.3)
                                                        })
                                                        .child(
                                                            div()
                                                                .size_full()
                                                                .rounded_full()
                                                                .overflow_hidden()
                                                                .relative()
                                                                .bg(avatar_color_for(&sender_clone))
                                                                .child(
                                                                    div()
                                                                        .size_full()
                                                                        .flex()
                                                                        .items_center()
                                                                        .justify_center()
                                                                        .text_color(rgb(0xffffff))
                                                                        .font_weight(FontWeight::BOLD)
                                                                        .text_xs()
                                                                        .child(
                                                                            display_name
                                                                                .chars()
                                                                                .next()
                                                                                .map(|c| c.to_uppercase().to_string())
                                                                                .unwrap_or_else(|| "?".to_string()),
                                                                        ),
                                                                )
                                                                .child(
                                                                    img(avatar_url)
                                                                        .absolute()
                                                                        .inset_0()
                                                                        .size_full()
                                                                        .rounded_full()
                                                                        .object_fit(ObjectFit::Cover),
                                                                ),
                                                        ),
                                                )
                                                .child(
                                                    v_flex()
                                                        .flex_1()
                                                        .overflow_hidden()
                                                        .gap_0p5()
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(text_color)
                                                                .child(display_name),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(muted_text)
                                                                .child(time_str),
                                                        ),
                                                )
                                        })),
                                )
                            } else {
                                None
                            }),
                    ),
            )
            // --- Right Main Viewer Pane ---
            .child(
                v_flex()
                    .flex_1()
                    .h_full()
                    .overflow_hidden()
                    .bg(if self.viewing_group.is_some() {
                        rgb(0x0b141a).into()
                    } else {
                        theme.muted.opacity(0.1)
                    })
                    .children(if let Some(group) = self.viewing_group.clone() {
                        let total = group.statuses.len();
                        let current_idx = self.viewing_index.min(total.saturating_sub(1));
                        let entry = group.statuses.get(current_idx).cloned();
                        let display_name = group.name.clone().unwrap_or_else(|| {
                            group.sender.split('@').next().unwrap_or(&group.sender).to_string()
                        });
                        let sender_clone = group.sender.clone();
                        let a_url = resolve_avatar_url(
                            group.avatar.as_deref().unwrap_or(""),
                            &group.sender,
                            &base_url,
                        );

                        Some(
                            v_flex()
                                .size_full()
                                .relative()
                                // Story Progress Segments at Top
                                .child(
                                    h_flex()
                                        .w_full()
                                        .px_4()
                                        .pt_3()
                                        .gap_1p5()
                                        .children((0..total).map(|i| {
                                            div()
                                                .flex_1()
                                                .h(px(3.0))
                                                .rounded_full()
                                                .bg(if i <= current_idx {
                                                    rgb(0xffffff)
                                                } else {
                                                    rgba(0xffffff40)
                                                })
                                        })),
                                )
                                // Story Header (Avatar, Name, Timestamp, Close Button)
                                .child(
                                    h_flex()
                                        .w_full()
                                        .px_4()
                                        .py_3()
                                        .justify_between()
                                        .items_center()
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .gap_3()
                                                .child(
                                                    div()
                                                        .w(px(36.0))
                                                        .h(px(36.0))
                                                        .rounded_full()
                                                        .overflow_hidden()
                                                        .relative()
                                                        .bg(avatar_color_for(&sender_clone))
                                                        .child(
                                                            div()
                                                                .size_full()
                                                                .flex()
                                                                .items_center()
                                                                .justify_center()
                                                                .text_color(rgb(0xffffff))
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_xs()
                                                                .child(
                                                                    display_name
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
                                                            div()
                                                                .text_sm()
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(rgb(0xffffff))
                                                                .child(display_name),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(rgba(0xffffff99))
                                                                .child(if let Some(ref e) = entry {
                                                                    format_status_time(e.timestamp)
                                                                } else {
                                                                    String::new()
                                                                }),
                                                        ),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .cursor_pointer()
                                                .px_3()
                                                .py_1p5()
                                                .rounded_lg()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(rgba(0xffffffb3))
                                                .hover(|s| s.bg(rgba(0xffffff20)).text_color(rgb(0xffffff)))
                                                .child("Tutup")
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.viewing_group = None;
                                                    this.viewing_index = 0;
                                                    this.load_statuses(cx);
                                                    cx.notify();
                                                })),
                                        ),
                                )
                                // Story Body + Navigation Zones
                                .child(
                                    div()
                                        .flex_1()
                                        .relative()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .overflow_hidden()
                                        // Media / Text View
                                        .child(if let Some(ref e) = entry {
                                            if e.status_type == "image" && e.media_url.is_some() {
                                                let img_url = resolve_media_url(e.media_url.as_ref().unwrap(), &base_url);
                                                div()
                                                    .size_full()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(
                                                        img(img_url)
                                                            .max_w_full()
                                                            .max_h_full()
                                                            .object_fit(ObjectFit::Contain),
                                                    )
                                            } else {
                                                // Text status
                                                let content = e.content.clone().unwrap_or_default();
                                                div()
                                                    .size_full()
                                                    .bg(rgb(0x075e54))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .p_10()
                                                    .child(
                                                        div()
                                                            .max_w(px(500.0))
                                                            .text_2xl()
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(rgb(0xffffff))
                                                            .child(content),
                                                    )
                                            }
                                        } else {
                                            div().child("No status")
                                        })
                                        // Left Click Nav Area
                                        .child(
                                            div()
                                                .absolute()
                                                .left_0()
                                                .top_0()
                                                .bottom_0()
                                                .w(px(120.0))
                                                .cursor_pointer()
                                                .flex()
                                                .items_center()
                                                .justify_start()
                                                .pl_4()
                                                .child(if current_idx > 0 {
                                                    svg().data(CHEVRON_LEFT_SVG).size(px(32.0)).text_color(rgba(0xffffff80))
                                                } else {
                                                    svg()
                                                })
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.navigate(-1, cx);
                                                })),
                                        )
                                        // Right Click Nav Area
                                        .child(
                                            div()
                                                .absolute()
                                                .right_0()
                                                .top_0()
                                                .bottom_0()
                                                .w(px(120.0))
                                                .cursor_pointer()
                                                .flex()
                                                .items_center()
                                                .justify_end()
                                                .pr_4()
                                                .child(if current_idx + 1 < total {
                                                    svg().data(CHEVRON_RIGHT_SVG).size(px(32.0)).text_color(rgba(0xffffff80))
                                                } else {
                                                    svg()
                                                })
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.navigate(1, cx);
                                                })),
                                        ),
                                ),
                        )
                    } else {
                        // Empty state placeholder
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
                                        .bg(theme.muted.opacity(0.3))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(
                                            svg()
                                                .data(CIRCLE_DASHED_SVG)
                                                .size(px(48.0))
                                                .text_color(muted_text),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_3xl()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(text_color)
                                        .child("Bagikan status"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(muted_text)
                                        .child("Bagikan foto, video, dan teks yang hilang setelah 24 jam."),
                                ),
                        )
                    }),
            )
            // --- Status Composer Modal ---
            .children(if self.composer_open {
                let current_bg = argb_to_rgb(self.composer_bg);
                let text_val = self.composer_text.clone();
                let is_posting = self.is_posting;

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
                                                .child("New status"),
                                        )
                                        .child(
                                            div()
                                                .cursor_pointer()
                                                .p_1()
                                                .rounded_full()
                                                .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                .child(svg().data(X_SVG).size(px(16.0)).text_color(muted_text))
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.composer_open = false;
                                                    cx.notify();
                                                })),
                                        ),
                                )
                                // Colored Textarea Preview Box
                                .child(
                                    v_flex()
                                        .h(px(160.0))
                                        .p_4()
                                        .rounded_xl()
                                        .bg(current_bg)
                                        .items_center()
                                        .justify_center()
                                        .cursor_text()
                                        .track_focus(&self.composer_focus)
                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                            this.composer_focus.focus(window, cx);
                                        }))
                                        .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                                            match ev.keystroke.key.as_str() {
                                                "backspace" => {
                                                    this.composer_text.pop();
                                                    cx.notify();
                                                }
                                                "space" => {
                                                    this.composer_text.push(' ');
                                                    cx.notify();
                                                }
                                                "enter" => {
                                                    this.composer_text.push('\n');
                                                    cx.notify();
                                                }
                                                k if k.len() == 1 => {
                                                    this.composer_text.push_str(k);
                                                    cx.notify();
                                                }
                                                _ => {}
                                            }
                                        }))
                                        .child(
                                            div()
                                                .text_lg()
                                                .text_color(if text_val.is_empty() {
                                                    rgba(0xffffff99)
                                                } else {
                                                    rgb(0xffffff)
                                                })
                                                .child(if text_val.is_empty() {
                                                    "Type a status…".to_string()
                                                } else {
                                                    text_val
                                                }),
                                        ),
                                )
                                // Background Color Swatches
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .children(BACKGROUNDS.iter().map(|&color| {
                                            let is_selected = self.composer_bg == color;
                                            div()
                                                .cursor_pointer()
                                                .w(px(28.0))
                                                .h(px(28.0))
                                                .rounded_full()
                                                .bg(argb_to_rgb(color))
                                                .border_2()
                                                .border_color(if is_selected {
                                                    theme.primary
                                                } else {
                                                    rgba(0x00000000).into()
                                                })
                                                .hover(|s| s.opacity(0.85))
                                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                    this.composer_bg = color;
                                                    cx.notify();
                                                }))
                                        })),
                                )
                                // Footer Actions
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
                                                    this.composer_open = false;
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            div()
                                                .cursor_pointer()
                                                .px_5()
                                                .py_2()
                                                .rounded_xl()
                                                .bg(if is_posting || self.composer_text.trim().is_empty() {
                                                    theme.primary.opacity(0.5)
                                                } else {
                                                    theme.primary
                                                })
                                                .text_color(theme.primary_foreground)
                                                .text_sm()
                                                .font_weight(FontWeight::BOLD)
                                                .hover(|s| {
                                                    s.opacity(0.9)
                                                })
                                                .child(if is_posting { "Posting…" } else { "Post" })
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.post_status(cx);
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

//! Calls view matching Web CallHistoryPage 1:1.

use gpui::*;
use gpui_component::{h_flex, v_flex};
use wabot_backend_client::client::HttpClient;
use wabot_backend_client::dto::{CallDirection, CallHistoryFilter, CallLog, CallStatus, CallType};

use crate::icons::*;
use crate::router::{AppRoute, Router};
use crate::state::auth::AuthState;
use crate::state::call::CallManager;
use crate::theme::manager::AppThemeExt;
use crate::TOKIO_RT;

const PAGE_SIZE: usize = 25;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CallFilterKey {
    #[default]
    All,
    Incoming,
    Outgoing,
    Missed,
}

impl CallFilterKey {
    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Incoming => "Incoming",
            Self::Outgoing => "Outgoing",
            Self::Missed => "Missed",
        }
    }
}

fn is_missed(log: &CallLog) -> bool {
    matches!(log.status, CallStatus::Missed | CallStatus::Rejected | CallStatus::Failed)
}

fn display_name(log: &CallLog) -> String {
    if let Some(ref grp) = log.group_jid {
        return grp.clone();
    }
    let target = &log.target;
    target.split('@').next().unwrap_or(target).to_string()
}

fn initials(log: &CallLog) -> String {
    let name = display_name(log);
    let mut chars = name.chars().filter(|c| c.is_alphanumeric());
    let first = chars.next().map(|c| c.to_ascii_uppercase()).unwrap_or('?');
    let second = chars.next().map(|c| c.to_ascii_uppercase()).unwrap_or(' ');
    if second != ' ' {
        format!("{}{}", first, second)
    } else {
        format!("{}", first)
    }
}

fn format_duration(ms: Option<i64>) -> String {
    match ms {
        Some(ms) if ms > 0 => {
            let total_secs = ms / 1000;
            let m = total_secs / 60;
            let s = total_secs % 60;
            format!("{}:{:02}", m, s)
        }
        _ => "—".to_string(),
    }
}

fn format_call_time(ts: i64) -> String {
    if ts <= 0 {
        return String::new();
    }
    let ts_sec = if ts > 10_000_000_000 { ts / 1000 } else { ts };
    let hours = (ts_sec / 3600) % 24;
    let mins = (ts_sec / 60) % 60;
    format!("{:02}:{:02}", hours, mins)
}

fn format_full_date_time(ts: i64) -> String {
    if ts <= 0 {
        return "—".to_string();
    }
    let ts_sec = if ts > 10_000_000_000 { ts / 1000 } else { ts };
    let hours = (ts_sec / 3600) % 24;
    let mins = (ts_sec / 60) % 60;
    format!("{:02}:{:02} (ts: {})", hours, mins, ts_sec)
}

fn group_label(ts: i64) -> String {
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

fn status_label(status: &CallStatus) -> &'static str {
    match status {
        CallStatus::Preparing => "Preparing",
        CallStatus::Initiating => "Initiating",
        CallStatus::Ringing => "Ringing",
        CallStatus::Connecting => "Connecting",
        CallStatus::Connected => "Connected",
        CallStatus::Ending => "Ending",
        CallStatus::Ended => "Ended",
        CallStatus::Rejected => "Rejected",
        CallStatus::Missed => "Missed",
        CallStatus::Busy => "Busy",
        CallStatus::Failed => "Failed",
        CallStatus::Interrupted => "Interrupted",
        CallStatus::Unknown => "Unknown",
    }
}

fn direction_label(direction: &CallDirection) -> &'static str {
    match direction {
        CallDirection::Incoming => "Incoming",
        CallDirection::Outgoing => "Outgoing",
        CallDirection::Unknown => "Unknown",
    }
}

fn type_label(call_type: &CallType) -> &'static str {
    match call_type {
        CallType::Audio => "Audio",
        CallType::Video => "Video",
        CallType::GroupAudio => "Group Audio",
        CallType::GroupVideo => "Group Video",
        CallType::Unknown => "Unknown",
    }
}

pub struct CallsView {
    logs: Vec<CallLog>,
    is_loading: bool,
    is_loading_more: bool,
    has_more: bool,
    filter: CallFilterKey,
    search_query: String,
    search_focus: FocusHandle,
    selected_log: Option<CallLog>,
    error_message: Option<String>,
    toast_message: Option<String>,
}

impl CallsView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let search_focus = cx.focus_handle();
        let mut this = Self {
            logs: Vec::new(),
            is_loading: true,
            is_loading_more: false,
            has_more: true,
            filter: CallFilterKey::All,
            search_query: String::new(),
            search_focus,
            selected_log: None,
            error_message: None,
            toast_message: None,
        };
        this.load_calls(true, cx);
        this
    }

    pub fn set_filter(&mut self, filter: CallFilterKey, cx: &mut Context<Self>) {
        if self.filter != filter {
            self.filter = filter;
            self.load_calls(true, cx);
        }
    }

    pub fn load_calls(&mut self, reset: bool, cx: &mut Context<Self>) {
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        if reset {
            self.is_loading = true;
            self.error_message = None;
        } else {
            self.is_loading_more = true;
        }
        cx.notify();

        let before = if !reset && !self.logs.is_empty() {
            self.logs.last().map(|l| l.started_at)
        } else {
            None
        };

        let filter_api = match self.filter {
            CallFilterKey::All => CallHistoryFilter {
                limit: Some(PAGE_SIZE),
                before,
                ..Default::default()
            },
            CallFilterKey::Incoming => CallHistoryFilter {
                limit: Some(PAGE_SIZE),
                before,
                direction: Some("incoming".to_string()),
                ..Default::default()
            },
            CallFilterKey::Outgoing => CallHistoryFilter {
                limit: Some(PAGE_SIZE),
                before,
                direction: Some("outgoing".to_string()),
                ..Default::default()
            },
            CallFilterKey::Missed => CallHistoryFilter {
                limit: Some(PAGE_SIZE),
                before,
                status: Some("missed".to_string()),
                ..Default::default()
            },
        };

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT.spawn(async move {
                    client.get_call_history(Some(&filter_api)).await
                }).await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_loading = false;
                            this.is_loading_more = false;
                            match res {
                                Ok(Ok(resp)) => {
                                    this.has_more = resp.logs.len() >= PAGE_SIZE;
                                    if reset {
                                        this.logs = resp.logs;
                                    } else {
                                        for l in resp.logs {
                                            if !this.logs.iter().any(|ex| ex.id == l.id) {
                                                this.logs.push(l);
                                            }
                                        }
                                    }
                                }
                                Ok(Err(e)) => {
                                    this.error_message = Some(e.to_string());
                                }
                                _ => {
                                    this.error_message = Some("Failed to load calls".to_string());
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

    pub fn call_back(&mut self, log: &CallLog, cx: &mut Context<Self>) {
        let is_video = matches!(log.call_type, CallType::Video | CallType::GroupVideo);
        let call_type = if is_video { CallType::Video } else { CallType::Audio };
        let target = log.target.clone();

        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        self.toast_message = Some(format!("Calling {}…", display_name(log)));

        // Show the full-screen call overlay right away, then adopt the state the
        // backend returns (mirrors the Web `call_back` flow).
        if cx.has_global::<CallManager>() {
            CallManager::global_mut(cx).initiate_call(&target, call_type.clone(), None);
            cx.notify();
        }

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let call_target = target.clone();
                let request_type = call_type.clone();
                let res = TOKIO_RT
                    .spawn(async move {
                        let client = HttpClient::new(&base_url);
                        client.create_call(&call_target, request_type).await
                    })
                    .await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            if cx.has_global::<CallManager>() {
                                match res {
                                    Ok(Ok(call_state)) => CallManager::global_mut(cx).adopt(call_state),
                                    _ => {
                                        CallManager::global_mut(cx).end_local(None);
                                        this.error_message = Some("Failed to start call".to_string());
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

    pub fn view_chat(&mut self, target: &str, cx: &mut Context<Self>) {
        if cx.has_global::<Router>() {
            Router::global_mut(cx).navigate(AppRoute::ChatDetail(target.to_string()));
        }
    }
}

impl Render for CallsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.app_theme();
        let border_color = theme.border;
        let text_color = theme.foreground;
        let muted_text = theme.muted_foreground;
        let bg_color = theme.background;
        let card_bg = theme.card;

        // Filter and search visible logs
        let q = self.search_query.trim().to_lowercase();
        let visible_logs: Vec<CallLog> = self.logs.iter().filter(|l| {
            if self.filter == CallFilterKey::Missed && !is_missed(l) {
                return false;
            }
            if !q.is_empty() {
                let name = display_name(l).to_lowercase();
                let target = l.target.to_lowercase();
                let grp = l.group_jid.as_deref().unwrap_or("").to_lowercase();
                name.contains(&q) || target.contains(&q) || grp.contains(&q)
            } else {
                true
            }
        }).cloned().collect();

        // Group logs by date
        let mut grouped: Vec<(String, Vec<CallLog>)> = Vec::new();
        for log in &visible_logs {
            let label = group_label(log.started_at);
            if let Some((last_label, list)) = grouped.last_mut() {
                if last_label == &label {
                    list.push(log.clone());
                    continue;
                }
            }
            grouped.push((label, vec![log.clone()]));
        }

        let total_count = self.logs.len();
        let selected_id = self.selected_log.as_ref().map(|l| l.id.clone());

        h_flex()
            .id("calls-view")
            .size_full()
            .overflow_hidden()
            .bg(bg_color)
            .relative()
            // --- Left Sidebar (360px) ---
            .child(
                v_flex()
                    .w(px(360.0))
                    .h_full()
                    .flex_shrink_0()
                    .border_r_1()
                    .border_color(border_color)
                    .bg(bg_color)
                    // Sidebar Header
                    .child(
                        v_flex()
                            .p_5()
                            .gap_4()
                            .border_b_1()
                            .border_color(border_color)
                            // Top Row: Title, Count, Refresh
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        h_flex()
                                            .items_baseline()
                                            .gap_2p5()
                                            .child(
                                                div()
                                                    .text_2xl()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(text_color)
                                                    .child("Calls"),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(muted_text)
                                                    .child(format!("{total_count}")),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .cursor_pointer()
                                            .p_1p5()
                                            .rounded_full()
                                            .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                            .child(
                                                svg()
                                                    .data(REFRESH_CW_SVG)
                                                    .size(px(16.0))
                                                    .text_color(muted_text),
                                            )
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                this.load_calls(true, cx);
                                            })),
                                    ),
                            )
                            // Search Field
                            .child(
                                h_flex()
                                    .px_3()
                                    .py_2()
                                    .rounded_xl()
                                    .bg(theme.muted.opacity(0.4))
                                    .items_center()
                                    .gap_2()
                                    .track_focus(&self.search_focus)
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                        this.search_focus.focus(window, cx);
                                    }))
                                    .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                                        match ev.keystroke.key.as_str() {
                                            "backspace" => {
                                                this.search_query.pop();
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
                                    .child(svg().data(SEARCH_SVG).size(px(16.0)).text_color(muted_text))
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_sm()
                                            .text_color(if self.search_query.is_empty() { muted_text } else { text_color })
                                            .child(if self.search_query.is_empty() {
                                                "Search calls".to_string()
                                            } else {
                                                self.search_query.clone()
                                            }),
                                    )
                                    .children(if !self.search_query.is_empty() {
                                        Some(
                                            div()
                                                .cursor_pointer()
                                                .p_1()
                                                .child(svg().data(X_SVG).size(px(14.0)).text_color(muted_text))
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.search_query.clear();
                                                    cx.notify();
                                                })),
                                        )
                                    } else {
                                        None
                                    }),
                            )
                            // Filter Chips Row
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .overflow_x_hidden()
                                    .children([
                                        CallFilterKey::All,
                                        CallFilterKey::Incoming,
                                        CallFilterKey::Outgoing,
                                        CallFilterKey::Missed,
                                    ].into_iter().map(|f| {
                                        let is_active = self.filter == f;
                                        div()
                                            .cursor_pointer()
                                            .px_3p5()
                                            .py_1p5()
                                            .rounded_full()
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .bg(if is_active {
                                                theme.primary
                                            } else {
                                                theme.muted.opacity(0.5)
                                            })
                                            .text_color(if is_active {
                                                theme.primary_foreground
                                            } else {
                                                muted_text
                                            })
                                            .hover(|s| {
                                                if !is_active {
                                                    s.bg(theme.muted.opacity(0.8)).text_color(text_color)
                                                } else {
                                                    s
                                                }
                                            })
                                            .child(f.label())
                                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                this.set_filter(f, cx);
                                            }))
                                    })),
                            ),
                    )
                    // Calls List Scroll Area
                    .child(
                        v_flex()
                            .id("calls-list-scroll")
                            .flex_1()
                            .overflow_y_scroll()
                            .px_2()
                            .py_2()
                            .children(if self.is_loading && self.logs.is_empty() {
                                Some(
                                    div()
                                        .py_12()
                                        .flex()
                                        .justify_center()
                                        .text_xs()
                                        .text_color(muted_text)
                                        .child("Loading calls…"),
                                )
                            } else if visible_logs.is_empty() {
                                Some(
                                    v_flex()
                                        .py_16()
                                        .items_center()
                                        .justify_center()
                                        .gap_2()
                                        .child(svg().data(PHONE_OFF_SVG).size(px(36.0)).text_color(muted_text.opacity(0.5)))
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(text_color)
                                                .child(if self.search_query.is_empty() {
                                                    "No calls yet"
                                                } else {
                                                    "No results found"
                                                }),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(muted_text)
                                                .child(if self.search_query.is_empty() {
                                                    "Your call history will appear here."
                                                } else {
                                                    "Nothing matches your search."
                                                }),
                                        ),
                                )
                            } else {
                                None
                            })
                            // Date Groups
                            .children(grouped.into_iter().map(|(label, items)| {
                                v_flex()
                                    .w_full()
                                    .gap_1()
                                    // Group Sticky Header
                                    .child(
                                        div()
                                            .px_3()
                                            .pt_3()
                                            .pb_1()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(muted_text)
                                            .child(label),
                                    )
                                    // Call Rows in Group
                                    .children(items.into_iter().map(|log| {
                                        let is_sel = selected_id.as_deref() == Some(&log.id);
                                        let missed = is_missed(&log);
                                        let is_video = matches!(log.call_type, CallType::Video | CallType::GroupVideo);
                                        let name = display_name(&log);
                                        let time_str = format_call_time(log.started_at);
                                        let dur_str = format_duration(log.duration_ms);
                                        let log_clone = log.clone();

                                        h_flex()
                                            .id(SharedString::from(format!("call-row-{}", log.id)))
                                            .w_full()
                                            .p_3()
                                            .rounded_2xl()
                                            .items_center()
                                            .gap_3p5()
                                            .cursor_pointer()
                                            .bg(if is_sel {
                                                theme.primary.opacity(0.14)
                                            } else {
                                                rgba(0x00000000).into()
                                            })
                                            .hover(|s| {
                                                if is_sel {
                                                    s.bg(theme.primary.opacity(0.2))
                                                } else {
                                                    s.bg(theme.muted.opacity(0.4))
                                                }
                                            })
                                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                this.selected_log = Some(log_clone.clone());
                                                cx.notify();
                                            }))
                                            // Status Icon Circle
                                            .child(
                                                div()
                                                    .relative()
                                                    .w(px(42.0))
                                                    .h(px(42.0))
                                                    .rounded_full()
                                                    .flex_shrink_0()
                                                    .bg(if missed {
                                                        theme.destructive.opacity(0.12)
                                                    } else {
                                                        theme.muted.opacity(0.6)
                                                    })
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(
                                                        svg()
                                                            .data(if missed {
                                                                PHONE_MISSED_SVG
                                                            } else if log.direction == CallDirection::Incoming {
                                                                ARROW_DOWN_LEFT_SVG
                                                            } else {
                                                                ARROW_UP_RIGHT_SVG
                                                            })
                                                            .size(px(18.0))
                                                            .text_color(if missed {
                                                                theme.destructive
                                                            } else {
                                                                theme.primary
                                                            }),
                                                    )
                                                    .children(if is_video && !missed {
                                                        Some(
                                                            div()
                                                                .absolute()
                                                                .bottom(px(-2.0))
                                                                .right(px(-2.0))
                                                                .p_0p5()
                                                                .rounded_full()
                                                                .bg(bg_color)
                                                                .child(
                                                                    svg()
                                                                        .data(VIDEO_SVG)
                                                                        .size(px(12.0))
                                                                        .text_color(muted_text),
                                                                ),
                                                        )
                                                    } else {
                                                        None
                                                    }),
                                            )
                                            // Contact Name + Meta
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
                                                            .overflow_hidden()
                                                            .child(name),
                                                    )
                                                    .child(
                                                        h_flex()
                                                            .items_center()
                                                            .gap_1p5()
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(if missed {
                                                                        theme.destructive
                                                                    } else {
                                                                        muted_text
                                                                    })
                                                                    .child(if missed {
                                                                        "Missed".to_string()
                                                                    } else {
                                                                        format!(
                                                                            "{} · {}",
                                                                            direction_label(&log.direction),
                                                                            if is_video { "Video" } else { "Voice" }
                                                                        )
                                                                    }),
                                                            )
                                                            .child(div().text_xs().text_color(muted_text.opacity(0.5)).child("·"))
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(muted_text)
                                                                    .child(dur_str),
                                                            ),
                                                    ),
                                            )
                                            // Timestamp & Missed Dot
                                            .child(
                                                v_flex()
                                                    .items_end()
                                                    .gap_1()
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(muted_text)
                                                            .child(time_str),
                                                    )
                                                    .children(if missed {
                                                        Some(
                                                            div()
                                                                .w(px(8.0))
                                                                .h(px(8.0))
                                                                .rounded_full()
                                                                .bg(theme.destructive),
                                                        )
                                                    } else {
                                                        None
                                                    }),
                                            )
                                    }))
                            }))
                            // Load More Button
                            .children(if self.has_more && !self.is_loading {
                                Some(
                                    h_flex()
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
                                                .child(if self.is_loading_more { "Loading…" } else { "Load more" })
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.load_calls(false, cx);
                                                })),
                                        ),
                                )
                            } else {
                                None
                            }),
                    ),
            )
            // --- Right Main Detail Pane ---
            .child(
                v_flex()
                    .flex_1()
                    .h_full()
                    .overflow_hidden()
                    .bg(bg_color)
                    .children(if let Some(log) = self.selected_log.clone() {
                        let missed = is_missed(&log);
                        let is_video = matches!(log.call_type, CallType::Video | CallType::GroupVideo);
                        let name = display_name(&log);
                        let target_clone = log.target.clone();
                        let log_clone = log.clone();

                        Some(
                            v_flex()
                                .id("call-detail-scroll")
                                .size_full()
                                .overflow_y_scroll()
                                .p_6()
                                .gap_6()
                                // Top Close Action
                                .child(
                                    h_flex()
                                        .justify_between()
                                        .items_center()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(muted_text)
                                                .child("CALL DETAILS"),
                                        )
                                        .child(
                                            div()
                                                .cursor_pointer()
                                                .p_1p5()
                                                .rounded_full()
                                                .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                .child(svg().data(X_SVG).size(px(16.0)).text_color(muted_text))
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.selected_log = None;
                                                    cx.notify();
                                                })),
                                        ),
                                )
                                // Hero Card
                                .child(
                                    v_flex()
                                        .items_center()
                                        .p_6()
                                        .rounded_2xl()
                                        .bg(card_bg)
                                        .border_1()
                                        .border_color(border_color)
                                        .gap_3()
                                        // Large Hero Avatar
                                        .child(
                                            div()
                                                .w(px(68.0))
                                                .h(px(68.0))
                                                .rounded_full()
                                                .bg(if missed {
                                                    theme.destructive.opacity(0.12)
                                                } else {
                                                    theme.primary.opacity(0.12)
                                                })
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .child(if missed {
                                                    svg().data(PHONE_MISSED_SVG).size(px(32.0)).text_color(theme.destructive).into_any_element()
                                                } else {
                                                    div().text_xl().font_weight(FontWeight::BOLD).text_color(theme.primary).child(initials(&log)).into_any_element()
                                                }),
                                        )
                                        // Name & Subtitle
                                        .child(
                                            div()
                                                .text_xl()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(text_color)
                                                .child(name),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(if missed { theme.destructive } else { muted_text })
                                                .child(format!(
                                                    "{} · {}",
                                                    if missed { "Missed call" } else { direction_label(&log.direction) },
                                                    if is_video { "Video" } else { "Voice" }
                                                )),
                                        )
                                        // Action Buttons: View chat & Call back
                                        .child(
                                            h_flex()
                                                .gap_3()
                                                .pt_2()
                                                .child(
                                                    h_flex()
                                                        .cursor_pointer()
                                                        .px_4()
                                                        .py_2()
                                                        .rounded_xl()
                                                        .border_1()
                                                        .border_color(border_color)
                                                        .gap_2()
                                                        .items_center()
                                                        .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                            this.view_chat(&target_clone, cx);
                                                        }))
                                                        .child(svg().data(MESSAGE_SQUARE_SVG).size(px(16.0)).text_color(text_color))
                                                        .child(div().text_sm().font_weight(FontWeight::MEDIUM).text_color(text_color).child("View chat")),
                                                )
                                                .child(
                                                    h_flex()
                                                        .cursor_pointer()
                                                        .px_4()
                                                        .py_2()
                                                        .rounded_xl()
                                                        .bg(theme.primary)
                                                        .gap_2()
                                                        .items_center()
                                                        .hover(|s| s.opacity(0.9))
                                                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                            this.call_back(&log_clone, cx);
                                                        }))
                                                        .child(svg().data(PHONE_SVG).size(px(16.0)).text_color(theme.primary_foreground))
                                                        .child(div().text_sm().font_weight(FontWeight::BOLD).text_color(theme.primary_foreground).child("Call back")),
                                                ),
                                        ),
                                )
                                // Detailed Metadata Table
                                .child(
                                    v_flex()
                                        .p_4()
                                        .rounded_2xl()
                                        .bg(card_bg)
                                        .border_1()
                                        .border_color(border_color)
                                        .gap_3()
                                        .child(Self::detail_row("Status", status_label(&log.status), text_color, muted_text))
                                        .child(Self::detail_row("Direction", direction_label(&log.direction), text_color, muted_text))
                                        .child(Self::detail_row("Type", type_label(&log.call_type), text_color, muted_text))
                                        .child(Self::detail_row("Duration", &format_duration(log.duration_ms), text_color, muted_text))
                                        .child(Self::detail_row("Started", &format_full_date_time(log.started_at), text_color, muted_text))
                                        .children(if let Some(ended) = log.ended_at {
                                            Some(Self::detail_row("Ended", &format_full_date_time(ended), text_color, muted_text))
                                        } else {
                                            None
                                        })
                                        .child(Self::detail_row("Target", &log.target, text_color, muted_text))
                                        .children(if let Some(ref grp) = log.group_jid {
                                            Some(Self::detail_row("Group", grp, text_color, muted_text))
                                        } else {
                                            None
                                        })
                                        .children(if let Some(ref err) = log.error_message {
                                            Some(Self::detail_row("Error", err, theme.destructive, muted_text))
                                        } else {
                                            None
                                        })
                                        .child(Self::detail_row("ID", &log.id, muted_text, muted_text)),
                                ),
                        )
                    } else {
                        // Empty State Graphic
                        Some(
                            v_flex()
                                .id("call-empty-state")
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
                                                .data(PHONE_SVG)
                                                .size(px(40.0))
                                                .text_color(muted_text.opacity(0.5)),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_2xl()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(text_color)
                                        .child("Select a call"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(muted_text)
                                        .child("Select a call from history to view details."),
                                ),
                        )
                    }),
            )
    }
}

impl CallsView {
    fn detail_row(label: &str, value: &str, val_color: Hsla, label_color: Hsla) -> impl IntoElement {
        h_flex()
            .justify_between()
            .items_center()
            .py_1()
            .child(div().text_xs().text_color(label_color).child(label.to_string()))
            .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(val_color).child(value.to_string()))
    }
}

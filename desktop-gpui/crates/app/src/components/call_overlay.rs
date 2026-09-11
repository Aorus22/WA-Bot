//! Full-screen call UI matching the Web `CallOverlay` and `IncomingCallOverlay`.
//!
//! The overlay is mounted once by [`crate::views::shell::AppShellView`] and reads
//! the global [`CallManager`], so an active call covers whichever route is open —
//! exactly like the Web client's `fixed inset-0 z-[90]` dialog.

use std::time::Duration;

use gpui::*;
use gpui_component::spinner::Spinner;
use gpui_component::{h_flex, v_flex};
use wabot_backend_client::client::HttpClient;
use wabot_backend_client::dto::CallType;

use crate::icons::*;
use crate::state::auth::AuthState;
use crate::state::call::{format_duration, now_millis, CallManager};
use crate::state::chat::ChatStore;
use crate::theme::manager::{ActiveTokens, AppThemeExt};
use crate::TOKIO_RT;

/// Async action currently in flight, used to disable the controls.
#[derive(Clone, Copy, PartialEq)]
enum CallAction {
    Answer,
    Reject,
    Hangup,
    Video,
    VideoUpgrade,
}

/// Testable helpers around call state (no window required).
pub struct CallOverlayHelper;

impl CallOverlayHelper {
    pub fn format_duration(seconds: u64) -> String {
        format_duration(seconds)
    }

    pub fn is_overlay_needed(mgr: &CallManager) -> bool {
        mgr.is_overlay_visible() || mgr.is_incoming_visible()
    }
}

// Overlay chrome is always light-on-dark because the backdrop is the Web
// client's hardcoded `#0a0a0a` scrim.
const ON_DARK: u32 = 0xffffff;
const DANGER: u32 = 0xef4444;
const AMBER: u32 = 0xf59e0b;
const SKY: u32 = 0x0ea5e9;

pub struct CallOverlayView {
    _call_sub: Subscription,
    now_ms: i64,
    action: Option<CallAction>,
    error: Option<String>,
}

impl CallOverlayView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let call_sub = cx.observe_global::<CallManager>(|_this, cx| {
            cx.notify();
        });

        // 1s ticker keeps the connected duration live while a call is up.
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let executor = cx.background_executor().clone();
            let cx_handle = cx.clone();
            async move {
                loop {
                    executor.timer(Duration::from_millis(1000)).await;

                    let mut app = cx_handle.clone();
                    let alive = this
                        .update(&mut app, |view, cx| {
                            let visible = cx.has_global::<CallManager>()
                                && CallManager::global(cx).is_overlay_visible();
                            if visible {
                                view.now_ms = now_millis();
                                cx.notify();
                            }
                            visible
                        })
                        .is_ok();
                    if !alive {
                        break;
                    }
                }
            }
        })
        .detach();

        Self {
            _call_sub: call_sub,
            now_ms: now_millis(),
            action: None,
            error: None,
        }
    }

    // --- State helpers ---

    fn base_url(cx: &Context<Self>) -> String {
        if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        }
    }

    fn active_id(cx: &Context<Self>) -> Option<String> {
        if cx.has_global::<CallManager>() {
            CallManager::global(cx)
                .active_call
                .as_ref()
                .map(|c| c.id.clone())
        } else {
            None
        }
    }

    fn incoming_id(cx: &Context<Self>) -> Option<String> {
        if cx.has_global::<CallManager>() {
            CallManager::global(cx)
                .incoming_call
                .as_ref()
                .map(|c| c.id.clone())
        } else {
            None
        }
    }

    // --- Actions ---

    fn toggle_mic(&mut self, cx: &mut Context<Self>) {
        if cx.has_global::<CallManager>() {
            CallManager::global_mut(cx).toggle_mic();
        }
        cx.notify();
    }

    fn toggle_speaker(&mut self, cx: &mut Context<Self>) {
        if cx.has_global::<CallManager>() {
            CallManager::global_mut(cx).toggle_speaker();
        }
        cx.notify();
    }

    fn end_call(&mut self, cx: &mut Context<Self>) {
        let Some(id) = Self::active_id(cx) else {
            return;
        };
        if self.action.is_some() {
            return;
        }
        self.action = Some(CallAction::Hangup);
        self.error = None;
        cx.notify();

        let base_url = Self::base_url(cx);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let call_id = id.clone();
                let res = TOKIO_RT
                    .spawn(async move { HttpClient::new(&base_url).hangup_call(&call_id).await })
                    .await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.action = None;
                            match res {
                                Ok(Ok(_)) => {
                                    if cx.has_global::<CallManager>() {
                                        CallManager::global_mut(cx).end_local(Some(&id));
                                    }
                                }
                                Ok(Err(e)) => this.error = Some(format!("Failed to end call: {e}")),
                                Err(_) => this.error = Some("Failed to end call".to_string()),
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn toggle_video(&mut self, cx: &mut Context<Self>) {
        let Some(id) = Self::active_id(cx) else {
            return;
        };
        if self.action.is_some() {
            return;
        }

        let (video_on, remote_on) = {
            let mgr = CallManager::global(cx);
            mgr.active_call
                .as_ref()
                .map(|c| (c.video_enabled, c.remote_video_enabled))
                .unwrap_or((false, false))
        };

        self.action = Some(CallAction::Video);
        self.error = None;
        // Optimistic flip so the button reacts immediately; the WS
        // `call.video_state` event confirms it.
        CallManager::global_mut(cx).patch_video(&id, !video_on, remote_on);
        cx.notify();

        let base_url = Self::base_url(cx);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let enabling = !video_on;
                let call_id = id.clone();
                let res = TOKIO_RT
                    .spawn(async move {
                        let client = HttpClient::new(&base_url);
                        if enabling {
                            client.start_video(&call_id).await
                        } else {
                            client.stop_video(&call_id).await
                        }
                    })
                    .await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.action = None;
                            match res {
                                Ok(Ok(_)) => {}
                                Ok(Err(e)) => {
                                    // Roll the optimistic flip back.
                                    CallManager::global_mut(cx).patch_video(&id, video_on, remote_on);
                                    this.error = Some(format!("Failed to switch video: {e}"));
                                }
                                Err(_) => {
                                    CallManager::global_mut(cx).patch_video(&id, video_on, remote_on);
                                    this.error = Some("Failed to switch video".to_string());
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

    fn answer_call(&mut self, cx: &mut Context<Self>) {
        let Some(id) = Self::incoming_id(cx) else {
            return;
        };
        if self.action.is_some() {
            return;
        }
        self.action = Some(CallAction::Answer);
        self.error = None;
        cx.notify();

        let base_url = Self::base_url(cx);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let call_id = id.clone();
                let res = TOKIO_RT
                    .spawn(async move { HttpClient::new(&base_url).answer_call(&call_id).await })
                    .await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.action = None;
                            match res {
                                Ok(Ok(_)) => {
                                    if cx.has_global::<CallManager>() {
                                        CallManager::global_mut(cx).connect_active();
                                    }
                                }
                                Ok(Err(e)) => this.error = Some(format!("Failed to answer: {e}")),
                                Err(_) => this.error = Some("Failed to answer".to_string()),
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn reject_call(&mut self, cx: &mut Context<Self>) {
        let Some(id) = Self::incoming_id(cx) else {
            return;
        };
        if self.action.is_some() {
            return;
        }
        self.action = Some(CallAction::Reject);
        self.error = None;
        cx.notify();

        let base_url = Self::base_url(cx);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let call_id = id.clone();
                let res = TOKIO_RT
                    .spawn(async move { HttpClient::new(&base_url).reject_call(&call_id).await })
                    .await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.action = None;
                            match res {
                                Ok(Ok(_)) => {
                                    if cx.has_global::<CallManager>() {
                                        CallManager::global_mut(cx).end_local(Some(&id));
                                    }
                                }
                                Ok(Err(e)) => this.error = Some(format!("Failed to decline: {e}")),
                                Err(_) => this.error = Some("Failed to decline".to_string()),
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn respond_video_upgrade(&mut self, accept: bool, cx: &mut Context<Self>) {
        let Some(id) = Self::active_id(cx).or_else(|| Self::incoming_id(cx)) else {
            return;
        };
        if self.action.is_some() {
            return;
        }
        self.action = Some(CallAction::VideoUpgrade);
        self.error = None;
        cx.notify();

        let base_url = Self::base_url(cx);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let call_id = id.clone();
                let res = TOKIO_RT
                    .spawn(async move {
                        let client = HttpClient::new(&base_url);
                        if accept {
                            client.accept_video(&call_id).await
                        } else {
                            client.reject_video(&call_id).await
                        }
                    })
                    .await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.action = None;
                            match res {
                                Ok(Ok(_)) => {
                                    if cx.has_global::<CallManager>() {
                                        if accept {
                                            CallManager::global_mut(cx).accept_video_upgrade();
                                        } else {
                                            CallManager::global_mut(cx).video_upgrade_requested = false;
                                        }
                                    }
                                }
                                Ok(Err(e)) => this.error = Some(format!("Failed: {e}")),
                                Err(_) => this.error = Some("Failed".to_string()),
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    // --- Rendering ---

    fn render_active(
        &self,
        mgr: &CallManager,
        _theme: &ActiveTokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let peer = mgr.peer();
        let name = peer_display_name(&peer, cx);
        let initials = peer_initials(&name);
        let status_text = mgr.status_label(self.now_ms);
        let connected = mgr.is_connected();
        let video_on = mgr
            .active_call
            .as_ref()
            .map(|c| c.video_enabled)
            .unwrap_or(false);
        let is_video = mgr.is_video_call() || video_on;
        let is_group = mgr.is_group_call();

        let participants = mgr
            .active_call
            .as_ref()
            .and_then(|c| c.participants.clone())
            .unwrap_or_default();
        let participant_labels: Vec<String> = participants
            .iter()
            .take(8)
            .map(|p| peer_display_name(p, cx))
            .collect();

        let mut controls = h_flex().items_end().justify_center().gap_5();

        controls = controls.child(control_button(
            if mgr.mic_muted { MIC_OFF_SVG } else { MIC_SVG },
            if mgr.mic_muted { "Unmute" } else { "Mute" },
            if mgr.mic_muted {
                rgb(AMBER).opacity(0.85)
            } else {
                rgb(ON_DARK).opacity(0.10)
            },
            if mgr.mic_muted {
                rgb(AMBER).opacity(0.95)
            } else {
                rgb(ON_DARK).opacity(0.16)
            },
            if mgr.mic_muted {
                rgb(ON_DARK)
            } else {
                rgb(ON_DARK).opacity(0.9)
            },
            cx.listener(|this, _, _, cx| this.toggle_mic(cx)),
        ));

        let speaker_on = !mgr.speaker_muted;
        controls = controls.child(control_button(
            if speaker_on { VOLUME_2_SVG } else { VOLUME_X_SVG },
            "Speaker",
            if speaker_on {
                rgb(ON_DARK).opacity(0.10)
            } else {
                rgb(0x3f3f46).opacity(0.6)
            },
            if speaker_on {
                rgb(ON_DARK).opacity(0.16)
            } else {
                rgb(0x3f3f46).opacity(0.7)
            },
            if speaker_on {
                rgb(ON_DARK).opacity(0.9)
            } else {
                rgb(ON_DARK).opacity(0.5)
            },
            cx.listener(|this, _, _, cx| this.toggle_speaker(cx)),
        ));

        // Camera control: offered for voice calls, and kept while the local
        // camera is on so an upgrade can be switched back off.
        if !mgr.is_video_call() || video_on {
            let camera = if self.action == Some(CallAction::Video) {
                control_button_spinner(
                    Spinner::new().color(rgb(ON_DARK).into()),
                    if video_on { "Stop video" } else { "Video" },
                    if video_on {
                        rgb(SKY).opacity(0.85)
                    } else {
                        rgb(ON_DARK).opacity(0.10)
                    },
                    if video_on {
                        rgb(SKY).opacity(0.95)
                    } else {
                        rgb(ON_DARK).opacity(0.16)
                    },
                )
            } else {
                control_button(
                    if video_on { VIDEO_OFF_SVG } else { VIDEO_SVG },
                    if video_on { "Stop video" } else { "Video" },
                    if video_on {
                        rgb(SKY).opacity(0.85)
                    } else {
                        rgb(ON_DARK).opacity(0.10)
                    },
                    if video_on {
                        rgb(SKY).opacity(0.95)
                    } else {
                        rgb(ON_DARK).opacity(0.16)
                    },
                    rgb(ON_DARK).opacity(0.9),
                    cx.listener(|this, _, _, cx| this.toggle_video(cx)),
                )
            };
            controls = controls.child(camera);
        }

        let end_button = if self.action == Some(CallAction::Hangup) {
            control_button_spinner(
                Spinner::new().color(rgb(ON_DARK).into()),
                "Ending",
                rgb(DANGER).opacity(0.9),
                rgb(0xdc2626),
            )
        } else {
            control_button(
                PHONE_OFF_SVG,
                "End",
                rgb(DANGER),
                rgb(0xdc2626),
                rgb(ON_DARK),
                cx.listener(|this, _, _, cx| this.end_call(cx)),
            )
        };
        controls = controls.child(end_button);

        let mut column = v_flex()
            .flex_1()
            .min_h_0()
            .items_center()
            .justify_center()
            .gap_5()
            .p_6()
            .child(avatar_circle(
                &initials,
                if connected { 0x10b981 } else { AMBER },
            ))
            .child(
                v_flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(ON_DARK).opacity(0.95))
                            .child(name),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(ON_DARK).opacity(0.5))
                            .child(peer),
                    ),
            );

        if is_group && !participant_labels.is_empty() {
            column = column.child(
                h_flex()
                    .flex_wrap()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(ON_DARK).opacity(0.45))
                            .child(format!("{} participants", participant_labels.len())),
                    )
                    .children(participant_labels.into_iter().map(|label| {
                        div()
                            .px_2()
                            .py_1()
                            .rounded_full()
                            .bg(rgb(ON_DARK).opacity(0.08))
                            .border_1()
                            .border_color(rgb(ON_DARK).opacity(0.10))
                            .text_xs()
                            .text_color(rgb(ON_DARK).opacity(0.8))
                            .child(label)
                    })),
            );
        }

        let status_element = if connected {
            div()
                .text_lg()
                .text_color(rgb(ON_DARK).opacity(0.75))
                .child(status_text)
                .into_any_element()
        } else {
            h_flex()
                .items_center()
                .gap_2()
                .child(Spinner::new().color(rgb(ON_DARK).opacity(0.7).into()))
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(ON_DARK).opacity(0.75))
                        .child(status_text),
                )
                .into_any_element()
        };

        v_flex()
            .id("call-overlay")
            .absolute()
            .inset_0()
            .child(
                div()
                    .id("call-overlay-backdrop")
                    .absolute()
                    .inset_0()
                    .bg(rgb(0x0a0a0a).opacity(0.92))
                    .on_mouse_down(MouseButton::Left, |_, _, _| {}),
            )
            .child(
                v_flex()
                    .absolute()
                    .inset_0()
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .p_5()
                            .child(call_kind_pill(
                                if is_video { "Video call" } else { "Voice call" },
                                connected,
                            )),
                    )
                    .child(column)
                    .child(
                        v_flex()
                            .items_center()
                            .gap_4()
                            .px_6()
                            .pb_8()
                            .child(status_element)
                            .child(controls)
                            .children(self.error.as_ref().map(|err| {
                                div()
                                    .text_xs()
                                    .text_color(rgb(DANGER))
                                    .child(err.clone())
                            })),
                    ),
            )
            .into_any_element()
    }

    fn render_incoming(
        &self,
        mgr: &CallManager,
        theme: &ActiveTokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let call = match mgr.incoming_call.as_ref() {
            Some(call) => call.clone(),
            None => return div().into_any_element(),
        };
        let name = peer_display_name(&call.target, cx);
        let initials = peer_initials(&name);
        let is_video = matches!(call.call_type, CallType::Video | CallType::GroupVideo);
        let upgrade = mgr.video_upgrade_requested;
        let busy = self.action.is_some();

        v_flex()
            .id("incoming-call-overlay")
            .absolute()
            .inset_0()
            .items_center()
            .justify_center()
            .child(
                div()
                    .id("incoming-call-backdrop")
                    .absolute()
                    .inset_0()
                    .bg(rgb(0x0a0a0a).opacity(0.7))
                    .on_mouse_down(MouseButton::Left, |_, _, _| {}),
            )
            .child(
                v_flex()
                    .w(px(360.0))
                    .p_6()
                    .gap_4()
                    .items_center()
                    .rounded_2xl()
                    .bg(theme.card)
                    .border_1()
                    .border_color(theme.border)
                    .shadow_xl()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .px_3()
                            .py_1()
                            .rounded_full()
                            .bg(theme.muted.opacity(0.6))
                            .child(call_kind_dot(true))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.muted_foreground)
                                    .child(format!(
                                        "INCOMING {} CALL",
                                        if is_video { "VIDEO" } else { "VOICE" }
                                    )),
                            ),
                    )
                    .child(avatar_circle_small(&initials, theme))
                    .child(
                        v_flex()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .text_2xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.foreground)
                                    .child(name),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(call.target.clone()),
                            ),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child("Ringing…"),
                    )
                    .children(upgrade.then(|| {
                        v_flex()
                            .w_full()
                            .gap_2()
                            .p_3()
                            .rounded_xl()
                            .bg(rgb(SKY).opacity(0.12))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(0x0284c7))
                                    .child("Caller wants to turn on video"),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .flex_1()
                                            .py_2()
                                            .rounded_full()
                                            .bg(rgb(0x0284c7))
                                            .text_xs()
                                            .text_color(rgb(ON_DARK))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .cursor_pointer()
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|this, _, _, cx| {
                                                    this.respond_video_upgrade(true, cx)
                                                }),
                                            )
                                            .child("Accept video"),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .py_2()
                                            .rounded_full()
                                            .border_1()
                                            .border_color(theme.border)
                                            .text_xs()
                                            .text_color(theme.foreground)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .cursor_pointer()
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|this, _, _, cx| {
                                                    this.respond_video_upgrade(false, cx)
                                                }),
                                            )
                                            .child("Keep voice"),
                                    ),
                            )
                    }))
                    .children(self.error.as_ref().map(|err| {
                        div()
                            .text_xs()
                            .text_color(theme.destructive)
                            .child(err.clone())
                    }))
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .justify_center()
                            .gap_6()
                            .pt_2()
                            .child(round_action(
                                PHONE_OFF_SVG,
                                "Decline",
                                DANGER,
                                0xdc2626,
                                busy.then(|| Spinner::new().color(rgb(ON_DARK).into())),
                                cx.listener(|this, _, _, cx| this.reject_call(cx)),
                            ))
                            .child(round_action(
                                PHONE_SVG,
                                "Accept",
                                0x10b981,
                                0x059669,
                                busy.then(|| Spinner::new().color(rgb(ON_DARK).into())),
                                cx.listener(|this, _, _, cx| this.answer_call(cx)),
                            )),
                    ),
            )
            .into_any_element()
    }
}

impl Render for CallOverlayView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // `app_theme` borrows the context, so take a copy before the overlay
        // needs `cx` mutably for its listeners.
        let theme = *cx.app_theme();
        let mgr = if cx.has_global::<CallManager>() {
            CallManager::global(cx).clone()
        } else {
            CallManager::new()
        };

        if mgr.is_incoming_visible() {
            return self.render_incoming(&mgr, &theme, cx).into_any_element();
        }
        if !mgr.is_overlay_visible() {
            // Nothing to cover: a zero-size element keeps the app interactive.
            return div().into_any_element();
        }
        self.render_active(&mgr, &theme, cx).into_any_element()
    }
}

// --- Small shared pieces ---

/// Turn a JID/target into a human label, preferring the chat's stored name.
fn peer_display_name(peer: &str, app: &App) -> String {
    if app.has_global::<ChatStore>() {
        if let Some(chat) = ChatStore::global(app).chats.iter().find(|c| c.id == peer) {
            let name = chat.name.trim();
            if !name.is_empty() {
                return name.to_string();
            }
        }
    }

    let local = peer.split('@').next().unwrap_or(peer);
    let digits_only = !local.is_empty() && local.chars().all(|c| c.is_ascii_digit());
    if digits_only && local.len() > 7 {
        let (a, rest) = local.split_at(3);
        let (b, c) = rest.split_at(3);
        format!("{a} {b} {c}")
    } else if local.is_empty() {
        peer.to_string()
    } else {
        local.to_string()
    }
}

fn peer_initials(name: &str) -> String {
    let cleaned: String = name
        .split('@')
        .next()
        .unwrap_or(name)
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    let parts: Vec<&str> = cleaned.split_whitespace().collect();
    match parts.as_slice() {
        [] => "WA".to_string(),
        [only] => only.chars().take(2).collect::<String>().to_uppercase(),
        [first, second, ..] => {
            let a = first.chars().next().unwrap_or('W');
            let b = second.chars().next().unwrap_or('A');
            format!("{a}{b}").to_uppercase()
        }
    }
}

fn call_kind_dot(connected: bool) -> AnyElement {
    div()
        .w(px(8.0))
        .h(px(8.0))
        .rounded_full()
        .bg(if connected {
            rgb(0x10b981)
        } else {
            rgb(AMBER)
        })
        .into_any_element()
}

fn call_kind_pill(label: &str, connected: bool) -> AnyElement {
    h_flex()
        .items_center()
        .gap_2()
        .px_3()
        .py_1()
        .rounded_full()
        .bg(rgb(ON_DARK).opacity(0.05))
        .border_1()
        .border_color(rgb(ON_DARK).opacity(0.10))
        .child(call_kind_dot(connected))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(ON_DARK).opacity(0.7))
                .child(label.to_uppercase()),
        )
        .into_any_element()
}

fn avatar_circle(initials: &str, ring: u32) -> AnyElement {
    div()
        .w(px(112.0))
        .h(px(112.0))
        .rounded_full()
        .bg(rgb(0x18181b))
        .border_1()
        .border_color(rgb(ring).opacity(0.5))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(36.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(ON_DARK))
        .child(initials.to_string())
        .into_any_element()
}

fn avatar_circle_small(initials: &str, theme: &ActiveTokens) -> AnyElement {
    div()
        .w(px(92.0))
        .h(px(92.0))
        .rounded_full()
        .bg(rgb(0x18181b))
        .border_1()
        .border_color(theme.border)
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(28.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(ON_DARK))
        .child(initials.to_string())
        .into_any_element()
}

fn control_button<F>(
    icon: &'static [u8],
    label: &'static str,
    bg: Rgba,
    hover_bg: Rgba,
    icon_color: Rgba,
    on_click: F,
) -> AnyElement
where
    F: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
{
    v_flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .w(px(60.0))
                .h(px(60.0))
                .rounded_full()
                .border_1()
                .border_color(rgb(ON_DARK).opacity(0.15))
                .bg(bg)
                .hover(move |s| s.bg(hover_bg))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .child(svg().data(icon).size(px(20.0)).text_color(icon_color))
                .on_mouse_down(MouseButton::Left, on_click),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(ON_DARK).opacity(0.55))
                .child(label),
        )
        .into_any_element()
}

fn control_button_spinner(
    spinner: Spinner,
    label: &'static str,
    bg: Rgba,
    hover_bg: Rgba,
) -> AnyElement {
    v_flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .w(px(60.0))
                .h(px(60.0))
                .rounded_full()
                .border_1()
                .border_color(rgb(ON_DARK).opacity(0.15))
                .bg(bg)
                .hover(move |s| s.bg(hover_bg))
                .flex()
                .items_center()
                .justify_center()
                .child(spinner),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(ON_DARK).opacity(0.55))
                .child(label),
        )
        .into_any_element()
}

fn round_action<F>(
    icon: &'static [u8],
    label: &'static str,
    bg_hex: u32,
    hover_hex: u32,
    spinner: Option<Spinner>,
    on_click: F,
) -> AnyElement
where
    F: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
{
    let mut button = div()
        .w(px(64.0))
        .h(px(64.0))
        .rounded_full()
        .bg(rgb(bg_hex))
        .hover(move |s| s.bg(rgb(hover_hex)))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, on_click);

    button = match spinner {
        Some(spinner) => button.child(spinner),
        None => button.child(svg().data(icon).size(px(22.0)).text_color(rgb(ON_DARK))),
    };

    v_flex()
        .items_center()
        .gap_2()
        .child(button)
        .child(
            div()
                .text_xs()
                .text_color(rgb(ON_DARK).opacity(0.55))
                .child(label),
        )
        .into_any_element()
}

//! Realtime connection status indicator and reconnection banner.

use gpui::{
    div, px, AnimationExt as _, AnyElement, App, Global, IntoElement, ParentElement, RenderOnce,
    Styled, Window,
};
use gpui_component::{h_flex, Icon, IconName};

use crate::components::motion;

use crate::theme::manager::AppThemeExt;

/// Current connection status of the backend sidecar and WebSocket client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnectionStatus {
    #[default]
    Connected,
    Connecting,
    Reconnecting(u32),
    Disconnected,
}

/// Global connection status tracker.
#[derive(Debug, Default)]
pub struct ConnectionState {
    pub status: ConnectionStatus,
}

impl Global for ConnectionState {}

impl ConnectionState {
    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }

    pub fn set_status(&mut self, status: ConnectionStatus, cx: &mut App) {
        if self.status != status {
            self.status = status;
            cx.refresh_windows();
        }
    }
}

#[derive(Default, IntoElement)]
pub struct ConnectionBanner;

impl ConnectionBanner {
    pub fn new() -> Self {
        Self
    }
}

impl RenderOnce for ConnectionBanner {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let status = if cx.has_global::<ConnectionState>() {
            ConnectionState::global(cx).status
        } else {
            ConnectionStatus::Connected
        };

        let theme = cx.app_theme();

        let (key, banner): (&'static str, AnyElement) = match status {
            ConnectionStatus::Connected => return div().into_any_element(),
            ConnectionStatus::Connecting => ("connecting", {
                h_flex()
                    .w_full()
                    .h(px(36.))
                    .bg(theme.accent)
                    .border_b_1()
                    .border_color(theme.border)
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.accent_foreground)
                    .child(motion::spinning(Icon::new(IconName::LoaderCircle)))
                    .child("Connecting to WhatsApp backend...")
                    .into_any_element()
            }),
            ConnectionStatus::Reconnecting(attempt) => ("reconnecting", {
                h_flex()
                    .w_full()
                    .h(px(36.))
                    .bg(theme.accent)
                    .border_b_1()
                    .border_color(theme.border)
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.accent_foreground)
                    .child(motion::spinning(Icon::new(IconName::RotateCw)))
                    .child(format!("Connection lost. Attempting to reconnect (attempt {})...", attempt))
                    .into_any_element()
            }),
            ConnectionStatus::Disconnected => ("disconnected", {
                h_flex()
                    .w_full()
                    .h(px(36.))
                    .bg(theme.destructive)
                    .border_b_1()
                    .border_color(theme.border)
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.destructive_foreground)
                    .child(Icon::new(IconName::TriangleAlert))
                    .child("Disconnected from WhatsApp. Please check your backend connection.")
                    .into_any_element()
            }),
        };

        // Slides open from nothing rather than popping in and shoving the
        // page down; each status restarts it so a change reads as an update.
        div()
            .w_full()
            .overflow_hidden()
            .child(banner)
            .with_animation(key, motion::enter(motion::BASE_MS), |el, d| {
                el.h(px(36. * d)).opacity(d)
            })
            .into_any_element()
    }
}

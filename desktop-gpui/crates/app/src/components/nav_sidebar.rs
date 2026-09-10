//! Navigation sidebar 1:1 with web client NavigationSidebar.tsx.

use gpui::{
    div, prelude::FluentBuilder as _, px, App, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, StatefulInteractiveElement, Styled, Window,
};
use gpui_component::{v_flex, Icon, IconName};

use crate::router::{AppRoute, Router};
use crate::theme::manager::AppThemeExt;

pub const SIDEBAR_WIDTH: f32 = 68.0;

#[derive(Default, IntoElement)]
pub struct NavigationSidebar;

impl NavigationSidebar {
    pub fn new() -> Self {
        Self
    }
}

impl RenderOnce for NavigationSidebar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.app_theme();
        let current_prefix = if cx.has_global::<Router>() {
            Router::global(cx).current_route.path_prefix()
        } else {
            "/chat"
        };

        v_flex()
            .id("nav-sidebar")
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .bg(theme.background)
            .border_r_1()
            .border_color(theme.border.opacity(0.4))
            .items_center()
            .py_6()
            .flex_shrink_0()
            .justify_between()
            .child(
                // Top nav items
                v_flex()
                    .gap_3()
                    .items_center()
                    .w_full()
                    .child(Self::render_item("Chats", "/chat", IconName::Inbox, current_prefix == "/chat", AppRoute::Chat, cx))
                    .child(Self::render_item("Status", "/status", IconName::CircleCheck, current_prefix == "/status", AppRoute::Status, cx))
                    .child(Self::render_item("Channels", "/channels", IconName::Bell, current_prefix == "/channels", AppRoute::Channels, cx))
                    .child(Self::render_item("Calls", "/calls", IconName::Network, current_prefix == "/calls", AppRoute::Calls, cx))
                    .child(Self::render_item("Triggers", "/triggers", IconName::Bot, current_prefix == "/triggers", AppRoute::Triggers, cx))
                    .child(Self::render_item("Cron Jobs", "/cron", IconName::Calendar, current_prefix == "/cron", AppRoute::Cron, cx))
                    .child(Self::render_item("Webhooks", "/webhooks", IconName::Globe, current_prefix == "/webhooks", AppRoute::Webhooks, cx))
                    .child(Self::render_item("Documentation", "/documentation", IconName::FileText, current_prefix == "/documentation", AppRoute::Documentation, cx)),
            )
            .child(
                // Bottom settings item
                v_flex()
                    .w_full()
                    .items_center()
                    .child(Self::render_item("Settings", "/settings", IconName::Settings, current_prefix == "/settings", AppRoute::Settings, cx)),
            )
    }
}

impl NavigationSidebar {
    fn render_item(
        label: &'static str,
        _prefix: &'static str,
        icon: IconName,
        is_active: bool,
        route: AppRoute,
        cx: &App,
    ) -> impl IntoElement {
        let theme = cx.app_theme();

        div()
            .id(label)
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .w_full()
            .cursor_pointer()
            .on_click(move |_, _, cx| {
                if cx.has_global::<Router>() {
                    Router::global_mut(cx).navigate(route.clone());
                    cx.refresh_windows();
                }
            })
            .child(
                // Active indicator bar on left edge
                div()
                    .when(is_active, |this| {
                        this.absolute()
                            .left_0()
                            .w(px(3.))
                            .h(px(24.))
                            .bg(theme.primary)
                            .rounded_r_full()
                    }),
            )
            .child(
                // Button icon container
                div()
                    .p_3()
                    .rounded_2xl()
                    .when(is_active, |s| {
                        s.bg(theme.primary)
                            .text_color(theme.primary_foreground)
                    })
                    .when(!is_active, |s| {
                        s.text_color(theme.muted_foreground)
                            .hover(|h| h.bg(theme.muted).text_color(theme.foreground))
                    })
                    .child(Icon::new(icon)),
            )
    }
}

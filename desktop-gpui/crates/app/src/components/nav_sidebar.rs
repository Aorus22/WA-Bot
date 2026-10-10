//! Navigation sidebar 1:1 with web client NavigationSidebar.tsx.

use gpui::{
    div, prelude::FluentBuilder as _, px, AnimationExt as _, AnyElement, App, ElementId, InteractiveElement,
    IntoElement, ParentElement, RenderOnce, StatefulInteractiveElement, Styled, Window,
};
use gpui_component::tooltip::Tooltip;
use gpui_component::{v_flex, Icon, IconName};

use crate::components::motion;
use crate::router::{AppRoute, Router};
use crate::theme::manager::{AppThemeExt, ThemeManager};
use crate::window_material;

pub const SIDEBAR_WIDTH: f32 = 68.0;

#[derive(Default, IntoElement)]
pub struct NavigationSidebar;

impl NavigationSidebar {
    pub fn new() -> Self {
        Self
    }
}

impl RenderOnce for NavigationSidebar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.app_theme();
        let window_transparency = ThemeManager::global(cx).window_transparency;
        let framed = !crate::popover::is_popover(cx)
            && window_material::is_framed_window(window);
        let current_prefix = if cx.has_global::<Router>() {
            Router::global(cx).current_route.path_prefix()
        } else {
            "/chat"
        };

        v_flex()
            .id("nav-sidebar")
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .bg(window_material::glass_fill(theme.background, window_transparency))
            .border_r_1()
            .border_color(theme.border.opacity(0.4))
            .items_center()
            .py_6()
            .flex_shrink_0()
            .when(framed, |s| {
                s.rounded_bl(px(window_material::WINDOW_CORNER_RADIUS))
            })
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

        // The active pill and its edge indicator ease in when an item becomes
        // active (id differs from the inactive one, so the animation restarts).
        let indicator: AnyElement = if is_active {
            div()
                .absolute()
                .left_0()
                .w(px(3.))
                .bg(theme.primary)
                .rounded_r_full()
                .with_animation(
                    (ElementId::from("nav-indicator"), label),
                    motion::enter(motion::BASE_MS),
                    |el, d| el.h(px(24. * d)).opacity(d),
                )
                .into_any_element()
        } else {
            div().into_any_element()
        };

        let button = div()
            .id(label)
            .p_3()
            .rounded_2xl()
            .cursor_pointer()
            .tooltip(move |window, cx| Tooltip::new(label).build(window, cx))
            .on_click(move |_, _, cx| {
                if cx.has_global::<Router>() {
                    Router::global_mut(cx).navigate(route.clone());
                    cx.refresh_windows();
                }
            })
            .when(!is_active, |s| {
                s.text_color(theme.muted_foreground)
                    .hover(|h| h.bg(theme.muted.opacity(0.8)).text_color(theme.foreground))
            })
            .child(Icon::new(icon));

        let button: AnyElement = if is_active {
            let (bg, from_fg, to_fg) = (theme.primary, theme.muted_foreground, theme.primary_foreground);
            button
                .hover(|h| h.bg(theme.primary.opacity(0.85)))
                .with_animation(
                    (ElementId::from("nav-active"), label),
                    motion::enter(motion::BASE_MS),
                    move |el, d| {
                        el.bg(motion::lerp_color(bg.opacity(0.), bg, d))
                            .text_color(motion::lerp_color(from_fg, to_fg, d))
                    },
                )
                .into_any_element()
        } else {
            button.into_any_element()
        };

        div()
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .w_full()
            .child(indicator)
            .child(button)
    }
}

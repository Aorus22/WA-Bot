//! Main application shell view.

use gpui::{
    div, AnyElement, App, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render, Styled, Window,
};
use gpui_component::{h_flex, v_flex};

use crate::components::connection_banner::{ConnectionBanner, ConnectionState};
use crate::components::nav_sidebar::NavigationSidebar;
use crate::components::titlebar::AppTitleBar;
use crate::router::{AppRoute, Router};
use crate::theme::manager::AppThemeExt;
use crate::views::chat::ChatView;
use crate::views::settings::SettingsView;

pub struct AppShellView {
    _router_sub: gpui::Subscription,
    _conn_sub: gpui::Subscription,
    settings_view: Entity<SettingsView>,
    chat_view: Entity<ChatView>,
}

impl AppShellView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let router_sub = cx.observe_global::<Router>(|_this, cx| {
            cx.notify();
        });
        let conn_sub = cx.observe_global::<ConnectionState>(|_this, cx| {
            cx.notify();
        });
        let settings_view = cx.new(|cx| SettingsView::new(cx));
        let chat_view = cx.new(|cx| ChatView::new(cx));
        Self {
            _router_sub: router_sub,
            _conn_sub: conn_sub,
            settings_view,
            chat_view,
        }
    }
}

impl Render for AppShellView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.app_theme();
        let current_route = if cx.has_global::<Router>() {
            Router::global(cx).current_route.clone()
        } else {
            AppRoute::Chat
        };

        v_flex()
            .id("app-shell")
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .overflow_hidden()
            .child(AppTitleBar::new())
            .child(
                h_flex()
                    .size_full()
                    .overflow_hidden()
                    .child(NavigationSidebar::new())
                    .child(
                        v_flex()
                            .size_full()
                            .overflow_hidden()
                            .child(ConnectionBanner::new())
                            .child(
                                div()
                                    .id("router-outlet")
                                    .size_full()
                                    .overflow_hidden()
                                    .child(self.render_route_outlet(&current_route, cx)),
                            ),
                    ),
            )
    }
}

impl AppShellView {
    fn render_route_outlet(&self, route: &AppRoute, cx: &App) -> AnyElement {
        let theme = cx.app_theme();

        match route {
            AppRoute::Chat | AppRoute::ChatDetail(_) => {
                self.chat_view.clone().into_any_element()
            }
            AppRoute::Status => {
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(div().text_lg().font_weight(gpui::FontWeight::SEMIBOLD).child("Status Updates"))
                    .child(div().text_sm().text_color(theme.muted_foreground).child("View status updates from your contacts"))
                    .into_any_element()
            }
            AppRoute::Channels => {
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(div().text_lg().font_weight(gpui::FontWeight::SEMIBOLD).child("Channels"))
                    .child(div().text_sm().text_color(theme.muted_foreground).child("Stay updated on topics that interest you"))
                    .into_any_element()
            }
            AppRoute::Calls => {
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(div().text_lg().font_weight(gpui::FontWeight::SEMIBOLD).child("Calls History"))
                    .child(div().text_sm().text_color(theme.muted_foreground).child("No recent call history"))
                    .into_any_element()
            }
            AppRoute::Triggers | AppRoute::TriggerDetail(_) | AppRoute::TriggerNew => {
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(div().text_lg().font_weight(gpui::FontWeight::SEMIBOLD).child("Bot Triggers"))
                    .child(div().text_sm().text_color(theme.muted_foreground).child("Configure automated bot keyword responses"))
                    .into_any_element()
            }
            AppRoute::Cron | AppRoute::CronDetail(_) | AppRoute::CronNew => {
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(div().text_lg().font_weight(gpui::FontWeight::SEMIBOLD).child("Cron Jobs"))
                    .child(div().text_sm().text_color(theme.muted_foreground).child("Schedule periodic automated messages and actions"))
                    .into_any_element()
            }
            AppRoute::Webhooks
            | AppRoute::WebhookDetail(_)
            | AppRoute::WebhookNew
            | AppRoute::WebhookLogs => {
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(div().text_lg().font_weight(gpui::FontWeight::SEMIBOLD).child("Webhooks"))
                    .child(div().text_sm().text_color(theme.muted_foreground).child("Forward WhatsApp events to external services"))
                    .into_any_element()
            }
            AppRoute::Documentation => {
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(div().text_lg().font_weight(gpui::FontWeight::SEMIBOLD).child("Documentation"))
                    .child(div().text_sm().text_color(theme.muted_foreground).child("API documentation and guides from backend"))
                    .into_any_element()
            }
            AppRoute::Settings => {
                self.settings_view.clone().into_any_element()
            }
        }
    }
}

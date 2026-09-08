//! Main application shell view.

use gpui::{
    div, App, Context, InteractiveElement, IntoElement, ParentElement, Render, Styled, Window,
};
use gpui_component::{h_flex, v_flex};

use crate::components::connection_banner::ConnectionBanner;
use crate::components::nav_sidebar::NavigationSidebar;
use crate::components::titlebar::AppTitleBar;
use crate::router::{AppRoute, Router};
use crate::theme::manager::AppThemeExt;

pub struct AppShellView;

impl AppShellView {
    pub fn new() -> Self {
        Self
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
                                    .child(Self::render_route_outlet(&current_route, cx)),
                            ),
                    ),
            )
    }
}

impl AppShellView {
    fn render_route_outlet(route: &AppRoute, cx: &App) -> impl IntoElement {
        let theme = cx.app_theme();

        match route {
            AppRoute::Chat | AppRoute::ChatDetail(_) => {
                h_flex()
                    .size_full()
                    .overflow_hidden()
                    .child(
                        v_flex()
                            .w(gpui::px(340.0))
                            .h_full()
                            .border_r_1()
                            .border_color(theme.border)
                            .bg(theme.card)
                            .child(
                                v_flex()
                                    .p_3()
                                    .gap_2()
                                    .border_b_1()
                                    .border_color(theme.border)
                                    .child(
                                        h_flex()
                                            .justify_between()
                                            .items_center()
                                            .child(div().text_base().font_weight(gpui::FontWeight::BOLD).child("Chats"))
                                            .child(
                                                h_flex()
                                                    .gap_2()
                                                    .child(div().text_xs().text_color(theme.primary).child("+ New Group"))
                                                    .child(div().text_xs().text_color(theme.muted_foreground).child("Join via Link"))
                                            )
                                    )
                                    .child(
                                        div()
                                            .px_3()
                                            .py_1()
                                            .bg(theme.background)
                                            .rounded_md()
                                            .border_1()
                                            .border_color(theme.border)
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child("Search or start new chat...")
                                    )
                            )
                            .child(
                                v_flex()
                                    .flex_1()
                                    .overflow_hidden()
                                    .p_2()
                                    .child(
                                        div()
                                            .p_3()
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child("Conversations ready. Click to open.")
                                    )
                            )
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .h_full()
                            .bg(theme.background)
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .child(div().text_base().font_weight(gpui::FontWeight::SEMIBOLD).child("WA Bot Desktop"))
                            .child(div().text_xs().text_color(theme.muted_foreground).child("Send and receive messages with 1:1 web parity."))
                    )
            }
            AppRoute::Status => {
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(div().text_lg().font_weight(gpui::FontWeight::SEMIBOLD).child("Status Updates"))
                    .child(div().text_sm().text_color(theme.muted_foreground).child("View status updates from your contacts"))
            }
            AppRoute::Channels => {
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(div().text_lg().font_weight(gpui::FontWeight::SEMIBOLD).child("Channels"))
                    .child(div().text_sm().text_color(theme.muted_foreground).child("Stay updated on topics that interest you"))
            }
            AppRoute::Calls => {
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(div().text_lg().font_weight(gpui::FontWeight::SEMIBOLD).child("Calls History"))
                    .child(div().text_sm().text_color(theme.muted_foreground).child("No recent call history"))
            }
            AppRoute::Triggers | AppRoute::TriggerDetail(_) | AppRoute::TriggerNew => {
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(div().text_lg().font_weight(gpui::FontWeight::SEMIBOLD).child("Bot Triggers"))
                    .child(div().text_sm().text_color(theme.muted_foreground).child("Configure automated bot keyword responses"))
            }
            AppRoute::Cron | AppRoute::CronDetail(_) | AppRoute::CronNew => {
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(div().text_lg().font_weight(gpui::FontWeight::SEMIBOLD).child("Cron Jobs"))
                    .child(div().text_sm().text_color(theme.muted_foreground).child("Schedule periodic automated messages and actions"))
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
            }
            AppRoute::Documentation => {
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(div().text_lg().font_weight(gpui::FontWeight::SEMIBOLD).child("Documentation"))
                    .child(div().text_sm().text_color(theme.muted_foreground).child("API documentation and guides from backend"))
            }
            AppRoute::Settings => {
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(div().text_lg().font_weight(gpui::FontWeight::SEMIBOLD).child("Settings"))
                    .child(div().text_sm().text_color(theme.muted_foreground).child("App preferences, active themes, and connection configurations"))
            }
        }
    }
}

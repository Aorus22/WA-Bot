//! Main application shell view.

use gpui::{
    div, prelude::FluentBuilder as _, px, AnyElement, App, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render, Styled, Window,
};
use gpui_component::{h_flex, v_flex};

use crate::components::call_overlay::CallOverlayView;
use crate::components::connection_banner::{ConnectionBanner, ConnectionState};
use crate::components::motion;
use crate::components::nav_sidebar::NavigationSidebar;
use crate::components::titlebar::AppTitleBar;
use crate::router::{AppRoute, Router};
use crate::theme::manager::{AppThemeExt, ThemeManager};
use crate::window_material;
use crate::state::auth::{AuthState, AuthStatus};
use crate::views::auth::login::LoginView;
use crate::views::calls::CallsView;
use crate::views::channels::ChannelsView;
use crate::views::chat::ChatView;
use crate::views::settings::SettingsView;
use crate::views::status::StatusView;

pub struct AppShellView {
    _router_sub: gpui::Subscription,
    _conn_sub: gpui::Subscription,
    _auth_sub: gpui::Subscription,
    login_view: Entity<LoginView>,
    settings_view: Entity<SettingsView>,
    chat_view: Entity<ChatView>,
    status_view: Entity<StatusView>,
    channels_view: Entity<ChannelsView>,
    calls_view: Entity<CallsView>,
    call_overlay: Entity<CallOverlayView>,
}

impl AppShellView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let router_sub = cx.observe_global::<Router>(|_this, cx| {
            cx.notify();
        });
        let conn_sub = cx.observe_global::<ConnectionState>(|_this, cx| {
            cx.notify();
        });
        let auth_sub = cx.observe_global::<AuthState>(|_this, cx| {
            cx.notify();
        });
        let login_view = cx.new(|cx| LoginView::new(cx));
        let settings_view = cx.new(|cx| SettingsView::new(cx));
        let chat_view = cx.new(|cx| ChatView::new(cx));
        let status_view = cx.new(|cx| StatusView::new(cx));
        let channels_view = cx.new(|cx| ChannelsView::new(cx));
        let calls_view = cx.new(|cx| CallsView::new(cx));
        // Mounted last so an active call paints above every route, like the
        // Web client's `fixed inset-0 z-[90]` overlay.
        let call_overlay = cx.new(|cx| CallOverlayView::new(cx));
        Self {
            _router_sub: router_sub,
            _conn_sub: conn_sub,
            _auth_sub: auth_sub,
            login_view,
            settings_view,
            chat_view,
            status_view,
            channels_view,
            calls_view,
            call_overlay,
        }
    }
}

impl Render for AppShellView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.app_theme();
        let current_route = if cx.has_global::<Router>() {
            Router::global(cx).current_route.clone()
        } else {
            AppRoute::Chat
        };
        // Keyed by section (not the full route) so moving between a list and
        // its detail page doesn't replay the transition.
        let route_key = current_route.path_prefix();

        // The chat page puts the window controls into its own headers; every
        // other page keeps the title bar. Without a linked account the chat page
        // shows the login, which needs the title bar too.
        let unlinked = cx.has_global::<AuthState>() && AuthState::global(cx).status == AuthStatus::Unauthenticated;
        let chat_chrome = matches!(current_route, AppRoute::Chat | AppRoute::ChatDetail(_))
            && !unlinked
            && !crate::popover::is_popover(cx);

        v_flex()
            .id("app-shell")
            .relative()
            .size_full()
            .text_color(theme.foreground)
            .overflow_hidden()
            .when(!chat_chrome, |s| s.child(AppTitleBar::new()))
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
                                    .child(motion::rise_in(
                                        div()
                                            .relative()
                                            .size_full()
                                    .child(self.render_route_outlet(&current_route, window, cx)),
                                        route_key,
                                        motion::BASE_MS,
                                        10.0,
                                    )),
                            ),
                    ),
            )
            .child(self.call_overlay.clone())
            .when(chat_chrome, |s| s.child(crate::components::titlebar::controls_overlay(window, cx)))
    }
}

impl AppShellView {
    fn render_route_outlet(&self, route: &AppRoute, window: &Window, cx: &App) -> AnyElement {
        let theme = cx.app_theme();
        let framed = !crate::popover::is_popover(cx)
            && window_material::is_framed_window(window);
        let page_background = window_material::glass_fill(
            theme.background,
            ThemeManager::global(cx).window_transparency,
        );

        // Without a linked account the pages that show WhatsApp data give way
        // to the login; the navigation and the other pages stay usable.
        let unlinked = cx.has_global::<AuthState>()
            && AuthState::global(cx).status == AuthStatus::Unauthenticated;
        if unlinked && route.needs_whatsapp() {
            return self.login_view.clone().into_any_element();
        }

        match route {
            AppRoute::Chat | AppRoute::ChatDetail(_) => {
                self.chat_view.clone().into_any_element()
            }
            AppRoute::Status => {
                self.status_view.clone().into_any_element()
            }
            AppRoute::Channels => {
                self.channels_view.clone().into_any_element()
            }
            AppRoute::Calls => {
                self.calls_view.clone().into_any_element()
            }
            AppRoute::Triggers | AppRoute::TriggerDetail(_) | AppRoute::TriggerNew => {
                v_flex()
                    .size_full()
                    .bg(page_background)
                    .when(framed, |s| s.rounded_br(px(window_material::WINDOW_CORNER_RADIUS)))
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
                    .bg(page_background)
                    .when(framed, |s| s.rounded_br(px(window_material::WINDOW_CORNER_RADIUS)))
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
                    .bg(page_background)
                    .when(framed, |s| s.rounded_br(px(window_material::WINDOW_CORNER_RADIUS)))
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
                    .bg(page_background)
                    .when(framed, |s| s.rounded_br(px(window_material::WINDOW_CORNER_RADIUS)))
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

//! Main application shell view.

mod navigation_sidebar;

use gpui::{
    div, prelude::FluentBuilder as _, px, AnyElement, App, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render, Styled, Window,
};
use crate::components::ui::{h_flex, v_flex};
use crate::components::call_overlay::CallOverlayView;
use crate::components::connection_banner::{ConnectionBanner, ConnectionState};
use crate::components::motion;
use navigation_sidebar::NavigationSidebar;
use crate::theme::shape::ShapeExt;
use crate::components::titlebar::AppTitleBar;
use crate::router::{AppRoute, Router};
use crate::theme::manager::AppThemeExt;
use crate::theme::surface::SurfaceExt;
use crate::window_material;
use crate::state::auth::{AuthState, AuthStatus};
use crate::views::auth::login::LoginView;
use crate::views::calls::CallsView;
use crate::views::channels::ChannelsView;
use crate::views::chat::ChatView;
use crate::views::settings::SettingsView;
use crate::views::status::StatusView;

/// Fluent's content sheet corner: the content layer is rounded at the top left
/// where it meets the base layer. Pages paint their own square backgrounds, so the
/// rounded edge is made by painting the base color back over the corner outside the
/// arc, then stroking the arc.
fn content_corner(base: gpui::Hsla, stroke: gpui::Hsla) -> impl IntoElement {
    const RADIUS: f32 = 8.0;
    gpui::canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let origin = bounds.origin;
            let at = |x: f32, y: f32| gpui::point(origin.x + px(x), origin.y + px(y));
            let r = RADIUS - 0.5;
            let radii = gpui::point(px(r), px(r));
            let mut fill = gpui::PathBuilder::fill();
            fill.move_to(at(0.0, 0.0));
            fill.line_to(at(RADIUS, 0.5));
            fill.arc_to(radii, px(0.0), false, false, at(0.5, RADIUS));
            fill.close();
            if let Ok(path) = fill.build() {
                window.paint_path(path, base);
            }
            let mut line = gpui::PathBuilder::stroke(px(1.0));
            line.move_to(at(RADIUS, 0.5));
            line.arc_to(radii, px(0.0), false, false, at(0.5, RADIUS));
            if let Ok(path) = line.build() {
                window.paint_path(path, stroke);
            }
        },
    )
    .absolute()
    // Absolute children sit inside the 1px border; the arc starts at its outer edge.
    .top(px(-1.0))
    .left(px(-1.0))
    .w(px(RADIUS))
    .h(px(RADIUS))
}

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
                            .relative()
                            // Fluent: the content layer is a sheet over the base layer,
                            // outlined on its left and top edges.
                            .when(cx.shape().winui, |s| {
                                s.border_l_1().border_t_1().border_color(cx.surface().card_stroke)
                            })
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
                            )
                            .when(cx.shape().winui, |s| {
                                s.child(content_corner(cx.surface().window_fill, cx.surface().card_stroke))
                            }),
                    ),
            )
            .child(self.call_overlay.clone())
            .when(chat_chrome, |s| s.child(crate::components::titlebar::controls_overlay(window, cx)))
    }
}

impl AppShellView {
    fn render_route_outlet(&self, route: &AppRoute, window: &Window, cx: &App) -> AnyElement {
        let theme = cx.app_theme();
        let surface = cx.surface();
        let framed = !crate::popover::is_popover(cx)
            && window_material::is_framed_window(window);
        let page_background = surface.page_background;

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
                    .when(framed, |s| s.rounded_br(px(window_material::corner_radius(cx))))
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
                    .when(framed, |s| s.rounded_br(px(window_material::corner_radius(cx))))
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
                    .when(framed, |s| s.rounded_br(px(window_material::corner_radius(cx))))
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
                    .when(framed, |s| s.rounded_br(px(window_material::corner_radius(cx))))
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

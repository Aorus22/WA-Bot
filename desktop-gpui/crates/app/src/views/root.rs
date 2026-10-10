//! Root gate view managing the Checking -> Login -> Shell states.

use gpui::{
    div, prelude::FluentBuilder as _, px, AnyElement, AppContext, Context, CursorStyle, Decorations,
    Div, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement, Render, ResizeEdge,
    Styled, Tiling, Window,
};
use gpui_component::{v_flex, Icon, IconName};

use crate::components::motion;
use crate::state::auth::{AuthState, AuthStatus};
use crate::theme::manager::{AppThemeExt, ThemeManager};
use crate::window_material;
use crate::views::auth::login::LoginView;
use crate::views::shell::AppShellView;

pub struct RootGateView {
    login_view: Entity<LoginView>,
    shell_view: Entity<AppShellView>,
    _auth_sub: gpui::Subscription,
}

impl RootGateView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let auth_sub = cx.observe_global::<AuthState>(|_this, cx| {
            cx.notify();
        });
        Self {
            login_view: cx.new(|cx| LoginView::new(cx)),
            shell_view: cx.new(|cx| AppShellView::new(cx)),
            _auth_sub: auth_sub,
        }
    }
}

impl Render for RootGateView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.app_theme();
        window.set_background_appearance(window_material::background_appearance(
            ThemeManager::global(cx).window_blur_enabled,
        ));
        let is_popover = crate::popover::is_popover(cx);
        let framed = !is_popover && window_material::is_framed_window(window);
        if !is_popover {
            let viewport = window.viewport_size();
            let (corner_radius, inset) = if framed {
                (
                    window_material::WINDOW_CORNER_RADIUS,
                    window_material::WINDOW_SHADOW_MARGIN,
                )
            } else {
                (0.0, 0.0)
            };
            window_material::set_blur_regions(
                window,
                &window_material::inset_rounded_blur_regions(
                    f32::from(viewport.width),
                    f32::from(viewport.height),
                    inset,
                    corner_radius,
                ),
            );
        }
        let window_transparency = ThemeManager::global(cx).window_transparency;
        let status = if cx.has_global::<AuthState>() {
            AuthState::global(cx).status
        } else {
            AuthStatus::Checking
        };

        let content = match status {
            AuthStatus::Checking => {
                v_flex()
                    .size_full()
                    .bg(window_material::glass_fill(theme.background, window_transparency))
                    .when(framed, |s| s.rounded(px(window_material::WINDOW_CORNER_RADIUS)))
                    .child(crate::components::titlebar::AppTitleBar::new())
                    .child(
                        v_flex()
                            .flex_1()
                            .items_center()
                            .justify_center()
                            .gap_3()
                            .child(
                                motion::spinning(
                                    Icon::new(IconName::LoaderCircle)
                                        .size(px(40.))
                                        .text_color(theme.primary),
                                ),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child("Connecting to WA Bot..."),
                            ),
                    )
                    .into_any_element()
            }
            AuthStatus::Unauthenticated => {
                v_flex()
                    .size_full()
                    .child(crate::components::titlebar::AppTitleBar::new())
                    .child(
                        motion::rise_in(
                            div()
                                .relative()
                                .flex_1()
                                .overflow_hidden()
                                .child(self.login_view.clone()),
                            "login-in",
                            motion::BASE_MS + 80,
                            14.0,
                        ),
                    )
                    .into_any_element()
            }
            AuthStatus::Authenticated => {
                div()
                    .size_full()
                    .child(self.shell_view.clone())
                    .into_any_element()
            }
        };

        if framed {
            let tiling = match window.window_decorations() {
                Decorations::Client { tiling } => tiling,
                _ => Tiling::default(),
            };
            div().relative().size_full().child(
                div()
                    .relative()
                    .size_full()
                    .p(px(window_material::WINDOW_SHADOW_MARGIN))
                    .child(
                        div()
                            .relative()
                            .size_full()
                            .rounded(px(window_material::WINDOW_CORNER_RADIUS))
                            .border_1()
                            .border_color(theme.border)
                            .overflow_hidden()
                            .shadow(window_material::frame_shadow())
                            .child(content),
                    ),
            )
            .child(resize_hit_zones(&tiling))
        } else {
            div().relative().size_full().child(content)
        }
    }
}

const RESIZE_HIT: f32 = window_material::WINDOW_SHADOW_MARGIN;

fn cursor_for_edge(edge: ResizeEdge) -> CursorStyle {
    match edge {
        ResizeEdge::Top | ResizeEdge::Bottom => CursorStyle::ResizeUpDown,
        ResizeEdge::Left | ResizeEdge::Right => CursorStyle::ResizeLeftRight,
        ResizeEdge::TopLeft | ResizeEdge::BottomRight => CursorStyle::ResizeUpLeftDownRight,
        ResizeEdge::TopRight | ResizeEdge::BottomLeft => CursorStyle::ResizeUpRightDownLeft,
    }
}

fn resize_hit_zones(tiling: &Tiling) -> impl IntoElement {
    let hit = px(RESIZE_HIT);
    let mut zones: Vec<AnyElement> = Vec::new();
    let mut edge_zone = |edge: ResizeEdge, el: Div| {
        zones.push(
            el.cursor(cursor_for_edge(edge))
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    window.start_window_resize(edge);
                    cx.stop_propagation();
                })
                .into_any_element(),
        );
    };
    if !tiling.top {
        edge_zone(ResizeEdge::Top, div().absolute().top_0().left_0().right_0().h(hit));
        if !tiling.left {
            edge_zone(ResizeEdge::TopLeft, div().absolute().top_0().left_0().w(hit).h(hit));
        }
        if !tiling.right {
            edge_zone(ResizeEdge::TopRight, div().absolute().top_0().right_0().w(hit).h(hit));
        }
    }
    if !tiling.bottom {
        edge_zone(ResizeEdge::Bottom, div().absolute().bottom_0().left_0().right_0().h(hit));
        if !tiling.left {
            edge_zone(ResizeEdge::BottomLeft, div().absolute().bottom_0().left_0().w(hit).h(hit));
        }
        if !tiling.right {
            edge_zone(ResizeEdge::BottomRight, div().absolute().bottom_0().right_0().w(hit).h(hit));
        }
    }
    if !tiling.left {
        edge_zone(ResizeEdge::Left, div().absolute().left_0().top_0().bottom_0().w(hit));
    }
    if !tiling.right {
        edge_zone(ResizeEdge::Right, div().absolute().right_0().top_0().bottom_0().w(hit));
    }
    div().absolute().size_full().children(zones)
}

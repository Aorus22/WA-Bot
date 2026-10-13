//! Root gate view managing the Checking -> Login -> Shell states.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    canvas, div, prelude::FluentBuilder as _, px, AppContext, Context, CursorStyle, Decorations,
    DispatchPhase, Entity, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent,
    ParentElement, Pixels, Point, Render, ResizeEdge, Size, Styled, Tiling, Window,
};
use gpui_component::{v_flex, Icon, IconName};

use crate::components::motion;
use crate::state::auth::{AuthState, AuthStatus};
use crate::theme::manager::{AppThemeExt, ThemeManager};
use crate::window_material;
use crate::views::shell::AppShellView;

pub struct RootGateView {
    shell_view: Entity<AppShellView>,
    /// Edge of the window the pointer is over, so the cursor and the resize
    /// handle follow it.
    resize_hover: Rc<Cell<Option<ResizeEdge>>>,
    _auth_sub: gpui::Subscription,
}

impl RootGateView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let auth_sub = cx.observe_global::<AuthState>(|_this, cx| {
            cx.notify();
        });
        Self {
            shell_view: cx.new(|cx| AppShellView::new(cx)),
            resize_hover: Rc::new(Cell::new(None)),
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
            // Tell the compositor where the visible frame is: the margin around
            // it is only our shadow. Without this GNOME treats the whole surface
            // as the window, so its blur extends past the frame and there are no
            // resize handles at the frame's edge. The inset is constant: the
            // backend drops it by itself on edges that are maximized, fullscreen
            // or tiled, and changing it here would lag one configure behind.
            window.set_client_inset(px(window_material::WINDOW_SHADOW_MARGIN));
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
            // The shell shows either way: without a linked account its WhatsApp
            // pages carry the login (see `AppShellView`).
            AuthStatus::Unauthenticated | AuthStatus::Authenticated => {
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
            .child(resize_layer(tiling, self.resize_hover.clone()))
        } else {
            div().relative().size_full().child(content)
        }
    }
}

/// Width of the grab area along each edge, measured from the window's outer
/// edge: the transparent shadow margin plus a sliver of the visible frame.
const RESIZE_EDGE: f32 = window_material::WINDOW_SHADOW_MARGIN + 3.0;
/// Corners are easier to hit with a larger square.
const RESIZE_CORNER: f32 = window_material::WINDOW_SHADOW_MARGIN + 12.0;

fn cursor_for_edge(edge: ResizeEdge) -> CursorStyle {
    match edge {
        ResizeEdge::Top | ResizeEdge::Bottom => CursorStyle::ResizeUpDown,
        ResizeEdge::Left | ResizeEdge::Right => CursorStyle::ResizeLeftRight,
        ResizeEdge::TopLeft | ResizeEdge::BottomRight => CursorStyle::ResizeUpLeftDownRight,
        ResizeEdge::TopRight | ResizeEdge::BottomLeft => CursorStyle::ResizeUpRightDownLeft,
    }
}

/// Which resize handle (if any) the point `(x, y)` falls on in a window of
/// `width` x `height`. Edges the compositor reports as tiled cannot be resized.
fn resize_edge_at(x: f32, y: f32, width: f32, height: f32, tiling: Tiling) -> Option<ResizeEdge> {
    let near = |distance: f32, limit: f32| distance >= 0.0 && distance < limit;
    let left = !tiling.left && near(x, RESIZE_EDGE);
    let right = !tiling.right && near(width - x, RESIZE_EDGE);
    let top = !tiling.top && near(y, RESIZE_EDGE);
    let bottom = !tiling.bottom && near(height - y, RESIZE_EDGE);
    let corner_x = |from_left: bool| {
        if from_left { near(x, RESIZE_CORNER) } else { near(width - x, RESIZE_CORNER) }
    };
    let corner_y = |from_top: bool| {
        if from_top { near(y, RESIZE_CORNER) } else { near(height - y, RESIZE_CORNER) }
    };
    if !tiling.top && !tiling.left && near(x, RESIZE_CORNER) && near(y, RESIZE_CORNER) && (left || top) {
        return Some(ResizeEdge::TopLeft);
    }
    if !tiling.top && !tiling.right && corner_x(false) && corner_y(true) && (right || top) {
        return Some(ResizeEdge::TopRight);
    }
    if !tiling.bottom && !tiling.left && corner_x(true) && corner_y(false) && (left || bottom) {
        return Some(ResizeEdge::BottomLeft);
    }
    if !tiling.bottom && !tiling.right && corner_x(false) && corner_y(false) && (right || bottom) {
        return Some(ResizeEdge::BottomRight);
    }
    if top {
        Some(ResizeEdge::Top)
    } else if bottom {
        Some(ResizeEdge::Bottom)
    } else if left {
        Some(ResizeEdge::Left)
    } else if right {
        Some(ResizeEdge::Right)
    } else {
        None
    }
}

/// Window resize handles for the client-side frame. They are driven by window
/// level pointer events instead of per-edge elements, so no widget or overlay
/// painted above the margin can swallow them: the cursor follows the pointer
/// and a press starts a compositor resize.
fn resize_layer(tiling: Tiling, hover: Rc<Cell<Option<ResizeEdge>>>) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let size: Size<Pixels> = bounds.size;
            let edge_at = move |position: Point<Pixels>| {
                let local = position - bounds.origin;
                resize_edge_at(
                    f32::from(local.x),
                    f32::from(local.y),
                    f32::from(size.width),
                    f32::from(size.height),
                    tiling,
                )
            };
            if let Some(edge) = edge_at(window.mouse_position()) {
                window.set_window_cursor_style(cursor_for_edge(edge));
            }
            window.on_mouse_event({
                let hover = hover.clone();
                move |event: &MouseMoveEvent, _, window, _| {
                    let edge = edge_at(event.position);
                    if hover.replace(edge) != edge {
                        window.refresh();
                    }
                }
            });
            window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
                    if let Some(edge) = edge_at(event.position) {
                        window.start_window_resize(edge);
                        cx.stop_propagation();
                    }
                }
            });
        },
    )
    // Pinned to the top-left: an absolute child without insets would sit at
    // its static position, i.e. below the window content.
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: f32 = 1100.0;
    const H: f32 = 720.0;

    fn edge(x: f32, y: f32) -> Option<ResizeEdge> {
        resize_edge_at(x, y, W, H, Tiling::default())
    }

    #[test]
    fn shadow_margin_resizes_from_every_side() {
        assert_eq!(edge(4.0, H / 2.0), Some(ResizeEdge::Left));
        assert_eq!(edge(W - 2.0, 300.0), Some(ResizeEdge::Right));
        assert_eq!(edge(500.0, 5.0), Some(ResizeEdge::Top));
        assert_eq!(edge(500.0, H - 3.0), Some(ResizeEdge::Bottom));
    }

    #[test]
    fn corners_win_over_edges() {
        assert_eq!(edge(3.0, 3.0), Some(ResizeEdge::TopLeft));
        assert_eq!(edge(W - 3.0, 20.0), Some(ResizeEdge::TopRight));
        assert_eq!(edge(10.0, H - 3.0), Some(ResizeEdge::BottomLeft));
        assert_eq!(edge(W - 25.0, H - 4.0), Some(ResizeEdge::BottomRight));
    }

    #[test]
    fn the_window_body_is_not_a_handle() {
        assert_eq!(edge(W / 2.0, H / 2.0), None);
        assert_eq!(edge(60.0, 400.0), None);
        assert_eq!(edge(500.0, 50.0), None);
    }

    #[test]
    fn tiled_edges_cannot_be_resized() {
        let tiled = Tiling { left: true, ..Tiling::default() };
        assert_eq!(resize_edge_at(4.0, 300.0, W, H, tiled), None);
        assert_eq!(resize_edge_at(W - 4.0, 300.0, W, H, tiled), Some(ResizeEdge::Right));
    }
}

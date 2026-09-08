//! Frameless window title bar with drag area and window control buttons.

use gpui::{
    div, px, App, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement, Styled, Window, WindowControlArea,
};
use gpui_component::h_flex;

use crate::theme::manager::AppThemeExt;

pub const TITLEBAR_HEIGHT: f32 = 48.0;

#[derive(Default, IntoElement)]
pub struct AppTitleBar;

impl AppTitleBar {
    pub fn new() -> Self {
        Self
    }
}

impl RenderOnce for AppTitleBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.app_theme();
        let _is_maximized = window.is_maximized();

        h_flex()
            .id("app-titlebar")
            .w_full()
            .h(px(TITLEBAR_HEIGHT))
            .bg(theme.secondary)
            .border_b_1()
            .border_color(theme.border)
            .items_center()
            .justify_between()
            .px_3()
            .flex_shrink_0()
            .window_control_area(WindowControlArea::Drag)
            .child(
                // Left app label
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.foreground)
                            .opacity(0.7)
                            .child("WA Bot"),
                    ),
            )
            .child(
                // Right window controls
                h_flex()
                    .items_center()
                    .h_full()
                    .child(
                        // Minimize button
                        div()
                            .id("btn-min")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(44.))
                            .h_full()
                            .text_color(theme.foreground)
                            .hover(|s| s.bg(theme.muted))
                            .cursor_pointer()
                            .on_click(|_, window, _| {
                                window.minimize_window();
                            })
                            .child(
                                div()
                                    .w(px(12.))
                                    .h(px(1.5))
                                    .bg(theme.foreground),
                            ),
                    )
                    .child(
                        // Maximize/Restore button
                        div()
                            .id("btn-max")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(44.))
                            .h_full()
                            .text_color(theme.foreground)
                            .hover(|s| s.bg(theme.muted))
                            .cursor_pointer()
                            .on_click(|_, window, _| {
                                window.zoom_window();
                            })
                            .child(
                                div()
                                    .w(px(10.))
                                    .h(px(10.))
                                    .border_1()
                                    .border_color(theme.foreground),
                            ),
                    )
                    .child(
                        // Close button
                        div()
                            .id("btn-close")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(44.))
                            .h_full()
                            .text_color(theme.foreground)
                            .hover(|s| s.bg(theme.destructive).text_color(theme.destructive_foreground))
                            .cursor_pointer()
                            .on_click(|_, window, _| {
                                window.remove_window();
                            })
                            .child(
                                div()
                                    .text_sm()
                                    .child("✕"),
                            ),
                    ),
            )
    }
}

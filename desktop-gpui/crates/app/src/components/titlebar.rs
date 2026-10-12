//! Frameless window title bar with drag area and window control buttons.

use gpui::{
    div, prelude::FluentBuilder as _, px, rgb, svg, App, InteractiveElement, IntoElement, MouseButton, ParentElement,
    RenderOnce, Styled, Window, WindowControlArea,
};
use gpui_component::h_flex;
#[cfg(target_os = "windows")]
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use crate::icons::{MAXIMIZE_SVG, MINIMIZE_SVG, RESTORE_SVG, X_SVG};
use crate::theme::manager::{AppThemeExt, ThemeManager};
use crate::window_material;

pub const TITLEBAR_HEIGHT: f32 = 48.0;

/// Check if the window is currently maximized.
pub fn is_window_maximized(window: &Window) -> bool {
    #[cfg(target_os = "windows")]
    {
        if let Ok(handle) = HasWindowHandle::window_handle(window) {
            if let RawWindowHandle::Win32(h) = handle.as_raw() {
                let hwnd = h.hwnd.get();
                extern "system" {
                    fn IsZoomed(hwnd: isize) -> i32;
                }
                return unsafe { IsZoomed(hwnd) != 0 };
            }
        }
    }
    window.is_maximized()
}

/// Toggle maximize / restore matching webterm implementation.
pub fn toggle_maximize(window: &mut Window) -> bool {
    #[cfg(target_os = "windows")]
    {
        if let Ok(handle) = HasWindowHandle::window_handle(window) {
            if let RawWindowHandle::Win32(h) = handle.as_raw() {
                let hwnd = h.hwnd.get();
                extern "system" {
                    fn IsZoomed(hwnd: isize) -> i32;
                    fn ShowWindowAsync(hwnd: isize, nCmdShow: i32) -> i32;
                    fn DwmSetWindowAttribute(
                        hwnd: isize,
                        dwAttribute: u32,
                        pvAttribute: *const std::ffi::c_void,
                        cbAttribute: u32,
                    ) -> i32;
                    fn RedrawWindow(
                        hwnd: isize,
                        lprcUpdate: *const std::ffi::c_void,
                        hrgnUpdate: isize,
                        flags: u32,
                    ) -> i32;
                }
                const SW_RESTORE: i32 = 9;
                const SW_MAXIMIZE: i32 = 3;
                const RDW_INVALIDATE: u32 = 0x0001;
                const RDW_UPDATENOW: u32 = 0x0100;
                const RDW_ALLCHILDREN: u32 = 0x0080;

                let is_max = unsafe { IsZoomed(hwnd) != 0 };
                let dark: i32 = 1;

                unsafe {
                    DwmSetWindowAttribute(hwnd, 20, &dark as *const _ as _, 4);
                    DwmSetWindowAttribute(hwnd, 19, &dark as *const _ as _, 4);

                    if is_max {
                        ShowWindowAsync(hwnd, SW_RESTORE);
                    } else {
                        ShowWindowAsync(hwnd, SW_MAXIMIZE);
                    }
                    RedrawWindow(
                        hwnd,
                        std::ptr::null(),
                        0,
                        RDW_INVALIDATE | RDW_UPDATENOW | RDW_ALLCHILDREN,
                    );
                }

                window.on_next_frame(|window, _cx| {
                    window.refresh();
                });

                return !is_max;
            }
        }
    }
    let was_max = window.is_maximized();
    window.zoom_window();
    window.on_next_frame(|window, _cx| {
        window.refresh();
    });
    !was_max
}

#[derive(Default, IntoElement)]
pub struct AppTitleBar;

impl AppTitleBar {
    pub fn new() -> Self {
        Self
    }
}

impl RenderOnce for AppTitleBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // A panel popover has no window chrome.
        if crate::popover::is_popover(cx) {
            return div().into_any_element();
        }
        let theme = cx.app_theme();
        let window_transparency = ThemeManager::global(cx).window_transparency;
        let is_max = is_window_maximized(window);
        let framed = window_material::is_framed_window(window);

        h_flex()
            .id("app-titlebar")
            .w_full()
            .h(px(TITLEBAR_HEIGHT))
            .bg(window_material::glass_fill(theme.background, window_transparency))
            .border_b_1()
            .border_color(theme.border.opacity(0.4))
            .items_center()
            .justify_between()
            .pl_3()
            .flex_shrink_0()
            .when(framed, |s| {
                s.rounded_tl(px(window_material::WINDOW_CORNER_RADIUS))
                    .rounded_tr(px(window_material::WINDOW_CORNER_RADIUS))
            })
            // NOTE: Do not set window_control_area(Drag) here so children remain interactive!
            // Left app label
            .child(
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
            // Middle drag spacer
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .window_control_area(WindowControlArea::Drag)
                    .on_mouse_down(MouseButton::Left, |_, window, cx| {
                        // The compositor takes over the drag and swallows the
                        // release, so a selection begun by this press would
                        // stay armed and grow with the pointer afterwards.
                        gpui_base::GlobalState::suppress_text_selection(cx);
                        window.defer(cx, |window, cx| gpui_base::TextSelection::clear(window, cx));
                        window.start_window_move();
                    }),
            )
            // Right window controls matching webterm exactly
            .child(
                h_flex()
                    .items_center()
                    .h_full()
                    // Minimize button
                    .child(
                        div()
                            .id("btn-min")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(44.0))
                            .h_full()
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.muted))
                            .child(
                                svg()
                                    .data(MINIMIZE_SVG)
                                    .size(px(12.0))
                                    .text_color(theme.foreground),
                            )
                            .on_mouse_down(MouseButton::Left, |_, window, _| {
                                window.minimize_window();
                            }),
                    )
                    // Maximize/Restore button
                    .child(
                        div()
                            .id("btn-max")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(44.0))
                            .h_full()
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.muted))
                            .child(
                                svg()
                                    .data(if is_max { RESTORE_SVG } else { MAXIMIZE_SVG })
                                    .size(px(12.0))
                                    .text_color(theme.foreground),
                            )
                            .on_mouse_down(MouseButton::Left, |_, window, _| {
                                toggle_maximize(window);
                            }),
                    )
                    // Close button
                    .child(
                        div()
                            .id("btn-close")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(46.0))
                            .h_full()
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0xe81123)).text_color(rgb(0xffffff)))
                            .child(
                                svg()
                                    .data(X_SVG)
                                    .size(px(13.0))
                                    .text_color(theme.foreground),
                            )
                            .on_mouse_down(MouseButton::Left, |_, window, _| {
                                window.remove_window();
                            }),
                    ),
            )
            .into_any_element()
    }
}

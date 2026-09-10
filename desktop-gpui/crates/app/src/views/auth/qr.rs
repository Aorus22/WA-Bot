//! QR Code view component for WhatsApp device pairing.

use gpui::{
    canvas, div, hsla, px, App, Bounds, IntoElement, ParentElement, Pixels, Point,
    RenderOnce, Size, Styled, Window,
};
use gpui_component::{button::Button, h_flex, v_flex, Icon, IconName};

use crate::state::auth::AuthState;
use crate::theme::manager::AppThemeExt;

#[derive(Default, IntoElement)]
pub struct QrCodeView;

impl QrCodeView {
    pub fn new() -> Self {
        Self
    }
}

impl RenderOnce for QrCodeView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.app_theme();
        let (qr_code, is_loading, error_msg) = if cx.has_global::<AuthState>() {
            let auth = AuthState::global(cx);
            (auth.qr_code.clone(), auth.qr_loading, auth.qr_error.clone())
        } else {
            (None, false, None)
        };

        v_flex()
            .items_center()
            .gap_6()
            .w_full()
            .child(
                // QR Box Container (min 280x280)
                div()
                    .p_4()
                    .rounded_2xl()
                    .bg(theme.muted)
                    .border_1()
                    .border_color(theme.border)
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(288.))
                    .h(px(288.))
                    .child(
                        if is_loading {
                            v_flex()
                                .items_center()
                                .gap_3()
                                .child(Icon::new(IconName::LoaderCircle).size(px(32.)))
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(theme.muted_foreground)
                                        .child("Loading QR code..."),
                                )
                                .into_any_element()
                        } else if let Some(err) = error_msg {
                            v_flex()
                                .items_center()
                                .gap_4()
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(theme.muted_foreground)
                                        .child(err),
                                )
                                .child(
                                    Button::new("btn-retry")
                                        .label("Retry")
                                        .on_click(|_, _, cx| {
                                            trigger_qr_refresh(cx);
                                        }),
                                )
                                .into_any_element()
                        } else if let Some(ref code_str) = qr_code {
                            Self::render_qr_matrix(code_str)
                        } else {
                            v_flex()
                                .items_center()
                                .gap_3()
                                .child(Icon::new(IconName::LoaderCircle).size(px(32.)))
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(theme.muted_foreground)
                                        .child("No QR code available yet"),
                                )
                                .into_any_element()
                        },
                    ),
            )
            .child(
                // Refresh QR button
                Button::new("btn-refresh-qr")
                    .label("Refresh QR")
                    .on_click(|_, _, cx| {
                        trigger_qr_refresh(cx);
                    }),
            )
            .child(
                // 3 numbered instructions
                v_flex()
                    .gap_3()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .w_full()
                    .max_w(px(320.))
                    .child(Self::render_step(1, "Open WhatsApp on your phone", cx))
                    .child(Self::render_step(
                        2,
                        "Tap Menu or Settings and select Linked Devices",
                        cx,
                    ))
                    .child(Self::render_step(
                        3,
                        "Point your phone to scan the QR code",
                        cx,
                    )),
            )
    }
}

impl QrCodeView {
    fn render_qr_matrix(code_str: &str) -> gpui::AnyElement {
        if let Ok(qr) = qrcode::QrCode::new(code_str.as_bytes()) {
            let width = qr.width();
            let modules: Vec<bool> = qr
                .into_colors()
                .into_iter()
                .map(|c| c != qrcode::Color::Light)
                .collect();
            let total_size = 256.0;
            let quiet_zone = 16.0;
            let matrix_size = total_size - (quiet_zone * 2.0);
            let cell_size = matrix_size / width as f32;

            canvas(
                move |bounds: Bounds<Pixels>, _, _| (bounds, modules, width, cell_size, quiet_zone),
                move |bounds: Bounds<Pixels>, (_bounds_prep, modules, width, cell_size, quiet_zone), window, _| {
                    // Paint pure white background (quiet zone)
                    window.paint_quad(gpui::fill(
                        bounds,
                        hsla(0.0, 0.0, 1.0, 1.0),
                    ));

                    // Paint black modules with quiet zone offset
                    for y in 0..width {
                        for x in 0..width {
                            let is_dark = modules[y * width + x];
                            if is_dark {
                                let cell_bounds = Bounds {
                                    origin: Point {
                                        x: bounds.origin.x + px(quiet_zone + x as f32 * cell_size),
                                        y: bounds.origin.y + px(quiet_zone + y as f32 * cell_size),
                                    },
                                    size: Size {
                                        width: px(cell_size + 0.3),
                                        height: px(cell_size + 0.3),
                                    },
                                };
                                window.paint_quad(gpui::fill(
                                    cell_bounds,
                                    hsla(0.0, 0.0, 0.0, 1.0),
                                ));
                            }
                        }
                    }
                },
            )
            .w(px(256.))
            .h(px(256.))
            .rounded_xl()
            .overflow_hidden()
            .into_any_element()
        } else {
            div().child("Invalid QR code").into_any_element()
        }
    }

    fn render_step(num: usize, text: &'static str, cx: &App) -> impl IntoElement {
        let theme = cx.app_theme();

        h_flex()
            .gap_3()
            .items_center()
            .child(
                div()
                    .flex_shrink_0()
                    .w(px(24.))
                    .h(px(24.))
                    .rounded_full()
                    .bg(theme.muted)
                    .border_1()
                    .border_color(theme.border)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.foreground)
                    .child(num.to_string()),
            )
            .child(div().child(text))
    }
}

fn trigger_qr_refresh(cx: &mut App) {
    if cx.has_global::<AuthState>() {
        let base_url = AuthState::global(cx).base_url.clone();
        AuthState::global_mut(cx).set_qr_loading(true);
        cx.refresh_windows();

        cx.spawn(async move |cx| {
            let res = crate::TOKIO_RT
                .spawn(async move {
                    let client = wabot_backend_client::client::HttpClient::new(&base_url);
                    client.get_qr_code().await
                })
                .await;

            let _ = cx.update(|cx| {
                if cx.has_global::<AuthState>() {
                    let auth = AuthState::global_mut(cx);
                    match res {
                        Ok(Ok(qr)) if !qr.code.is_empty() => {
                            auth.set_qr_code(Some(qr.code));
                        }
                        _ => {
                            auth.set_qr_error(Some("Failed to fetch QR code from backend.".into()));
                        }
                    }
                    cx.refresh_windows();
                }
            });
        })
        .detach();
    }
}


//! Login page matching web client LoginPage.tsx 1:1.

use gpui::{
    div, prelude::FluentBuilder as _, px, Context, InteractiveElement,
    IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled, Window,
};
use gpui_component::{
    button::Button,
    h_flex, v_flex, Icon, IconName,
};

use crate::state::auth::{AuthState, LoginTab};
use crate::theme::manager::AppThemeExt;
use super::qr::QrCodeView;

pub struct LoginView;

impl LoginView {
    pub fn new() -> Self {
        Self
    }
}

impl Render for LoginView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.app_theme();
        let active_tab = if cx.has_global::<AuthState>() {
            AuthState::global(cx).active_tab
        } else {
            LoginTab::Qr
        };

        v_flex()
            .id("login-view")
            .size_full()
            .bg(theme.background)
            .items_center()
            .justify_center()
            .p_4()
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(512.))
                    .items_center()
                    .gap_6()
                    .child(
                        // Header
                        v_flex()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .w(px(56.))
                                    .h(px(56.))
                                    .rounded_2xl()
                                    .bg(theme.accent)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(theme.primary)
                                    .mb_2()
                                    .child(Icon::new(IconName::Inbox).size(px(28.))),
                            )
                            .child(
                                div()
                                    .text_2xl()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(theme.foreground)
                                    .child("Link your device"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child("Scan the QR code with your WhatsApp to connect"),
                            ),
                    )
                    .child(
                        // Tab Switcher
                        h_flex()
                            .w_full()
                            .max_w(px(320.))
                            .p_1()
                            .rounded_xl()
                            .bg(theme.muted)
                            .border_1()
                            .border_color(theme.border)
                            .gap_1()
                            .child(
                                div()
                                    .id("tab-qr")
                                    .flex_1()
                                    .py_2()
                                    .px_4()
                                    .rounded_lg()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .when(active_tab == LoginTab::Qr, |s| {
                                        s.bg(theme.card)
                                            .text_color(theme.card_foreground)
                                            .border_1()
                                            .border_color(theme.border)
                                    })
                                    .when(active_tab != LoginTab::Qr, |s| {
                                        s.text_color(theme.muted_foreground)
                                    })
                                    .on_click(|_, _, cx| {
                                        if cx.has_global::<AuthState>() {
                                            AuthState::global_mut(cx).set_tab(LoginTab::Qr);
                                            cx.refresh_windows();
                                        }
                                    })
                                    .child("QR Code"),
                            )
                            .child(
                                div()
                                    .id("tab-phone")
                                    .flex_1()
                                    .py_2()
                                    .px_4()
                                    .rounded_lg()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .when(active_tab == LoginTab::Phone, |s| {
                                        s.bg(theme.card)
                                            .text_color(theme.card_foreground)
                                            .border_1()
                                            .border_color(theme.border)
                                    })
                                    .when(active_tab != LoginTab::Phone, |s| {
                                        s.text_color(theme.muted_foreground)
                                    })
                                    .on_click(|_, _, cx| {
                                        if cx.has_global::<AuthState>() {
                                            AuthState::global_mut(cx).set_tab(LoginTab::Phone);
                                            cx.refresh_windows();
                                        }
                                    })
                                    .child("Phone"),
                            ),
                    )
                    .child(
                        // Main Card
                        div()
                            .w_full()
                            .p_6()
                            .rounded_2xl()
                            .bg(theme.card)
                            .border_1()
                            .border_color(theme.border)
                            .child(
                                match active_tab {
                                    LoginTab::Qr => QrCodeView::new().into_any_element(),
                                    LoginTab::Phone => {
                                        v_flex()
                                            .items_center()
                                            .gap_6()
                                            .py_6()
                                            .child(
                                                div()
                                                    .w(px(64.))
                                                    .h(px(64.))
                                                    .rounded_2xl()
                                                    .bg(theme.accent)
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .text_color(theme.primary)
                                                    .child(Icon::new(IconName::Network).size(px(32.))),
                                            )
                                            .child(
                                                v_flex()
                                                    .items_center()
                                                    .gap_1()
                                                    .child(
                                                        div()
                                                            .text_lg()
                                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                                            .text_color(theme.foreground)
                                                            .child("Link with phone number"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .text_color(theme.muted_foreground)
                                                            .child("This feature is coming soon to the bot interface."),
                                                    ),
                                            )
                                            .child(
                                                Button::new("btn-back-qr")
                                                    .label("Back to QR code")
                                                    .on_click(|_, _, cx| {
                                                        if cx.has_global::<AuthState>() {
                                                            AuthState::global_mut(cx).set_tab(LoginTab::Qr);
                                                            cx.refresh_windows();
                                                        }
                                                    }),
                                            )
                                            .into_any_element()
                                    }
                                }
                            ),
                    )
                    .child(
                        // Footer
                        h_flex()
                            .gap_2()
                            .items_center()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(Icon::new(IconName::CircleCheck).size(px(14.)))
                            .child("End-to-end encrypted"),
                    ),
            )
    }
}

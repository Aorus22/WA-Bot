//! Root gate view managing the Checking -> Login -> Shell states.

use gpui::{
    div, px, AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window,
};
use gpui_component::{v_flex, Icon, IconName, Root};

use crate::components::motion;
use crate::state::auth::{AuthState, AuthStatus};
use crate::theme::manager::AppThemeExt;
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
        // Dialogs opened through `window.open_*_dialog` (e.g. the logout
        // confirmation) only show if some view mounts this layer.
        let dialog_layer = Root::render_dialog_layer(window, cx);

        let theme = cx.app_theme();
        let status = if cx.has_global::<AuthState>() {
            AuthState::global(cx).status
        } else {
            AuthStatus::Checking
        };

        let content = match status {
            AuthStatus::Checking => {
                v_flex()
                    .size_full()
                    .bg(theme.background)
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
                    .bg(theme.background)
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

        div()
            .relative()
            .size_full()
            .child(content)
            .children(dialog_layer)
    }
}

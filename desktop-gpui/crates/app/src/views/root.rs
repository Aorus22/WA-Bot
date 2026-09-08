//! Root gate view managing the Checking -> Login -> Shell states.

use gpui::{
    div, px, AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window,
};
use gpui_component::{v_flex, Icon, IconName};

use crate::state::auth::{AuthState, AuthStatus};
use crate::theme::manager::AppThemeExt;
use crate::views::auth::login::LoginView;
use crate::views::shell::AppShellView;

pub struct RootGateView {
    login_view: Entity<LoginView>,
    shell_view: Entity<AppShellView>,
}

impl RootGateView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            login_view: cx.new(|_| LoginView::new()),
            shell_view: cx.new(|_| AppShellView::new()),
        }
    }
}

impl Render for RootGateView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.app_theme();
        let status = if cx.has_global::<AuthState>() {
            AuthState::global(cx).status
        } else {
            AuthStatus::Checking
        };

        match status {
            AuthStatus::Checking => {
                v_flex()
                    .size_full()
                    .bg(theme.background)
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(IconName::LoaderCircle)
                            .size(px(40.))
                            .text_color(theme.primary),
                    )
                    .into_any_element()
            }
            AuthStatus::Unauthenticated => {
                div()
                    .size_full()
                    .child(self.login_view.clone())
                    .into_any_element()
            }
            AuthStatus::Authenticated => {
                div()
                    .size_full()
                    .child(self.shell_view.clone())
                    .into_any_element()
            }
        }
    }
}

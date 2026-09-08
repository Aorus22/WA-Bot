//! Toast notification helpers wrapping gpui-component notification system.

use gpui::{App, SharedString, Window};
use gpui_component::{
    notification::{Notification, NotificationType},
    WindowExt as _,
};

/// Helper functions to show toast messages.
pub struct Toast;

impl Toast {
    pub fn info(message: impl Into<SharedString>, window: &mut Window, cx: &mut App) {
        window.push_notification(Notification::info(message), cx);
    }

    pub fn success(message: impl Into<SharedString>, window: &mut Window, cx: &mut App) {
        window.push_notification(Notification::success(message), cx);
    }

    pub fn warning(message: impl Into<SharedString>, window: &mut Window, cx: &mut App) {
        window.push_notification(
            Notification::new()
                .message(message)
                .with_type(NotificationType::Warning),
            cx,
        );
    }

    pub fn error(message: impl Into<SharedString>, window: &mut Window, cx: &mut App) {
        window.push_notification(
            Notification::new()
                .message(message)
                .with_type(NotificationType::Error),
            cx,
        );
    }
}

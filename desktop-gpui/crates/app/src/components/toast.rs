//! Global in-app toast notifications.
//!
//! Mirrors the web app's `sonner`-style usage: any view, dialog, async task,
//! or the WS event loop can raise a toast with nothing but `&mut App`
//! (`Context<T>` derefs to `App`, so plain view handlers work too).
//!
//! Toasts render through gpui-component's notification system (already hosted
//! by the `gpui_component::Root` wrapper created in `main.rs`), so they
//! auto-hide after ~5s, stack, and can be dismissed manually.
//!
//! Usage:
//! ```ignore
//! toast::success("Pesan terkirim", cx);
//! toast::error("Gagal menghapus pesan", cx);
//! ```

use gpui::{AnyWindowHandle, App, Global, SharedString};
use gpui_component::{
    notification::{Notification, NotificationType},
    WindowExt as _,
};

/// Global bus holding the main window handle so toasts can be pushed without
/// a `&mut Window` in scope. Registered in `main.rs` after the window opens.
#[derive(Default)]
pub struct ToastBus {
    window: Option<AnyWindowHandle>,
}

impl Global for ToastBus {}

impl ToastBus {
    /// Remember which window should host toasts.
    pub fn set_window(&mut self, handle: AnyWindowHandle) {
        self.window = Some(handle);
    }

    fn window(&self, cx: &App) -> Option<AnyWindowHandle> {
        // Prefer the registered main window; fall back to the only open
        // window if registration hasn't happened yet.
        self.window.or_else(|| {
            let mut windows = cx.windows();
            if windows.len() == 1 {
                windows.pop()
            } else {
                None
            }
        })
    }
}

fn push(note: Notification, cx: &mut App) {
    let handle = cx
        .try_global::<ToastBus>()
        .and_then(|bus| bus.window(cx));
    if let Some(handle) = handle {
        let _ = handle.update(cx, |_, window, cx| {
            window.push_notification(note, cx);
        });
    }
}

/// Show a neutral/info toast.
pub fn info(message: impl Into<SharedString>, cx: &mut App) {
    push(Notification::info(message), cx);
}

/// Show a success toast.
pub fn success(message: impl Into<SharedString>, cx: &mut App) {
    push(Notification::success(message), cx);
}

/// Show a warning toast.
pub fn warning(message: impl Into<SharedString>, cx: &mut App) {
    push(
        Notification::new()
            .message(message)
            .with_type(NotificationType::Warning),
        cx,
    );
}

/// Show an error toast.
pub fn error(message: impl Into<SharedString>, cx: &mut App) {
    push(
        Notification::new()
            .message(message)
            .with_type(NotificationType::Error),
        cx,
    );
}

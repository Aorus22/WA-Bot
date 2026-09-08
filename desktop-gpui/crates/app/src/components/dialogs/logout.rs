//! Confirmation dialog for WhatsApp logout.

use gpui::{App, Window};
use gpui_component::{
    button::ButtonVariant,
    dialog::DialogButtonProps,
    WindowExt as _,
};

use std::rc::Rc;

/// Open the logout confirmation dialog.
pub fn open_logout_dialog<F>(window: &mut Window, cx: &mut App, on_confirm: F)
where
    F: Fn(&mut Window, &mut App) + 'static,
{
    let on_confirm = Rc::new(on_confirm);
    window.open_alert_dialog(cx, move |alert, _, _| {
        let on_confirm = on_confirm.clone();
        alert
            .title("Log out of WhatsApp?")
            .description("You will need to scan the QR code again to reconnect.")
            .confirm()
            .button_props(
                DialogButtonProps::default()
                    .ok_text("Log Out")
                    .ok_variant(ButtonVariant::Danger)
                    .cancel_text("Cancel")
                    .show_cancel(true),
            )
            .on_ok(move |_, window, cx| {
                on_confirm(window, cx);
                true
            })
    });
}

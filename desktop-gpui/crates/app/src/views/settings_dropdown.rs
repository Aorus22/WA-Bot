//! The list a Settings dropdown opens below its button.
//!
//! It is drawn deferred, so the rows underneath cannot paint over it, and on a
//! solid background, so their text does not show through the glass fill.

use gpui::*;

const GAP_BELOW_BUTTON: f32 = 36.0;

/// `items` are the option rows. Put the result next to the button, inside a
/// wrapper that is `relative()` and exactly as wide as the button.
pub(super) fn dropdown_list(width: f32, background: Hsla, border: Hsla, items: impl IntoIterator<Item = Div>) -> impl IntoElement {
    let solid = Hsla { a: 1.0, ..background };
    deferred(
        anchored().anchor(Anchor::TopRight).offset(point(px(width), px(GAP_BELOW_BUTTON))).child(
            div()
                .w(px(width))
                .rounded_md()
                .border_1()
                .border_color(border)
                .bg(solid)
                .shadow_lg()
                .py_1()
                .occlude()
                .children(items),
        ),
    )
    .with_priority(gpui_base::POPUP_PRIORITY)
}

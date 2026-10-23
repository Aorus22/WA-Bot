//! Form row layout for a label, optional hint, and trailing control.

use gpui::{div, AnyElement, Div, ParentElement, Styled};
use gpui_component::{h_flex, v_flex};

/// Arrange form content without owning any of its visual styling.
pub fn form_row(label: AnyElement, hint: Option<AnyElement>, control: AnyElement) -> Div {
    let mut text = v_flex().flex_1().min_w_0().gap_0p5().child(label);
    if let Some(hint) = hint {
        text = text.child(hint);
    }

    h_flex()
        .w_full()
        .items_center()
        .justify_between()
        .gap_4()
        .child(text)
        .child(div().flex_none().child(control))
}

//! Anchored dropdown list surface.
//!
//! It is drawn deferred so rows underneath cannot paint over it, and uses the
//! shared popover surface, border, radius, and shadow tokens.

use gpui::*;

use crate::theme::manager::ThemeManager;

const OFFSET_BELOW_TRIGGER: f32 = 36.0;

#[derive(Clone, Copy)]
pub struct DropdownMenuStyle {
    background: Hsla,
    border: Hsla,
    radius: f32,
    shadow: bool,
}

impl DropdownMenuStyle {
    pub fn from_theme(cx: &App) -> Self {
        let theme = ThemeManager::global(cx);
        Self {
            background: theme.surface.popover,
            border: theme.tokens.border,
            radius: theme.shape.radius_popover,
            shadow: theme.shape.shadows,
        }
    }
}

/// Render option rows below a trigger with the app's shared popup styling.
/// Place it inside a relative wrapper sized to the trigger.
pub fn dropdown_list(
    style: DropdownMenuStyle,
    width: f32,
    items: impl IntoIterator<Item = Div>,
) -> impl IntoElement {
    let solid = Hsla { a: 1.0, ..style.background };
    let mut surface = div()
        .w(px(width))
        .rounded(px(style.radius))
        .border_1()
        .border_color(style.border)
        .bg(solid)
        .py_1()
        .occlude()
        .children(items);
    if style.shadow {
        surface = surface.shadow_lg();
    }

    deferred(
        anchored()
            .anchor(Anchor::TopRight)
            .offset(point(px(width), px(OFFSET_BELOW_TRIGGER)))
            .child(surface),
    )
    .with_priority(gpui_base::POPUP_PRIORITY)
}

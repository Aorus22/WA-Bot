//! Themed card surface; pages control only content and spacing.

use gpui::{App, Div, Styled, div, px};
use gpui::prelude::FluentBuilder as _;

use crate::theme::manager::ThemeManager;
use crate::theme::shape::ShapeExt;

/// Create a themed card surface with the shared border, radius, and elevation.
pub fn card(cx: &App) -> Div {
    let theme = ThemeManager::global(cx);
    let shape = theme.shape;
    let colors = theme.tokens;
    let surfaces = theme.surface;

    div()
        .w_full()
        .rounded(px(shape.radius_card))
        .bg(if shape.adwaita { surfaces.card } else { colors.card })
        .border_1()
        .border_color(if shape.adwaita { surfaces.border } else { colors.border })
        .when(shape.shadows, |card| card.shadow_sm())
}

/// A clipped card for grouped settings and related rows.
pub fn card_group(cx: &App) -> Div {
    card(cx)
        .rounded(px(cx.shape().radius_group))
        .overflow_hidden()
}

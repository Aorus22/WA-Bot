//! Themed card surface; pages control only content and spacing.

use gpui::{AnyElement, App, Div, Hsla, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px};
use gpui::prelude::FluentBuilder as _;

use crate::theme::manager::ThemeManager;
use crate::theme::shape::ShapeExt;

/// Create a themed card surface with the shared border, radius, and elevation.
pub fn card(cx: &App) -> Div {
    let theme = ThemeManager::global(cx);
    let shape = theme.shape;
    let surfaces = theme.surface;

    div()
        .w_full()
        .rounded(px(shape.radius_card))
        .bg(surfaces.card)
        .border_1()
        .border_color(if shape.winui { surfaces.card_stroke } else { surfaces.border })
        .when(shape.shadows && !shape.winui, |card| card.shadow_sm())
}

/// A clipped card for grouped settings and related rows.
pub fn card_group(cx: &App) -> Div {
    card(cx)
        .rounded(px(cx.shape().radius_group))
        .overflow_hidden()
}

enum GroupItem {
    Row(AnyElement),
    Divider(Hsla),
}

/// Rows of a settings section. Other profiles draw one clipped card with divider
/// lines; the Fluent profile draws each row as its own card (WinUI `SettingsCard`:
/// 68px minimum, 4px radius, 1px stroke, 4px apart) and drops the dividers.
#[derive(IntoElement)]
pub struct SettingsGroup {
    items: Vec<GroupItem>,
}

pub fn settings_group() -> SettingsGroup {
    SettingsGroup { items: Vec::new() }
}

impl SettingsGroup {
    /// Line between two rows; only drawn where the rows share one card.
    pub fn divider(mut self, color: Hsla) -> Self {
        self.items.push(GroupItem::Divider(color));
        self
    }
}

impl ParentElement for SettingsGroup {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.items.extend(elements.into_iter().map(GroupItem::Row));
    }
}

impl RenderOnce for SettingsGroup {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = ThemeManager::global(cx);
        let shape = theme.shape;
        let surfaces = theme.surface;
        if shape.winui {
            div()
                .w_full()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .children(self.items.into_iter().filter_map(|item| match item {
                    GroupItem::Row(row) => Some(
                        div()
                            .w_full()
                            .min_h(px(68.0))
                            .flex()
                            .flex_col()
                            .justify_center()
                            .rounded(px(shape.radius_control))
                            .bg(surfaces.card)
                            .border_1()
                            .border_color(surfaces.card_stroke)
                            .overflow_hidden()
                            .child(row),
                    ),
                    GroupItem::Divider(_) => None,
                }))
        } else {
            card_group(cx).children(self.items.into_iter().map(|item| match item {
                GroupItem::Row(row) => row,
                GroupItem::Divider(color) => div().h(px(1.0)).bg(color).w_full().into_any_element(),
            }))
        }
    }
}

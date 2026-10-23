//! App button component backed by the shared GPUI implementation.

use gpui::{App, ElementId, IntoElement, ParentElement, SharedString, px};
use gpui_component::{Disableable, Size, Sizable};

use crate::theme::manager::ThemeManager;

pub use gpui_component::button::{Button, ButtonVariant, ButtonVariants};

/// Create a standard app button with shared size and shape tokens.
///
/// The caller chooses the semantic variant (`primary`, `secondary`, `ghost`,
/// and so on) and supplies the label or content.
pub fn button(id: impl Into<ElementId>, cx: &App) -> Button {
    Button::new(id)
        .with_size(Size::Medium)
        .rounded(px(ThemeManager::global(cx).shape.radius_button))
}

/// Create a labeled app button with a semantic variant and disabled state.
pub fn action_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    variant: ButtonVariant,
    enabled: bool,
    cx: &App,
) -> Button {
    button(id, cx)
        .with_variant(variant)
        .label(label)
        .disabled(!enabled)
}

/// Create a destructive action button.
pub fn destructive_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    enabled: bool,
    cx: &App,
) -> Button {
    action_button(id, label, ButtonVariant::Danger, enabled, cx)
}

/// A standard ghost icon button with the app's shared size, radius, tooltip,
/// and accessible name. Pages add only its action.
pub fn icon_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    icon: impl IntoElement,
    cx: &App,
) -> Button {
    let label = label.into();
    button(id, cx)
        .ghost()
        .accessibility_label(label.clone())
        .tooltip(label)
        .child(icon.into_any_element())
}

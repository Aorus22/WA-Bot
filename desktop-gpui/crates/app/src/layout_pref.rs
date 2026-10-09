//! Chat screen layout preference (pages vs sidebar), see `ChatLayout`.

use gpui::{App, Global};
use wabot_settings::{ChatLayout, DesktopSettings};

/// The saved preference, as a global so any view can resolve the layout.
pub struct ChatLayoutPref(pub ChatLayout);
impl Global for ChatLayoutPref {}

/// What a view actually lays out after resolving `Auto`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutKind {
    Pages,
    Sidebar,
}

/// Resolve a preference for a surface: `Auto` means pages in the popover and a
/// sidebar in a normal window.
pub fn resolve(pref: ChatLayout, popover: bool) -> LayoutKind {
    match pref {
        ChatLayout::Pages => LayoutKind::Pages,
        ChatLayout::Sidebar => LayoutKind::Sidebar,
        ChatLayout::Auto | ChatLayout::Unknown => {
            if popover {
                LayoutKind::Pages
            } else {
                LayoutKind::Sidebar
            }
        }
    }
}

pub fn preference(cx: &App) -> ChatLayout {
    if cx.has_global::<ChatLayoutPref>() {
        cx.global::<ChatLayoutPref>().0
    } else {
        ChatLayout::Auto
    }
}

/// Layout for the current surface.
pub fn effective(cx: &App) -> LayoutKind {
    resolve(preference(cx), crate::popover::is_popover(cx))
}

/// Change the preference: update the global, persist it, repaint.
pub fn set(cx: &mut App, pref: ChatLayout) {
    cx.set_global(ChatLayoutPref(pref));
    if let Ok(mut settings) = DesktopSettings::load() {
        settings.chat_layout = pref;
        let _ = settings.save();
    }
    cx.refresh_windows();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_follows_the_surface() {
        assert_eq!(resolve(ChatLayout::Auto, true), LayoutKind::Pages);
        assert_eq!(resolve(ChatLayout::Auto, false), LayoutKind::Sidebar);
        assert_eq!(resolve(ChatLayout::Unknown, true), LayoutKind::Pages);
    }

    #[test]
    fn explicit_choice_wins_on_every_surface() {
        assert_eq!(resolve(ChatLayout::Sidebar, true), LayoutKind::Sidebar);
        assert_eq!(resolve(ChatLayout::Pages, false), LayoutKind::Pages);
    }
}

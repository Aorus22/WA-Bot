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

/// Room the navigation rail takes beside the chat list.
const NAV_RAIL_WIDTH: f32 = 72.0;
/// The conversation needs at least this much next to the list, or its bubbles
/// get cut off.
const MIN_CONVERSATION_WIDTH: f32 = 560.0;

/// Whether `Auto` has to give up the side-by-side layout: the window is too
/// narrow for the rail, a list `list_width` wide and a usable conversation (for
/// example when it is tiled to half of a small screen).
pub fn too_narrow_for_sidebar(window_width: f32, list_width: f32) -> bool {
    window_width < NAV_RAIL_WIDTH + list_width + MIN_CONVERSATION_WIDTH
}

/// Like `effective`, knowing how wide the window and the chat list are.
pub fn effective_for_width(cx: &App, window_width: f32, list_width: f32) -> LayoutKind {
    let pref = preference(cx);
    let auto = matches!(pref, ChatLayout::Auto | ChatLayout::Unknown);
    if auto && !crate::popover::is_popover(cx) && too_narrow_for_sidebar(window_width, list_width) {
        return LayoutKind::Pages;
    }
    resolve(pref, crate::popover::is_popover(cx))
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
    fn narrow_windows_give_up_the_sidebar() {
        // 940 px with a 450 px list leaves under 420 px for the conversation.
        assert!(too_narrow_for_sidebar(940.0, 450.0));
        assert!(!too_narrow_for_sidebar(1300.0, 380.0));
        // A wider list needs a wider window.
        assert!(too_narrow_for_sidebar(1100.0, 560.0));
    }

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

//! Adwaita surface and state tokens, derived from the active app palette.

use gpui::Hsla;
use gpui_component::theme::Colorize;

use super::manager::ActiveTokens;

/// Raw surface colors exposed by a desktop GTK theme when available.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SurfaceOverrides {
    pub window: Option<Hsla>,
    pub view: Option<Hsla>,
    pub sidebar: Option<Hsla>,
    pub headerbar: Option<Hsla>,
    pub card: Option<Hsla>,
    pub dialog: Option<Hsla>,
    pub popover: Option<Hsla>,
}

/// GTK-style colors used by native views. They are computed even when the
/// style is off, but consumers must only apply them when `ShapeTokens::adwaita`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceTokens {
    pub window: Hsla,
    pub view: Hsla,
    pub sidebar: Hsla,
    pub secondary_sidebar: Hsla,
    pub headerbar: Hsla,
    pub card: Hsla,
    pub dialog: Hsla,
    pub popover: Hsla,
    pub window_backdrop: Hsla,
    pub view_backdrop: Hsla,
    pub sidebar_backdrop: Hsla,
    pub secondary_sidebar_backdrop: Hsla,
    pub headerbar_backdrop: Hsla,
    pub card_backdrop: Hsla,
    pub dialog_backdrop: Hsla,
    pub popover_backdrop: Hsla,
    pub fg_muted: Hsla,
    pub border: Hsla,
    pub sidebar_border: Hsla,
    pub card_shade: Hsla,
    pub pinned_banner: Hsla,
    pub scrim: Hsla,
    pub scrim_default: Hsla,
    pub selection_focused: Hsla,
    pub selection_unfocused: Hsla,
    pub active_toggle_bg: Hsla,
    pub is_dark: bool,
}

pub trait SurfaceExt {
    fn surface(&self) -> SurfaceTokens;
}

impl SurfaceExt for gpui::App {
    fn surface(&self) -> SurfaceTokens {
        super::manager::ThemeManager::global(self).surface
    }
}

impl<T> SurfaceExt for gpui::Context<'_, T> {
    fn surface(&self) -> SurfaceTokens {
        super::manager::ThemeManager::global(self).surface
    }
}

impl SurfaceTokens {
    pub fn from_tokens(
        tokens: &ActiveTokens,
        is_dark: bool,
        overrides: Option<SurfaceOverrides>,
    ) -> Self {
        let white = Hsla::from(gpui::rgb(0xffffff));
        let black = Hsla::from(gpui::rgb(0x000000));
        // Colorize::mix_oklab uses its factor as the weight of `self`.
        let (view_mix, sidebar_mix, secondary_mix) = if is_dark {
            (black, white, white)
        } else {
            (white, black, black)
        };
        let window = overrides
            .and_then(|o| o.window)
            .unwrap_or(tokens.background);
        let view = overrides
            .and_then(|o| o.view)
            .unwrap_or_else(|| tokens.background.mix_oklab(view_mix, 0.96));
        let sidebar = overrides
            .and_then(|o| o.sidebar)
            .unwrap_or_else(|| tokens.background.mix_oklab(sidebar_mix, 0.94));
        let secondary_sidebar = tokens
            .background
            .mix_oklab(secondary_mix, if is_dark { 0.965 } else { 0.97 });
        let headerbar = overrides
            .and_then(|o| o.headerbar)
            .unwrap_or(if is_dark { sidebar } else { view });
        let card = overrides
            .and_then(|o| o.card)
            .unwrap_or(if is_dark {
                view.mix_oklab(tokens.foreground, 0.92)
            } else {
                white
            });
        let dialog = overrides
            .and_then(|o| o.dialog)
            .unwrap_or(if is_dark {
                sidebar.mix_oklab(white, 0.98)
            } else {
                window
            });
        let popover = overrides
            .and_then(|o| o.popover)
            .unwrap_or(if is_dark {
                sidebar.mix_oklab(white, 0.98)
            } else {
                card
            });
        let pinned_banner = Hsla::from(gpui::rgb(0x7d7d83)).mix_oklab(window, 0.30);
        Self {
            window,
            view,
            sidebar,
            secondary_sidebar,
            headerbar,
            card,
            dialog,
            popover,
            window_backdrop: window,
            view_backdrop: view,
            sidebar_backdrop: if is_dark {
                tokens.background.mix_oklab(black, 0.98)
            } else {
                tokens.background.mix_oklab(white, 0.985)
            },
            secondary_sidebar_backdrop: if is_dark {
                tokens.background.mix_oklab(white, 0.985)
            } else {
                tokens.background.mix_oklab(white, 0.98)
            },
            headerbar_backdrop: if is_dark {
                tokens.background.mix_oklab(black, 0.98)
            } else {
                tokens.background.mix_oklab(white, 0.985)
            },
            card_backdrop: card,
            dialog_backdrop: dialog,
            popover_backdrop: popover,
            fg_muted: tokens.foreground.opacity(0.5),
            border: tokens.foreground.opacity(0.15),
            sidebar_border: if is_dark {
                black.opacity(0.36)
            } else {
                tokens.foreground.opacity(0.07)
            },
            card_shade: tokens.foreground.opacity(0.08),
            pinned_banner,
            scrim: black.opacity(0.50),
            scrim_default: Hsla::from(gpui::rgba(0x00000088)),
            selection_focused: tokens.accent.opacity(0.30),
            selection_unfocused: tokens.foreground.opacity(0.10),
            active_toggle_bg: if is_dark {
                view.mix_oklab(tokens.foreground, 0.80)
            } else {
                view.mix_oklab(white, 0.06)
            },
            is_dark,
        }
    }

    /// Foreground-tinted transparent state color (the Adwaita `tint(n)` rule).
    pub fn tint(tokens: &ActiveTokens, opacity: f32) -> Hsla {
        tokens.foreground.opacity(opacity.clamp(0.0, 1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::manager::ActiveTokens;
    use crate::theme::preset::find_preset;

    fn tokens(name: &str) -> ActiveTokens {
        ActiveTokens::from_preset(find_preset(name).expect("theme preset exists"))
    }

    #[test]
    fn derives_dark_and_light_surfaces_from_the_active_palette() {
        let dark_palette = tokens("default-dark");
        let light_palette = tokens("default-light");
        let dark = SurfaceTokens::from_tokens(&dark_palette, true, None);
        let light = SurfaceTokens::from_tokens(&light_palette, false, None);

        assert_eq!(dark.window, dark_palette.background);
        assert_ne!(dark.view, dark.window);
        assert_ne!(dark.sidebar, dark.window);
        assert_eq!(dark.fg_muted.a, 0.5);
        assert_eq!(dark.border.a, 0.15);
        assert_eq!(dark.border.a, 0.15);
        assert_eq!(dark.scrim.a, 0.5);
        let banner_gray = Hsla::from(gpui::rgb(0x7d7d83));
        assert_eq!(dark.pinned_banner, banner_gray.mix_oklab(dark.window, 0.30));
        assert_eq!(light.pinned_banner, banner_gray.mix_oklab(light.window, 0.30));
        assert_eq!(dark.view_backdrop, dark.view);
        assert_eq!(dark.card_backdrop, dark.card);
        assert_eq!(dark.dialog_backdrop, dark.dialog);
        assert_eq!(dark.popover_backdrop, dark.popover);
        assert_eq!(
            dark.secondary_sidebar_backdrop,
            dark_palette
                .background
                .mix_oklab(Hsla::from(gpui::rgb(0xffffff)), 0.985)
        );
        assert!(light.view.l > light_palette.background.l);
        assert!(light.sidebar.l < light_palette.background.l);
        assert_eq!(light.card, Hsla::from(gpui::rgb(0xffffff)));
    }

    #[test]
    fn explicit_gtk_surface_colors_override_only_their_own_tokens() {
        let palette = tokens("default-dark");
        let override_color = Hsla::parse_hex("#123456").unwrap();
        let surface = SurfaceTokens::from_tokens(
            &palette,
            true,
            Some(SurfaceOverrides {
                view: Some(override_color),
                ..Default::default()
            }),
        );

        assert_eq!(surface.view, override_color);
        assert_eq!(surface.window, palette.background);
        assert_eq!(surface.card, surface.view.mix_oklab(palette.foreground, 0.92));
    }
}

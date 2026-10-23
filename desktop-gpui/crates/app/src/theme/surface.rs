//! Adwaita surface and state tokens, derived from the active app palette.

use gpui::Hsla;
use gpui_component::theme::Colorize;

use super::manager::ActiveTokens;
use super::shape::AppearanceStyle;

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
    /// Active profile used to resolve all appearance roles in one place.
    pub style: AppearanceStyle,
    /// Effective fills used by the shell, window and common chat surfaces.
    pub window_fill: Hsla,
    pub window_fill_backdrop: Hsla,
    pub titlebar_background: Hsla,
    pub titlebar_backdrop: Hsla,
    pub titlebar_border: Hsla,
    pub titlebar_unfocused_opacity: f32,
    pub page_background: Hsla,
    pub chat_view: Hsla,
    pub chat_header: Hsla,
    pub chat_header_backdrop: Hsla,
    pub conversation_background: Hsla,
    pub composer_background: Hsla,
    pub chat_sidebar_background: Hsla,
    pub chat_sidebar_backdrop: Hsla,
    pub navigation_sidebar_background: Hsla,
    pub navigation_sidebar_backdrop: Hsla,
    pub chat_sidebar_border: Hsla,
    pub navigation_sidebar_border: Hsla,
    pub window: Hsla,
    pub view: Hsla,
    pub sidebar: Hsla,
    pub secondary_sidebar: Hsla,
    pub headerbar: Hsla,
    pub card: Hsla,
    pub dialog: Hsla,
    pub popover: Hsla,
    pub popover_frame_fill: Hsla,
    pub popover_frame_border: Hsla,
    pub menu_surface: Hsla,
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
    pub sheet_border: Hsla,
    pub menu_border: Hsla,
    pub sidebar_border: Hsla,
    pub card_shade: Hsla,
    pub pinned_banner: Hsla,
    pub scrim: Hsla,
    pub scrim_default: Hsla,
    pub modal_scrim: Hsla,
    pub selection_focused: Hsla,
    pub selection_unfocused: Hsla,
    pub active_toggle_bg: Hsla,
    pub attachment_hover: Hsla,
    pub attachment_separator: Hsla,
    pub control_hover: Hsla,
    pub row_hover: Hsla,
    pub row_active: Hsla,
    pub menu_hover: Hsla,
    pub menu_item_hover: Hsla,
    pub dialog_row_hover: Hsla,
    pub chat_header_hover: Hsla,
    pub navigation_hover: Hsla,
    pub navigation_selected_hover: Hsla,
    pub navigation_selected: Hsla,
    pub archived_row_hover: Hsla,
    pub chat_list_selected: Hsla,
    pub chat_list_selected_hover: Hsla,
    pub chat_list_hover: Hsla,
    pub composer_action_active: Hsla,
    pub composer_action_hover: Hsla,
    pub composer_field_fill: Hsla,
    pub composer_field_border: Hsla,
    pub pinned_banner_hover: Hsla,
    pub selected_call_row: Hsla,
    pub selected_call_row_hover: Hsla,
    pub call_row_hover: Hsla,
    pub status_view_background: Hsla,
    /// Fluent control tokens: 1px stroke, its brighter/darker bottom edge, resting and hover fills.
    pub control_stroke: Hsla,
    pub control_stroke_bottom: Hsla,
    pub control_fill: Hsla,
    pub control_fill_hover: Hsla,
    pub control_fill_pressed: Hsla,
    /// Border of a control's top edge (Fluent: brighter than the sides in dark).
    pub control_stroke_top: Hsla,
    /// Outline of unchecked toggles, checkboxes and radios.
    pub strong_stroke: Hsla,
    /// Fill of unchecked toggles, checkboxes and radios, and its hover/pressed.
    pub alt_fill: Hsla,
    pub alt_fill_hover: Hsla,
    pub alt_fill_pressed: Hsla,
    /// Slider thumb (ControlSolidFillColorDefault).
    pub thumb_fill: Hsla,
    pub card_stroke: Hsla,
    /// Text and accent roles, so Fluent can replace the palette's own.
    pub text_primary: Hsla,
    pub accent_fill: Hsla,
    pub text_on_accent: Hsla,
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
                view.mix_oklab(tokens.foreground, 0.82)
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
        let sidebar_backdrop = if is_dark {
            tokens.background.mix_oklab(black, 0.98)
        } else {
            tokens.background.mix_oklab(white, 0.985)
        };
        let secondary_sidebar_backdrop = if is_dark {
            tokens.background.mix_oklab(white, 0.985)
        } else {
            tokens.background.mix_oklab(white, 0.98)
        };
        let headerbar_backdrop = if is_dark {
            tokens.background.mix_oklab(black, 0.98)
        } else {
            tokens.background.mix_oklab(white, 0.985)
        };
        Self {
            style: AppearanceStyle::Adwaita,
            window_fill: window,
            window_fill_backdrop: window,
            titlebar_background: headerbar,
            titlebar_backdrop: headerbar_backdrop,
            titlebar_border: tokens.foreground.opacity(0.15),
            titlebar_unfocused_opacity: 0.5,
            page_background: view,
            chat_view: view,
            chat_header: headerbar,
            chat_header_backdrop: headerbar_backdrop,
            conversation_background: view,
            composer_background: window,
            chat_sidebar_background: secondary_sidebar,
            chat_sidebar_backdrop: secondary_sidebar_backdrop,
            navigation_sidebar_background: sidebar,
            navigation_sidebar_backdrop: sidebar_backdrop,
            chat_sidebar_border: if is_dark { black.opacity(0.36) } else { tokens.foreground.opacity(0.07) },
            navigation_sidebar_border: if is_dark { black.opacity(0.36) } else { tokens.foreground.opacity(0.07) },
            window,
            view,
            sidebar,
            secondary_sidebar,
            headerbar,
            card,
            dialog,
            popover,
            popover_frame_fill: popover,
            popover_frame_border: tokens.foreground.opacity(0.15),
            menu_surface: popover,
            window_backdrop: window,
            view_backdrop: view,
            sidebar_backdrop,
            secondary_sidebar_backdrop,
            headerbar_backdrop,
            card_backdrop: card,
            dialog_backdrop: dialog,
            popover_backdrop: popover,
            fg_muted: tokens.foreground.opacity(0.5),
            border: tokens.foreground.opacity(0.15),
            sheet_border: tokens.foreground.opacity(0.15),
            menu_border: tokens.foreground.opacity(0.15),
            sidebar_border: if is_dark {
                black.opacity(0.36)
            } else {
                tokens.foreground.opacity(0.07)
            },
            card_shade: tokens.foreground.opacity(0.08),
            pinned_banner,
            scrim: black.opacity(0.50),
            scrim_default: Hsla::from(gpui::rgba(0x00000088)),
            modal_scrim: black.opacity(0.50),
            selection_focused: tokens.accent.opacity(0.30),
            selection_unfocused: tokens.foreground.opacity(0.10),
            active_toggle_bg: if is_dark {
                view.mix_oklab(tokens.foreground, 0.80)
            } else {
                view.mix_oklab(white, 0.06)
            },
            attachment_hover: tokens.foreground.opacity(0.05),
            attachment_separator: tokens.foreground.opacity(0.10),
            control_hover: tokens.foreground.opacity(0.15),
            row_hover: tokens.foreground.opacity(0.03),
            row_active: tokens.foreground.opacity(0.08),
            menu_hover: tokens.foreground.opacity(0.15),
            menu_item_hover: tokens.foreground.opacity(0.15),
            dialog_row_hover: tokens.foreground.opacity(0.03),
            chat_header_hover: tokens.foreground.opacity(0.15),
            navigation_hover: tokens.foreground.opacity(0.05),
            navigation_selected_hover: tokens.foreground.opacity(0.15),
            navigation_selected: tokens.foreground.opacity(0.10),
            archived_row_hover: tokens.foreground.opacity(0.05),
            chat_list_selected: tokens.foreground.opacity(0.10),
            chat_list_selected_hover: tokens.foreground.opacity(0.08),
            chat_list_hover: tokens.foreground.opacity(0.03),
            composer_action_active: tokens.foreground.opacity(0.10),
            composer_action_hover: tokens.foreground.opacity(0.15),
            composer_field_fill: tokens.foreground.opacity(0.10),
            composer_field_border: Hsla::transparent_black(),
            pinned_banner_hover: pinned_banner,
            selected_call_row: tokens.foreground.opacity(0.08),
            selected_call_row_hover: tokens.foreground.opacity(0.08),
            call_row_hover: tokens.foreground.opacity(0.03),
            status_view_background: view,
            control_stroke: tokens.foreground.opacity(0.15),
            control_stroke_bottom: tokens.foreground.opacity(0.15),
            control_fill: tokens.foreground.opacity(0.10),
            control_fill_hover: tokens.foreground.opacity(0.15),
            control_fill_pressed: tokens.foreground.opacity(0.20),
            control_stroke_top: tokens.foreground.opacity(0.15),
            strong_stroke: tokens.foreground.opacity(0.50),
            alt_fill: tokens.foreground.opacity(0.10),
            alt_fill_hover: tokens.foreground.opacity(0.15),
            alt_fill_pressed: tokens.foreground.opacity(0.20),
            thumb_fill: Hsla::from(gpui::rgb(0xffffff)),
            card_stroke: tokens.foreground.opacity(0.15),
            text_primary: tokens.foreground,
            accent_fill: tokens.primary,
            text_on_accent: tokens.primary_foreground,
            is_dark,
        }
    }

    pub fn for_style(
        tokens: &ActiveTokens,
        is_dark: bool,
        overrides: Option<SurfaceOverrides>,
        style: AppearanceStyle,
        window_transparency: u8,
    ) -> Self {
        let mut surface = Self::from_tokens(tokens, is_dark, overrides);
        surface.style = style;
        if style == AppearanceStyle::Default {
            let transparent_fill = |color: Hsla| color.opacity(1.0 - window_transparency.min(100) as f32 / 100.0);
            let background = tokens.background;
            surface.window = background;
            surface.window_fill = transparent_fill(background);
            surface.window_fill_backdrop = transparent_fill(background);
            surface.titlebar_background = transparent_fill(background);
            surface.titlebar_backdrop = surface.titlebar_background;
            surface.titlebar_border = tokens.border.opacity(0.4);
            surface.titlebar_unfocused_opacity = 0.7;
            surface.view = background;
            surface.page_background = transparent_fill(background);
            surface.chat_view = tokens.chat.map(|chat| chat.chat_bg).unwrap_or(background);
            surface.chat_header = tokens.chat.map(|chat| chat.header).unwrap_or(background)
                .opacity(1.0 - window_transparency.min(100) as f32 / 100.0);
            surface.chat_header_backdrop = surface.chat_header;
            surface.conversation_background = tokens.chat.map(|chat| chat.chat_bg).unwrap_or(background)
                .opacity(1.0 - window_transparency.min(100) as f32 / 100.0);
            surface.composer_background = Hsla::transparent_black();
            surface.sidebar = transparent_fill(background);
            surface.secondary_sidebar = transparent_fill(background);
            surface.chat_sidebar_background = transparent_fill(background);
            surface.chat_sidebar_backdrop = transparent_fill(background);
            surface.navigation_sidebar_background = transparent_fill(background);
            surface.navigation_sidebar_backdrop = transparent_fill(background);
            surface.headerbar = tokens.muted.opacity(0.2);
            surface.card = tokens.card;
            surface.dialog = tokens.card;
            surface.popover = tokens.card;
            surface.menu_surface = tokens.card;
            surface.popover_frame_fill = transparent_fill(background);
            surface.popover_frame_border = tokens.border;
            surface.window_backdrop = background;
            surface.view_backdrop = background;
            surface.sidebar_backdrop = background;
            surface.secondary_sidebar_backdrop = background;
            surface.headerbar_backdrop = tokens.muted.opacity(0.2);
            surface.card_backdrop = tokens.card;
            surface.dialog_backdrop = tokens.card;
            surface.popover_backdrop = tokens.card;
            surface.fg_muted = tokens.muted_foreground;
            surface.border = tokens.border;
            surface.sheet_border = tokens.border.opacity(0.4);
            surface.menu_border = tokens.border.opacity(0.35);
            surface.sidebar_border = tokens.border;
            surface.chat_sidebar_border = tokens.border.opacity(0.4);
            surface.navigation_sidebar_border = tokens.border.opacity(0.4);
            surface.card_shade = tokens.primary.opacity(0.12);
            surface.scrim = surface.scrim_default;
            surface.modal_scrim = Hsla::from(gpui::rgb(0x0a0a0a)).opacity(0.7);
            surface.pinned_banner = tokens.card.opacity(0.9);
            surface.pinned_banner_hover = tokens.muted.opacity(0.5);
            surface.composer_action_active = tokens.primary.opacity(0.15);
            surface.composer_action_hover = tokens.muted.opacity(0.65);
            surface.composer_field_fill = tokens.muted.opacity(0.5);
            surface.composer_field_border = tokens.border.opacity(0.2);
            surface.selected_call_row = tokens.primary.opacity(0.14);
            surface.selected_call_row_hover = tokens.primary.opacity(0.2);
            surface.call_row_hover = tokens.muted.opacity(0.4);
            surface.status_view_background = tokens.muted.opacity(0.1);
            surface.selection_focused = tokens.accent.opacity(0.30);
            surface.selection_unfocused = tokens.foreground.opacity(0.10);
            surface.active_toggle_bg = tokens.muted.opacity(0.9);
            surface.attachment_hover = surface.active_toggle_bg;
            surface.attachment_separator = tokens.muted.opacity(0.45);
            surface.control_hover = tokens.muted;
            surface.row_hover = tokens.muted.opacity(0.5);
            surface.row_active = tokens.muted.opacity(0.5);
            surface.menu_hover = tokens.foreground.opacity(0.12);
            surface.menu_item_hover = tokens.muted.opacity(0.6);
            surface.dialog_row_hover = tokens.muted.opacity(0.4);
            surface.chat_header_hover = tokens.muted.opacity(0.65);
            surface.navigation_hover = tokens.muted.opacity(0.8);
            surface.navigation_selected_hover = tokens.primary.opacity(0.85);
            surface.navigation_selected = tokens.primary;
            surface.archived_row_hover = tokens.muted.opacity(0.4);
            surface.chat_list_selected = tokens.chat.map(|chat| chat.list_selected).unwrap_or(tokens.primary.opacity(0.14));
            surface.chat_list_selected_hover = tokens.chat.map(|chat| chat.list_selected).unwrap_or(tokens.primary.opacity(0.22));
            surface.chat_list_hover = tokens.chat.map(|chat| chat.list_hover).unwrap_or(tokens.muted.opacity(0.65));
        } else if style == AppearanceStyle::WinUi {
            surface.apply_fluent(tokens, is_dark, window_transparency);
        }
        surface
    }

    /// Windows 11 layers with the exact values of WinUI's theme resources
    /// (microsoft-ui-xaml `Common_themeresources_any.xaml`): neutral Mica base,
    /// content layer, card, flyout, strokes and the subtle-fill ramp. Only the
    /// accent comes from the active palette.
    fn apply_fluent(&mut self, tokens: &ActiveTokens, is_dark: bool, window_transparency: u8) {
        let alpha = 1.0 - window_transparency.min(100) as f32 / 100.0;
        let hex = |value: u32| Hsla::from(gpui::rgb(value));
        let white = hex(0xffffff);
        let black = hex(0x000000);
        // Ink is the color translucent fills are made of: white on dark, black on light.
        let ink = if is_dark { white } else { black };

        // SolidBackgroundFillColorBase / Secondary / Quarternary and the layer fill
        // (#3A3A3A at 30% over base on dark, white at 50% on light) flattened.
        let (base, _secondary, view, flyout, dialog) = if is_dark {
            (hex(0x202020), hex(0x1c1c1c), hex(0x282828), hex(0x2c2c2c), hex(0x202020))
        } else {
            (hex(0xf3f3f3), hex(0xeeeeee), hex(0xf9f9f9), hex(0xfcfcfc), hex(0xf3f3f3))
        };
        let text_primary = if is_dark { white } else { black.opacity(0.8956) };
        let text_secondary = if is_dark { white.opacity(0.7725) } else { black.opacity(0.6196) };
        // Subtle fill secondary / tertiary: hover and pressed of list items and menus.
        let (hover, pressed) = if is_dark { (0.0588, 0.0392) } else { (0.0353, 0.0235) };
        let selected = hover;
        let stroke = if is_dark { white.opacity(0.0706) } else { black.opacity(0.0588) };
        let divider = if is_dark { white.opacity(0.0824) } else { black.opacity(0.0588) };
        let card_stroke = if is_dark { black.opacity(0.10) } else { black.opacity(0.0588) };
        let card_fill = if is_dark { white.opacity(0.0512) } else { white.opacity(0.70) };
        let flyout_stroke = if is_dark { black.opacity(0.20) } else { black.opacity(0.0588) };
        let dialog_stroke = hex(0x757575).opacity(0.40);

        // Accent: WinUI uses the lighter accent shade on dark and the darker on light,
        // with black text on the light one.
        let primary = tokens.primary;
        let accent_fill = if is_dark { primary.mix_oklab(white, 0.70) } else { primary.mix_oklab(black, 0.90) };
        let accent_luma = {
            let rgb = accent_fill.to_rgb();
            0.2126 * rgb.r + 0.7152 * rgb.g + 0.0722 * rgb.b
        };
        let text_on_accent = if accent_luma > 0.55 { black } else { white };

        self.window = base.opacity(alpha);
        self.window_fill = self.window;
        self.window_backdrop = base;
        self.window_fill_backdrop = base.opacity(alpha);
        self.view = view.opacity(alpha);
        self.view_backdrop = view;
        self.page_background = self.view;
        self.chat_view = self.view;
        self.conversation_background = self.view;
        self.status_view_background = self.view;
        // The navigation pane, title bar and secondary panes are the base layer: the
        // same Mica color, with the content layer lifted above them.
        self.sidebar = base.opacity(alpha);
        self.sidebar_backdrop = base.opacity(alpha);
        // Secondary panes (the chat list) live in the content layer, split by a divider.
        self.secondary_sidebar = view.opacity(alpha);
        self.secondary_sidebar_backdrop = view.opacity(alpha);
        self.navigation_sidebar_background = self.sidebar;
        self.navigation_sidebar_backdrop = self.sidebar_backdrop;
        self.chat_sidebar_background = self.secondary_sidebar;
        self.chat_sidebar_backdrop = self.secondary_sidebar_backdrop;
        self.headerbar = base.opacity(alpha);
        self.headerbar_backdrop = base.opacity(alpha);
        self.titlebar_background = self.headerbar;
        self.titlebar_backdrop = self.headerbar_backdrop;
        self.titlebar_border = Hsla::transparent_black();
        self.titlebar_unfocused_opacity = 0.6;
        self.chat_header = self.view;
        self.chat_header_backdrop = self.view;
        self.composer_background = self.view;
        self.card = card_fill;
        self.card_backdrop = card_fill;
        self.dialog = dialog;
        self.dialog_backdrop = dialog;
        self.popover = flyout;
        self.popover_backdrop = flyout;
        self.menu_surface = flyout;
        self.popover_frame_fill = flyout;
        self.fg_muted = text_secondary;
        self.border = stroke;
        self.sheet_border = dialog_stroke;
        self.menu_border = flyout_stroke;
        self.popover_frame_border = flyout_stroke;
        self.sidebar_border = Hsla::transparent_black();
        self.chat_sidebar_border = divider;
        self.navigation_sidebar_border = Hsla::transparent_black();
        self.card_stroke = card_stroke;
        self.card_shade = divider;
        self.pinned_banner = flyout;
        self.pinned_banner_hover = ink.opacity(hover);
        self.scrim = black.opacity(0.30);
        self.modal_scrim = black.opacity(0.30);
        self.selection_focused = accent_fill.opacity(0.40);
        self.selection_unfocused = ink.opacity(0.10);
        self.active_toggle_bg = ink.opacity(selected);
        self.attachment_hover = ink.opacity(hover);
        self.attachment_separator = divider;
        self.control_hover = ink.opacity(hover);
        self.row_hover = ink.opacity(hover);
        self.row_active = ink.opacity(selected);
        self.menu_hover = ink.opacity(hover);
        self.menu_item_hover = ink.opacity(hover);
        self.dialog_row_hover = ink.opacity(hover);
        self.chat_header_hover = ink.opacity(hover);
        self.navigation_hover = ink.opacity(hover);
        self.navigation_selected = ink.opacity(selected);
        self.navigation_selected_hover = ink.opacity(pressed);
        self.archived_row_hover = ink.opacity(hover);
        self.chat_list_hover = ink.opacity(hover);
        self.chat_list_selected = ink.opacity(selected);
        self.chat_list_selected_hover = ink.opacity(pressed);
        self.composer_action_active = ink.opacity(pressed);
        self.composer_action_hover = ink.opacity(hover);
        let (control_fill, control_hover, control_pressed) = if is_dark {
            (white.opacity(0.0588), white.opacity(0.0824), white.opacity(0.0314))
        } else {
            (white.opacity(0.70), hex(0xf9f9f9).opacity(0.50), hex(0xf9f9f9).opacity(0.30))
        };
        self.composer_field_fill = control_fill;
        self.composer_field_border = stroke;
        self.selected_call_row = ink.opacity(selected);
        self.selected_call_row_hover = ink.opacity(pressed);
        self.call_row_hover = ink.opacity(hover);
        self.control_stroke = stroke;
        self.control_stroke_top = if is_dark { white.opacity(0.0941) } else { stroke };
        self.control_stroke_bottom = if is_dark { stroke } else { black.opacity(0.16) };
        self.control_fill = control_fill;
        self.control_fill_hover = control_hover;
        self.control_fill_pressed = control_pressed;
        self.strong_stroke = if is_dark { white.opacity(0.5451) } else { black.opacity(0.4458) };
        // ControlAltFillColor Secondary / Tertiary / Quarternary.
        self.alt_fill = if is_dark { black.opacity(0.10) } else { black.opacity(0.0235) };
        self.alt_fill_hover = if is_dark { white.opacity(0.0431) } else { black.opacity(0.0588) };
        self.alt_fill_pressed = if is_dark { white.opacity(0.0706) } else { black.opacity(0.0941) };
        self.thumb_fill = if is_dark { hex(0x454545) } else { white };
        self.text_primary = text_primary;
        self.accent_fill = accent_fill;
        self.text_on_accent = text_on_accent;
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
    fn resolves_semantic_surface_roles_for_each_appearance_profile() {
        let palette = tokens("default-dark");
        let default = SurfaceTokens::for_style(&palette, true, None, AppearanceStyle::Default, 28);
        let fluent = SurfaceTokens::for_style(&palette, true, None, AppearanceStyle::WinUi, 28);

        assert_eq!(default.view, palette.background);
        assert_eq!(default.headerbar, palette.muted.opacity(0.2));
        assert_eq!(default.sheet_border, palette.border.opacity(0.4));
        assert_eq!(default.window_fill.a, 0.72);
        assert_eq!(fluent.style, AppearanceStyle::WinUi);
        assert_eq!(fluent.titlebar_background, fluent.headerbar);
        assert!(fluent.view.a < 1.0);
        // Navigation pane, title bar and window share the base layer, which is
        // neutral: palette hue does not leak into the Fluent surfaces.
        assert_eq!(fluent.sidebar.h, fluent.window.h);
        assert_eq!(fluent.window.s, 0.0);
        assert_ne!(fluent.view, fluent.window);
        // Cards are a translucent overlay on the layer, not an opaque palette color.
        assert!(fluent.card.a < 0.2);
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
        assert_eq!(surface.card, surface.view.mix_oklab(palette.foreground, 0.82));
    }
}

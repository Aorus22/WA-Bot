//! Theme manager managing active preset, mode, and GPUI runtime synchronization.

use gpui::{App, BorrowAppContext, Global, Hsla, Window};
use gpui_component::theme::Colorize;
use gpui_component::{Theme, ThemeMode as GpuiThemeMode};
use wabot_settings::{
    DesktopSettings, MAX_FONT_SCALE, MAX_WINDOW_TRANSPARENCY, MIN_FONT_SCALE,
    ThemeMode as SettingsThemeMode,
};

use super::color::parse_hex_hsla;
use super::gtk::{self, GTK_PRESET_NAME};
use super::preset::{default_preset, find_preset, ThemePreset};
use super::shape::{AppearanceStyle, ShapeTokens};
use super::surface::SurfaceTokens;

/// Strongly typed token values for direct GPUI styling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActiveTokens {
    pub background: Hsla,
    pub foreground: Hsla,
    pub card: Hsla,
    pub card_foreground: Hsla,
    pub primary: Hsla,
    pub primary_foreground: Hsla,
    pub secondary: Hsla,
    pub secondary_foreground: Hsla,
    pub muted: Hsla,
    pub muted_foreground: Hsla,
    pub accent: Hsla,
    pub accent_foreground: Hsla,
    pub destructive: Hsla,
    pub destructive_foreground: Hsla,
    pub border: Hsla,
    pub input: Hsla,
    pub ring: Hsla,
    /// Chat-specific colours of presets that restyle the conversation.
    pub chat: Option<ChatTokens>,
}

/// Parsed [`super::preset::ThemeExtras`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChatTokens {
    pub chat_bg: Hsla,
    pub header: Hsla,
    pub bubble_out: Hsla,
    pub bubble_out_text: Hsla,
    pub bubble_in: Hsla,
    pub bubble_in_text: Hsla,
    pub list_selected: Hsla,
    pub list_hover: Hsla,
}

impl ActiveTokens {
    pub fn from_preset(preset: &ThemePreset) -> Self {
        let c = &preset.colors;
        Self {
            background: parse_hex_hsla(&c.background),
            foreground: parse_hex_hsla(&c.foreground),
            card: parse_hex_hsla(&c.card),
            card_foreground: parse_hex_hsla(&c.card_foreground),
            primary: parse_hex_hsla(&c.primary),
            primary_foreground: parse_hex_hsla(&c.primary_foreground),
            secondary: parse_hex_hsla(&c.secondary),
            secondary_foreground: parse_hex_hsla(&c.secondary_foreground),
            muted: parse_hex_hsla(&c.muted),
            muted_foreground: parse_hex_hsla(&c.muted_foreground),
            accent: parse_hex_hsla(&c.accent),
            accent_foreground: parse_hex_hsla(&c.accent_foreground),
            destructive: parse_hex_hsla(&c.destructive),
            destructive_foreground: parse_hex_hsla(&c.destructive_foreground),
            border: parse_hex_hsla(&c.border),
            input: parse_hex_hsla(&c.input),
            ring: parse_hex_hsla(&c.ring),
            chat: preset.extras.as_ref().map(|e| ChatTokens {
                chat_bg: parse_hex_hsla(&e.chat_bg),
                header: parse_hex_hsla(&e.header),
                bubble_out: parse_hex_hsla(&e.bubble_out),
                bubble_out_text: parse_hex_hsla(&e.bubble_out_text),
                bubble_in: parse_hex_hsla(&e.bubble_in),
                bubble_in_text: parse_hex_hsla(&e.bubble_in_text),
                list_selected: parse_hex_hsla(&e.list_selected),
                list_hover: parse_hex_hsla(&e.list_hover),
            }),
        }
    }
}

fn effective_tokens(palette: ActiveTokens, surface: SurfaceTokens, styled: bool) -> ActiveTokens {
    if !styled {
        return palette;
    }

    let fluent = surface.style == AppearanceStyle::WinUi;
    ActiveTokens {
        background: surface.view,
        card: surface.card,
        muted: surface.secondary_sidebar,
        muted_foreground: surface.fg_muted,
        border: surface.border,
        input: surface.window.mix_oklab(palette.foreground, 0.90),
        foreground: if fluent { surface.text_primary } else { palette.foreground },
        primary: if fluent { surface.accent_fill } else { palette.primary },
        primary_foreground: if fluent { surface.text_on_accent } else { palette.primary_foreground },
        accent: if fluent { surface.accent_fill } else { palette.accent },
        accent_foreground: if fluent { surface.text_on_accent } else { palette.accent_foreground },
        ring: if fluent { surface.accent_fill } else { palette.ring },
        ..palette
    }
}

/// Global theme state manager.
#[derive(Debug, Clone)]
pub struct ThemeManager {
    pub active_preset: ThemePreset,
    pub mode: SettingsThemeMode,
    pub window_transparency: u8,
    pub window_blur_enabled: bool,
    pub gtk_style: bool,
    pub winui_style: bool,
    pub shape: ShapeTokens,
    pub surface: SurfaceTokens,
    /// The active preset's font family before applying an appearance style.
    original_font_family: Option<gpui::SharedString>,
    /// Interface text scale in percent (drives the window's rem size).
    pub ui_font_scale: u8,
    /// Extra scale in percent for text inside message bubbles.
    pub chat_font_scale: u8,
    /// Effective colors exposed to views; Adwaita surface roles are substituted
    /// while the style is on, while the original palette remains below.
    pub tokens: ActiveTokens,
    palette_tokens: ActiveTokens,
    /// The desktop (GTK) palette, when the user's GTK config provides one.
    pub gtk_preset: Option<ThemePreset>,
    /// Last seen state of the GTK config files (live-update detector).
    pub gtk_key: gtk::ThemeKey,
}

impl Global for ThemeManager {}

impl ThemeManager {
    /// Initialize with values read from DesktopSettings.
    pub fn init_from_settings(settings: &DesktopSettings) -> Self {
        let gtk_preset = gtk::preset();
        let preset = if settings.theme_preset == GTK_PRESET_NAME {
            gtk_preset.clone()
        } else {
            find_preset(&settings.theme_preset).cloned()
        }
        .unwrap_or_else(|| default_preset().clone());
        let palette_tokens = ActiveTokens::from_preset(&preset);
        let tokens = palette_tokens;
        let is_dark = match settings.theme_mode {
            SettingsThemeMode::Dark => true,
            SettingsThemeMode::Light => false,
            SettingsThemeMode::System | SettingsThemeMode::Unknown => preset.is_dark(),
        };
        // WinUI wins if an older or manually edited settings file enables both.
        let winui_style = settings.winui_style;
        let gtk_style = settings.gtk_style && !winui_style;
        let style = if winui_style {
            AppearanceStyle::WinUi
        } else if gtk_style {
            AppearanceStyle::Adwaita
        } else {
            AppearanceStyle::Default
        };
        let surface = SurfaceTokens::for_style(
            &tokens,
            is_dark,
            (preset.name == GTK_PRESET_NAME)
                .then(gtk::surface_overrides)
                .flatten(),
            style,
            settings.window_transparency.min(MAX_WINDOW_TRANSPARENCY),
        );

        Self {
            active_preset: preset,
            mode: settings.theme_mode,
            window_transparency: settings.window_transparency.min(MAX_WINDOW_TRANSPARENCY),
            window_blur_enabled: settings.window_blur_enabled,
            gtk_style,
            winui_style,
            shape: ShapeTokens::for_style(style),
            surface,
            original_font_family: None,
            ui_font_scale: settings.ui_font_scale.clamp(MIN_FONT_SCALE, MAX_FONT_SCALE),
            chat_font_scale: settings.chat_font_scale.clamp(MIN_FONT_SCALE, MAX_FONT_SCALE),
            tokens,
            palette_tokens,
            gtk_preset,
            gtk_key: gtk::current_key(),
        }
    }

    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }

    pub fn is_dark(&self) -> bool {
        match self.mode {
            SettingsThemeMode::Dark => true,
            SettingsThemeMode::Light => false,
            SettingsThemeMode::System | SettingsThemeMode::Unknown => self.active_preset.is_dark(),
        }
    }

    /// Change active theme preset by name and synchronize with gpui-component & settings.
    pub fn change_preset(&mut self, name: &str, window: Option<&mut Window>, cx: &mut App) {
        let resolved = if name == GTK_PRESET_NAME {
            self.gtk_preset = gtk::preset();
            self.gtk_key = gtk::current_key();
            self.gtk_preset.clone()
        } else {
            find_preset(name).cloned()
        };
        if let Some(preset) = resolved {
            self.palette_tokens = ActiveTokens::from_preset(&preset);
            self.tokens = self.palette_tokens;
            self.active_preset = preset;
            self.refresh_surfaces();

            // Synchronize with gpui-component theme mode
            let gpui_mode = if self.is_dark() {
                GpuiThemeMode::Dark
            } else {
                GpuiThemeMode::Light
            };
            Theme::change(gpui_mode, window, cx);
            if self.gtk_style || self.winui_style {
                self.original_font_family = Some(Theme::global(cx).font_family.clone());
            }
            self.sync_font_size(cx);

            // Persist to DesktopSettings
            if let Ok(mut settings) = DesktopSettings::load() {
                settings.theme_preset = name.to_string();
                let _ = settings.save();
            }

            cx.refresh_windows();
        }
    }

    /// Change theme mode (System, Light, Dark) and persist.
    pub fn change_mode(&mut self, mode: SettingsThemeMode, window: Option<&mut Window>, cx: &mut App) {
        self.mode = mode;

        let gpui_mode = if self.is_dark() {
            GpuiThemeMode::Dark
        } else {
            GpuiThemeMode::Light
        };
        Theme::change(gpui_mode, window, cx);
        self.refresh_surfaces();
        if self.gtk_style || self.winui_style {
            self.original_font_family = Some(Theme::global(cx).font_family.clone());
        }
        self.sync_font_size(cx);

        if let Ok(mut settings) = DesktopSettings::load() {
            settings.theme_mode = mode;
            let _ = settings.save();
        }

        cx.refresh_windows();
    }

    /// Push the interface scale into gpui-component's theme, whose `font_size`
    /// becomes the window's rem size (16px at 100%). Loading a theme resets it,
    /// so this runs after every `Theme::change`.
    pub fn sync_font_size(&mut self, cx: &mut App) {
        let preferences = self.gtk_style.then(gtk::preferences);
        let gtk_font = preferences.as_ref().and_then(|prefs| prefs.font.as_ref());
        let styled = self.gtk_style || self.winui_style;
        if styled && self.original_font_family.is_none() {
            self.original_font_family = Some(Theme::global(cx).font_family.clone());
        }
        let font_family = if self.gtk_style {
            gtk_font.map(|font| gpui::SharedString::from(font.family.as_str()))
        } else if self.winui_style {
            let family = if cfg!(target_os = "windows") { "Segoe UI Variable" } else { "Selawik" };
            Some(gpui::SharedString::from(family))
        } else {
            None
        };
        let base_font_px = gtk_font
            .map(|font| font.size_points * (4.0 / 3.0))
            .unwrap_or(16.0);
        let gtk_style = self.gtk_style;
        let winui_style = self.winui_style;
        let font_size = gpui::px(base_font_px * self.ui_font_scale as f32 / 100.0);
        let radius = gpui::px(self.shape.widget_radius);
        let radius_lg = gpui::px(self.shape.widget_radius_lg);
        let scrollbar_width = gpui::px(self.shape.scrollbar_width);
        let shadow = self.shape.shadows;
        self.tokens = effective_tokens(self.palette_tokens, self.surface, styled);
        let active_tokens = self.tokens;
        let surface = self.surface;

        Theme::update(cx, |theme| {
            theme.adwaita = gtk_style;
            theme.winui = winui_style;
            theme.winui_colors = gpui_component::theme::WinUiColors {
                strong_stroke: surface.strong_stroke,
                alt_fill: surface.alt_fill,
                alt_fill_hover: surface.alt_fill_hover,
                alt_fill_pressed: surface.alt_fill_pressed,
                thumb_fill: surface.thumb_fill,
                control_fill: surface.control_fill,
                control_fill_hover: surface.control_fill_hover,
                control_fill_pressed: surface.control_fill_pressed,
                control_stroke_top: surface.control_stroke_top,
                control_stroke: surface.control_stroke,
                control_stroke_bottom: surface.control_stroke_bottom,
            };
            theme.font_size = font_size;
            // The shape tokens also drive the stock widgets, which read these.
            theme.radius = radius;
            theme.radius_lg = radius_lg;
            theme.shadow = shadow;
            theme.adwaita_dialog_bg = styled.then_some(surface.dialog);
            theme.adwaita_scrollbar_width = styled.then_some(scrollbar_width);
            if styled {
                let foreground = active_tokens.foreground;
                theme.background = surface.view;
                theme.foreground = foreground;
                theme.primary = active_tokens.primary;
                theme.primary_foreground = active_tokens.primary_foreground;
                theme.secondary = surface.secondary_sidebar;
                theme.secondary_foreground = foreground;
                theme.muted = surface.secondary_sidebar;
                theme.muted_foreground = surface.fg_muted;
                theme.accent = active_tokens.accent;
                theme.accent_foreground = active_tokens.accent_foreground;
                theme.border = surface.border;
                theme.input = surface
                    .window
                    .mix_oklab(foreground, 0.90);
                theme.ring = active_tokens.ring;
                theme.danger = active_tokens.destructive;
                theme.danger_foreground = active_tokens.destructive_foreground;
                theme.popover = surface.popover;
                theme.popover_foreground = foreground;
                theme.selection = surface.selection_focused;
                theme.sidebar = surface.sidebar;
                theme.sidebar_foreground = foreground;
                theme.sidebar_border = surface.sidebar_border;
                theme.sidebar_accent = SurfaceTokens::tint(&active_tokens, 0.10);
                theme.sidebar_accent_foreground = foreground;
                theme.sidebar_primary = active_tokens.primary;
                theme.sidebar_primary_foreground = active_tokens.primary_foreground;
                theme.title_bar = surface.headerbar;
                theme.title_bar_border = surface.border;
                theme.colors.list = surface.secondary_sidebar;
                theme.list_active = surface.selection_unfocused;
                theme.list_active_border = Hsla::transparent_black();
                theme.list_hover = SurfaceTokens::tint(&active_tokens, 0.03);
                theme.scrollbar = Hsla::transparent_black();
                theme.scrollbar_thumb = foreground.opacity(0.20);
                theme.scrollbar_thumb_hover = foreground.opacity(0.40);
                theme.slider_bar = active_tokens.accent;
            }
            if let Some(font_family) = font_family {
                theme.font_family = font_family;
            }
        });
        let reduce_motion = gtk_style
            && !preferences
                .and_then(|prefs| prefs.enable_animations)
                .unwrap_or(true);
        self.shape.transition_ms = if self.winui_style {
            167
        } else if gtk_style && !reduce_motion {
            200
        } else {
            0
        };
        cx.set_reduce_motion(reduce_motion);
    }

    fn change_ui_font_scale(&mut self, value: u8, persist: bool, cx: &mut App) {
        let value = value.clamp(MIN_FONT_SCALE, MAX_FONT_SCALE);
        let changed = self.ui_font_scale != value;
        self.ui_font_scale = value;

        if persist {
            if let Ok(mut settings) = DesktopSettings::load() {
                settings.ui_font_scale = value;
                let _ = settings.save();
            }
        }

        if changed {
            self.sync_font_size(cx);
            cx.refresh_windows();
        }
    }

    fn change_chat_font_scale(&mut self, value: u8, persist: bool, cx: &mut App) {
        let value = value.clamp(MIN_FONT_SCALE, MAX_FONT_SCALE);
        let changed = self.chat_font_scale != value;
        self.chat_font_scale = value;

        if persist {
            if let Ok(mut settings) = DesktopSettings::load() {
                settings.chat_font_scale = value;
                let _ = settings.save();
            }
        }

        if changed {
            cx.refresh_windows();
        }
    }

    pub fn preview_ui_font_scale(value: u8, cx: &mut App) {
        cx.update_global::<Self, _>(|this, cx| this.change_ui_font_scale(value, false, cx));
    }

    pub fn apply_ui_font_scale(value: u8, cx: &mut App) {
        cx.update_global::<Self, _>(|this, cx| this.change_ui_font_scale(value, true, cx));
    }

    pub fn preview_chat_font_scale(value: u8, cx: &mut App) {
        cx.update_global::<Self, _>(|this, cx| this.change_chat_font_scale(value, false, cx));
    }

    pub fn apply_chat_font_scale(value: u8, cx: &mut App) {
        cx.update_global::<Self, _>(|this, cx| this.change_chat_font_scale(value, true, cx));
    }

    fn change_window_transparency(&mut self, value: u8, persist: bool, cx: &mut App) {
        let value = value.min(MAX_WINDOW_TRANSPARENCY);
        let changed = self.window_transparency != value;
        self.window_transparency = value;
        if changed {
            self.refresh_surfaces();
        }

        if persist {
            if let Ok(mut settings) = DesktopSettings::load() {
                settings.window_transparency = value;
                let _ = settings.save();
            }
        }

        if changed {
            cx.refresh_windows();
        }
    }

    pub fn preview_window_transparency(value: u8, cx: &mut App) {
        cx.update_global::<Self, _>(|this, cx| {
            this.change_window_transparency(value, false, cx);
        });
    }

    pub fn apply_window_transparency(value: u8, cx: &mut App) {
        cx.update_global::<Self, _>(|this, cx| {
            this.change_window_transparency(value, true, cx);
        });
    }

    pub fn change_window_blur_enabled(&mut self, enabled: bool, cx: &mut App) {
        if self.window_blur_enabled == enabled {
            return;
        }
        self.window_blur_enabled = enabled;

        if let Ok(mut settings) = DesktopSettings::load() {
            settings.window_blur_enabled = enabled;
            let _ = settings.save();
        }

        cx.refresh_windows();
    }

    pub fn appearance_style(&self) -> AppearanceStyle {
        if self.winui_style {
            AppearanceStyle::WinUi
        } else if self.gtk_style {
            AppearanceStyle::Adwaita
        } else {
            AppearanceStyle::Default
        }
    }

    pub fn effective_window_blur_enabled(&self) -> bool {
        self.window_blur_enabled || self.winui_style
    }

    pub fn change_appearance_style(&mut self, style: AppearanceStyle, cx: &mut App) {
        let previous = self.appearance_style();
        if previous == style {
            return;
        }

        // Restore the unstyled font while switching profiles, then apply the
        // next profile's font in sync_font_size.
        if previous != AppearanceStyle::Default {
            if let Some(font_family) = self.original_font_family.clone() {
                Theme::update(cx, |theme| theme.font_family = font_family);
            }
        }
        if style == AppearanceStyle::Default && previous != AppearanceStyle::Default {
            let mode = if self.is_dark() {
                GpuiThemeMode::Dark
            } else {
                GpuiThemeMode::Light
            };
            Theme::change(mode, None, cx);
            if let Some(font_family) = self.original_font_family.take() {
                Theme::update(cx, |theme| theme.font_family = font_family);
            }
        }
        self.gtk_style = style == AppearanceStyle::Adwaita;
        self.winui_style = style == AppearanceStyle::WinUi;
        self.shape = ShapeTokens::for_style(style);
        self.refresh_surfaces();
        self.sync_font_size(cx);

        if let Ok(mut settings) = DesktopSettings::load() {
            settings.gtk_style = self.gtk_style;
            settings.winui_style = self.winui_style;
            let _ = settings.save();
        }

        cx.refresh_windows();
    }

    pub fn change_gtk_style(&mut self, enabled: bool, cx: &mut App) {
        let style = if enabled {
            AppearanceStyle::Adwaita
        } else if self.gtk_style {
            AppearanceStyle::Default
        } else {
            self.appearance_style()
        };
        self.change_appearance_style(style, cx);
    }

    pub fn change_winui_style(&mut self, enabled: bool, cx: &mut App) {
        let style = if enabled {
            AppearanceStyle::WinUi
        } else if self.winui_style {
            AppearanceStyle::Default
        } else {
            self.appearance_style()
        };
        self.change_appearance_style(style, cx);
    }

    /// Re-read the GTK config when it changed on disk (the desktop switched
    /// theme or dark/light). Keeps the palette list fresh and, when the
    /// desktop theme is the active one, restyles the app.
    pub fn sync_gtk(cx: &mut App) {
        let key = gtk::current_key();
        let (changed, active, gtk_style) = {
            let this = Self::global(cx);
            (
                this.gtk_key != key,
                this.active_preset.name == GTK_PRESET_NAME,
                this.gtk_style,
            )
        };
        if !changed {
            return;
        }
        if active {
            Self::apply_preset(GTK_PRESET_NAME, None, cx);
        } else {
            let this = Self::global_mut(cx);
            this.gtk_preset = gtk::preset();
            this.gtk_key = key;
            if gtk_style {
                cx.update_global::<Self, _>(|this, cx| this.sync_font_size(cx));
            }
        }
    }

    fn refresh_surfaces(&mut self) {
        let overrides = (self.active_preset.name == GTK_PRESET_NAME)
            .then(gtk::surface_overrides)
            .flatten();
        self.surface = SurfaceTokens::for_style(
            &self.palette_tokens,
            self.is_dark(),
            overrides,
            self.appearance_style(),
            self.window_transparency,
        );
    }

    /// Change active theme preset statically avoiding double borrow of App.
    pub fn apply_preset(name: &str, window: Option<&mut Window>, cx: &mut App) {
        cx.update_global::<Self, _>(|this, cx| {
            this.change_preset(name, window, cx);
        });
    }

    /// Change theme mode statically avoiding double borrow of App.
    pub fn apply_mode(mode: SettingsThemeMode, window: Option<&mut Window>, cx: &mut App) {
        cx.update_global::<Self, _>(|this, cx| {
            this.change_mode(mode, window, cx);
        });
    }

    pub fn apply_gtk_style(enabled: bool, cx: &mut App) {
        cx.update_global::<Self, _>(|this, cx| this.change_gtk_style(enabled, cx));
    }

    pub fn apply_winui_style(enabled: bool, cx: &mut App) {
        cx.update_global::<Self, _>(|this, cx| this.change_winui_style(enabled, cx));
    }

    /// Change backdrop blur without borrowing the global manager at call sites.
    pub fn apply_window_blur_enabled(enabled: bool, cx: &mut App) {
        cx.update_global::<Self, _>(|this, cx| {
            this.change_window_blur_enabled(enabled, cx);
        });
    }
}

/// Sizes text relative to the window's rem size, scaled by the chat text
/// setting (`scale` in percent). `base_rem`/`line_rem` are the Tailwind values,
/// e.g. `0.875`/`1.25` for `text_sm`.
pub trait ChatTextExt: gpui::Styled + Sized {
    fn chat_text(self, base_rem: f32, line_rem: f32, scale: u8) -> Self {
        let scale = scale as f32 / 100.0;
        self.text_size(gpui::rems(base_rem * scale))
            .line_height(gpui::rems(line_rem * scale))
    }
}

impl<T: gpui::Styled> ChatTextExt for T {}

/// Helper extension trait on `App` and `Context` for convenient token access.
pub trait AppThemeExt {
    fn app_theme(&self) -> &ActiveTokens;
}

impl AppThemeExt for App {
    fn app_theme(&self) -> &ActiveTokens {
        &ThemeManager::global(self).tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adwaita_effective_tokens_replace_only_surface_roles() {
        let palette = ActiveTokens::from_preset(find_preset("default-dark").unwrap());
        let surface = SurfaceTokens::from_tokens(&palette, true, None);
        let off = effective_tokens(palette, surface, false);
        let on = effective_tokens(palette, surface, true);

        assert_eq!(off, palette);
        assert_eq!(on.background, surface.view);
        assert_eq!(on.card, surface.card);
        assert_eq!(on.muted, surface.secondary_sidebar);
        assert_eq!(on.muted_foreground, surface.fg_muted);
        assert_eq!(on.border, surface.border);
        assert_eq!(on.primary, palette.primary);
        assert_eq!(on.accent, palette.accent);
    }

    #[test]
    fn test_theme_manager_init_from_settings() {
        let mut settings = DesktopSettings::default();
        settings.theme_preset = "ocean-light".to_string();
        settings.theme_mode = SettingsThemeMode::Light;

        let manager = ThemeManager::init_from_settings(&settings);
        assert_eq!(manager.active_preset.name, "ocean-light");
        assert_eq!(manager.mode, SettingsThemeMode::Light);
        assert!(!manager.is_dark());
    }
}

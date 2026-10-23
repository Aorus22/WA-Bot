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
use super::shape::ShapeTokens;
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

fn effective_tokens(palette: ActiveTokens, surface: SurfaceTokens, adwaita: bool) -> ActiveTokens {
    if !adwaita {
        return palette;
    }

    ActiveTokens {
        background: surface.view,
        card: surface.card,
        muted: surface.secondary_sidebar,
        muted_foreground: surface.fg_muted,
        border: surface.border,
        input: surface.window.mix_oklab(palette.foreground, 0.90),
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
    pub shape: ShapeTokens,
    pub surface: SurfaceTokens,
    /// The active preset's font family before applying the desktop GTK font.
    gtk_font_family_original: Option<gpui::SharedString>,
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
        let surface = SurfaceTokens::from_tokens(
            &tokens,
            is_dark,
            (preset.name == GTK_PRESET_NAME)
                .then(gtk::surface_overrides)
                .flatten(),
        );

        Self {
            active_preset: preset,
            mode: settings.theme_mode,
            window_transparency: settings.window_transparency.min(MAX_WINDOW_TRANSPARENCY),
            window_blur_enabled: settings.window_blur_enabled,
            gtk_style: settings.gtk_style,
            shape: ShapeTokens::new(settings.gtk_style),
            surface,
            gtk_font_family_original: None,
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
            if self.gtk_style {
                self.gtk_font_family_original = Some(Theme::global(cx).font_family.clone());
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
        if self.gtk_style {
            self.gtk_font_family_original = Some(Theme::global(cx).font_family.clone());
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
        if self.gtk_style && self.gtk_font_family_original.is_none() {
            self.gtk_font_family_original = Some(Theme::global(cx).font_family.clone());
        }
        let font_family = gtk_font.map(|font| gpui::SharedString::from(font.family.as_str()));
        let base_font_px = gtk_font
            .map(|font| font.size_points * (4.0 / 3.0))
            .unwrap_or(16.0);
        let gtk_style = self.gtk_style;
        let font_size = gpui::px(base_font_px * self.ui_font_scale as f32 / 100.0);
        let radius = gpui::px(self.shape.widget_radius);
        let radius_lg = gpui::px(self.shape.widget_radius_lg);
        let scrollbar_width = gpui::px(self.shape.scrollbar_width);
        let shadow = self.shape.shadows;
        self.tokens = effective_tokens(self.palette_tokens, self.surface, gtk_style);
        let active_tokens = self.tokens;
        let surface = self.surface;

        Theme::update(cx, |theme| {
            theme.adwaita = gtk_style;
            theme.font_size = font_size;
            // The shape tokens also drive the stock widgets, which read these.
            theme.radius = radius;
            theme.radius_lg = radius_lg;
            theme.shadow = shadow;
            theme.adwaita_dialog_bg = gtk_style.then_some(surface.dialog);
            theme.adwaita_scrollbar_width = gtk_style.then_some(scrollbar_width);
            if gtk_style {
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
        self.shape.transition_ms = if gtk_style && !reduce_motion { 200 } else { 0 };
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

    pub fn change_gtk_style(&mut self, enabled: bool, cx: &mut App) {
        if self.gtk_style == enabled {
            return;
        }
        if !enabled {
            let mode = if self.is_dark() {
                GpuiThemeMode::Dark
            } else {
                GpuiThemeMode::Light
            };
            Theme::change(mode, None, cx);
            if let Some(font_family) = self.gtk_font_family_original.take() {
                Theme::update(cx, |theme| theme.font_family = font_family);
            }
        }
        self.gtk_style = enabled;
        self.shape = ShapeTokens::new(enabled);
        self.sync_font_size(cx);

        if let Ok(mut settings) = DesktopSettings::load() {
            settings.gtk_style = enabled;
            let _ = settings.save();
        }

        cx.refresh_windows();
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
        self.surface = SurfaceTokens::from_tokens(&self.palette_tokens, self.is_dark(), overrides);
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

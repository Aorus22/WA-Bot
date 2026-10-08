//! Theme manager managing active preset, mode, and GPUI runtime synchronization.

use gpui::{App, BorrowAppContext, Global, Hsla, Window};
use gpui_component::{Theme, ThemeMode as GpuiThemeMode};
use wabot_settings::{DesktopSettings, ThemeMode as SettingsThemeMode};

use super::color::parse_hex_hsla;
use super::gtk::{self, GTK_PRESET_NAME};
use super::preset::{default_preset, find_preset, ThemePreset};

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
        }
    }
}

/// Global theme state manager.
#[derive(Debug, Clone)]
pub struct ThemeManager {
    pub active_preset: ThemePreset,
    pub mode: SettingsThemeMode,
    pub tokens: ActiveTokens,
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
        let tokens = ActiveTokens::from_preset(&preset);

        Self {
            active_preset: preset,
            mode: settings.theme_mode,
            tokens,
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
            self.tokens = ActiveTokens::from_preset(&preset);
            self.active_preset = preset;

            // Synchronize with gpui-component theme mode
            let gpui_mode = if self.is_dark() {
                GpuiThemeMode::Dark
            } else {
                GpuiThemeMode::Light
            };
            Theme::change(gpui_mode, window, cx);

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

        if let Ok(mut settings) = DesktopSettings::load() {
            settings.theme_mode = mode;
            let _ = settings.save();
        }

        cx.refresh_windows();
    }

    /// Re-read the GTK config when it changed on disk (the desktop switched
    /// theme or dark/light). Keeps the palette list fresh and, when the
    /// desktop theme is the active one, restyles the app.
    pub fn sync_gtk(cx: &mut App) {
        let key = gtk::current_key();
        let (changed, active) = {
            let this = Self::global(cx);
            (this.gtk_key != key, this.active_preset.name == GTK_PRESET_NAME)
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
        }
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
}

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

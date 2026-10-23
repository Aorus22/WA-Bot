//! WA Bot desktop settings store.
//!
//! Holds **UI preferences only**: theme selection, backend path override,
//! last backend URL, and window geometry. Server-owned data (AI keys, TTS
//! settings, read receipts, session state) lives in the backend's database
//! and is accessed via `GET/PUT /api/settings` — it must NEVER be duplicated
//! into this local JSON file (see T-01-03b).
//!
//! Future phases: do NOT add secret or server-setting fields here. If a new
//! preference is not renderable UI state, it belongs to the backend.

pub mod paths;

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Json(#[from] serde_json::Error),
}

/// App-level theme mode. Semantics match the web `wa-bot-theme` key
/// (`ThemeProvider defaultTheme="system"` in `web/src/App.tsx`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
    /// Forward-compat catch-all: an unknown value (e.g. a future `"auto"`
    /// from web) deserializes here instead of invalidating the whole file.
    /// Serializes as `"unknown"`, which round-trips back to `Unknown`.
    #[serde(other)]
    Unknown,
}

/// How the chat screen is laid out. `Auto` picks per surface: pages in the
/// panel popover (narrow), sidebar in a normal window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ChatLayout {
    #[default]
    Auto,
    /// Chat list and conversation are separate pages (with a back button).
    Pages,
    /// Chat list beside the conversation (collapsible).
    Sidebar,
    /// Forward-compat catch-all; treated as `Auto`.
    #[serde(other)]
    Unknown,
}

/// Window dimensions and layout state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct WindowState {
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub maximized: bool,
}

impl WindowState {
    /// Normalize to `None` when bounds are unusable (negative origin or
    /// zero dimension). Callers should store the normalized form.
    pub fn normalized(self) -> Option<WindowState> {
        match (self.x, self.y, self.width, self.height) {
            (Some(x), _, _, _) if x < 0 => None,
            (_, Some(y), _, _) if y < 0 => None,
            (_, _, Some(0), _) | (_, _, _, Some(0)) => None,
            _ => Some(self),
        }
    }
}

fn default_theme_preset() -> String {
    // Same default as web `useAppTheme()` (`wa-bot-theme-preset` fallback).
    "default-dark".to_string()
}

pub const DEFAULT_WINDOW_TRANSPARENCY: u8 = 28;
pub const MAX_WINDOW_TRANSPARENCY: u8 = 100;
/// Text scale in percent; 100 is the normal size.
pub const DEFAULT_FONT_SCALE: u8 = 100;
pub const MIN_FONT_SCALE: u8 = 75;
pub const MAX_FONT_SCALE: u8 = 150;
const LEGACY_MAX_WINDOW_TRANSPARENCY: u8 = 70;
const SETTINGS_SCHEMA_VERSION: u8 = 1;

fn default_window_transparency() -> u8 {
    DEFAULT_WINDOW_TRANSPARENCY
}

fn default_font_scale() -> u8 {
    DEFAULT_FONT_SCALE
}

fn default_window_blur_enabled() -> bool {
    true
}

fn default_notifications_enabled() -> bool {
    true
}

fn default_format_messages() -> bool {
    true
}

/// Desktop settings: UI preferences only (see module docs for the boundary).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesktopSettings {
    /// Active preset name; semantics identical to web `wa-bot-theme-preset`.
    /// Unknown values are preserved as-is (forward-compat with new web presets).
    #[serde(default = "default_theme_preset")]
    pub theme_preset: String,
    /// App theme mode; semantics identical to web `wa-bot-theme`.
    #[serde(default)]
    pub theme_mode: ThemeMode,
    /// Chat screen layout (list + conversation as pages or side by side).
    #[serde(default)]
    pub chat_layout: ChatLayout,
    /// Operator override for the backend binary path (dev/testing).
    /// Relative paths are rejected (normalized to `None` on load).
    #[serde(default)]
    pub backend_path_override: Option<PathBuf>,
    /// Last successfully connected backend URL (single-instance adopt).
    #[serde(default)]
    pub last_backend_url: Option<String>,
    #[serde(default)]
    pub window_state: Option<WindowState>,
    /// Schema version used to migrate local UI preferences.
    #[serde(default)]
    pub settings_version: u8,
    /// How much of the desktop shows through app surfaces, from 0 to 100 percent.
    #[serde(default = "default_window_transparency")]
    pub window_transparency: u8,
    /// Whether to request the compositor's default backdrop blur.
    #[serde(default = "default_window_blur_enabled")]
    pub window_blur_enabled: bool,
    /// GTK / Adwaita shapes (radii, flat surfaces) on top of any colour theme.
    #[serde(default)]
    pub gtk_style: bool,
    /// Scale of the interface text in percent (`MIN_FONT_SCALE`..=`MAX_FONT_SCALE`).
    #[serde(default = "default_font_scale")]
    pub ui_font_scale: u8,
    /// Scale of the text inside message bubbles, on top of the interface scale.
    #[serde(default = "default_font_scale")]
    pub chat_font_scale: u8,
    /// Whether incoming messages raise an OS notification.
    #[serde(default = "default_notifications_enabled")]
    pub notifications_enabled: bool,
    /// Whether WhatsApp text formatting (`*bold*`, `_italic_`, `~strike~`,
    /// `` `mono` ``) is rendered in message bubbles.
    #[serde(default = "default_format_messages")]
    pub format_messages: bool,

    /// Folder the Backup section writes to; unset until the user picks one.
    #[serde(default)]
    pub backup_folder: Option<String>,

    #[serde(skip)]
    pub custom_base: Option<PathBuf>,
}

impl Default for DesktopSettings {
    fn default() -> Self {
        Self {
            theme_preset: default_theme_preset(),
            theme_mode: ThemeMode::default(),
            chat_layout: ChatLayout::default(),
            backend_path_override: None,
            last_backend_url: None,
            window_state: None,
            settings_version: SETTINGS_SCHEMA_VERSION,
            window_transparency: default_window_transparency(),
            window_blur_enabled: default_window_blur_enabled(),
            gtk_style: false,
            ui_font_scale: default_font_scale(),
            chat_font_scale: default_font_scale(),
            notifications_enabled: default_notifications_enabled(),
            format_messages: default_format_messages(),
            backup_folder: None,
            custom_base: None,
        }
    }
}

impl DesktopSettings {
    /// Load settings from the default config directory.
    pub fn load() -> Result<Self, SettingsError> {
        let base = paths::default_base_dir();
        Self::load_from(&base)
    }

    /// Load settings using an injectable base directory (for testing).
    pub fn load_from(base: &Path) -> Result<Self, SettingsError> {
        let file_path = paths::settings_path_with_base(base);
        if !file_path.exists() {
            return Ok(Self {
                custom_base: Some(base.to_path_buf()),
                ..Default::default()
            });
        }

        let content = match fs::read_to_string(&file_path) {
            Ok(c) => c,
            // Only non-UTF8 content counts as corrupt here (`read_to_string`
            // surfaces bad UTF-8 as `InvalidData`). Any other IO failure
            // (permission-denied, etc.) says nothing about file content and
            // must propagate — never trigger destructive recovery.
            Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                let backup = corrupt_backup_path(&file_path);
                let _ = fs::rename(&file_path, &backup);
                let settings = Self {
                    custom_base: Some(base.to_path_buf()),
                    ..Default::default()
                };
                let _ = settings.save_to(base);
                return Ok(settings);
            }
            Err(e) => return Err(SettingsError::Io(e)),
        };
        match serde_json::from_str::<DesktopSettings>(&content) {
            Ok(mut settings) => {
                settings.custom_base = Some(base.to_path_buf());
                settings.normalize();
                Ok(settings)
            }
            Err(_) => {
                // Corrupt file recovery: back up with timestamp, never panic.
                let backup = corrupt_backup_path(&file_path);
                let _ = fs::rename(&file_path, &backup);
                let settings = Self {
                    custom_base: Some(base.to_path_buf()),
                    ..Default::default()
                };
                let _ = settings.save_to(base);
                Ok(settings)
            }
        }
    }

    /// Normalize in-memory invariants (validation without rejection).
    fn normalize(&mut self) {
        // Forward-compat: foreign theme presets pass through untouched.
        // Negative/zero window bounds reset to None.
        if let Some(ws) = self.window_state {
            self.window_state = ws.normalized();
        }
        if self.settings_version < SETTINGS_SCHEMA_VERSION {
            // The previous slider stopped at 70%, so a saved maximum means
            // the user had dragged it to the end of its range.
            if self.window_transparency == LEGACY_MAX_WINDOW_TRANSPARENCY {
                self.window_transparency = MAX_WINDOW_TRANSPARENCY;
            }
            self.settings_version = SETTINGS_SCHEMA_VERSION;
        }
        self.window_transparency = self.window_transparency.min(MAX_WINDOW_TRANSPARENCY);
        self.ui_font_scale = self.ui_font_scale.clamp(MIN_FONT_SCALE, MAX_FONT_SCALE);
        self.chat_font_scale = self.chat_font_scale.clamp(MIN_FONT_SCALE, MAX_FONT_SCALE);
        // Relative backend overrides are meaningless across cwd changes.
        if let Some(p) = &self.backend_path_override {
            if !p.is_absolute() {
                self.backend_path_override = None;
            }
        }
    }

    /// Save settings to current base directory (or default).
    pub fn save(&self) -> Result<(), SettingsError> {
        if let Some(ref base) = self.custom_base {
            self.save_to(base)
        } else {
            self.save_to(&paths::default_base_dir())
        }
    }

    /// Save settings atomically (write-temp-then-rename) to `base`.
    pub fn save_to(&self, base: &Path) -> Result<(), SettingsError> {
        paths::ensure_dirs_with_base(base)?;
        let file_path = paths::settings_path_with_base(base);
        let content = serde_json::to_string_pretty(self)?;
        let tmp_path = file_path.with_extension("json.tmp");
        fs::write(&tmp_path, content)?;
        fs::rename(&tmp_path, &file_path)?;
        Ok(())
    }
}

/// Backup path for a corrupt settings file, timestamped when possible.
/// The pid suffix keeps two corruptions within the same second from
/// overwriting each other's evidence.
fn corrupt_backup_path(file_path: &Path) -> PathBuf {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let pid = std::process::id();
    file_path.with_extension(format!("json.corrupt-{ts}-{pid}"))
}

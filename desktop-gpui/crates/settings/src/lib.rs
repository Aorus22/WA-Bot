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
    /// Operator override for the backend binary path (dev/testing).
    /// Relative paths are rejected (normalized to `None` on load).
    #[serde(default)]
    pub backend_path_override: Option<PathBuf>,
    /// Last successfully connected backend URL (single-instance adopt).
    #[serde(default)]
    pub last_backend_url: Option<String>,
    #[serde(default)]
    pub window_state: Option<WindowState>,

    #[serde(skip)]
    pub custom_base: Option<PathBuf>,
}

impl Default for DesktopSettings {
    fn default() -> Self {
        Self {
            theme_preset: default_theme_preset(),
            theme_mode: ThemeMode::default(),
            backend_path_override: None,
            last_backend_url: None,
            window_state: None,
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

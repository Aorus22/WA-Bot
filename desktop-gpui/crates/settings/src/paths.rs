//! Config-dir path helpers for WA Bot desktop.
use std::path::{Path, PathBuf};

/// Directory name inside the OS config dir (NOT webterm-desktop).
pub const APP_CONFIG_DIR: &str = "wa-bot-desktop";

/// Get the system default base config directory.
///
/// When the OS config dir is unavailable, falls back to the **absolute**
/// current directory — never the relative `"."` (which would silently
/// scatter settings per launch cwd). Absolute keeps the location
/// deterministic and debuggable in the degraded case.
pub fn default_base_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

/// Get the WA Bot config directory within the given base directory.
pub fn config_dir_with_base(base: &Path) -> PathBuf {
    base.join(APP_CONFIG_DIR)
}

/// Get the default WA Bot config directory.
pub fn config_dir() -> PathBuf {
    config_dir_with_base(&default_base_dir())
}

/// Get the settings.json path within the given base directory.
pub fn settings_path_with_base(base: &Path) -> PathBuf {
    config_dir_with_base(base).join("settings.json")
}

/// Get the default settings.json path.
pub fn settings_path() -> PathBuf {
    settings_path_with_base(&default_base_dir())
}

/// Ensure config directory exists for the given base directory.
pub fn ensure_dirs_with_base(base: &Path) -> std::io::Result<PathBuf> {
    let dir = config_dir_with_base(base);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Ensure the default config directory exists.
pub fn ensure_dirs() -> std::io::Result<PathBuf> {
    ensure_dirs_with_base(&default_base_dir())
}

/// Create `database/` and `media/` under `base` (paritas `desktop/main.js`
/// userData layout: the Go backend opens `database/*.db` relative to its
/// work dir). Idempotent. Returns `(db_dir, media_dir)`.
pub fn ensure_data_dirs(base: &Path) -> std::io::Result<(PathBuf, PathBuf)> {
    let db_dir = base.join("database");
    let media_dir = base.join("media");
    std::fs::create_dir_all(&db_dir)?;
    std::fs::create_dir_all(&media_dir)?;
    Ok((db_dir, media_dir))
}

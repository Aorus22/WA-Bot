//! Settings persistence tests — hermetic (temp base dirs only, never the
//! real OS config dir).

use std::path::PathBuf;
use wabot_settings::{paths, DesktopSettings, ThemeMode, WindowState};

fn tmp_base() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

#[test]
fn tracer_round_trip_save_load_restart() {
    let tmp = tmp_base();
    let base = tmp.path();

    let mut s = DesktopSettings::load_from(base).unwrap();
    assert_eq!(s.theme_preset, "default-dark");
    assert_eq!(s.theme_mode, ThemeMode::System);

    s.theme_preset = "ocean-dark".to_string();
    s.theme_mode = ThemeMode::Dark;
    s.last_backend_url = Some("http://127.0.0.1:51234".to_string());
    s.window_state = Some(WindowState {
        x: Some(100),
        y: Some(100),
        width: Some(1200),
        height: Some(800),
        maximized: false,
    });
    s.save_to(base).unwrap();
    assert!(paths::settings_path_with_base(base).exists());

    // Simulated restart: fresh load from the same disk location.
    let re = DesktopSettings::load_from(base).unwrap();
    assert_eq!(re.theme_preset, "ocean-dark");
    assert_eq!(re.theme_mode, ThemeMode::Dark);
    assert_eq!(
        re.last_backend_url.as_deref(),
        Some("http://127.0.0.1:51234")
    );
    assert_eq!(re.window_state.unwrap().width, Some(1200));
}

#[test]
fn corrupt_file_recovers_to_default_with_backup() {
    let tmp = tmp_base();
    let base = tmp.path();

    // Seed a good file first, then smash it with garbage bytes.
    DesktopSettings::load_from(base).unwrap().save_to(base).unwrap();
    std::fs::write(paths::settings_path_with_base(base), b"\x00\xff{not json").unwrap();

    let recovered = DesktopSettings::load_from(base).unwrap();
    let expected = DesktopSettings {
        custom_base: Some(base.to_path_buf()),
        ..DesktopSettings::default()
    };
    assert_eq!(recovered, expected);
    // Backup exists (timestamped corrupt file).
    let dir = paths::config_dir_with_base(base);
    let backups: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("settings.json.corrupt")
        })
        .collect();
    assert_eq!(backups.len(), 1, "expected one corrupt backup");
    // Normal save works after recovery.
    recovered.save_to(base).unwrap();
    assert!(paths::settings_path_with_base(base).exists());
}

#[test]
fn unknown_fields_tolerated_and_preset_forward_compat() {
    let tmp = tmp_base();
    let base = tmp.path();
    paths::ensure_dirs_with_base(base).unwrap();
    std::fs::write(
        paths::settings_path_with_base(base),
        r#"{"theme_preset": "future-neon-v9", "theme_mode": "dark", "brand_new_field": {"nested": true}, "another": [1,2,3]}"#,
    )
    .unwrap();
    let s = DesktopSettings::load_from(base).unwrap();
    // Foreign preset preserved as-is (never rejected).
    assert_eq!(s.theme_preset, "future-neon-v9");
    assert_eq!(s.theme_mode, ThemeMode::Dark);
}

#[test]
fn validation_normalizes_bad_values() {
    let tmp = tmp_base();
    let base = tmp.path();
    paths::ensure_dirs_with_base(base).unwrap();

    // Negative bounds -> None.
    std::fs::write(
        paths::settings_path_with_base(base),
        r#"{"window_state": {"x": -50, "y": 10, "width": 800, "height": 600, "maximized": false}}"#,
    )
    .unwrap();
    let s = DesktopSettings::load_from(base).unwrap();
    assert!(s.window_state.is_none());

    // Zero dimension -> None.
    std::fs::write(
        paths::settings_path_with_base(base),
        r#"{"window_state": {"x": 0, "y": 0, "width": 0, "height": 600, "maximized": false}}"#,
    )
    .unwrap();
    let s = DesktopSettings::load_from(base).unwrap();
    assert!(s.window_state.is_none());

    // Relative backend override -> None; absolute survives.
    std::fs::write(
        paths::settings_path_with_base(base),
        r#"{"backend_path_override": "relative/bin"}"#,
    )
    .unwrap();
    assert!(DesktopSettings::load_from(base).unwrap().backend_path_override.is_none());

    #[cfg(unix)]
    {
        std::fs::write(
            paths::settings_path_with_base(base),
            r#"{"backend_path_override": "/opt/wa-bot-backend"}"#,
        )
        .unwrap();
        assert_eq!(
            DesktopSettings::load_from(base).unwrap().backend_path_override,
            Some(PathBuf::from("/opt/wa-bot-backend"))
        );
    }
}

#[test]
fn config_dir_is_wabot_desktop_and_data_dirs_idempotent() {
    let tmp = tmp_base();
    let base = tmp.path();
    assert!(paths::config_dir_with_base(base).ends_with("wa-bot-desktop"));
    let (db1, media1) = paths::ensure_data_dirs(base).unwrap();
    assert_eq!(db1, base.join("database"));
    assert_eq!(media1, base.join("media"));
    let (db2, media2) = paths::ensure_data_dirs(base).unwrap();
    assert_eq!((db1, media1), (db2, media2));
}

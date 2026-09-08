//! Theme preset catalog loaded from web 1:1 definitions.

use std::sync::LazyLock;
use serde::{Deserialize, Serialize};

use super::color::is_dark_bg;

/// 17 semantic color tokens matching web CSS variables.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeColors {
    pub background: String,
    pub foreground: String,
    pub card: String,
    pub card_foreground: String,
    pub primary: String,
    pub primary_foreground: String,
    pub secondary: String,
    pub secondary_foreground: String,
    pub muted: String,
    pub muted_foreground: String,
    pub accent: String,
    pub accent_foreground: String,
    pub destructive: String,
    pub destructive_foreground: String,
    pub border: String,
    pub input: String,
    pub ring: String,
}

/// A theme preset defining a name, user-facing label, and 17 semantic colors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemePreset {
    pub name: String,
    pub label: String,
    pub colors: ThemeColors,
}

impl ThemePreset {
    /// Returns true if this preset's background luminance is < 0.5.
    pub fn is_dark(&self) -> bool {
        is_dark_bg(&self.colors.background)
    }
}

static THEMES_DATA: LazyLock<Vec<ThemePreset>> = LazyLock::new(|| {
    let json_bytes = include_bytes!("themes_data.json");
    serde_json::from_slice(json_bytes).expect("themes_data.json must be valid")
});

/// Returns all 81 loaded presets from web client catalog.
pub fn all_presets() -> &'static [ThemePreset] {
    &THEMES_DATA
}

/// Look up a preset by name. Falls back to default if not found.
pub fn find_preset(name: &str) -> Option<&'static ThemePreset> {
    all_presets().iter().find(|p| p.name == name)
}

/// Returns the fallback default preset ("default-dark").
pub fn default_preset() -> &'static ThemePreset {
    find_preset("default-dark").unwrap_or_else(|| &all_presets()[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_presets_loaded() {
        let presets = all_presets();
        assert_eq!(presets.len(), 81, "Must contain all 81 web presets");

        let default_dark = find_preset("default-dark").expect("default-dark must exist");
        assert_eq!(default_dark.label, "Default");
        assert!(default_dark.is_dark());

        let default_light = find_preset("default-light").expect("default-light must exist");
        assert_eq!(default_light.label, "Default");
        assert!(!default_light.is_dark());
    }
}

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

/// Optional chat-specific colors for presets that restyle the conversation
/// itself (the WhatsApp theme). Presets without it keep the default look.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeExtras {
    /// Conversation pane background (WhatsApp's wallpaper colour).
    pub chat_bg: String,
    /// Conversation header surface.
    pub header: String,
    pub bubble_out: String,
    pub bubble_out_text: String,
    pub bubble_in: String,
    pub bubble_in_text: String,
    /// Selected / hovered row in the chat list.
    pub list_selected: String,
    pub list_hover: String,
}

/// A theme preset defining a name, user-facing label, and 17 semantic colors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemePreset {
    pub name: String,
    pub label: String,
    pub colors: ThemeColors,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<ThemeExtras>,
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

/// Returns all loaded presets (web catalog + WhatsApp).
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
        assert_eq!(presets.len(), 83, "81 web presets + WhatsApp dark/light");

        let default_dark = find_preset("default-dark").expect("default-dark must exist");
        assert_eq!(default_dark.label, "Default");
        assert!(default_dark.is_dark());

        let default_light = find_preset("default-light").expect("default-light must exist");
        assert_eq!(default_light.label, "Default");
        assert!(!default_light.is_dark());
    }

    #[test]
    fn test_whatsapp_presets() {
        let dark = find_preset("whatsapp-dark").expect("whatsapp-dark must exist");
        let light = find_preset("whatsapp-light").expect("whatsapp-light must exist");
        assert!(dark.is_dark());
        assert!(!light.is_dark());
        assert_eq!(dark.label, "WhatsApp");
        assert_eq!(dark.extras.as_ref().unwrap().bubble_out, "#005C4B");
        assert_eq!(light.extras.as_ref().unwrap().bubble_out, "#D9FDD3");
        // Every other preset keeps the stock chat look.
        let with_extras = all_presets().iter().filter(|p| p.extras.is_some()).count();
        assert_eq!(with_extras, 2);
    }
}

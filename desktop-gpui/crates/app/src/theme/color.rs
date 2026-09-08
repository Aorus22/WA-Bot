//! Color parsing, conversion, and luminance calculation for theme tokens.

use gpui::{Hsla, Rgba};
use gpui_component::theme::Colorize;

/// Calculates relative luminance from a hex string (`#RRGGBB` or `#RRGGBBAA`).
/// Matches `web/src/components/AppThemeProvider.tsx` getLuminance implementation:
/// luminance < 0.5 designates dark mode.
pub fn get_luminance(hex: &str) -> f32 {
    let hex = hex.trim_start_matches('#');
    if hex.len() < 6 {
        return 0.0;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f32 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f32 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f32 / 255.0;

    let to_linear = |v: f32| {
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };

    0.2126 * to_linear(r) + 0.7152 * to_linear(g) + 0.0722 * to_linear(b)
}

/// Returns true if background hex color has luminance < 0.5.
pub fn is_dark_bg(hex: &str) -> bool {
    get_luminance(hex) < 0.5
}

/// Parse a hex string into a `gpui::Hsla` color.
/// Falls back to opaque black if invalid.
pub fn parse_hex_hsla(hex: &str) -> Hsla {
    Hsla::parse_hex(hex).unwrap_or_else(|_| Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.0,
        a: 1.0,
    })
}

/// Parse a hex string into a `gpui::Rgba` color.
pub fn parse_hex_rgba(hex: &str) -> Rgba {
    let hex_clean = hex.trim_start_matches('#');
    if hex_clean.len() >= 6 {
        let r = u8::from_str_radix(&hex_clean[0..2], 16).unwrap_or(0) as f32 / 255.0;
        let g = u8::from_str_radix(&hex_clean[2..4], 16).unwrap_or(0) as f32 / 255.0;
        let b = u8::from_str_radix(&hex_clean[4..6], 16).unwrap_or(0) as f32 / 255.0;
        let a = if hex_clean.len() >= 8 {
            u8::from_str_radix(&hex_clean[6..8], 16).unwrap_or(255) as f32 / 255.0
        } else {
            1.0
        };
        Rgba { r, g, b, a }
    } else {
        Rgba {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_luminance_parity_with_web() {
        // Default dark background: #0F172A
        let dark_lum = get_luminance("#0F172A");
        assert!(dark_lum < 0.5, "default-dark must have luminance < 0.5");
        assert!(is_dark_bg("#0F172A"));

        // Default light background: #F8FAFC
        let light_lum = get_luminance("#F8FAFC");
        assert!(light_lum >= 0.5, "default-light must have luminance >= 0.5");
        assert!(!is_dark_bg("#F8FAFC"));
    }

    #[test]
    fn test_hex_conversions() {
        let hsla = parse_hex_hsla("#FFFFFF");
        assert_eq!(hsla.l, 1.0);

        let rgba = parse_hex_rgba("#FF0000");
        assert_eq!(rgba.r, 1.0);
        assert_eq!(rgba.g, 0.0);
        assert_eq!(rgba.b, 0.0);
        assert_eq!(rgba.a, 1.0);
    }
}

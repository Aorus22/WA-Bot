//! "Desktop (GTK)" theme source: follows the colours the desktop publishes in
//! the user's GTK config, with no GTK dependency.
//!
//! Desktops write their palette as `@define-color` entries (GNOME/libadwaita,
//! and COSMIC, which generates `~/.config/gtk-4.0/cosmic/{dark,light}.css` and
//! symlinks `gtk.css` to the active one) or as `--name:` custom properties.
//! Files are read lowest priority first; later ones win.
//!
//! Everything here is plain data and pure functions, so the mapping is
//! unit-tested without a display. Ported from the `web-term` GTK probe, minus
//! the GTK3 style-engine fallback: when the user has no usable CSS the option
//! is simply not offered.

use std::collections::HashMap;
use std::path::PathBuf;

use gpui::Rgba;

use super::preset::{ThemeColors, ThemePreset};

pub const GTK_PRESET_NAME: &str = "gtk";
pub const GTK_PRESET_LABEL: &str = "Desktop (GTK)";

/// Files that make up the palette, **lowest priority first**.
const USER_CSS_FILES: [(&str, &str); 4] = [
    ("gtk-3.0", "colors.css"),
    ("gtk-3.0", "gtk.css"),
    ("gtk-4.0", "colors.css"),
    ("gtk-4.0", "gtk.css"),
];

const GNOME_BLUE: u32 = 0x3584e4;
const FALLBACK_ERROR: u32 = 0xef4444;
/// Accent vs background contrast below which the accent is considered
/// invisible (COSMIC themes may set the accent equal to the window colour).
const MIN_ACCENT_CONTRAST: f32 = 1.6;
const MIN_MUTED_CONTRAST: f32 = 4.5;

/// Named colours from the user's GTK CSS. Names are normalised
/// (`--window-bg-color`, `window_bg_color` and `@window_bg_color` collapse to
/// `window_bg_color`); values keep their raw text so `var()` / `@name`
/// indirection is followed at lookup time.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UserCss {
    vars: HashMap<String, String>,
    defines: HashMap<String, String>,
}

impl UserCss {
    pub fn is_empty(&self) -> bool {
        self.vars.is_empty() && self.defines.is_empty()
    }

    fn raw(&self, name: &str) -> Option<&str> {
        let key = normalize_css_name(name);
        self.vars
            .get(&key)
            .or_else(|| self.defines.get(&key))
            .map(|v| v.as_str())
    }

    pub fn get(&self, name: &str) -> Option<Rgba> {
        let mut value = self.raw(name)?.to_string();
        for _ in 0..4 {
            let reference = value
                .strip_prefix("var(")
                .and_then(|inner| inner.strip_suffix(')'))
                .map(|inner| inner.trim().to_string())
                .or_else(|| value.strip_prefix('@').map(|inner| inner.trim().to_string()));
            match reference {
                Some(reference) => value = self.raw(&reference)?.to_string(),
                None => return parse_css_color(&value),
            }
        }
        None
    }

    /// First of `names` that resolves.
    pub fn get_any(&self, names: &[&str]) -> Option<Rgba> {
        names.iter().find_map(|name| self.get(name))
    }
}

fn normalize_css_name(name: &str) -> String {
    name.trim()
        .trim_start_matches("--")
        .trim_start_matches('@')
        .trim()
        .to_ascii_lowercase()
        .replace('-', "_")
}

/// `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb()`/`rgba()` and `transparent`.
/// Anything else (`alpha()`, `color-mix`, `currentColor`) returns `None`.
fn parse_css_color(value: &str) -> Option<Rgba> {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix('#') {
        if !hex.is_ascii() {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        let nibble = |i: usize| u8::from_str_radix(&hex[i..i + 1], 16).ok().map(|v| v * 17);
        return match hex.len() {
            3 | 4 => {
                let alpha = if hex.len() == 4 { nibble(3)? } else { 0xff };
                Some(gpui::rgba(
                    ((nibble(0)? as u32) << 24)
                        | ((nibble(1)? as u32) << 16)
                        | ((nibble(2)? as u32) << 8)
                        | alpha as u32,
                ))
            }
            6 => Some(gpui::rgb(
                ((byte(0)? as u32) << 16) | ((byte(2)? as u32) << 8) | byte(4)? as u32,
            )),
            8 => Some(gpui::rgba(
                ((byte(0)? as u32) << 24)
                    | ((byte(2)? as u32) << 16)
                    | ((byte(4)? as u32) << 8)
                    | byte(6)? as u32,
            )),
            _ => None,
        };
    }
    if value.eq_ignore_ascii_case("transparent") {
        return Some(gpui::rgba(0));
    }
    let open = value.find('(')?;
    let func = value[..open].trim();
    if !func.eq_ignore_ascii_case("rgb") && !func.eq_ignore_ascii_case("rgba") {
        return None;
    }
    let inner = value[open + 1..].trim_end_matches(')').trim();
    let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
    if parts.len() < 3 {
        return None;
    }
    let channel = |p: &str| p.split('%').next()?.trim().parse::<f32>().ok();
    let (r, g, b) = (channel(parts[0])?, channel(parts[1])?, channel(parts[2])?);
    let a = match parts.get(3) {
        Some(p) => channel(p)?,
        None => 1.0,
    };
    let to_byte = |v: f32| (v.clamp(0.0, 255.0).round() as u32) & 0xff;
    Some(gpui::rgba(
        (to_byte(r) << 24)
            | (to_byte(g) << 16)
            | (to_byte(b) << 8)
            | (a.clamp(0.0, 1.0) * 255.0).round() as u32,
    ))
}

/// Drop `/* … */` spans, keeping their newlines so declarations around a
/// comment are not glued together.
fn strip_css_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        let body = &rest[start + 2..];
        let (skipped, next) = match body.find("*/") {
            Some(end) => (&body[..end], &body[end + 2..]),
            None => (body, ""),
        };
        out.extend(skipped.chars().filter(|c| *c == '\n'));
        rest = next;
    }
    out.push_str(rest);
    out
}

fn clean_css_value(value: &str) -> String {
    value.split(';').next().unwrap_or(value).trim().to_string()
}

/// Parse `--name: value;` custom properties and `@define-color name value;`
/// declarations out of one stylesheet.
pub fn parse_css_into(css: &str, out: &mut UserCss) {
    let css = strip_css_comments(css);
    let mut i = 0;
    while i < css.len() {
        if !css.is_char_boundary(i) {
            i += 1;
            continue;
        }
        let tail = &css[i..];
        if let Some(rest) = tail.strip_prefix("@define-color") {
            let end = rest.find(';').map(|end| i + "@define-color".len() + end);
            let decl = &css[i + "@define-color".len()..end.unwrap_or(css.len())];
            let mut parts = decl.trim().splitn(2, char::is_whitespace);
            if let (Some(name), Some(value)) = (parts.next(), parts.next()) {
                out.defines.insert(normalize_css_name(name), clean_css_value(value));
            }
            i = end.map(|end| end + 1).unwrap_or(css.len());
        } else if let Some(rest) = tail.strip_prefix("--") {
            let end = rest.find(';').map(|end| i + 2 + end);
            let decl = &css[i + 2..end.unwrap_or(css.len())];
            if let Some((name, value)) = decl.split_once(':') {
                out.vars.insert(normalize_css_name(name), clean_css_value(value));
            }
            i = end.map(|end| end + 1).unwrap_or(css.len());
        } else {
            i += tail.chars().next().map(char::len_utf8).unwrap_or(1);
        }
    }
}

fn config_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
}

fn user_css_paths() -> Vec<PathBuf> {
    let Some(config) = config_dir() else {
        return Vec::new();
    };
    USER_CSS_FILES
        .iter()
        .map(|(dir, file)| config.join(dir).join(file))
        .collect()
}

fn load_user_css() -> UserCss {
    let mut out = UserCss::default();
    for path in user_css_paths() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            parse_css_into(&text, &mut out);
        }
    }
    out
}

/// Change detector for live updates. It covers each file's resolved target
/// *and* its mtime: COSMIC switches dark/light by re-pointing the `gtk.css`
/// symlink, which alone changes neither file's mtime.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ThemeKey(Vec<(PathBuf, u64)>);

pub fn current_key() -> ThemeKey {
    ThemeKey(
        user_css_paths()
            .into_iter()
            .filter_map(|path| {
                let target = std::fs::canonicalize(&path).ok()?;
                let mtime = std::fs::metadata(&target)
                    .ok()?
                    .modified()
                    .ok()?
                    .duration_since(std::time::UNIX_EPOCH)
                    .ok()?
                    .as_secs();
                Some((target, mtime))
            })
            .collect(),
    )
}

// --- colour helpers (0xRRGGBB) ---

fn hex_from_rgba(color: Rgba) -> u32 {
    let channel = |v: f32| ((v.clamp(0.0, 1.0) * 255.0).round() as u32) & 0xff;
    (channel(color.r) << 16) | (channel(color.g) << 8) | channel(color.b)
}

fn mix_hex(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let lerp = |shift: u32| {
        let ca = ((a >> shift) & 0xff) as f32;
        let cb = ((b >> shift) & 0xff) as f32;
        ((ca + (cb - ca) * t).round() as u32) & 0xff
    };
    (lerp(16) << 16) | (lerp(8) << 8) | lerp(0)
}

fn luminance(hex: u32) -> f32 {
    let lin = |c: f32| {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let c = |shift: u32| lin(((hex >> shift) & 0xff) as f32 / 255.0);
    0.2126 * c(16) + 0.7152 * c(8) + 0.0722 * c(0)
}

fn contrast(a: u32, b: u32) -> f32 {
    let (l1, l2) = (luminance(a), luminance(b));
    let (hi, lo) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
    (hi + 0.05) / (lo + 0.05)
}

fn on_color(hex: u32) -> u32 {
    if luminance(hex) > 0.45 { 0x000000 } else { 0xffffff }
}

/// Nudge `muted` toward `fg` until it reads on every surface.
fn readable_muted(muted: u32, surfaces: &[u32], fg: u32) -> u32 {
    let ok = |c: u32| surfaces.iter().all(|s| contrast(c, *s) >= MIN_MUTED_CONTRAST);
    if ok(muted) {
        return muted;
    }
    (1..=20)
        .map(|step| mix_hex(muted, fg, step as f32 / 20.0))
        .find(|c| ok(*c))
        .unwrap_or(fg)
}

/// The resolved palette, as the 17 tokens the app styles with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GtkPalette {
    pub background: u32,
    pub foreground: u32,
    pub card: u32,
    pub primary: u32,
    pub primary_foreground: u32,
    pub secondary: u32,
    pub muted: u32,
    pub muted_foreground: u32,
    pub accent: u32,
    pub accent_foreground: u32,
    pub destructive: u32,
    pub border: u32,
}

/// Map named colours onto the token table (pure).
pub fn palette_from_css(css: &UserCss) -> Option<GtkPalette> {
    let bg = hex_from_rgba(css.get_any(&["window_bg_color", "theme_bg_color_breeze", "theme_bg_color"])?);
    let fg = hex_from_rgba(css.get_any(&[
        "window_fg_color",
        "theme_fg_color_breeze",
        "theme_fg_color",
        "theme_text_color",
    ])?);
    let base = css
        .get_any(&["view_bg_color", "theme_base_color_breeze", "theme_base_color"])
        .map(hex_from_rgba)
        .unwrap_or(bg);
    let card = css
        .get_any(&["card_bg_color", "popover_bg_color", "theme_card_bg_color"])
        .map(hex_from_rgba)
        .unwrap_or(base);

    let mut accent = css
        .get_any(&[
            "accent_bg_color",
            "accent_color",
            "theme_selected_bg_color_breeze",
            "theme_selected_bg_color",
            "blue_1",
        ])
        .map(hex_from_rgba)
        .unwrap_or(GNOME_BLUE);
    let mut on_accent = css
        .get_any(&["accent_fg_color", "theme_selected_fg_color_breeze", "theme_selected_fg_color"])
        .map(hex_from_rgba);
    if contrast(accent, bg) < MIN_ACCENT_CONTRAST {
        // The accent is indistinguishable from the window (some COSMIC
        // themes): use the palette's blue, else the foreground itself.
        accent = css
            .get("blue_1")
            .map(hex_from_rgba)
            .filter(|c| contrast(*c, bg) >= MIN_ACCENT_CONTRAST)
            .unwrap_or(fg);
        on_accent = None;
    }
    let on_accent = on_accent.unwrap_or_else(|| on_color(accent));

    let border = css
        .get_any(&["borders_breeze", "borders", "border_color", "sidebar_border_color"])
        .map(hex_from_rgba)
        .unwrap_or_else(|| mix_hex(base, fg, 0.18));

    let muted_fg = css
        .get_any(&["insensitive_fg_color_breeze", "insensitive_fg_color"])
        .map(|c| mix_hex(bg, hex_from_rgba(c), c.a))
        .unwrap_or_else(|| mix_hex(bg, fg, 0.55));

    let destructive = css
        .get_any(&["destructive_bg_color", "error_color_breeze", "error_color", "red_1"])
        .map(hex_from_rgba)
        .unwrap_or(FALLBACK_ERROR);

    Some(GtkPalette {
        background: bg,
        foreground: fg,
        card,
        primary: accent,
        primary_foreground: on_accent,
        secondary: mix_hex(card, fg, 0.08),
        muted: mix_hex(bg, fg, 0.06),
        muted_foreground: readable_muted(muted_fg, &[card, bg], fg),
        accent,
        accent_foreground: on_accent,
        destructive,
        border,
    })
}

fn hex(v: u32) -> String {
    format!("#{v:06x}")
}

impl GtkPalette {
    pub fn to_preset(&self) -> ThemePreset {
        ThemePreset {
            name: GTK_PRESET_NAME.to_string(),
            label: GTK_PRESET_LABEL.to_string(),
            colors: ThemeColors {
                background: hex(self.background),
                foreground: hex(self.foreground),
                card: hex(self.card),
                card_foreground: hex(self.foreground),
                primary: hex(self.primary),
                primary_foreground: hex(self.primary_foreground),
                secondary: hex(self.secondary),
                secondary_foreground: hex(self.foreground),
                muted: hex(self.muted),
                muted_foreground: hex(self.muted_foreground),
                accent: hex(self.accent),
                accent_foreground: hex(self.accent_foreground),
                destructive: hex(self.destructive),
                destructive_foreground: hex(on_color(self.destructive)),
                border: hex(self.border),
                input: hex(self.border),
                ring: hex(self.accent),
            },
            extras: None,
        }
    }
}

/// The desktop palette from the user's GTK CSS, or `None` without any.
pub fn preset() -> Option<ThemePreset> {
    let css = load_user_css();
    if css.is_empty() {
        return None;
    }
    palette_from_css(&css).map(|p| p.to_preset())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn css(text: &str) -> UserCss {
        let mut out = UserCss::default();
        parse_css_into(text, &mut out);
        out
    }

    #[test]
    fn parses_define_color_and_custom_properties() {
        let c = css("/* x */ @define-color window_bg_color rgba(65, 54, 54, 1.00);\n:root { --accent-bg-color: #3584e4; --fg: var(--accent-bg-color); }");
        assert_eq!(hex_from_rgba(c.get("window_bg_color").unwrap()), 0x413636);
        assert_eq!(hex_from_rgba(c.get("accent_bg_color").unwrap()), 0x3584e4);
        assert_eq!(hex_from_rgba(c.get("fg").unwrap()), 0x3584e4);
    }

    #[test]
    fn rejects_unsupported_color_functions() {
        assert!(parse_css_color("alpha(@x, 0.5)").is_none());
        assert!(parse_css_color("currentColor").is_none());
        assert!(parse_css_color("#12é").is_none());
    }

    #[test]
    fn maps_a_libadwaita_dark_palette() {
        let p = palette_from_css(&css(
            "@define-color window_bg_color #241f31;@define-color window_fg_color #ffffff;\
             @define-color card_bg_color #3d3846;@define-color accent_bg_color #3584e4;\
             @define-color accent_fg_color #ffffff;@define-color destructive_bg_color #c01c28;",
        ))
        .unwrap();
        assert_eq!(p.background, 0x241f31);
        assert_eq!(p.card, 0x3d3846);
        assert_eq!(p.primary, 0x3584e4);
        assert_eq!(p.primary_foreground, 0xffffff);
        assert_eq!(p.destructive, 0xc01c28);
        assert!(contrast(p.muted_foreground, p.background) >= MIN_MUTED_CONTRAST);
    }

    #[test]
    fn invisible_accent_falls_back_to_a_visible_colour() {
        // COSMIC dark theme here: accent equals the window colour.
        let p = palette_from_css(&css(
            "@define-color window_bg_color rgba(65,54,54,1);@define-color window_fg_color rgba(194,206,225,1);\
             @define-color accent_bg_color rgba(65,54,54,1);@define-color accent_fg_color rgba(0,0,0,1);",
        ))
        .unwrap();
        assert!(contrast(p.primary, p.background) >= MIN_ACCENT_CONTRAST);
        assert!(contrast(p.primary_foreground, p.primary) >= 4.5);
    }

    #[test]
    fn needs_background_and_foreground() {
        assert!(palette_from_css(&css("@define-color accent_bg_color #fff;")).is_none());
    }

    #[test]
    fn preset_is_dark_when_background_is() {
        let p = palette_from_css(&css(
            "@define-color window_bg_color #1e1e1e;@define-color window_fg_color #eeeeee;",
        ))
        .unwrap();
        assert!(p.to_preset().is_dark());
    }
}

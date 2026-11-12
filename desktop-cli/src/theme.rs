//! Colours and small helpers shared by every drawing module.
//!
//! The palette is picked once at startup (`--theme`, `auto` by default). `auto`
//! asks the terminal for its background colour (OSC 11) and builds the surfaces
//! on top of it, with accents taken from the terminal's own 16-colour palette, so
//! the client sits inside the terminal theme instead of painting a dark box into
//! it. Named presets are there for a fixed look, and Tokyo Night stays the default
//! when the terminal does not answer the query.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use chrono::{DateTime, Local};
use ratatui::style::Color;

/// The colour slots every drawing module reads through the accessors below.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub bg: Color,
    pub bg_dark: Color,
    pub surface: Color,
    pub select: Color,
    pub border: Color,
    pub fg: Color,
    pub dim: Color,
    /// Text drawn on top of a filled accent (pills, badges, tabs, avatars).
    pub on_accent: Color,
    pub blue: Color,
    pub green: Color,
    pub yellow: Color,
    pub red: Color,
    pub cyan: Color,
    pub magenta: Color,
    pub orange: Color,
}

impl Palette {
    /// Tokyo Night, the palette the client shipped with.
    pub const TOKYO: Palette = Palette {
        bg: Color::Rgb(0x1a, 0x1b, 0x26),
        bg_dark: Color::Rgb(0x16, 0x16, 0x1e),
        surface: Color::Rgb(0x1f, 0x23, 0x35),
        select: Color::Rgb(0x28, 0x34, 0x57),
        border: Color::Rgb(0x3b, 0x42, 0x61),
        fg: Color::Rgb(0xc0, 0xca, 0xf5),
        dim: Color::Rgb(0x56, 0x5f, 0x89),
        on_accent: Color::Rgb(0x16, 0x16, 0x1e),
        blue: Color::Rgb(0x7a, 0xa2, 0xf7),
        green: Color::Rgb(0x9e, 0xce, 0x6a),
        yellow: Color::Rgb(0xe0, 0xaf, 0x68),
        red: Color::Rgb(0xf7, 0x76, 0x8e),
        cyan: Color::Rgb(0x7d, 0xcf, 0xff),
        magenta: Color::Rgb(0xbb, 0x9a, 0xf7),
        orange: Color::Rgb(0xff, 0x9e, 0x64),
    };

    pub const CATPPUCCIN: Palette = Palette {
        bg: Color::Rgb(0x1e, 0x1e, 0x2e),
        bg_dark: Color::Rgb(0x18, 0x18, 0x25),
        surface: Color::Rgb(0x31, 0x32, 0x44),
        select: Color::Rgb(0x45, 0x47, 0x5a),
        border: Color::Rgb(0x58, 0x5b, 0x70),
        fg: Color::Rgb(0xcd, 0xd6, 0xf4),
        dim: Color::Rgb(0x7f, 0x84, 0x9c),
        on_accent: Color::Rgb(0x11, 0x11, 0x1b),
        blue: Color::Rgb(0x89, 0xb4, 0xfa),
        green: Color::Rgb(0xa6, 0xe3, 0xa1),
        yellow: Color::Rgb(0xf9, 0xe2, 0xaf),
        red: Color::Rgb(0xf3, 0x8b, 0xa8),
        cyan: Color::Rgb(0x89, 0xdc, 0xeb),
        magenta: Color::Rgb(0xcb, 0xa6, 0xf7),
        orange: Color::Rgb(0xfa, 0xb3, 0x87),
    };

    pub const GRUVBOX: Palette = Palette {
        bg: Color::Rgb(0x28, 0x28, 0x28),
        bg_dark: Color::Rgb(0x1d, 0x20, 0x21),
        surface: Color::Rgb(0x32, 0x30, 0x2f),
        select: Color::Rgb(0x3c, 0x38, 0x36),
        border: Color::Rgb(0x50, 0x49, 0x45),
        fg: Color::Rgb(0xeb, 0xdb, 0xb2),
        dim: Color::Rgb(0x92, 0x83, 0x74),
        on_accent: Color::Rgb(0x1d, 0x20, 0x21),
        blue: Color::Rgb(0x83, 0xa5, 0x98),
        green: Color::Rgb(0xb8, 0xbb, 0x26),
        yellow: Color::Rgb(0xfa, 0xbd, 0x2f),
        red: Color::Rgb(0xfb, 0x49, 0x34),
        cyan: Color::Rgb(0x8e, 0xc0, 0x7c),
        magenta: Color::Rgb(0xd3, 0x86, 0x9b),
        orange: Color::Rgb(0xfe, 0x80, 0x19),
    };

    pub const NORD: Palette = Palette {
        bg: Color::Rgb(0x2e, 0x34, 0x40),
        bg_dark: Color::Rgb(0x29, 0x2e, 0x39),
        surface: Color::Rgb(0x3b, 0x42, 0x52),
        select: Color::Rgb(0x43, 0x4c, 0x5e),
        border: Color::Rgb(0x4c, 0x56, 0x6a),
        fg: Color::Rgb(0xd8, 0xde, 0xe9),
        dim: Color::Rgb(0x7b, 0x88, 0xa1),
        on_accent: Color::Rgb(0x2e, 0x34, 0x40),
        blue: Color::Rgb(0x81, 0xa1, 0xc1),
        green: Color::Rgb(0xa3, 0xbe, 0x8c),
        yellow: Color::Rgb(0xeb, 0xcb, 0x8b),
        red: Color::Rgb(0xbf, 0x61, 0x6a),
        cyan: Color::Rgb(0x88, 0xc0, 0xd0),
        magenta: Color::Rgb(0xb4, 0x8e, 0xad),
        orange: Color::Rgb(0xd0, 0x87, 0x70),
    };
}

/// Assumed background when the terminal does not say (`--theme dark|light`).
const DARK_BG: Rgb = (0x1a, 0x1b, 0x26);
const LIGHT_BG: Rgb = (0xf7, 0xf7, 0xf9);

type Rgb = (u8, u8, u8);

static ACTIVE: OnceLock<Palette> = OnceLock::new();

/// Choose the palette. `spec` is the value of `--theme`:
/// `auto` (default), `dark`, `light`, `tokyo`, `catppuccin`, `gruvbox`, `nord`.
pub fn init(spec: &str) {
    let choice = spec.trim().to_ascii_lowercase();
    let palette = match choice.as_str() {
        "" | "auto" => auto(),
        "dark" => adaptive(probe_bg().unwrap_or(DARK_BG), false),
        "light" => adaptive(probe_bg().unwrap_or(LIGHT_BG), true),
        "tokyo" | "tokyo-night" => Palette::TOKYO,
        "catppuccin" | "mocha" => Palette::CATPPUCCIN,
        "gruvbox" | "gruvbox-dark" => Palette::GRUVBOX,
        "nord" => Palette::NORD,
        other => {
            eprintln!("--theme {other}: unknown theme, using auto ({})", names());
            auto()
        }
    };
    let _ = ACTIVE.set(palette);
}

/// The names `--theme` accepts, for `--help`.
pub fn names() -> &'static str {
    "auto|dark|light|tokyo|catppuccin|gruvbox|nord"
}

fn palette() -> Palette {
    *ACTIVE.get_or_init(|| Palette::TOKYO)
}

/// Follow the terminal: its background becomes the canvas, its palette the accents.
fn auto() -> Palette {
    match probe_bg() {
        Some(bg) => adaptive(bg, luminance(bg) > 0.5),
        // No answer (not a tty, tmux without passthrough): keep the shipped look.
        None => Palette::TOKYO,
    }
}

/// A palette that sits on `bg`. Surfaces are `bg` nudged towards the text colour;
/// accents are the terminal's own ANSI colours so they follow the theme.
fn adaptive(bg: Rgb, light: bool) -> Palette {
    let towards = if light { (0, 0, 0) } else { (255, 255, 255) };
    let shade = |t: f32| mix(bg, towards, t);
    // Bright ANSI colours are the readable ones on a dark background, the normal
    // ones on a light background: normal red on Gruvbox dark is nearly black.
    let ansi = |normal: u8, bright: u8| Color::Indexed(if light { normal } else { bright });
    Palette {
        // Reset leaves the terminal's own background alone: no painted box.
        bg: Color::Reset,
        bg_dark: mix(bg, (0, 0, 0), if light { 0.045 } else { 0.15 }),
        surface: shade(if light { 0.05 } else { 0.055 }),
        select: shade(if light { 0.11 } else { 0.115 }),
        border: shade(0.2),
        fg: Color::Reset,
        dim: shade(0.42),
        on_accent: Color::Indexed(if light { 15 } else { 0 }),
        blue: ansi(4, 12),
        green: ansi(2, 10),
        yellow: ansi(3, 11),
        red: ansi(1, 9),
        cyan: ansi(6, 14),
        magenta: ansi(5, 13),
        orange: ansi(3, 3),
    }
}

/// `a` blended towards `b` by `t` (0..=1).
fn mix(a: Rgb, b: Rgb, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let ch = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    Color::Rgb(ch(a.0, b.0), ch(a.1, b.1), ch(a.2, b.2))
}

fn luminance(bg: Rgb) -> f32 {
    (0.2126 * bg.0 as f32 + 0.7152 * bg.1 as f32 + 0.0722 * bg.2 as f32) / 255.0
}

/// Ask the terminal for its background colour with OSC 11.
///
/// The reply (`\x1b]11;rgb:1a1b/1b1b/2626\x07`, or with a string terminator) is
/// read here, before the input thread starts, so nothing else can swallow it.
/// Terminals that do not know the query stay silent and the lookup times out.
#[cfg(unix)]
fn probe_bg() -> Option<Rgb> {
    use std::io::{IsTerminal, Write};
    use std::os::unix::io::AsRawFd;

    // Piped output has no terminal behind it to ask, and nothing should be written
    // to a pipe (tests, `wabot-cli | tee`).
    if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
        return None;
    }
    let mut out = std::io::stdout();
    write!(out, "\x1b]11;?\x1b\\").ok()?;
    out.flush().ok()?;

    let mut stdin = std::io::stdin();
    // Non-blocking: a terminal that ignores the query sends nothing, and a resize
    // signal during startup must not park us inside read(). Flags are handed back
    // before the input thread takes over.
    let fd = stdin.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return None;
    }
    unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
    let reply = read_reply(&mut stdin);
    unsafe { libc::fcntl(fd, libc::F_SETFL, flags) };
    reply
}

/// Windows keeps the shipped palette: the console is not asked this way.
#[cfg(not(unix))]
fn probe_bg() -> Option<Rgb> {
    None
}

fn read_reply(stdin: &mut impl std::io::Read) -> Option<Rgb> {
    let deadline = Instant::now() + Duration::from_millis(150);
    let mut reply = Vec::new();
    let mut byte = [0u8; 1];
    while Instant::now() < deadline {
        match stdin.read(&mut byte) {
            Ok(1) => {
                reply.push(byte[0]);
                if reply.ends_with(&[0x07]) || reply.ends_with(b"\x1b\\") {
                    return parse_osc11(&reply);
                }
            }
            // Nothing there yet: wait a little, but not forever.
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(2));
            }
            Ok(_) | Err(_) => break,
        }
    }
    // The reply may have no terminator we know (a bare ESC): parse what arrived.
    parse_osc11(&reply)
}

/// Pull `rgb:rrrr/gggg/bbbb` (2 or 4 hex digits per channel) out of the reply.
fn parse_osc11(reply: &[u8]) -> Option<Rgb> {
    let text = String::from_utf8_lossy(reply);
    // Only answer 11 (background): 10 is the foreground, which some terminals also send.
    let start = text.find("]11;")?;
    let at = start + text[start..].find("rgb:")? + 4;
    let mut parts = text[at..].split('/');
    let mut channel = || {
        let raw = parts.next()?.trim_end_matches(|c: char| !c.is_ascii_hexdigit());
        let digits = &raw[..raw.len().min(4)];
        let value = u32::from_str_radix(digits, 16).ok()?;
        Some(match digits.len() {
            1 => (value * 17) as u8,
            2 => value as u8,
            3 => (value >> 4) as u8,
            _ => (value >> 8) as u8,
        })
    };
    Some((channel()?, channel()?, channel()?))
}

/// The canvas colour (the terminal's own, under `auto`).
pub fn bg() -> Color {
    palette().bg
}
/// Sidebar, input box, sheet backgrounds.
pub fn bg_dark() -> Color {
    palette().bg_dark
}
/// Message bubbles, statusline, hints.
pub fn surface() -> Color {
    palette().surface
}
/// Own messages and the selected row.
pub fn select() -> Color {
    palette().select
}
pub fn border() -> Color {
    palette().border
}
pub fn fg() -> Color {
    palette().fg
}
pub fn dim() -> Color {
    palette().dim
}
/// Text drawn on a filled accent.
pub fn on_accent() -> Color {
    palette().on_accent
}
pub fn blue() -> Color {
    palette().blue
}
pub fn green() -> Color {
    palette().green
}
pub fn yellow() -> Color {
    palette().yellow
}
pub fn red() -> Color {
    palette().red
}
pub fn cyan() -> Color {
    palette().cyan
}
pub fn magenta() -> Color {
    palette().magenta
}

/// A stable colour for a name (sender names, avatar tiles).
pub fn name_color(name: &str) -> Color {
    let p = palette();
    let colors = [p.cyan, p.magenta, p.yellow, p.orange, p.blue, p.red];
    let h = name.bytes().fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
    colors[h as usize % colors.len()]
}

pub fn local(ts_ms: i64) -> Option<DateTime<Local>> {
    DateTime::from_timestamp_millis(ts_ms).map(|d| d.with_timezone(&Local))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osc11_reply_is_read_in_every_shape() {
        // 16-bit channels: each byte repeated, as foot and kitty send them.
        assert_eq!(parse_osc11(b"\x1b]11;rgb:1a1a/1b1b/2626\x07"), Some((0x1a, 0x1b, 0x26)));
        assert_eq!(parse_osc11(b"\x1b]11;rgb:1a/1b/26\x1b\\"), Some((0x1a, 0x1b, 0x26)));
        assert_eq!(parse_osc11(b"\x1b]11;rgb:f7/f7/f7\x07x"), Some((0xf7, 0xf7, 0xf7)));
        assert_eq!(parse_osc11(b"\x1b]10;rgb:ffff/ffff/ffff\x07"), None);
        assert_eq!(parse_osc11(b"\x1b]11;rgb:zz/1b/26\x07"), None);
        assert_eq!(parse_osc11(b""), None);
    }

    #[test]
    fn mix_walks_between_the_ends() {
        assert_eq!(mix((0x10, 0x20, 0x30), (0xff, 0xff, 0xff), 0.0), Color::Rgb(0x10, 0x20, 0x30));
        assert_eq!(mix((0x10, 0x20, 0x30), (0xff, 0xff, 0xff), 1.0), Color::Rgb(0xff, 0xff, 0xff));
        assert_eq!(mix((0, 0, 0), (100, 100, 100), 0.5), Color::Rgb(50, 50, 50));
        // Out of range does not wrap.
        assert_eq!(mix((0, 0, 0), (255, 255, 255), 2.0), Color::Rgb(255, 255, 255));
    }

    #[test]
    fn auto_palettes_follow_the_background() {
        let dark = adaptive((0x1a, 0x1b, 0x26), false);
        assert_eq!(dark.bg, Color::Reset, "the terminal keeps its own background");
        assert_eq!(dark.fg, Color::Reset);
        assert_eq!(dark.on_accent, Color::Indexed(0));

        let light = adaptive((0xf7, 0xf7, 0xf9), true);
        assert_eq!(light.bg, Color::Reset);
        assert_eq!(light.on_accent, Color::Indexed(15));
        // Surfaces go the other way: darker than the paper.
        assert!(matches!(light.surface, Color::Rgb(r, _, _) if r < 0xf7));
        assert!(matches!(dark.surface, Color::Rgb(r, _, _) if r > 0x1a));
        assert!(matches!(dark.bg_dark, Color::Rgb(r, _, _) if r < 0x1a));
        assert_ne!(light.surface, light.select, "own messages stay distinguishable");
    }

    #[test]
    fn every_theme_name_loads() {
        for name in names().split('|') {
            init(name);
            assert!(matches!(bg(), Color::Reset | Color::Rgb(..)));
            assert!(!name_color("Budi").eq(&Color::Reset), "{name} must give names a hue");
        }
        init("nonsense");
        assert!(matches!(bg(), Color::Reset | Color::Rgb(..)), "an unknown name falls back");
    }

    #[test]
    fn name_color_is_stable_and_shared_out() {
        assert_eq!(name_color("Budi"), name_color("Budi"));
        let used: Vec<Color> = ["Budi", "Sari", "Andi", "Rina", "Tono", "Wati"]
            .iter()
            .map(|n| name_color(n))
            .collect();
        for c in &used {
            assert!(!matches!(c, Color::Reset), "a name keeps a visible hue");
        }
    }
}

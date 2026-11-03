//! The delivery ticks. WhatsApp's double tick is two overlapping checks, which
//! two plain cells cannot draw: `✓✓` comes out as two separate, jagged marks.
//! A Nerd Font has a single-cell "check-all" glyph that looks right, so it is used
//! when one is installed (`--ticks nerd|unicode|ascii|auto` overrides the guess).

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Style {
    Unicode = 0,
    Nerd = 1,
    Ascii = 2,
}

static STYLE: AtomicU8 = AtomicU8::new(Style::Unicode as u8);

fn style() -> Style {
    match STYLE.load(Ordering::Relaxed) {
        1 => Style::Nerd,
        2 => Style::Ascii,
        _ => Style::Unicode,
    }
}

/// Pick the tick style once at startup.
pub fn init(mode: &str) {
    let chosen = match mode {
        "nerd" => Style::Nerd,
        "ascii" => Style::Ascii,
        "unicode" => Style::Unicode,
        _ if nerd_font_installed() => Style::Nerd,
        _ => Style::Unicode,
    };
    STYLE.store(chosen as u8, Ordering::Relaxed);
}

/// Whether fontconfig knows a Nerd Font. Terminals fall back to it for the
/// private-use glyphs, so this is a good guess even when the terminal's own font is not one.
fn nerd_font_installed() -> bool {
    std::process::Command::new("fc-list")
        .args([":", "family"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase().contains("nerd font"))
        .unwrap_or(false)
}

/// Sent (one tick).
pub fn single() -> &'static str {
    match style() {
        Style::Nerd => "\u{f012c}",
        Style::Unicode => "✓",
        Style::Ascii => "+",
    }
}

/// Delivered or read (two ticks).
pub fn double() -> &'static str {
    match style() {
        Style::Nerd => "\u{f012d}",
        Style::Unicode => "✓✓",
        Style::Ascii => "++",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    #[test]
    fn every_style_has_a_tick_and_the_nerd_ones_take_one_cell() {
        for mode in ["unicode", "nerd", "ascii"] {
            init(mode);
            assert!(!single().is_empty() && !double().is_empty());
        }
        init("nerd");
        assert_eq!((single().width(), double().width()), (1, 1), "layout counts cells");
        init("unicode");
        assert_eq!(double(), "✓✓");
    }
}

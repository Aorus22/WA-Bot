//! WhatsApp text formatting: `*bold*`, `_italic_`, `~strike~`, `` `mono` `` and
//! ```` ```block``` ````.
//!
//! `parse` strips the markers and reports which byte ranges of the result carry
//! which style. It also maps byte offsets of the source onto the result, so
//! ranges computed against the unformatted text (mentions) can follow along.
//! The preference is a global so the settings view and the bubbles agree.

use std::ops::Range;

use gpui::{App, Global};
use wabot_settings::DesktopSettings;

/// Whether formatting is rendered, as a global so any view can resolve it.
pub struct FormatPref(pub bool);
impl Global for FormatPref {}

/// Formatting is on unless the user turned it off.
pub fn enabled(cx: &App) -> bool {
    cx.try_global::<FormatPref>().map_or(true, |p| p.0)
}

/// Change the preference: update the global and persist it.
pub fn set_enabled(cx: &mut App, enabled: bool) {
    cx.set_global(FormatPref(enabled));
    if let Ok(mut settings) = DesktopSettings::load() {
        settings.format_messages = enabled;
        let _ = settings.save();
    }
    cx.refresh_windows();
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub mono: bool,
}

impl Style {
    fn is_plain(self) -> bool {
        self == Self::default()
    }
}

/// The text with its markers removed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Formatted {
    pub text: String,
    /// Styled byte ranges of `text`, ascending and non-overlapping.
    pub spans: Vec<(Range<usize>, Style)>,
    /// For every byte offset of the source (and its end), the matching offset in `text`.
    offsets: Vec<usize>,
}

impl Formatted {
    /// Where a source offset lands in `text`.
    pub fn map(&self, source_offset: usize) -> usize {
        self.offsets.get(source_offset).copied().unwrap_or(self.text.len())
    }

    pub fn map_range(&self, range: &Range<usize>) -> Range<usize> {
        self.map(range.start)..self.map(range.end)
    }
}

pub fn parse(source: &str) -> Formatted {
    let mut out = Formatted {
        text: String::with_capacity(source.len()),
        spans: Vec::new(),
        offsets: vec![0; source.len() + 1],
    };
    parse_range(source, 0..source.len(), Style::default(), &mut out);
    out.offsets[source.len()] = out.text.len();
    out
}

fn emit(out: &mut Formatted, source: &str, range: Range<usize>, style: Style) {
    let start = out.text.len();
    for offset in &mut out.offsets[range.clone()] {
        *offset = start;
    }
    out.text.push_str(&source[range]);
    let end = out.text.len();
    if end == start || style.is_plain() {
        return;
    }
    match out.spans.last_mut() {
        Some((last, last_style)) if last.end == start && *last_style == style => last.end = end,
        _ => out.spans.push((start..end, style)),
    }
}

/// Marks source bytes that are dropped from the output.
fn skip(out: &mut Formatted, range: Range<usize>) {
    let at = out.text.len();
    for offset in &mut out.offsets[range] {
        *offset = at;
    }
}

fn style_for(marker: char, style: Style) -> Style {
    match marker {
        '*' => Style { bold: true, ..style },
        '_' => Style { italic: true, ..style },
        '~' => Style { strike: true, ..style },
        _ => Style { mono: true, ..style },
    }
}

fn before(source: &str, at: usize) -> Option<char> {
    source[..at].chars().next_back()
}

fn after(source: &str, at: usize) -> Option<char> {
    source[at..].chars().next()
}

fn word_char(c: Option<char>) -> bool {
    c.is_some_and(char::is_alphanumeric)
}

/// Finds the marker closing one opened at `open`, if the text between is formatted text.
fn find_close(source: &str, marker: char, open: usize, end: usize) -> Option<usize> {
    let inner_start = open + marker.len_utf8();
    for (offset, c) in source[inner_start..end].char_indices() {
        if c == '\n' {
            return None;
        }
        let at = inner_start + offset;
        let next = at + c.len_utf8();
        if c == marker
            && at > inner_start
            && !before(source, at).is_some_and(char::is_whitespace)
            && !word_char(after(source, next).filter(|_| next < end))
        {
            return Some(at);
        }
    }
    None
}

fn parse_range(source: &str, range: Range<usize>, style: Style, out: &mut Formatted) {
    let end = range.end;
    let mut i = range.start;
    while i < end {
        let c = after(source, i).expect("offset inside the source");
        let width = c.len_utf8();

        if !style.mono && source[i..end].starts_with("```") {
            if let Some(close) = source[i + 3..end].find("```").map(|p| i + 3 + p).filter(|&close| close > i + 3) {
                skip(out, i..i + 3);
                emit(out, source, i + 3..close, Style { mono: true, ..style });
                skip(out, close..close + 3);
                i = close + 3;
                continue;
            }
        }

        let opens = matches!(c, '*' | '_' | '~' | '`')
            && !(c == '`' && style.mono)
            && !word_char(before(source, i))
            && after(source, i + width).filter(|_| i + width < end).is_some_and(|n| !n.is_whitespace() && n != c);
        if opens {
            if let Some(close) = find_close(source, c, i, end) {
                skip(out, i..i + width);
                let inner = style_for(c, style);
                if c == '`' {
                    emit(out, source, i + width..close, inner);
                } else {
                    parse_range(source, i + width..close, inner, out);
                }
                skip(out, close..close + width);
                i = close + width;
                continue;
            }
        }

        emit(out, source, i..i + width, style);
        i += width;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bold() -> Style {
        Style { bold: true, ..Style::default() }
    }

    #[test]
    fn strips_markers_and_reports_styled_ranges() {
        let f = parse("a *bold* ~gone~ _it_ `mono` z");
        assert_eq!(f.text, "a bold gone it mono z");
        assert_eq!(
            f.spans,
            vec![
                (2..6, bold()),
                (7..11, Style { strike: true, ..Style::default() }),
                (12..14, Style { italic: true, ..Style::default() }),
                (15..19, Style { mono: true, ..Style::default() }),
            ]
        );
    }

    #[test]
    fn tilde_is_strikethrough_and_underscore_is_italic() {
        let f = parse("~text~");
        assert_eq!(f.spans, vec![(0..4, Style { strike: true, ..Style::default() })]);
        let f = parse("_text_");
        assert_eq!(f.spans, vec![(0..4, Style { italic: true, ..Style::default() })]);
    }

    #[test]
    fn markers_need_word_boundaries() {
        for plain in ["snake_case_name", "2 * 3 * 4", "a*b*c", "*not closed", "** **", "_ spaced _", "~~"] {
            let f = parse(plain);
            assert_eq!(f.text, plain, "{plain}");
            assert!(f.spans.is_empty(), "{plain}");
        }
    }

    #[test]
    fn formatting_does_not_cross_lines() {
        let f = parse("*one\ntwo*");
        assert_eq!(f.text, "*one\ntwo*");
        assert!(f.spans.is_empty());
    }

    #[test]
    fn styles_nest() {
        let f = parse("*_both_* and *bold _inner_*");
        assert_eq!(f.text, "both and bold inner");
        assert_eq!(f.spans[0], (0..4, Style { bold: true, italic: true, ..Style::default() }));
        assert_eq!(f.spans[1], (9..14, bold()));
        assert_eq!(f.spans[2], (14..19, Style { bold: true, italic: true, ..Style::default() }));
    }

    #[test]
    fn monospace_keeps_its_content_literal() {
        let f = parse("`*not bold*` and ```a\n_b_```");
        assert_eq!(f.text, "*not bold* and a\n_b_");
        assert!(f.spans.iter().all(|(_, s)| s.mono && !s.bold && !s.italic));
    }

    #[test]
    fn source_offsets_follow_the_stripped_text() {
        let source = "hi *@Budi* ok";
        let f = parse(source);
        assert_eq!(f.text, "hi @Budi ok");
        let mention = source.find('@').unwrap()..source.find("* ok").unwrap();
        assert_eq!(&f.text[f.map_range(&mention)], "@Budi");
        assert_eq!(f.map(source.len()), f.text.len());
    }

    #[test]
    fn handles_multibyte_text() {
        let f = parse("héllo *wörld* 🙂 _ok_");
        assert_eq!(f.text, "héllo wörld 🙂 ok");
        assert_eq!(&f.text[f.spans[0].0.clone()], "wörld");
        assert_eq!(&f.text[f.spans[1].0.clone()], "ok");
    }
}

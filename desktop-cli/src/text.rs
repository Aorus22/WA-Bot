//! Width-aware text helpers (terminal cells, not chars).

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Cut `s` to at most `max` cells, ending with `…` when something was dropped.
pub fn truncate(s: &str, max: usize) -> String {
    let s = s.replace(['\n', '\r'], " ");
    if width(&s) <= max {
        return s;
    }
    if max == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for ch in s.chars() {
        let w = ch.width().unwrap_or(0);
        if used + w > max - 1 {
            break;
        }
        out.push(ch);
        used += w;
    }
    out.push('…');
    out
}

/// Word-wrap to `max` cells. Keeps explicit newlines, splits words longer than a
/// line. Always returns at least one line.
pub fn wrap(text: &str, max: usize) -> Vec<String> {
    let max = max.max(1);
    let mut lines = Vec::new();
    for raw in text.replace('\r', "").split('\n') {
        let mut line = String::new();
        let mut used = 0;
        for word in raw.split(' ') {
            let ww = width(word);
            // A word that cannot fit on any line is cut char by char.
            if ww > max {
                if used > 0 {
                    lines.push(std::mem::take(&mut line));
                    used = 0;
                }
                for ch in word.chars() {
                    let w = ch.width().unwrap_or(0);
                    if used + w > max {
                        lines.push(std::mem::take(&mut line));
                        used = 0;
                    }
                    line.push(ch);
                    used += w;
                }
                continue;
            }
            let extra = if used == 0 { 0 } else { 1 };
            if used + extra + ww > max {
                lines.push(std::mem::take(&mut line));
                used = 0;
            }
            if used > 0 {
                line.push(' ');
                used += 1;
            }
            line.push_str(word);
            used += ww;
        }
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_by_cells() {
        assert_eq!(truncate("halo dunia", 6), "halo …");
        assert_eq!(truncate("halo", 6), "halo");
        assert_eq!(truncate("a\nb", 5), "a b");
    }

    #[test]
    fn wraps_words_and_long_runs() {
        assert_eq!(wrap("aaa bbb ccc", 7), vec!["aaa bbb", "ccc"]);
        assert_eq!(wrap("abcdefghij", 4), vec!["abcd", "efgh", "ij"]);
        assert_eq!(wrap("a\n\nb", 4), vec!["a", "", "b"]);
        assert_eq!(wrap("", 4), vec![""]);
    }

    #[test]
    fn wide_chars_count_two_cells() {
        assert_eq!(wrap("日本語日本語", 6), vec!["日本語", "日本語"]);
    }
}

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

/// A link inside one wrapped row: byte range of the visible text and the full address
/// (a link cut across rows has the same address on each of them).
#[derive(Clone, Debug, PartialEq)]
pub struct LinkSeg {
    pub start: usize,
    pub end: usize,
    pub url: String,
}

/// One wrapped row of text and the links on it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Wrapped {
    pub text: String,
    pub links: Vec<LinkSeg>,
}

/// Word-wrap to `max` cells. Keeps explicit newlines, splits words longer than a
/// line. Always returns at least one line.
pub fn wrap(text: &str, max: usize) -> Vec<String> {
    wrap_rows(text, max).into_iter().map(|r| r.text).collect()
}

/// The link inside one word, as a byte range and an address: `https://x.org/a).`
/// is the link `https://x.org/a`; `www.x.org` gets `https://` in front.
pub fn link_in_word(word: &str) -> Option<(usize, usize, String)> {
    let lead = word.len() - word.trim_start_matches(['(', '[', '<', '"', '\'', '*', '_']).len();
    let core = word[lead..].trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']', '>', '"', '\'', '*', '_']);
    let lower = core.to_ascii_lowercase();
    let ok = ["http://", "https://", "www."].iter().any(|p| lower.starts_with(p) && core.len() > p.len());
    if !ok {
        return None;
    }
    let url = if lower.starts_with("www.") { format!("https://{core}") } else { core.to_string() };
    Some((lead, lead + core.len(), url))
}

/// Like [`wrap`], and remembers where the links are.
pub fn wrap_rows(text: &str, max: usize) -> Vec<Wrapped> {
    let max = max.max(1);
    let mut rows: Vec<Wrapped> = Vec::new();
    // Adds `part` of a word at the end of `row`, as (part of) a link when `link` says so.
    fn add(row: &mut Wrapped, part: &str, link: Option<(usize, usize, &str)>, part_at: usize) {
        let base = row.text.len();
        row.text.push_str(part);
        let Some((ls, le, url)) = link else { return };
        let (a, b) = (ls.max(part_at), le.min(part_at + part.len()));
        if a >= b {
            return;
        }
        let (a, b) = (base + a - part_at, base + b - part_at);
        match row.links.last_mut() {
            Some(last) if last.end == a && last.url == url => last.end = b,
            _ => row.links.push(LinkSeg { start: a, end: b, url: url.to_string() }),
        }
    }
    for raw in text.replace('\r', "").split('\n') {
        let mut row = Wrapped::default();
        let mut used = 0;
        for word in raw.split(' ') {
            let ww = width(word);
            let link = link_in_word(word);
            let link_ref = link.as_ref().map(|(a, b, u)| (*a, *b, u.as_str()));
            // A word that cannot fit on any line is cut char by char.
            if ww > max {
                if used > 0 {
                    rows.push(std::mem::take(&mut row));
                    used = 0;
                }
                for (at, ch) in word.char_indices() {
                    let w = ch.width().unwrap_or(0);
                    if used + w > max {
                        rows.push(std::mem::take(&mut row));
                        used = 0;
                    }
                    add(&mut row, &word[at..at + ch.len_utf8()], link_ref, at);
                    used += w;
                }
                continue;
            }
            let extra = if used == 0 { 0 } else { 1 };
            if used + extra + ww > max {
                rows.push(std::mem::take(&mut row));
                used = 0;
            }
            if used > 0 {
                row.text.push(' ');
                used += 1;
            }
            add(&mut row, word, link_ref, 0);
            used += ww;
        }
        rows.push(row);
    }
    rows
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
    fn finds_links_in_words() {
        assert_eq!(link_in_word("https://x.org/a)."), Some((0, 15, "https://x.org/a".to_string())));
        assert_eq!(link_in_word("(www.x.org),"), Some((1, 10, "https://www.x.org".to_string())));
        assert_eq!(link_in_word("http://"), None);
        assert_eq!(link_in_word("halo"), None);
    }

    #[test]
    fn wrapped_rows_keep_their_links() {
        let rows = wrap_rows("buka https://example.com/panjang sekarang", 60);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].links, vec![LinkSeg { start: 5, end: 32, url: "https://example.com/panjang".into() }]);
        // A link longer than the row is cut, and every piece still opens the whole address.
        let rows = wrap_rows("https://example.com/panjang", 12);
        assert!(rows.len() >= 3);
        for r in &rows {
            assert_eq!(r.links.len(), 1);
            assert_eq!(r.links[0].url, "https://example.com/panjang");
            assert_eq!((r.links[0].start, r.links[0].end), (0, r.text.len()));
        }
        assert_eq!(rows.iter().map(|r| r.text.as_str()).collect::<String>(), "https://example.com/panjang");
    }

    #[test]
    fn wide_chars_count_two_cells() {
        assert_eq!(wrap("日本語日本語", 6), vec!["日本語", "日本語"]);
    }
}

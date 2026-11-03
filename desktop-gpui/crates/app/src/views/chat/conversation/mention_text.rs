//! Selectable message text with coloured mention spans.
//!
//! `gpui_base::SelectableText` cannot take highlights, so this mirrors its
//! selection wiring around a `StyledText` that does.

use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    App, BorderStyle, Bounds, Corners, CursorStyle, Edges, Element, ElementId, FontWeight,
    GlobalElementId, Hitbox, HitboxBehavior, HighlightStyle, Hsla, InspectorElementId,
    IntoElement, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point,
    SharedString, StrikethroughStyle, StyledText, Subscription, TextLayout, TextRun, TextStyle,
    UnderlineStyle, Window, px, transparent_black, FontStyle,
};
use gpui_base::{
    TextSelection, TextSelectionEvent, TextSelectionHandle, TextSelectionRegistration,
    TextSelectionRun, Theme,
};

use crate::components::emoji_picker::EMOJI_FONT_FAMILY;
use crate::text_format::Style as TextFormat;

/// Called with the mentioned JID when a mention is clicked.
pub type MentionClick = Rc<dyn Fn(&str, &mut Window, &mut App)>;

/// What a right-click on the text offers: the link under the pointer and/or
/// the selected text. Raised instead of the bubble's own menu.
#[derive(Clone, Debug, PartialEq)]
pub struct TextMenuRequest {
    pub position: Point<Pixels>,
    pub link: Option<String>,
    pub selection: Option<String>,
}

pub type TextMenuHandler = Rc<dyn Fn(TextMenuRequest, &mut Window, &mut App)>;

/// Selectable text whose mentions are coloured and clickable.
pub struct MentionText {
    id: ElementId,
    text: SharedString,
    styled_text: StyledText,
    mention_style: HighlightStyle,
    formats: Vec<(Range<usize>, TextFormat)>,
    targets: Vec<(Range<usize>, SharedString)>,
    /// Web addresses in the text, opened on click.
    links: Vec<Range<usize>>,
    link_color: Hsla,
    on_click: Option<MentionClick>,
    on_text_menu: Option<TextMenuHandler>,
    selection_color: Option<Hsla>,
}

/// Per-element state kept across frames.
#[derive(Clone)]
pub struct MentionState {
    handle: TextSelectionHandle,
    over_mention: Rc<Cell<bool>>,
    /// Whether this text has part of the selection, as of the last paint.
    has_selection: Rc<Cell<bool>>,
    // Keeps the redraw-on-selection-change subscription alive with the element.
    _redraw: Rc<Subscription>,
}

impl MentionText {
    pub fn new(
        id: impl Into<ElementId>,
        text: impl Into<SharedString>,
        mentions: Vec<(Range<usize>, SharedString)>,
        color: Hsla,
    ) -> Self {
        let text: SharedString = text.into();
        let style = HighlightStyle {
            color: Some(color),
            font_weight: Some(FontWeight::MEDIUM),
            ..Default::default()
        };
        Self {
            id: id.into(),
            styled_text: StyledText::new(text.clone()),
            mention_style: style,
            formats: Vec::new(),
            links: find_links(&text),
            link_color: color,
            text,
            targets: mentions,
            on_click: None,
            on_text_menu: None,
            selection_color: None,
        }
    }

    /// Colour of web addresses, which are underlined and clickable.
    pub fn link_color(mut self, color: Hsla) -> Self {
        self.link_color = color;
        self
    }

    /// Styles ranges of the text (WhatsApp `*bold*` and friends).
    pub fn formats(mut self, formats: Vec<(Range<usize>, TextFormat)>) -> Self {
        self.formats = formats;
        self
    }

    /// Called on a right-click over a link or over selected text.
    pub fn on_text_menu(mut self, handler: impl Fn(TextMenuRequest, &mut Window, &mut App) + 'static) -> Self {
        self.on_text_menu = Some(Rc::new(handler));
        self
    }

    pub fn on_click(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// Overrides the selection background, which would otherwise be the
    /// theme's token and can disappear against a coloured bubble.
    pub fn selection_color(mut self, color: Hsla) -> Self {
        self.selection_color = Some(color);
        self
    }
}

impl MentionText {
    /// One run per stretch of text sharing the same mention and format.
    fn text_runs(&self, base: &TextStyle, mono: SharedString) -> Vec<TextRun> {
        let mut cuts = vec![0, self.text.len()];
        // Emoji runs get the bundled color font as their own family: relying on the
        // window's text-style fallback left them blank (the system COLRv1 font won).
        let emoji = emoji_ranges(&self.text);
        for range in &emoji {
            cuts.push(range.start);
            cuts.push(range.end);
        }
        let ranges = self
            .targets
            .iter()
            .map(|(r, _)| r)
            .chain(self.formats.iter().map(|(r, _)| r))
            .chain(self.links.iter());
        for range in ranges {
            cuts.push(range.start.min(self.text.len()));
            cuts.push(range.end.min(self.text.len()));
        }
        cuts.sort_unstable();
        cuts.dedup();
        cuts.windows(2)
            .map(|pair| {
                let (start, end) = (pair[0], pair[1]);
                let mut style = base.clone();
                style.font_fallbacks = Some(gpui::FontFallbacks::from_fonts(vec![
                    EMOJI_FONT_FAMILY.to_string(),
                ]));
                if emoji.iter().any(|r| r.start <= start && end <= r.end) {
                    style.font_family = SharedString::from(EMOJI_FONT_FAMILY);
                }
                if let Some((_, format)) = self.formats.iter().find(|(r, _)| r.start <= start && end <= r.end) {
                    if format.bold {
                        style.font_weight = FontWeight::BOLD;
                    }
                    if format.italic {
                        style.font_style = FontStyle::Italic;
                    }
                    if format.strike {
                        style.strikethrough = Some(StrikethroughStyle { thickness: px(1.0), color: None });
                    }
                    if format.mono && !emoji.iter().any(|r| r.start <= start && end <= r.end) {
                        style.font_family = mono.clone();
                    }
                }
                if self.targets.iter().any(|(r, _)| r.start <= start && end <= r.end) {
                    style = style.highlight(self.mention_style);
                }
                if self.links.iter().any(|r| r.start <= start && end <= r.end) {
                    style.color = self.link_color;
                    style.underline = Some(UnderlineStyle {
                        thickness: px(1.0),
                        color: Some(self.link_color),
                        wavy: false,
                    });
                }
                style.to_run(end - start)
            })
            .collect()
    }
}

/// Emoji whose default look is the color picture even without U+FE0F.
fn is_default_emoji_symbol(c: char) -> bool {
    matches!(c as u32,
        0x231A | 0x231B | 0x23E9..=0x23EC | 0x23F0 | 0x23F3 | 0x25FD | 0x25FE | 0x2614 | 0x2615
        | 0x2648..=0x2653 | 0x267F | 0x2693 | 0x26A1 | 0x26AA | 0x26AB | 0x26BD | 0x26BE | 0x26C4
        | 0x26C5 | 0x26CE | 0x26D4 | 0x26EA | 0x26F2 | 0x26F3 | 0x26F5 | 0x26FA | 0x26FD | 0x2705
        | 0x270A | 0x270B | 0x2728 | 0x274C | 0x274E | 0x2753..=0x2755 | 0x2757 | 0x2795..=0x2797
        | 0x27B0 | 0x27BF | 0x2B1B | 0x2B1C | 0x2B50 | 0x2B55)
}

fn is_regional_indicator(c: char) -> bool {
    matches!(c as u32, 0x1F1E6..=0x1F1FF)
}

/// Pictographs and flags: always emoji.
fn is_pictograph(c: char) -> bool {
    matches!(c as u32, 0x1F1E6..=0x1F1FF | 0x1F300..=0x1FAFF)
}

/// Byte ranges of the emoji in `text`: pictographs, flags, keycaps, symbols
/// shown with U+FE0F, and the ZWJ / skin tone / tag sequences built from them.
pub fn emoji_ranges(text: &str) -> Vec<Range<usize>> {
    const ZWJ: char = '\u{200D}';
    const VS16: char = '\u{FE0F}';
    const KEYCAP: char = '\u{20E3}';
    let is_tag = |c: char| matches!(c as u32, 0xE0020..=0xE007F);
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out: Vec<Range<usize>> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let (start, c) = chars[i];
        let next = chars.get(i + 1).map(|&(_, n)| n);
        let third = chars.get(i + 2).map(|&(_, n)| n);
        let keycap = (c.is_ascii_digit() || c == '#' || c == '*')
            && (next == Some(KEYCAP) || (next == Some(VS16) && third == Some(KEYCAP)));
        let symbol_with_vs16 = next == Some(VS16) && (c as u32) >= 0x80;
        let begins = is_pictograph(c) || is_default_emoji_symbol(c) || symbol_with_vs16 || keycap;
        if !begins {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while let Some(&(_, n)) = chars.get(j) {
            if n == VS16 || n == KEYCAP || is_tag(n) || matches!(n as u32, 0x1F3FB..=0x1F3FF) {
                j += 1;
            } else if n == ZWJ {
                // The joiner pulls in the next symbol, even a plain BMP one.
                j += 1;
                if chars.get(j).is_some() {
                    j += 1;
                }
            } else if j == i + 1 && is_regional_indicator(c) && is_regional_indicator(n) {
                // Two regional indicators make one flag.
                j += 1;
            } else {
                break;
            }
        }
        let end = chars.get(j).map_or(text.len(), |&(b, _)| b);
        match out.last_mut() {
            Some(last) if last.end == start => last.end = end,
            _ => out.push(start..end),
        }
        i = j;
    }
    out
}

/// Byte ranges of the web addresses in `text`: `http(s)://…` and `www.…`,
/// ending at whitespace, without the punctuation that usually follows a link.
pub fn find_links(text: &str) -> Vec<Range<usize>> {
    let mut links = Vec::new();
    let mut from = 0;
    while from < text.len() {
        let rest = &text[from..];
        let Some(offset) = ["https://", "http://", "www."]
            .iter()
            .filter_map(|scheme| rest.find(scheme))
            .min()
        else {
            break;
        };
        let start = from + offset;
        // "awww.x" is not a link: it must begin a word.
        let begins_word = text[..start].chars().next_back().map_or(true, |c| !c.is_alphanumeric());
        let end = text[start..]
            .find(char::is_whitespace)
            .map_or(text.len(), |len| start + len);
        let mut end = end;
        while let Some(last) = text[start..end].chars().next_back() {
            let unbalanced_close = match last {
                ')' => text[start..end].matches('(').count() < text[start..end].matches(')').count(),
                ']' => text[start..end].matches('[').count() < text[start..end].matches(']').count(),
                _ => false,
            };
            if matches!(last, '.' | ',' | ';' | ':' | '!' | '?' | '\'' | '"' | '>' | '}') || unbalanced_close {
                end -= last.len_utf8();
            } else {
                break;
            }
        }
        let body = &text[start..end];
        let has_host = body.split("://").last().is_some_and(|host| host.len() > 4 && host.contains('.'));
        if begins_word && has_host {
            links.push(start..end);
        }
        from = end.max(start + 1);
    }
    links
}

/// The address to open: bare `www.` links need a scheme.
fn link_target(text: &str) -> String {
    if text.starts_with("www.") { format!("https://{text}") } else { text.to_string() }
}

/// One rectangle per visual line of `range`, as wide as the selected text on
/// it, so a selection never paints the empty space after a line break.
fn selection_rects(layout: &TextLayout, text: &str, range: Range<usize>) -> Vec<Bounds<Pixels>> {
    let line_height = layout.line_height();
    let mut rects: Vec<Bounds<Pixels>> = Vec::new();
    for (offset, ch) in text[range.clone()].char_indices() {
        if ch == '\n' {
            continue;
        }
        let index = range.start + offset;
        let (Some(from), Some(to)) = (
            layout.position_for_index(index),
            layout.position_for_index(index + ch.len_utf8()),
        ) else {
            continue;
        };
        // A soft wrap reports the end of the line it closes, so `to` shares
        // `from`'s line; anything else is a glyph with no extent.
        let right = if to.y == from.y { to.x } else { from.x };
        match rects.last_mut() {
            Some(last) if last.origin.y == from.y => {
                last.size.width = right - last.origin.x;
            }
            _ => rects.push(Bounds::new(from, gpui::size(right - from.x, line_height))),
        }
    }
    rects
}

impl IntoElement for MentionText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for MentionText {
    type RequestLayoutState = MentionState;
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let window_handle = window.window_handle();
        let handle = window.with_element_state(
            global_id.expect("MentionText must have a stable element id"),
            |retained: Option<MentionState>, _| {
                let state = retained.unwrap_or_else(|| {
                    let handle = TextSelectionHandle::new(self.text.clone(), cx);
                    // Redraw as soon as the drag moves the selection, rather
                    // than waiting for the next paint to notice the change.
                    // Deferred because the window is leased while it
                    // dispatches the mouse event that got us here.
                    let redraw = handle.subscribe(
                        move |event, cx| {
                            if matches!(event, TextSelectionEvent::SelectionChanged(_)) {
                                cx.defer(move |cx| {
                                    _ = window_handle.update(cx, |_, window, _| window.refresh());
                                });
                            }
                        },
                        cx,
                    );
                    // Selecting text takes keyboard focus off whatever had it (the
                    // composer, say) so Ctrl+C reaches the window's copy action.
                    handle.focus_with(|window, cx| window.blur(cx), cx);
                    MentionState {
                        handle,
                        over_mention: Rc::new(Cell::new(false)),
                        has_selection: Rc::new(Cell::new(false)),
                        _redraw: Rc::new(redraw),
                    }
                });
                (state.clone(), state)
            },
        );
        let runs = self.text_runs(&window.text_style(), Theme::global(cx).tokens.typography.mono.clone());
        self.styled_text = StyledText::new(self.text.clone()).with_runs(runs);
        let (layout_id, ()) = self.styled_text.request_layout(global_id, inspector_id, window, cx);
        (layout_id, handle)
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.styled_text.prepaint(global_id, inspector_id, bounds, &mut (), window, cx);
        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        let registration = TextSelectionRegistration::new(hitbox.clone(), bounds)
            .with_text_bounds(vec![bounds])
            .with_rendered_element(&state.handle, window, cx);
        state.handle.register(registration, window, cx);
        hitbox
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let layout = self.styled_text.layout().clone();
        let projection = state.handle.update_runs(
            &[TextSelectionRun::new(self.text.clone(), layout.clone(), bounds)],
            cx,
        );
        let color = self.selection_color.unwrap_or_else(|| Theme::global(cx).tokens.colors.selection);
        let mut selected_here = false;
        for range in projection.ranges().iter().flatten().cloned() {
            for quad in selection_rects(&layout, &self.text, range) {
                selected_here = true;
                window.paint_quad(PaintQuad {
                    bounds: quad,
                    background: color.into(),
                    corner_radii: Corners::default(),
                    border_widths: Edges::default(),
                    border_color: transparent_black(),
                    border_style: BorderStyle::default(),
                });
            }
        }
        state.has_selection.set(selected_here);
        self.styled_text.paint(global_id, inspector_id, bounds, &mut (), &mut (), window, cx);

        // Text reads as selectable text, and mentions as links.
        // What the pointer is over: a mention (when clickable) or a web address.
        #[derive(Clone)]
        enum Hit {
            Mention(SharedString),
            Link(String),
        }
        let targets = if self.on_click.is_some() { self.targets.clone() } else { Vec::new() };
        let links: Vec<(Range<usize>, String)> = self
            .links
            .iter()
            .map(|range| (range.clone(), link_target(&self.text[range.clone()])))
            .collect();
        let hit_at = {
            let layout = layout.clone();
            move |position: Point<Pixels>| -> Option<Hit> {
                let index = layout.index_for_position(position).ok()?;
                targets
                    .iter()
                    .find(|(range, _)| range.contains(&index))
                    .map(|(_, jid)| Hit::Mention(jid.clone()))
                    .or_else(|| {
                        links
                            .iter()
                            .find(|(range, _)| range.contains(&index))
                            .map(|(_, url)| Hit::Link(url.clone()))
                    })
            }
        };
        let over_link = hitbox.is_hovered(window) && hit_at(window.mouse_position()).is_some();
        window.set_cursor_style(
            if over_link { CursorStyle::PointingHand } else { CursorStyle::IBeam },
            hitbox,
        );
        if let Some(on_text_menu) = self.on_text_menu.clone() {
            window.on_mouse_event({
                let (hitbox, hit_at, has_selection) =
                    (hitbox.clone(), hit_at.clone(), state.has_selection.clone());
                move |event: &MouseDownEvent, phase, window, cx| {
                    if !phase.bubble() || event.button != MouseButton::Right || !hitbox.is_hovered(window) {
                        return;
                    }
                    let link = match hit_at(event.position) {
                        Some(Hit::Link(url)) => Some(url),
                        _ => None,
                    };
                    let selection = has_selection
                        .get()
                        .then(|| TextSelection::selected_text(window, cx))
                        .filter(|text| !text.trim().is_empty());
                    if link.is_none() && selection.is_none() {
                        return;
                    }
                    // Not the bubble's menu: this one is about the text.
                    cx.stop_propagation();
                    on_text_menu(TextMenuRequest { position: event.position, link, selection }, window, cx);
                }
            });
        }
        if self.on_click.is_none() && self.links.is_empty() {
            return;
        }
        let on_click = self.on_click.clone();
        window.on_mouse_event({
            let (hitbox, hit_at, over) = (hitbox.clone(), hit_at.clone(), state.over_mention.clone());
            move |event: &MouseMoveEvent, _, window, _| {
                let now = hitbox.is_hovered(window) && hit_at(event.position).is_some();
                if over.replace(now) != now {
                    window.refresh();
                }
            }
        });
        window.on_mouse_event({
            let hitbox = hitbox.clone();
            move |event: &MouseUpEvent, phase, window, cx| {
                // A drag that selected text is not a click.
                if phase.bubble()
                    && event.button == MouseButton::Left
                    && event.click_count == 1
                    && hitbox.is_hovered(window)
                    && TextSelection::selected_text(window, cx).is_empty()
                {
                    match hit_at(event.position) {
                        Some(Hit::Mention(jid)) => {
                            if let Some(on_click) = &on_click {
                                on_click(&jid, window, cx);
                            }
                        }
                        Some(Hit::Link(url)) => cx.open_url(&url),
                        None => {}
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn links(text: &str) -> Vec<&str> {
        find_links(text).into_iter().map(|r| &text[r]).collect()
    }

    fn emoji(text: &str) -> Vec<&str> {
        emoji_ranges(text).into_iter().map(|r| &text[r]).collect()
    }

    #[test]
    fn finds_emoji_and_leaves_plain_text() {
        assert_eq!(emoji("🗿"), vec!["🗿"]);
        assert_eq!(emoji("halo 👍🏽 ya"), vec!["👍🏽"]);
        assert_eq!(emoji("a ❤️ b ❤ c"), vec!["❤️"]);
        assert_eq!(emoji("✅ done ⭐"), vec!["✅", "⭐"]);
        assert_eq!(emoji("1️⃣ dan #️⃣ tapi 12 # *"), vec!["1️⃣", "#️⃣"]);
        assert_eq!(emoji("🇮🇩🇯🇵"), vec!["🇮🇩🇯🇵"]);
        assert_eq!(emoji("👩\u{200d}❤\u{fe0f}\u{200d}👨!"), vec!["👩\u{200d}❤\u{fe0f}\u{200d}👨"]);
        assert!(emoji("plain text, © ™ → ✓ 123").is_empty());
        assert!(emoji("").is_empty());
    }

    #[test]
    fn finds_addresses_and_leaves_trailing_punctuation() {
        assert_eq!(
            links("https://www.instagram.com/p/DeZOc8KkyHX/\n?rpxt=MXZmcjFoemIweXAwNQ=="),
            vec!["https://www.instagram.com/p/DeZOc8KkyHX/"]
        );
        assert_eq!(links("lihat https://example.com/a?b=1, ya."), vec!["https://example.com/a?b=1"]);
        assert_eq!(links("(http://example.com/x) dan www.contoh.id!"), vec!["http://example.com/x", "www.contoh.id"]);
        assert_eq!(links("https://en.wikipedia.org/wiki/Foo_(bar)"), vec!["https://en.wikipedia.org/wiki/Foo_(bar)"]);
    }

    #[test]
    fn ignores_text_that_only_looks_like_a_link() {
        assert!(links("awww.example.com bukan link").is_empty());
        assert!(links("http:// kosong").is_empty());
        assert!(links("tanpa tautan sama sekali").is_empty());
    }

    #[test]
    fn bare_www_gets_a_scheme() {
        assert_eq!(link_target("www.contoh.id"), "https://www.contoh.id");
        assert_eq!(link_target("https://a.b"), "https://a.b");
    }
}

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
    IntoElement, LayoutId, MouseButton, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point,
    SharedString, StyledText, Subscription, Window, transparent_black,
};
use gpui_base::{
    TextSelection, TextSelectionEvent, TextSelectionHandle, TextSelectionRegistration,
    TextSelectionRun, Theme,
};

/// Called with the mentioned JID when a mention is clicked.
pub type MentionClick = Rc<dyn Fn(&str, &mut Window, &mut App)>;

/// Selectable text whose mentions are coloured and clickable.
pub struct MentionText {
    id: ElementId,
    text: SharedString,
    styled_text: StyledText,
    targets: Vec<(Range<usize>, SharedString)>,
    on_click: Option<MentionClick>,
    selection_color: Option<Hsla>,
}

/// Per-element state kept across frames.
#[derive(Clone)]
pub struct MentionState {
    handle: TextSelectionHandle,
    over_mention: Rc<Cell<bool>>,
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
        let highlights = mentions.iter().map(|(range, _)| (range.clone(), style)).collect::<Vec<_>>();
        Self {
            id: id.into(),
            styled_text: StyledText::new(text.clone()).with_highlights(highlights),
            text,
            targets: mentions,
            on_click: None,
            selection_color: None,
        }
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

fn selection_quads(start: Point<Pixels>, end: Point<Pixels>, bounds: Bounds<Pixels>, line_height: Pixels) -> Vec<Bounds<Pixels>> {
    if start.y == end.y {
        return vec![Bounds::from_corners(start, Point::new(end.x, end.y + line_height))];
    }
    let mut quads = vec![Bounds::from_corners(start, Point::new(bounds.right(), start.y + line_height))];
    if end.y > start.y + line_height {
        quads.push(Bounds::from_corners(
            Point::new(bounds.left(), start.y + line_height),
            Point::new(bounds.right(), end.y),
        ));
    }
    quads.push(Bounds::from_corners(Point::new(bounds.left(), end.y), Point::new(end.x, end.y + line_height)));
    quads
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
                    MentionState {
                        handle,
                        over_mention: Rc::new(Cell::new(false)),
                        _redraw: Rc::new(redraw),
                    }
                });
                (state.clone(), state)
            },
        );
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
        for range in projection.ranges().iter().flatten().cloned() {
            let (Some(start), Some(end)) = (
                layout.position_for_index(range.start),
                layout.position_for_index(range.end),
            ) else {
                continue;
            };
            for quad in selection_quads(start, end, layout.bounds(), layout.line_height()) {
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
        self.styled_text.paint(global_id, inspector_id, bounds, &mut (), &mut (), window, cx);

        // Text reads as selectable text, and mentions as links.
        let targets = self.targets.clone();
        let mention_at = {
            let layout = layout.clone();
            move |position: Point<Pixels>| -> Option<SharedString> {
                let index = layout.index_for_position(position).ok()?;
                targets.iter().find(|(range, _)| range.contains(&index)).map(|(_, jid)| jid.clone())
            }
        };
        let over_link = self.on_click.is_some()
            && hitbox.is_hovered(window)
            && mention_at(window.mouse_position()).is_some();
        window.set_cursor_style(
            if over_link { CursorStyle::PointingHand } else { CursorStyle::IBeam },
            hitbox,
        );
        let Some(on_click) = self.on_click.clone() else { return };
        window.on_mouse_event({
            let (hitbox, mention_at, over) = (hitbox.clone(), mention_at.clone(), state.over_mention.clone());
            move |event: &MouseMoveEvent, _, window, _| {
                let now = hitbox.is_hovered(window) && mention_at(event.position).is_some();
                if over.replace(now) != now {
                    window.refresh();
                }
            }
        });
        window.on_mouse_event({
            let hitbox = hitbox.clone();
            move |event: &MouseUpEvent, phase, window, cx| {
                // A drag that selected text is not a click on the mention.
                if phase.bubble()
                    && event.button == MouseButton::Left
                    && event.click_count == 1
                    && hitbox.is_hovered(window)
                    && TextSelection::selected_text(window, cx).is_empty()
                {
                    if let Some(jid) = mention_at(event.position) {
                        on_click(&jid, window, cx);
                    }
                }
            }
        });
    }
}

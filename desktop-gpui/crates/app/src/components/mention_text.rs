//! Selectable message text with coloured mention spans.
//!
//! `gpui_base::SelectableText` cannot take highlights, so this mirrors its
//! selection wiring around a `StyledText` that does.

use std::ops::Range;

use gpui::{
    App, BorderStyle, Bounds, Corners, Edges, Element, ElementId, FontWeight, GlobalElementId,
    Hitbox, HitboxBehavior, Hsla, HighlightStyle, InspectorElementId, IntoElement, LayoutId,
    PaintQuad, Pixels, Point, SharedString, StyledText, Window, transparent_black,
};
use gpui_base::{
    TextSelection, TextSelectionHandle, TextSelectionRegistration, TextSelectionRun, Theme,
};

pub struct MentionText {
    id: ElementId,
    text: SharedString,
    styled_text: StyledText,
}

impl MentionText {
    pub fn new(
        id: impl Into<ElementId>,
        text: impl Into<SharedString>,
        mentions: &[Range<usize>],
        color: Hsla,
    ) -> Self {
        let text: SharedString = text.into();
        let style = HighlightStyle {
            color: Some(color),
            font_weight: Some(FontWeight::MEDIUM),
            ..Default::default()
        };
        let highlights = mentions.iter().map(|range| (range.clone(), style)).collect::<Vec<_>>();
        Self {
            id: id.into(),
            styled_text: StyledText::new(text.clone()).with_highlights(highlights),
            text,
        }
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
    type RequestLayoutState = TextSelectionHandle;
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
        let handle = window.with_element_state(
            global_id.expect("MentionText must have a stable element id"),
            |retained: Option<TextSelectionHandle>, _| {
                let handle = retained.unwrap_or_else(|| TextSelectionHandle::new(self.text.clone(), cx));
                (handle.clone(), handle)
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
        handle: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.styled_text.prepaint(global_id, inspector_id, bounds, &mut (), window, cx);
        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        let registration = TextSelectionRegistration::new(hitbox.clone(), bounds)
            .with_text_bounds(vec![bounds])
            .with_rendered_element(handle, window, cx);
        handle.register(registration, window, cx);
        hitbox
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        handle: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let layout = self.styled_text.layout().clone();
        let selected_before = TextSelection::selected_text(window, cx);
        let projection = handle.update_runs(
            &[TextSelectionRun::new(self.text.clone(), layout.clone(), bounds)],
            cx,
        );
        if selected_before != TextSelection::selected_text(window, cx) {
            window.refresh();
        }
        let color = Theme::global(cx).tokens.colors.selection;
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
    }
}

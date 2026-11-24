use crate::components::ui::scroll_area::ScrollbarHandle;
use crate::views::chat::*;

/// Fixed virtual height per row used to compute a stable scrollbar thumb size
/// that does not grow and shrink as GPUI lazily measures mixed text and media rows.
const VIRTUAL_ROW_PX: f32 = 72.0;

/// Index-proportional scrollbar handle over `gpui::ListState`.
///
/// `ListState`'s default `content_size` fluctuates whenever the user scrolls
/// between short text rows and tall media rows because off-screen items are
/// estimated from the running average of measured items. Basing the virtual
/// content height solely on `item_count()` keeps the thumb size steady while
/// mapping drag/scroll progress smoothly onto the real list offset.
#[derive(Clone)]
pub(crate) struct StableListScrollbar(pub ListState);

impl ScrollbarHandle for StableListScrollbar {
    fn viewport_bounds(&self) -> Bounds<Pixels> {
        self.0.viewport_bounds()
    }

    fn content_size(&self) -> Size<Pixels> {
        let vp = self.0.viewport_bounds().size;
        let count = self.0.item_count();
        let real_max = f32::from(self.0.max_offset_for_scrollbar().y);
        if real_max <= 0.0 || count <= 1 {
            return vp;
        }
        let virtual_h = (count as f32 * VIRTUAL_ROW_PX).max(f32::from(vp.height) + 1.0);
        size(vp.width, px(virtual_h))
    }

    fn offset(&self) -> Point<Pixels> {
        let vp_h = f32::from(self.0.viewport_bounds().size.height);
        let virtual_max = (f32::from(self.content_size().height) - vp_h).max(0.0);
        if virtual_max <= 0.0 {
            return Point::default();
        }
        if self.0.is_following_tail() {
            return point(px(0.0), px(-virtual_max));
        }
        let real_max = f32::from(self.0.max_offset_for_scrollbar().y);
        if real_max <= 0.0 {
            return Point::default();
        }
        let real_y = -f32::from(self.0.scroll_px_offset_for_scrollbar().y);
        let progress = (real_y / real_max).clamp(0.0, 1.0);
        point(px(0.0), px(-progress * virtual_max))
    }

    fn set_offset(&self, offset: Point<Pixels>) {
        let vp_h = f32::from(self.0.viewport_bounds().size.height);
        let virtual_max = (f32::from(self.content_size().height) - vp_h).max(0.0);
        let real_max = f32::from(self.0.max_offset_for_scrollbar().y);
        if virtual_max <= 0.0 || real_max <= 0.0 {
            return;
        }
        let progress = (-f32::from(offset.y) / virtual_max).clamp(0.0, 1.0);
        self.0
            .set_offset_from_scrollbar(point(px(0.0), px(-progress * real_max)));
    }

    fn start_drag(&self) {
        self.0.scrollbar_drag_started();
    }

    fn end_drag(&self) {
        self.0.scrollbar_drag_ended();
    }
}

impl ChatView {
    /// Centered date pill, shared by the in-flow day separators and the
    /// sticky header at the top of the message list.
    pub(crate) fn day_pill(label: String, bg: Hsla, border_color: Hsla, text: Hsla) -> AnyElement {
        // The original small pill. Digits have one width ("tnum"), so
        // "16 OCTOBER 2026" and "18 OCTOBER 2026" come out the same size.
        div()
            .px_3()
            .py_1()
            .rounded_full()
            .bg(bg)
            .border_1()
            .border_color(border_color)
            .text_xs()
            .font_weight(FontWeight::MEDIUM)
            .font_features(FontFeatures(std::sync::Arc::new(vec![("tnum".to_string(), 1)])))
            .text_color(text)
            .child(label)
            .into_any_element()
    }

    /// Date label of the message at `index` in the cached list (clamped).
    pub(crate) fn date_label_at(&self, index: usize) -> Option<String> {
        let len = self.cached_render_messages.len();
        if len == 0 {
            return None;
        }
        let msg_idx = index.saturating_sub(1).min(len - 1);
        self.cached_render_messages
            .get(msg_idx)
            .map(|m| MessageBubbleHelper::format_date_label(m.msg.timestamp).to_uppercase())
    }

    /// Label for the floating day header — the web's `sticky top-0` date pill:
    /// the day group occupying the top of the message list. `None` while that
    /// group's own in-flow pill is still on screen (it *is* the header), and
    /// while the next group's pill has scrolled up to take its place.
    pub(crate) fn sticky_day_label(&self) -> Option<String> {
        let len = self.cached_render_messages.len();
        if len == 0 {
            return None;
        }
        let state = &self.messages_list_state;
        let msg_idx = state
            .logical_scroll_top()
            .item_ix
            .saturating_sub(1)
            .min(len - 1);

        let timestamps: Vec<i64> = self
            .cached_render_messages
            .iter()
            .map(|m| m.msg.timestamp)
            .collect();

        // The in-flow pill for this day is still visible → the real pill wins.
        // Unknown (no layout yet) counts as "not above" so the header never
        // doubles up with the pill it mirrors.
        let pill_ix = MessageBubbleHelper::day_group_pill_index(&timestamps, msg_idx);
        if !state.item_is_above_viewport(pill_ix).unwrap_or(false) {
            return None;
        }

        // The next day's pill has reached the top → it takes over from here.
        if let Some(next_ix) = MessageBubbleHelper::next_day_group_pill_index(&timestamps, msg_idx) {
            if let Some(bounds) = state.bounds_for_item(next_ix) {
                // Header inset (8px) plus the pill's own height (~24px).
                if bounds.top() <= state.viewport_bounds().top() + px(32.0) {
                    return None;
                }
            }
        }

        self.cached_render_messages
            .get(msg_idx)
            .map(|m| MessageBubbleHelper::format_date_label(m.msg.timestamp).to_uppercase())
    }

    /// Whether the user has scrolled up away from the newest messages. The
    /// list follows the tail while it sits at the bottom, so that is the signal.
    pub(crate) fn should_show_scroll_to_bottom(&self) -> bool {
        !self.cached_render_messages.is_empty()
            && (self.has_more_next || !self.messages_list_state.is_following_tail())
    }

    /// Scroll to the newest messages at the bottom.
    pub(crate) fn scroll_to_bottom(&mut self, cx: &mut Context<Self>) {
        if self.has_more_next {
            if let Some(chat_id) = self.selected_chat_id.clone() {
                self.load_messages(&chat_id, cx);
            }
        }
        self.messages_list_state.scroll_to_end();
        cx.notify();
    }


    /// PageUp/PageDown/Home/End scroll the conversation. Returns whether the
    /// key was consumed.
    ///
    /// They act while the composer or nothing in particular has focus; any
    /// other text field keeps them for its caret. In the composer Home/End stay
    /// caret keys once there is a draft to move through.
    pub(crate) fn handle_scroll_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !matches!(key, "pageup" | "pagedown" | "home" | "end")
            || self.cached_render_messages.is_empty()
            || self.preview_image_url.is_some()
            || !cx
                .try_global::<crate::router::Router>()
                .is_some_and(|router| router.current_route.is_active("/chat"))
        {
            return false;
        }
        let composer_focused = self
            .compose_textarea
            .as_ref()
            .is_some_and(|textarea| textarea.read(cx).focus_handle(cx).is_focused(window));
        if !composer_focused && window.focused(cx).is_some() {
            return false;
        }
        if composer_focused && matches!(key, "home" | "end") && !self.compose_text.is_empty() {
            return false;
        }

        let state = &self.messages_list_state;
        match key {
            // A little less than a screen, so a line of context carries over.
            "pageup" => state.scroll_by(-state.viewport_bounds().size.height * 0.9),
            "pagedown" => state.scroll_by(state.viewport_bounds().size.height * 0.9),
            "home" => state.scroll_to(ListOffset { item_ix: 0, offset_in_item: px(0.) }),
            _ => self.scroll_to_bottom(cx),
        }
        cx.notify();
        true
    }
}

use crate::views::chat::*;

impl ChatView {
    /// Draggable divider between the sidebar and the conversation.
    pub(crate) fn render_sidebar_resize_handle(
        &mut self,
        cx: &mut Context<Self>,
        pages: bool,
        show_list: bool,
    ) -> impl IntoElement {
        let theme = *cx.app_theme();
        let primary_color = theme.primary;
        div()
            .id("sidebar-resize-handle")
            .when(pages || !show_list, |el| el.hidden())
            .absolute()
            .top_0()
            .bottom_0()
            .left(px(self.sidebar_width - SIDEBAR_HANDLE_WIDTH / 2.0))
            .w(px(SIDEBAR_HANDLE_WIDTH))
            .cursor_col_resize()
            .hover(|s| s.bg(primary_color.opacity(0.18)))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, ev: &MouseDownEvent, _, cx| {
                this.sidebar_resize_drag = Some((f32::from(ev.position.x), this.sidebar_width));
                cx.notify();
            }))
    }

    /// Full-screen catcher that keeps the pointer glued to the divider while dragging.
    pub(crate) fn render_sidebar_resize_overlay(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        self.sidebar_resize_drag.is_some().then(|| {
            div()
                .id("sidebar-resize-overlay")
                .absolute()
                .inset_0()
                .cursor_col_resize()
                .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                    if let Some((start_x, start_width)) = this.sidebar_resize_drag {
                        let moved = f32::from(ev.position.x) - start_x;
                        this.sidebar_width = (start_width + moved)
                            .clamp(SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH);
                        cx.notify();
                    }
                }))
                .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| {
                    this.sidebar_resize_drag = None;
                    cx.notify();
                }))
        })
    }
}

use crate::views::chat::*;
use crate::theme::surface::SurfaceExt;

impl ChatView {
    /// Left column: header, filter chips, archived/pinned rows and the conversation list.
    pub(crate) fn render_sidebar(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
        pages: bool,
        show_list: bool,
        has_selection: bool,
        archived_count: usize,
        pinned_count: usize,
    ) -> AnyElement {
        let theme = *cx.app_theme();
        let border_color = theme.border;
        let bg_color = theme.background;
        let window_transparency = ThemeManager::global(cx).window_transparency;
        let shape = cx.shape();
        let surface = cx.surface();
        let background = if shape.adwaita {
            if window.is_window_active() {
                surface.secondary_sidebar
            } else {
                surface.secondary_sidebar_backdrop
            }
        } else {
            window_material::glass_fill(bg_color, window_transparency)
        };
        let border = if shape.adwaita {
            surface.sidebar_border
        } else {
            border_color.opacity(0.4)
        };
        let sidebar = v_flex()
            .w(px(self.sidebar_width))
            .when(pages, |el| el.w_full())
            .when(!show_list, |el| el.hidden())
            .h_full()
            .border_r_1()
            .border_color(border)
            .bg(background)
            .flex_shrink_0()
            // Sidebar Header
            .child(self.render_sidebar_header(cx, pages, has_selection))
            // Archived is always the first row in the conversation list.
            .children(self.render_archived_row(cx, archived_count))
            .children(self.render_pinned_label(cx, pinned_count))
            // Chat Scroll List (Uniform 72px item height strictly enforced) with scrollbar
            .child(self.render_chat_list(cx));
        if pages || !show_list || !self.sidebar_animate {
            return sidebar.into_any_element();
        }
        // The list keeps its width and is clipped by a slot that grows or
        // shrinks, so its content never reflows mid-animation.
        let width = self.sidebar_width;
        let exiting = self.sidebar_exiting;
        div()
            .h_full()
            .flex_shrink_0()
            .overflow_hidden()
            .child(sidebar)
            .with_animation(
                if exiting { "chat-sidebar-out" } else { "chat-sidebar-in" },
                crate::components::motion::enter(SHEET_ANIM_MS),
                move |el, delta| {
                    let delta = delta.clamp(0.0, 1.0);
                    el.w(px(width * if exiting { 1.0 - delta } else { delta }))
                },
            )
            .into_any_element()
    }
}

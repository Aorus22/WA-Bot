use crate::views::chat::*;

impl ChatView {
    /// Left column: header, filter chips, archived/pinned rows and the conversation list.
    pub(crate) fn render_sidebar(
        &mut self,
        cx: &mut Context<Self>,
        pages: bool,
        show_list: bool,
        has_selection: bool,
        archived_count: usize,
        pinned_count: usize,
    ) -> impl IntoElement {
        let theme = *cx.app_theme();
        let border_color = theme.border;
        let bg_color = theme.background;
        let window_transparency = ThemeManager::global(cx).window_transparency;
        v_flex()
            .w(px(self.sidebar_width))
            .when(pages, |el| el.w_full())
            .when(!show_list, |el| el.hidden())
            .h_full()
            .border_r_1()
            .border_color(border_color.opacity(0.4))
            .bg(window_material::glass_fill(bg_color, window_transparency))
            .flex_shrink_0()
            // Sidebar Header
            .child(self.render_sidebar_header(cx, pages, has_selection))
            // Archived is always the first row in the conversation list.
            .children(self.render_archived_row(cx, archived_count))
            .children(self.render_pinned_label(cx, pinned_count))
            // Chat Scroll List (Uniform 72px item height strictly enforced) with scrollbar
            .child(self.render_chat_list(cx))
    }
}

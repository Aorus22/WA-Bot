
use crate::views::chat::*;

impl ChatView {
    /// Right-click menu for the text of a bubble: open or copy the link under
    /// the pointer, copy the selection. Raised by `MentionText` in place of
    /// the bubble's own menu.
    pub(crate) fn render_text_menu(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let request = self.text_menu.clone()?;
        let theme = *cx.app_theme();
        let shape = cx.shape();
        let hover_bg: Hsla = theme.foreground.opacity(if shape.adwaita { 0.15 } else { 0.12 });
        macro_rules! styled_menu_row {
            ($($args:tt)*) => {
                menu_row($($args)*)
                    .when(shape.adwaita, |row| row.px_3().py_1p5().rounded(px(shape.radius_button)).overflow_hidden())
            };
        }
        let (muted_text, text_color) = (theme.muted_foreground, theme.foreground);

        let mut card = menu_surface(208.0, theme.card, theme.border, cx)
            .when(shape.adwaita, |menu| menu.shadow_sm());
        if let Some(url) = request.link.clone() {
            card = card
                .child(
                    styled_menu_row!(window, cx, "tm-open", EXTERNAL_LINK_SVG, "Open in browser", muted_text, text_color, hover_bg)
                        .on_mouse_down(MouseButton::Left, cx.listener({
                            let url = url.clone();
                            move |this, _, _, cx| {
                                cx.open_url(&url);
                                this.text_menu = None;
                                cx.notify();
                            }
                        })),
                )
                .child(
                    styled_menu_row!(window, cx, "tm-copy-link", LINK_SVG, "Copy link", muted_text, text_color, hover_bg)
                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(url.clone()));
                            toast::success("Disalin ke clipboard", cx);
                            this.text_menu = None;
                            cx.notify();
                        })),
                );
        }
        if let Some(text) = request.selection.clone() {
            card = card.child(
                styled_menu_row!(window, cx, "tm-copy", COPY_SVG, "Copy", muted_text, text_color, hover_bg)
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                        toast::success("Disalin ke clipboard", cx);
                        this.text_menu = None;
                        cx.notify();
                    })),
            );
        }

        Some(
            deferred(anchored().child(
                div()
                    .id("text-menu-backdrop")
                    .w(window.bounds().size.width)
                    .h(window.bounds().size.height)
                    .child(
                        gpui_base::Positioner::corner(Anchor::TopLeft, request.position)
                            .margin(px(8.0))
                            .occlude()
                            .child(
                                v_flex()
                                    .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                        this.text_menu = None;
                                        cx.notify();
                                    }))
                                    .child(card)
                                    .with_animation(
                                        "text-menu-in",
                                        Animation::new(std::time::Duration::from_millis(140))
                                            .with_easing(|d| 1.0 - (1.0 - d).powi(5)),
                                        move |el, delta| el.opacity(delta),
                                    ),
                            ),
                    ),
            ))
            .with_priority(gpui_base::POPUP_PRIORITY),
        )
    }
}

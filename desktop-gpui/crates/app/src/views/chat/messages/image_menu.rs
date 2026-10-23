
use crate::views::chat::*;

/// Container format of an image, from its first bytes.
pub(crate) fn sniff_image_format(bytes: &[u8]) -> Option<ImageFormat> {
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => Some(ImageFormat::Png),
        [0xFF, 0xD8, 0xFF, ..] => Some(ImageFormat::Jpeg),
        [b'G', b'I', b'F', b'8', ..] => Some(ImageFormat::Gif),
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => Some(ImageFormat::Webp),
        [b'B', b'M', ..] => Some(ImageFormat::Bmp),
        _ => None,
    }
}

/// A file name for an image address: its last path segment.
pub(crate) fn image_file_name(url: &str) -> String {
    url.split(['?', '#'])
        .next()
        .and_then(|path| path.rsplit('/').next())
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "image.jpg".to_string())
}

impl ChatView {
    /// Download an image and put it on the clipboard as a picture.
    pub(crate) fn copy_image(url: String, cx: &mut Context<Self>) {
        let base_url = Self::compose_base_url(cx);
        cx.spawn(move |_this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT.spawn(async move { client.download_bytes(&url).await }).await;
                let _ = cx_handle.update(|cx: &mut App| match res {
                    Ok(Ok(bytes)) => match sniff_image_format(&bytes) {
                        Some(format) => {
                            cx.write_to_clipboard(ClipboardItem::new_image(&Image::from_bytes(format, bytes.to_vec())));
                            toast::success("Gambar disalin ke clipboard", cx);
                        }
                        None => toast::error("Format gambar tidak dikenali", cx),
                    },
                    _ => toast::error("Gagal mengunduh gambar", cx),
                });
            }
        })
        .detach();
    }

    /// Right-click menu on a picture, in the conversation or the viewer.
    pub(crate) fn render_image_menu(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let menu = self.image_menu.clone()?;
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

        let card = menu_surface(208.0, theme.card, theme.border, cx)
            .when(shape.adwaita, |menu| menu.shadow_sm())
            .child(
                styled_menu_row!(window, cx, "im-copy", COPY_SVG, "Copy image", muted_text, text_color, hover_bg)
                    .on_mouse_down(MouseButton::Left, cx.listener({
                        let url = menu.url.clone();
                        move |this, _, _, cx| {
                            Self::copy_image(url.clone(), cx);
                            this.image_menu = None;
                            cx.notify();
                        }
                    })),
            )
            .child(
                styled_menu_row!(window, cx, "im-save", DOWNLOAD_SVG, "Save image as…", muted_text, text_color, hover_bg)
                    .on_mouse_down(MouseButton::Left, cx.listener({
                        let (url, name) = (menu.url.clone(), menu.file_name.clone());
                        move |this, _, _, cx| {
                            Self::save_file(url.clone(), name.clone(), "Gambar", cx);
                            this.image_menu = None;
                            cx.notify();
                        }
                    })),
            );

        Some(
            deferred(anchored().child(
                div()
                    .id("image-menu-backdrop")
                    .w(window.bounds().size.width)
                    .h(window.bounds().size.height)
                    .child(
                        gpui_base::Positioner::corner(Anchor::TopLeft, menu.position)
                            .margin(px(8.0))
                            .occlude()
                            .child(
                                v_flex()
                                    .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                        this.image_menu = None;
                                        cx.notify();
                                    }))
                                    .child(card)
                                    .with_animation(
                                        "image-menu-in",
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

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;

    #[test]
    fn recognises_common_image_containers() {
        assert_eq!(sniff_image_format(&[0x89, b'P', b'N', b'G', 0, 0]), Some(ImageFormat::Png));
        assert_eq!(sniff_image_format(&[0xFF, 0xD8, 0xFF, 0xE0]), Some(ImageFormat::Jpeg));
        assert_eq!(sniff_image_format(b"GIF89a"), Some(ImageFormat::Gif));
        assert_eq!(sniff_image_format(b"RIFF\x10\0\0\0WEBPVP8 "), Some(ImageFormat::Webp));
        assert_eq!(sniff_image_format(b"<html>"), None);
        assert_eq!(sniff_image_format(&[]), None);
    }

    #[test]
    fn file_name_comes_from_the_last_path_segment() {
        assert_eq!(image_file_name("http://x/api/media/img_1.jpg?token=1"), "img_1.jpg");
        assert_eq!(image_file_name("http://x/"), "image.jpg");
    }
}


use crate::theme::surface::SurfaceExt;
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

fn write_image_to_clipboard(
    format: gpui::ImageFormat,
    bytes: &[u8],
    cx: &mut gpui::App,
) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        // GPUI's Linux clipboard backend currently exports text and its own
        // private image MIME type. Use the same arboard backend as the Linux
        // clipboard-history app so other applications receive image/png.
        let _ = (format, cx);
        let rgba = image::load_from_memory(bytes)
            .map_err(|error| format!("Gambar tidak bisa dibaca: {error}"))?
            .to_rgba8();
        let (width, height) = (rgba.width(), rgba.height());
        let rgba_bytes = rgba.into_raw();
        let mut clipboard = arboard::Clipboard::new().map_err(|error| error.to_string())?;
        clipboard
            .set_image(arboard::ImageData {
                width: width as usize,
                height: height as usize,
                bytes: rgba_bytes.clone().into(),
            })
            .map_err(|error| error.to_string())?;

        let observed = clipboard.get_image().map_err(|error| error.to_string())?;
        if observed.width != width as usize
            || observed.height != height as usize
            || observed.bytes.as_ref() != rgba_bytes.as_slice()
        {
            return Err("Clipboard image verification returned different data".to_string());
        }
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    {
        cx.write_to_clipboard(ClipboardItem::new_image(&Image::from_bytes(
            format,
            bytes.to_vec(),
        )));
        Ok(())
    }
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
                        Some(format) => match write_image_to_clipboard(format, &bytes, cx) {
                            Ok(()) => toast::success_at(
                                "Gambar disalin ke clipboard",
                                Anchor::BottomRight,
                                cx,
                            ),
                            Err(error) => toast::error_at(
                                format!("Gagal menyalin gambar: {error}"),
                                Anchor::BottomRight,
                                cx,
                            ),
                        },
                        None => toast::error_at(
                            "Format gambar tidak dikenali",
                            Anchor::BottomRight,
                            cx,
                        ),
                    },
                    _ => toast::error_at("Gagal mengunduh gambar", Anchor::BottomRight, cx),
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
        let surface = cx.surface();
        let hover_bg = surface.menu_hover;
        macro_rules! styled_menu_row {
            ($($args:tt)*) => {
                menu_row($($args)*)
                    .when(shape.styled, |row| row.px_3().py_1p5().rounded(px(shape.radius_menu_item)).overflow_hidden())
            };
        }
        let (muted_text, text_color) = (theme.muted_foreground, theme.foreground);

        let card = menu_surface(208.0, cx)
            .when(shape.styled && !shape.winui, |menu| menu.shadow_sm())
                .when(shape.winui, |menu| menu.shadow(crate::theme::shape::flyout_shadow()))
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

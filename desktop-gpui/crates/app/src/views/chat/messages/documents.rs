use crate::views::chat::*;

impl ChatView {
    /// Document attachment card, mirroring the web's document bubble: file
    /// icon tile, file name, uppercase extension, then Open / Save As.
    pub(crate) fn render_document_card(
        file_name: &str,
        media_url: Option<String>,
        is_dark_bg: bool,
        primary_color: Hsla,
        bubble_text: Rgba,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let name = if file_name.trim().is_empty() {
            "Document".to_string()
        } else {
            file_name.trim().to_string()
        };
        let extension = name
            .rsplit_once('.')
            .map(|(_, ext)| ext.to_uppercase())
            .filter(|ext| !ext.is_empty())
            .unwrap_or_else(|| "FILE".to_string());

        let actions = media_url.map(|url| {
            let open_url = url.clone();
            let download_url = url.clone();
            let save_name = name.clone();
            h_flex()
                .w_full()
                .gap_2()
                .pt_2()
                .border_t_1()
                .border_color(bubble_text.opacity(0.12))
                .child(
                    h_flex()
                        .flex_1()
                        .justify_center()
                        .items_center()
                        .gap_1p5()
                        .py_2()
                        .rounded_lg()
                        .cursor_pointer()
                        .bg(primary_color.opacity(0.12))
                        .hover(move |s| s.bg(primary_color.opacity(0.22)))
                        .child(svg().data(EXTERNAL_LINK_SVG).size(px(14.0)).text_color(primary_color))
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(primary_color)
                                .child("Open"),
                        )
                        .on_mouse_down(MouseButton::Left, cx.listener(move |_this, _, _, cx| {
                            cx.open_url(&open_url);
                        }))
                        .into_any_element(),
                )
                .child(
                    h_flex()
                        .flex_1()
                        .justify_center()
                        .items_center()
                        .gap_1p5()
                        .py_2()
                        .rounded_lg()
                        .cursor_pointer()
                        .bg(bubble_text.opacity(0.08))
                        .hover(move |s| s.bg(bubble_text.opacity(0.16)))
                        .child(svg().data(DOWNLOAD_SVG).size(px(14.0)).text_color(bubble_text))
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(bubble_text)
                                .child("Save As"),
                        )
                        .on_mouse_down(MouseButton::Left, cx.listener(move |_this, _, _, cx| {
                            Self::save_document(download_url.clone(), save_name.clone(), cx);
                        }))
                        .into_any_element(),
                )
                .into_any_element()
        });

        v_flex()
            .mb_1p5()
            .p_2()
            .gap_2()
            .min_w(px(210.0))
            .max_w(px(300.0))
            .rounded_xl()
            .bg(if is_dark_bg {
                rgba(0xffffff14)
            } else {
                rgba(0x00000010)
            })
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .w(px(40.0))
                            .h(px(40.0))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_lg()
                            .bg(primary_color.opacity(0.15))
                            .child(svg().data(FILE_TEXT_SVG).size(px(22.0)).text_color(primary_color)),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w(px(0.0))
                            .overflow_hidden()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(bubble_text)
                                    .overflow_hidden()
                                    .child(name),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(bubble_text.opacity(0.6))
                                    .child(extension),
                            ),
                    ),
            )
            .children(actions)
            .into_any_element()
    }

    /// "Save As" for a document: ask for a destination, download, write it.
    pub(crate) fn save_document(url: String, file_name: String, cx: &mut Context<Self>) {
        Self::save_file(url, file_name, "Dokumen", cx);
    }

    /// Save any downloaded file (`what` names it in the toasts).
    pub(crate) fn save_file(url: String, file_name: String, what: &'static str, cx: &mut Context<Self>) {
        let directory = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let base_url = Self::compose_base_url(cx);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let receiver = cx_handle.update(|cx: &mut App| {
                    cx.prompt_for_new_path(&directory, Some(file_name.as_str()))
                });
                let target = match receiver.await {
                    Ok(Ok(Some(path))) => path,
                    _ => return,
                };

                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move { client.download_bytes(&url).await })
                    .await;

                // The write happens outside `update`, where awaiting is not allowed.
                let mut downloaded = false;
                let saved = match res {
                    Ok(Ok(bytes)) => {
                        downloaded = true;
                        let write_path = target.clone();
                        TOKIO_RT
                            .spawn_blocking(move || std::fs::write(&write_path, bytes).is_ok())
                            .await
                            .unwrap_or(false)
                    }
                    _ => false,
                };

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |_this, cx| {
                            if saved {
                                toast::success(&format!("{what} disimpan"), cx);
                            } else if downloaded {
                                toast::error(&format!("Gagal menyimpan {}", what.to_lowercase()), cx);
                            } else {
                                toast::error(&format!("Gagal mengunduh {}", what.to_lowercase()), cx);
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

}

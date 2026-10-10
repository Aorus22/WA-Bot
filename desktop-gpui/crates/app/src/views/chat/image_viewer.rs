use crate::views::chat::*;

impl ChatView {
    /// Open the full-screen image viewer, fitted to the window.
    pub fn open_preview(&mut self, url: String, msg: Option<(String, Message)>, cx: &mut Context<Self>) {
        self.preview_image_url = Some(url);
        self.preview_msg = msg;
        self.preview_zoom = 1.0;
        self.preview_pan = (0.0, 0.0);
        self.preview_drag = None;
        self.preview_needs_focus = true;
        cx.notify();
    }

    pub fn close_preview(&mut self, cx: &mut Context<Self>) {
        self.preview_image_url = None;
        self.preview_msg = None;
        self.preview_drag = None;
        cx.notify();
    }

    pub fn reset_preview_zoom(&mut self, cx: &mut Context<Self>) {
        self.preview_zoom = 1.0;
        self.preview_pan = (0.0, 0.0);
        self.preview_drag = None;
        cx.notify();
    }

    /// Multiply the zoom by `factor`, keeping the point under `anchor` (overlay
    /// coordinates) fixed; `None` zooms around the centre.
    pub fn zoom_preview(&mut self, factor: f32, anchor: Option<(f32, f32)>, cx: &mut Context<Self>) {
        let (area_w, area_h, ox, oy) = preview_geometry(self.preview_bounds.get());
        let old = self.preview_zoom;
        let new = (old * factor).clamp(PREVIEW_MIN_ZOOM, PREVIEW_MAX_ZOOM);
        if (new - old).abs() < 1e-4 {
            return;
        }
        let anchor = anchor.unwrap_or((ox, oy));
        self.preview_zoom = new;
        self.preview_pan = if new <= 1.0 + 1e-4 {
            (0.0, 0.0)
        } else {
            clamp_pan(
                pan_after_zoom(anchor, (ox, oy), self.preview_pan, old, new),
                area_w,
                area_h,
                new,
            )
        };
        cx.notify();
    }

    /// Image messages of the chat behind the open preview, oldest first.
    pub(crate) fn preview_gallery(&self, cx: &App) -> Vec<Message> {
        let Some((chat_id, _)) = self.preview_msg.as_ref() else { return Vec::new() };
        if !cx.has_global::<ChatStore>() {
            return Vec::new();
        }
        ChatStore::global(cx)
            .messages_by_chat
            .get(chat_id)
            .map(|entry| {
                entry
                    .messages
                    .iter()
                    .filter(|m| {
                        m.media_url.is_some()
                            && (m.message_type == "image" || m.content == "[Image]" || m.content.is_empty())
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Step to the previous (`-1`) or next (`1`) image of the chat.
    pub fn step_preview(&mut self, step: i32, cx: &mut Context<Self>) {
        let gallery = self.preview_gallery(cx);
        let Some((chat_id, current)) = self.preview_msg.clone() else { return };
        let Some(pos) = gallery.iter().position(|m| m.id == current.id) else { return };
        let next = pos as i32 + step;
        if next < 0 || next as usize >= gallery.len() {
            return;
        }
        let msg = gallery[next as usize].clone();
        let Some(media) = msg.media_url.clone() else { return };
        let url = resolve_media_url(&media, &Self::compose_base_url(cx));
        self.open_preview(url, Some((chat_id, msg)), cx);
    }


    /// Full-screen image viewer: fit-to-window, wheel zoom, drag to pan, double-click to zoom.
    pub(crate) fn render_image_viewer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        if let Some(ref img_url) = self.preview_image_url {
            let url = img_url.clone();
            let bounds_cell = self.preview_bounds.clone();
            let (area_w, area_h, ox, oy) = preview_geometry(bounds_cell.get());
            let zoom = self.preview_zoom;
            let (pan_x, pan_y) = self.preview_pan;
            // Natural size of the picture (from gpui's asset cache, the same one
            // the img element uses). Fit = scale so it fits the area, never
            // upscaled past 100%; zoom multiplies that.
            let load = preview_load_state(&url, window, cx);
            let (box_w, box_h) = match load {
                PreviewLoad::Ready(nw, nh) if nw > 0.0 && nh > 0.0 => {
                    let fit = (area_w / nw).min(area_h / nh).min(1.0);
                    (nw * fit * zoom, nh * fit * zoom)
                }
                _ => (area_w * zoom, area_h * zoom),
            };
            let (left, top) = (ox + pan_x - box_w / 2.0, oy + pan_y - box_h / 2.0);
            let gallery = self.preview_gallery(cx);
            let position = self
                .preview_msg
                .as_ref()
                .and_then(|(_, m)| gallery.iter().position(|g| g.id == m.id));
            let has_msg = self.preview_msg.is_some();
            let icon: Hsla = rgb(0xe9edef).into();
            let btn_hover: Hsla = rgba(0xffffff1f).into();
            let zoom_label = format!("{}%", (zoom * 100.0).round() as i32);

            // Toolbar button: 36px round hit area with a white icon.
            let tool = |id: &'static str, svg_data: &'static [u8]| {
                div()
                    .id(id)
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(36.0))
                    .h(px(36.0))
                    .rounded_full()
                    .cursor_pointer()
                    .hover(move |s| s.bg(btn_hover))
                    .child(svg().data(svg_data).size(px(18.0)).text_color(icon))
            };

            let mut toolbar = h_flex()
                .absolute()
                .top_0()
                .left_0()
                .w_full()
                .h(px(PREVIEW_TOOLBAR_H))
                .px_4()
                .items_center()
                .justify_between()
                .bg(rgba(0x0b141acc))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .text_sm()
                        .text_color(icon)
                        .child(match position {
                            Some(i) if gallery.len() > 1 => format!("{} / {}", i + 1, gallery.len()),
                            _ => String::new(),
                        }),
                );
            let mut actions = h_flex().items_center().gap_1();
            actions = actions
                .child(tool("pv-zoom-out", MINUS_SVG).on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                    this.zoom_preview(1.0 / 1.25, None, cx);
                    cx.stop_propagation();
                })))
                .child(
                    div()
                        .id("pv-zoom-label")
                        .w(px(48.0))
                        .flex()
                        .justify_center()
                        .text_xs()
                        .text_color(icon)
                        .child(zoom_label),
                )
                .child(tool("pv-zoom-in", PLUS_SVG).on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                    this.zoom_preview(1.25, None, cx);
                    cx.stop_propagation();
                })))
                .child(tool("pv-fit", MAXIMIZE_SVG).on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                    this.reset_preview_zoom(cx);
                    cx.stop_propagation();
                })))
                .child(tool("pv-save", DOWNLOAD_SVG).on_mouse_down(MouseButton::Left, cx.listener({
                    let url = url.clone();
                    move |this, _, _, cx| {
                        match this.preview_msg.clone() {
                            Some((_, m)) => this.save_message_media(&m, cx),
                            None => {
                                let name = url
                                    .split(['?', '#'])
                                    .next()
                                    .and_then(|p| p.rsplit('/').next())
                                    .filter(|n| !n.is_empty())
                                    .unwrap_or("image")
                                    .to_string();
                                Self::save_file(url.clone(), name, "Gambar", cx);
                            }
                        }
                        cx.stop_propagation();
                    }
                })));
            if has_msg {
                actions = actions.child(tool("pv-forward", FORWARD_SVG).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        if let Some((chat_id, m)) = this.preview_msg.clone() {
                            this.close_preview(cx);
                            this.open_forward(chat_id, m, cx);
                        }
                        cx.stop_propagation();
                    }),
                ));
            }
            actions = actions.child(tool("pv-close", X_SVG).on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.close_preview(cx);
                    cx.stop_propagation();
                }),
            ));
            toolbar = toolbar.child(actions);

            let mut overlay = div()
                .id("image-viewer")
                .absolute()
                .inset_0()
                .overflow_hidden()
                .bg(rgba(0x0b141aee))
                .track_focus(&self.preview_focus)
                .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                    match ev.keystroke.key.as_str() {
                        "escape" => this.close_preview(cx),
                        "left" => this.step_preview(-1, cx),
                        "right" => this.step_preview(1, cx),
                        "+" | "=" | "plus" => this.zoom_preview(1.25, None, cx),
                        "-" | "minus" => this.zoom_preview(1.0 / 1.25, None, cx),
                        "0" => this.reset_preview_zoom(cx),
                        _ => {}
                    }
                    cx.stop_propagation();
                }))
                .on_scroll_wheel(cx.listener(|this, ev: &ScrollWheelEvent, _, cx| {
                    // Wheel up gives a positive delta: zoom in.
                    let dy = f32::from(ev.delta.pixel_delta(px(20.0)).y);
                    if dy != 0.0 {
                        let b = this.preview_bounds.get();
                        let anchor = (
                            f32::from(ev.position.x - b.origin.x),
                            f32::from(ev.position.y - b.origin.y),
                        );
                        this.zoom_preview((dy * 0.006).exp(), Some(anchor), cx);
                    }
                    cx.stop_propagation();
                }))
                .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                    if let Some((sx, sy, px0, py0)) = this.preview_drag {
                        let (area_w, area_h, _, _) = preview_geometry(this.preview_bounds.get());
                        let pan = (
                            px0 + (f32::from(ev.position.x) - sx),
                            py0 + (f32::from(ev.position.y) - sy),
                        );
                        this.preview_pan = clamp_pan(pan, area_w, area_h, this.preview_zoom);
                        cx.notify();
                    }
                }))
                .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| {
                    if this.preview_drag.take().is_some() {
                        cx.notify();
                    }
                }))
                // Click on the empty backdrop closes the viewer.
                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                    this.close_preview(cx);
                }))
                .child({
                    // Measures the overlay so wheel zoom can anchor on the cursor.
                    let cell = bounds_cell.clone();
                    canvas(
                        move |bounds, window, _| {
                            if cell.replace(bounds) != bounds {
                                window.refresh();
                            }
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .size_full()
                })
                .children(if matches!(load, PreviewLoad::Ready(..)) { Some(
                    // The image box is the fit area times the zoom; `Contain`
                    // scales the picture to the box, so zoom 1 is fit-to-window.
                    div()
                        .id("preview-image-hit")
                        .absolute()
                        .left(px(left))
                        .top(px(top))
                        .w(px(box_w))
                        .h(px(box_h))
                        .cursor(if zoom > 1.0 + 1e-3 { CursorStyle::OpenHand } else { CursorStyle::Arrow })
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, ev: &MouseDownEvent, _, cx| {
                            if ev.click_count >= 2 {
                                // Double-click: toggle between fit and 2.5x at the cursor.
                                if this.preview_zoom > 1.0 + 1e-3 {
                                    this.reset_preview_zoom(cx);
                                } else {
                                    let b = this.preview_bounds.get();
                                    let anchor = (
                                        f32::from(ev.position.x - b.origin.x),
                                        f32::from(ev.position.y - b.origin.y),
                                    );
                                    this.zoom_preview(2.5, Some(anchor), cx);
                                }
                            } else if this.preview_zoom > 1.0 + 1e-3 {
                                this.preview_drag = Some((
                                    f32::from(ev.position.x),
                                    f32::from(ev.position.y),
                                    this.preview_pan.0,
                                    this.preview_pan.1,
                                ));
                            }
                            cx.stop_propagation();
                        }))
                        .child(
                            img(url)
                                .id("preview-image-frame")
                                // Same aspect ratio as the picture, so the element's
                                // own aspect-ratio sizing agrees with the box.
                                .w(px(box_w))
                                .h(px(box_h))
                                .object_fit(ObjectFit::Contain),
                        ),
                ) } else { None })
                // Shown immediately on open: a spinner while the image downloads,
                // or the failure, so the viewer never looks frozen.
                .children(match load {
                    PreviewLoad::Ready(..) => None,
                    PreviewLoad::Loading | PreviewLoad::Failed => Some(
                        v_flex()
                            .id("preview-status")
                            .absolute()
                            .left(px(ox - 100.0))
                            .top(px(oy - 44.0))
                            .w(px(200.0))
                            .items_center()
                            .gap_3()
                            .children(if load == PreviewLoad::Loading {
                                Some(Spinner::new().color(icon))
                            } else {
                                None
                            })
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(icon.opacity(0.85))
                                    .child(if load == PreviewLoad::Loading {
                                        "Loading image…"
                                    } else {
                                        "Couldn't load this image"
                                    }),
                            ),
                    ),
                })
                .child(toolbar);

            // Prev / next among the chat's images.
            if let Some(i) = position {
                let side = |id: &'static str, icon_data: &'static [u8], left_side: bool| {
                    let b = div()
                        .id(id)
                        .absolute()
                        .top(px(oy - 20.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .w(px(40.0))
                        .h(px(40.0))
                        .rounded_full()
                        .bg(rgba(0x0b141acc))
                        .cursor_pointer()
                        .hover(move |s| s.bg(btn_hover))
                        .child(svg().data(icon_data).size(px(20.0)).text_color(icon));
                    if left_side { b.left(px(12.0)) } else { b.right(px(12.0)) }
                };
                if i > 0 {
                    overlay = overlay.child(side("pv-prev", CHEVRON_LEFT_SVG, true).on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.step_preview(-1, cx);
                            cx.stop_propagation();
                        }),
                    ));
                }
                if i + 1 < gallery.len() {
                    overlay = overlay.child(side("pv-next", CHEVRON_RIGHT_SVG, false).on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.step_preview(1, cx);
                            cx.stop_propagation();
                        }),
                    ));
                }
            }
            Some(
                crate::components::motion::fade_in(overlay, "image-viewer-in", 180)
                    .into_any_element(),
            )
        } else {
            None
        }
    }
}

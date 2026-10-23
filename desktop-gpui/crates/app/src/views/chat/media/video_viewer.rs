use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use super::video_player::{self, VideoInfo, VideoPlayer};
use crate::views::chat::*;

const PLAY_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><path d="M8 5.5v13a1 1 0 0 0 1.5.86l10.5-6.5a1 1 0 0 0 0-1.72L9.5 4.64A1 1 0 0 0 8 5.5z" fill="currentColor"/></svg>"#;
const PAUSE_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><rect x="6" y="5" width="4" height="14" rx="1" fill="currentColor"/><rect x="14" y="5" width="4" height="14" rx="1" fill="currentColor"/></svg>"#;
const VOLUME_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11 4.7a.7.7 0 0 0-1.2-.5L6.4 7.6A1.4 1.4 0 0 1 5.4 8H3a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2.4a1.4 1.4 0 0 1 1 .4l3.4 3.4a.7.7 0 0 0 1.2-.5z"/><path d="M16 9a5 5 0 0 1 0 6"/><path d="M19.4 18.4a9 9 0 0 0 0-12.8"/></svg>"#;
const MUTED_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11 4.7a.7.7 0 0 0-1.2-.5L6.4 7.6A1.4 1.4 0 0 1 5.4 8H3a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2.4a1.4 1.4 0 0 1 1 .4l3.4 3.4a.7.7 0 0 0 1.2-.5z"/><line x1="22" x2="16" y1="9" y2="15"/><line x1="16" x2="22" y1="9" y2="15"/></svg>"#;

/// Height of the control bar under the video.
const CONTROLS_H: f32 = 64.0;
/// Arrow keys jump this far.
const SEEK_STEP: f32 = 5.0;

/// The full-screen video viewer: what plays and what is on screen.
pub(crate) struct VideoView {
    source: String,
    msg: Option<(String, Message)>,
    info: Option<VideoInfo>,
    player: Option<VideoPlayer>,
    /// Frame on screen; replaced (and its texture freed) with every new one.
    frame: Option<Arc<RenderImage>>,
    error: Option<String>,
    muted: bool,
    /// Bumped on every (re)start so a late result of an older one is ignored.
    generation: u64,
    /// Position to show while a seek starts the player again.
    pending_position: Option<f32>,
    overlay_bounds: Rc<Cell<Bounds<Pixels>>>,
    seek_bounds: Rc<Cell<Bounds<Pixels>>>,
}

impl ChatView {
    /// Play a video inside the app. Without ffmpeg it opens in the system's player.
    pub fn open_video(&mut self, url: String, msg: Option<(String, Message)>, cx: &mut Context<Self>) {
        if !video_player::ffmpeg_available() {
            cx.open_url(&url);
            return;
        }
        self.video_view = Some(VideoView {
            source: url.clone(),
            msg,
            info: None,
            player: None,
            frame: None,
            error: None,
            muted: false,
            generation: 0,
            pending_position: None,
            overlay_bounds: Rc::new(Cell::new(Bounds::default())),
            seek_bounds: Rc::new(Cell::new(Bounds::default())),
        });
        self.video_needs_focus = true;
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let probe_url = url.clone();
                let info = TOKIO_RT
                    .spawn_blocking(move || video_player::probe(&probe_url))
                    .await
                    .unwrap_or_else(|e| Err(e.to_string()));
                cx.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            let Some(video) = this.video_view.as_mut().filter(|v| v.source == url) else { return };
                            match info {
                                Ok(info) => {
                                    video.info = Some(info);
                                    this.start_video(0.0, false, cx);
                                }
                                Err(error) => video.error = Some(error),
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
        cx.notify();
    }

    pub fn close_video(&mut self, window: Option<&mut Window>, cx: &mut Context<Self>) {
        if let Some(video) = self.video_view.take() {
            if let Some(frame) = video.frame {
                cx.drop_image(frame, window);
            }
            // Dropping the player stops ffmpeg and the sound; it can take a
            // moment, so it happens off the UI thread.
            if let Some(player) = video.player {
                TOKIO_RT.spawn_blocking(move || drop(player));
            }
        }
        cx.notify();
    }

    /// (Re)start playback at `at` seconds. Opening the speaker can block, so
    /// the player is built off the UI thread.
    fn start_video(&mut self, at: f32, paused: bool, cx: &mut Context<Self>) {
        let Some(video) = self.video_view.as_mut() else { return };
        let Some(info) = video.info.clone() else { return };
        video.generation += 1;
        video.pending_position = Some(at);
        video.error = None;
        let generation = video.generation;
        let source = video.source.clone();
        let volume = if video.muted { 0.0 } else { 1.0 };
        if let Some(old) = video.player.take() {
            TOKIO_RT.spawn_blocking(move || drop(old));
        }
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let started = TOKIO_RT
                    .spawn_blocking(move || VideoPlayer::start(&source, info, at, volume, paused))
                    .await
                    .unwrap_or_else(|e| Err(e.to_string()));
                let alive = cx.update(|cx: &mut App| {
                    let Some(view) = this.upgrade() else { return false };
                    view.update(cx, |this, cx| {
                        let current = this.video_view.as_mut().filter(|v| v.generation == generation);
                        match (current, started) {
                            (Some(video), Ok(player)) => {
                                video.player = Some(player);
                                video.pending_position = None;
                            }
                            (Some(video), Err(error)) => video.error = Some(error),
                            (None, Ok(player)) => {
                                TOKIO_RT.spawn_blocking(move || drop(player));
                                return false;
                            }
                            (None, Err(_)) => return false,
                        }
                        cx.notify();
                        true
                    })
                });
                if !alive {
                    return;
                }
                // Repaint while it plays, so new frames and the time show up.
                loop {
                    cx.background_executor().timer(std::time::Duration::from_millis(16)).await;
                    let keep = cx.update(|cx: &mut App| {
                        let Some(view) = this.upgrade() else { return false };
                        view.update(cx, |this, cx| {
                            match this.video_view.as_ref() {
                                Some(video) if video.generation == generation => {
                                    let playing = video.player.as_ref().is_some_and(|p| !p.is_paused() && !p.has_ended());
                                    if playing || video.player.as_ref().is_some_and(|p| p.frames_shown() <= 1) {
                                        cx.notify();
                                    }
                                    true
                                }
                                _ => false,
                            }
                        })
                    });
                    if !keep {
                        break;
                    }
                }
            }
        })
        .detach();
    }

    fn video_position(&self) -> f32 {
        let Some(video) = self.video_view.as_ref() else { return 0.0 };
        video
            .pending_position
            .or_else(|| video.player.as_ref().map(|p| p.position()))
            .unwrap_or(0.0)
    }

    pub fn toggle_video_pause(&mut self, cx: &mut Context<Self>) {
        let Some(video) = self.video_view.as_ref() else { return };
        match video.player.as_ref() {
            // Play again from the start once it has ended.
            Some(player) if player.has_ended() => self.start_video(0.0, false, cx),
            Some(player) => player.set_paused(!player.is_paused()),
            None => {}
        }
        cx.notify();
    }

    pub fn seek_video(&mut self, to: f32, cx: &mut Context<Self>) {
        let Some(video) = self.video_view.as_ref() else { return };
        let Some(duration) = video.info.as_ref().map(|i| i.duration) else { return };
        let paused = video.player.as_ref().is_some_and(|p| p.is_paused() && !p.has_ended());
        self.start_video(to.clamp(0.0, (duration - 0.1).max(0.0)), paused, cx);
        cx.notify();
    }

    pub fn toggle_video_mute(&mut self, cx: &mut Context<Self>) {
        let Some(video) = self.video_view.as_mut() else { return };
        video.muted = !video.muted;
        if let Some(player) = video.player.as_ref() {
            player.set_volume(if video.muted { 0.0 } else { 1.0 });
        }
        cx.notify();
    }

    /// Swap in the newest decoded frame, freeing the texture of the old one.
    fn refresh_video_frame(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(video) = self.video_view.as_mut() else { return };
        let Some(player) = video.player.as_ref() else { return };
        let Some(bytes) = player.take_frame() else { return };
        let (w, h) = player.info().frame_size();
        let Some(buffer) = image::RgbaImage::from_raw(w, h, bytes) else { return };
        let fresh = Arc::new(RenderImage::new(vec![image::Frame::new(buffer)]));
        if let Some(old) = video.frame.replace(fresh) {
            cx.drop_image(old, Some(window));
        }
    }

    pub(crate) fn render_video_viewer(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.video_view.as_ref()?;
        self.refresh_video_frame(window, cx);
        let position = self.video_position();
        let video = self.video_view.as_ref()?;
        let duration = video.info.as_ref().map(|i| i.duration).unwrap_or(0.0);
        let paused = video.player.as_ref().is_none_or(|p| p.is_paused() || p.has_ended());
        let muted = video.muted;
        let error = video.error.clone().or_else(|| video.player.as_ref().and_then(|p| p.error()));
        let frame = video.frame.clone();
        let has_msg = video.msg.is_some();
        let overlay_cell = video.overlay_bounds.clone();
        let seek_cell = video.seek_bounds.clone();

        let icon: Hsla = rgb(0xe9edef).into();
        let btn_hover: Hsla = rgba(0xffffff1f).into();
        let accent: Hsla = rgb(0x25d366).into();

        // The video box: the frame's aspect ratio fitted into the free area.
        let overlay = overlay_cell.get();
        let (w, h) = (f32::from(overlay.size.width), f32::from(overlay.size.height));
        let (w, h) = if w < 50.0 || h < 50.0 { (880.0, 620.0) } else { (w, h) };
        let area_w = (w - 48.0).max(100.0);
        let area_h = (h - PREVIEW_TOOLBAR_H - CONTROLS_H - 16.0).max(100.0);
        let aspect = video
            .info
            .as_ref()
            .map(|i| { let (fw, fh) = i.frame_size(); fw as f32 / fh as f32 })
            .unwrap_or(16.0 / 9.0);
        let (box_w, box_h) = if area_w / area_h > aspect { (area_h * aspect, area_h) } else { (area_w, area_w / aspect) };
        let box_left = (w - box_w) / 2.0;
        let box_top = PREVIEW_TOOLBAR_H + 8.0 + (area_h - box_h) / 2.0;

        let tool = |id: &'static str, svg_data: &'static [u8], size: f32| {
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
                .child(svg().data(svg_data).size(px(size)).text_color(icon))
        };

        let mut actions = h_flex().items_center().gap_1();
        if has_msg {
            actions = actions.child(tool("vv-save", DOWNLOAD_SVG, 18.0).on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if let Some((_, m)) = this.video_view.as_ref().and_then(|v| v.msg.clone()) {
                        this.save_message_media(&m, cx);
                    }
                    cx.stop_propagation();
                }),
            ));
            actions = actions.child(tool("vv-forward", FORWARD_SVG, 18.0).on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    if let Some((chat_id, m)) = this.video_view.as_ref().and_then(|v| v.msg.clone()) {
                        this.close_video(Some(window), cx);
                        this.open_forward(chat_id, m, cx);
                    }
                    cx.stop_propagation();
                }),
            ));
        }
        actions = actions.child(tool("vv-close", X_SVG, 18.0).on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, window, cx| {
                this.close_video(Some(window), cx);
                cx.stop_propagation();
            }),
        ));
        let toolbar = h_flex()
            .absolute()
            .top_0()
            .left_0()
            .w_full()
            .h(px(PREVIEW_TOOLBAR_H))
            .pl_4()
            // The window controls sit over the top right corner.
            .pr(px(crate::components::titlebar::controls_room(cx) + 8.0))
            .items_center()
            .justify_end()
            .bg(rgba(0x0b141acc))
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(actions);

        let progress = if duration > 0.0 { (position / duration).clamp(0.0, 1.0) } else { 0.0 };
        let seek_bar = div()
            .id("vv-seek")
            .flex_1()
            .h(px(20.0))
            .flex()
            .items_center()
            .cursor_pointer()
            .child({
                let cell = seek_cell.clone();
                canvas(move |bounds, _, _| cell.set(bounds), |_, _, _, _| {}).absolute().size_full()
            })
            .child(
                div()
                    .relative()
                    .w_full()
                    .h(px(4.0))
                    .rounded_full()
                    .bg(rgba(0xffffff40))
                    .child(div().absolute().left_0().top_0().h_full().w(relative(progress)).rounded_full().bg(accent))
                    .child(
                        div()
                            .absolute()
                            .top(px(-4.0))
                            .left(relative(progress))
                            .ml(px(-6.0))
                            .size(px(12.0))
                            .rounded_full()
                            .bg(accent),
                    ),
            )
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                let bounds = seek_cell.get();
                let width = f32::from(bounds.size.width);
                if width > 0.0 {
                    let fraction = (f32::from(ev.position.x - bounds.origin.x) / width).clamp(0.0, 1.0);
                    this.seek_video(fraction * duration, cx);
                }
                cx.stop_propagation();
            }));

        let controls = h_flex()
            .absolute()
            .bottom_0()
            .left_0()
            .w_full()
            .h(px(CONTROLS_H))
            .px_4()
            .gap_3()
            .items_center()
            .bg(rgba(0x0b141acc))
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                tool("vv-play", if paused { PLAY_SVG } else { PAUSE_SVG }, 20.0)
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                        this.toggle_video_pause(cx);
                        cx.stop_propagation();
                    })),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(icon)
                    .child(format!("{} / {}", video_player::format_time(position), video_player::format_time(duration))),
            )
            .child(seek_bar)
            .child(
                tool("vv-mute", if muted { MUTED_SVG } else { VOLUME_SVG }, 18.0)
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                        this.toggle_video_mute(cx);
                        cx.stop_propagation();
                    })),
            );

        let status: Option<AnyElement> = match (&error, &frame) {
            (Some(error), _) => Some(div().text_sm().text_color(icon.opacity(0.85)).child(format!("Couldn't play this video: {error}")).into_any_element()),
            (None, None) => Some(Spinner::new().color(icon).into_any_element()),
            _ => None,
        };

        let overlay_el = div()
            .id("video-viewer")
            .absolute()
            .occlude()
            .inset_0()
            .overflow_hidden()
            .bg(rgba(0x0b141aee))
            .track_focus(&self.video_focus)
            .on_key_down(cx.listener(move |this, ev: &KeyDownEvent, window, cx| {
                match ev.keystroke.key.as_str() {
                    "escape" => this.close_video(Some(window), cx),
                    "space" => this.toggle_video_pause(cx),
                    "left" => this.seek_video(position - SEEK_STEP, cx),
                    "right" => this.seek_video(position + SEEK_STEP, cx),
                    "m" => this.toggle_video_mute(cx),
                    _ => {}
                }
                cx.stop_propagation();
            }))
            // A click beside the video closes the viewer, like the image viewer.
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                this.close_video(Some(window), cx);
            }))
            .child({
                let cell = overlay_cell.clone();
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
            .child(
                div()
                    .id("video-box")
                    .absolute()
                    .left(px(box_left))
                    .top(px(box_top))
                    .w(px(box_w))
                    .h(px(box_h))
                    .bg(rgb(0x000000))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    // Clicking the picture plays or pauses it.
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                        this.toggle_video_pause(cx);
                        cx.stop_propagation();
                    }))
                    .children(frame.map(|frame| {
                        img(ImageSource::Render(frame))
                            .w(px(box_w))
                            .h(px(box_h))
                            .object_fit(ObjectFit::Contain)
                    }))
                    .children(status),
            )
            .child(toolbar)
            .child(controls);

        Some(crate::components::motion::fade_in(overlay_el, "video-viewer-in", 180).into_any_element())
    }
}

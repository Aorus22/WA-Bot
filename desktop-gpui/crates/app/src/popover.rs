//! Panel-popover mode: the app stays resident and shows its UI as a
//! layer-shell surface next to a panel button (see `popover-applet`).
//!
//! * `wabot --popover` starts the resident instance with no window.
//! * `wabot --toggle` asks the running instance to show/hide the popover
//!   (and becomes the resident instance itself when none is running).
//!
//! The surface is a fixed layer-shell popover: anchored to the panel edge,
//! not draggable, like a native DE popover.

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gpui::layer_shell::{Anchor, KeyboardInteractivity, Layer, LayerShellOptions};
use gpui::{
    div, point, px, size, AnyView, AnyWindowHandle, App, AppContext, Bounds, Context, Global,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement,
    Render, Styled, Window, WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions,
};

use crate::theme::manager::AppThemeExt;

/// Popover content size in logical pixels.
pub const POPOVER_WIDTH: f32 = 880.0;
pub const POPOVER_HEIGHT: f32 = 620.0;

/// Marker global: set when the UI runs as a panel popover (no title bar).
pub struct PopoverMode;
impl Global for PopoverMode {}

pub fn is_popover(cx: &App) -> bool {
    cx.has_global::<PopoverMode>()
}

/// Title bar height; the popover has no title bar.
pub fn titlebar_height(cx: &App) -> f32 {
    if is_popover(cx) {
        0.0
    } else {
        crate::components::titlebar::TITLEBAR_HEIGHT
    }
}

fn socket_path() -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    dir.join("wabot-popover.sock")
}

/// Ask a running instance to toggle. `true` when one answered.
pub fn signal_running() -> bool {
    match UnixStream::connect(socket_path()) {
        Ok(mut stream) => stream.write_all(b"toggle\n").is_ok(),
        Err(_) => false,
    }
}

/// Listen for `toggle` requests; one unit message per request.
/// `None` when another instance already owns the socket.
pub fn listen() -> Option<flume::Receiver<()>> {
    let path = socket_path();
    if UnixStream::connect(&path).is_ok() {
        return None;
    }
    let _ = std::fs::remove_file(&path);
    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[popover] cannot bind {path:?}: {e}");
            return None;
        }
    };
    let (tx, rx) = flume::unbounded();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buf = String::new();
            let _ = stream.read_to_string(&mut buf);
            if buf.trim() == "toggle" && tx.send(()).is_err() {
                break;
            }
        }
    });
    Some(rx)
}

/// Panel edge the popover hangs from (`POPOVER_ANCHOR`, set by the applet).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

impl Edge {
    pub fn from_env() -> Self {
        match std::env::var("POPOVER_ANCHOR").unwrap_or_default().as_str() {
            "bottom" => Edge::Bottom,
            "left" => Edge::Left,
            "right" => Edge::Right,
            _ => Edge::Top,
        }
    }
}

/// Where to put the card: the pointer position at toggle time (the click was
/// on the panel button) and the output size, both in logical pixels.
#[derive(Copy, Clone, Debug)]
pub struct Placement {
    pub edge: Edge,
    pub pointer: Option<(f32, f32)>,
    pub screen: (f32, f32),
}

/// Gap between the panel and the card.
const PANEL_GAP: f32 = 4.0;
/// Minimum distance from the screen edges.
const SCREEN_MARGIN: f32 = 8.0;
pub const CARD_RADIUS: f32 = 14.0;

/// Layer-shell options for the card: anchored to the screen corner nearest the
/// button, then pushed along the panel edge with a margin so it sits under the
/// button. Vertical placement stays with the compositor (it respects the
/// panel's exclusive zone), so the card is always flush with the panel.
pub fn card_layer_options(p: &Placement) -> LayerShellOptions {
    let (sw, sh) = p.screen;
    let along = |pointer: Option<f32>, extent: f32, card: f32| -> f32 {
        let max = (extent - card - SCREEN_MARGIN).max(SCREEN_MARGIN);
        match pointer {
            Some(c) => (c - card / 2.0).clamp(SCREEN_MARGIN, max),
            None => max, // unknown: hug the far end, where tray-like applets live
        }
    };
    let x = along(p.pointer.map(|q| q.0), sw, POPOVER_WIDTH);
    let y = along(p.pointer.map(|q| q.1), sh, POPOVER_HEIGHT);
    let (anchor, margin) = match p.edge {
        Edge::Top => (Anchor::TOP | Anchor::LEFT, (PANEL_GAP, 0.0, 0.0, x)),
        Edge::Bottom => (Anchor::BOTTOM | Anchor::LEFT, (0.0, 0.0, PANEL_GAP, x)),
        Edge::Left => (Anchor::TOP | Anchor::LEFT, (y, 0.0, 0.0, PANEL_GAP)),
        Edge::Right => (Anchor::TOP | Anchor::RIGHT, (y, PANEL_GAP, 0.0, 0.0)),
    };
    LayerShellOptions {
        namespace: "wabot-popover".into(),
        layer: Layer::Overlay,
        anchor,
        exclusive_zone: None,
        exclusive_edge: None,
        margin: Some((px(margin.0), px(margin.1), px(margin.2), px(margin.3))),
        keyboard_interactivity: KeyboardInteractivity::OnDemand,
    }
}

/// Full-output transparent surface above the panel. It does two jobs: its
/// first pointer event reveals where the panel button is (Wayland gives
/// clients no global pointer position), and it swallows clicks outside the
/// card so they dismiss the popover like a native one.
pub fn backdrop_layer_options() -> LayerShellOptions {
    LayerShellOptions {
        namespace: "wabot-popover-backdrop".into(),
        layer: Layer::Overlay,
        anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
        exclusive_zone: Some(px(-1.0)),
        exclusive_edge: None,
        margin: None,
        keyboard_interactivity: KeyboardInteractivity::None,
    }
}

/// Open windows of the current popover session.
#[derive(Default)]
pub struct PopoverState {
    backdrop: Option<AnyWindowHandle>,
    card: Option<AnyWindowHandle>,
    /// A card was already requested (pointer arrived or the timeout fired).
    card_requested: bool,
}
impl Global for PopoverState {}

pub fn is_open(cx: &App) -> bool {
    cx.has_global::<PopoverState>() && cx.global::<PopoverState>().backdrop.is_some()
}

pub fn set_card(cx: &mut App, handle: AnyWindowHandle) {
    cx.global_mut::<PopoverState>().card = Some(handle);
}

/// Close the card and the backdrop.
pub fn close_all(cx: &mut App) {
    if !cx.has_global::<PopoverState>() {
        return;
    }
    let state = std::mem::take(cx.global_mut::<PopoverState>());
    for handle in [state.card, state.backdrop].into_iter().flatten() {
        let _ = handle.update(cx, |_, window, _| window.remove_window());
    }
}

type OpenCard = Rc<dyn Fn(&mut App, Placement)>;

fn request_card(cx: &mut App, open_card: &OpenCard, pointer: Option<(f32, f32)>, screen: (f32, f32)) {
    let state = cx.global_mut::<PopoverState>();
    if state.backdrop.is_none() || state.card_requested {
        return;
    }
    state.card_requested = true;
    eprintln!("[popover] card placement: pointer={pointer:?} screen={screen:?}");
    open_card(cx, Placement { edge: Edge::from_env(), pointer, screen });
}

/// Start a popover session: open the backdrop and, once the pointer is known
/// (or after a short timeout), ask `open_card` to create the card window.
pub fn open(cx: &mut App, open_card: OpenCard) {
    if !cx.has_global::<PopoverState>() {
        cx.set_global(PopoverState::default());
    }
    let screen = cx
        .primary_display()
        .map(|d| {
            let b = d.bounds();
            (f32::from(b.size.width), f32::from(b.size.height))
        })
        .unwrap_or((1920.0, 1080.0));

    let backdrop_card = open_card.clone();
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: point(px(0.), px(0.)),
            size: size(px(screen.0), px(screen.1)),
        })),
        titlebar: None,
        app_id: Some("dev.wabot.popover".to_string()),
        window_background: WindowBackgroundAppearance::Transparent,
        kind: WindowKind::LayerShell(backdrop_layer_options()),
        ..Default::default()
    };
    match cx.open_window(options, move |_, cx| {
        cx.new(|_| Backdrop { open_card: backdrop_card })
    }) {
        Ok(handle) => {
            *cx.global_mut::<PopoverState>() = PopoverState {
                backdrop: Some(handle.into()),
                card: None,
                card_requested: false,
            };
        }
        Err(e) => {
            eprintln!("[popover] backdrop failed: {e:?}");
            return;
        }
    }

    // No pointer event (e.g. the compositor only reports one on motion):
    // open anyway, hugging the far end of the panel.
    cx.spawn(async move |cx| {
        cx.background_executor().timer(Duration::from_millis(350)).await;
        let _ = cx.update(|cx| request_card(cx, &open_card, None, screen));
    })
    .detach();
}

struct Backdrop {
    open_card: OpenCard,
}

impl Render for Backdrop {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("popover-backdrop")
            .size_full()
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, window, cx| {
                let size = window.viewport_size();
                let screen = (f32::from(size.width), f32::from(size.height));
                let pointer = (f32::from(ev.position.x), f32::from(ev.position.y));
                let open_card = this.open_card.clone();
                window.defer(cx, move |_, cx| request_card(cx, &open_card, Some(pointer), screen));
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _: &MouseDownEvent, window, cx| {
                    window.defer(cx, |_, cx| close_all(cx));
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|_, _: &MouseDownEvent, window, cx| {
                    window.defer(cx, |_, cx| close_all(cx));
                }),
            )
    }
}

/// Rounded card around the app UI. gpui cannot clip children to a rounded
/// rect, so the content is inset by a few pixels: a rectangular child corner
/// then falls inside the arc instead of poking out of it.
pub struct PopoverFrame {
    pub child: AnyView,
}

impl Render for PopoverFrame {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.app_theme();
        div()
            .size_full()
            .rounded(px(CARD_RADIUS))
            .bg(theme.background)
            .border_1()
            .border_color(theme.border)
            .p(px(5.0))
            .overflow_hidden()
            .child(self.child.clone())
    }
}

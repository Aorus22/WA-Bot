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
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use gpui::layer_shell::{Anchor, KeyboardInteractivity, Layer, LayerShellOptions};
use gpui::{
    div, point, AnimationExt as _, px, size, AnyView, AnyWindowHandle, App, AppContext, Bounds, Context, Global, Pixels,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement,
    Render, Styled, Window, WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions,
};

use crate::theme::shape::ShapeExt;
use crate::theme::surface::SurfaceExt;
use crate::window_material;

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
        cx.shape().titlebar_height
    }
}

fn socket_path() -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    dir.join("wabot-popover.sock")
}

/// What another launch asks the resident instance to do.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Signal {
    /// `--toggle`: show the popover, or hide it when it is already up.
    Toggle,
    /// A plain launch: bring the app to the front.
    Open,
}

impl Signal {
    fn wire(self) -> &'static [u8] {
        match self {
            Signal::Toggle => b"toggle\n",
            Signal::Open => b"open\n",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "toggle" => Some(Signal::Toggle),
            "open" => Some(Signal::Open),
            _ => None,
        }
    }
}

/// Ask a running instance to act. `true` when one answered.
pub fn signal_running(signal: Signal) -> bool {
    match UnixStream::connect(socket_path()) {
        Ok(mut stream) => stream.write_all(signal.wire()).is_ok(),
        Err(_) => false,
    }
}

/// Listen for requests from later launches.
/// `None` when another instance already owns the socket.
pub fn listen() -> Option<flume::Receiver<Signal>> {
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
            if let Some(signal) = Signal::parse(&buf) {
                if tx.send(signal).is_err() {
                    break;
                }
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
    /// A card was already requested (cached position, pointer, or timeout).
    card_requested: bool,
    /// The first pointer event of this session (the panel button) was stored.
    pointer_saved: bool,
    /// Which `open()` this state belongs to, so stale timers do nothing.
    generation: u64,
    started: Option<Instant>,
}
impl Global for PopoverState {}

static GENERATION: AtomicU64 = AtomicU64::new(0);

/// Safety net: a backdrop without a card must never outlive this.
const CARD_DEADLINE: Duration = Duration::from_secs(3);
/// Wait for the first pointer event only when the button position is unknown.
const POINTER_TIMEOUT: Duration = Duration::from_millis(120);

pub fn is_open(cx: &App) -> bool {
    cx.has_global::<PopoverState>() && cx.global::<PopoverState>().backdrop.is_some()
}

pub fn set_card(cx: &mut App, handle: AnyWindowHandle) {
    let state = cx.global_mut::<PopoverState>();
    if state.backdrop.is_none() {
        // The session was closed while the card was being created.
        remove(cx, handle);
        return;
    }
    state.card = Some(handle);
    if let Some(t) = state.started {
        log(&format!("card ready after {} ms", t.elapsed().as_millis()));
    }
}

/// Remove a window; if it is busy (re-entrant update), retry once the current
/// effect cycle is done so a surface is never leaked.
fn remove(cx: &mut App, handle: AnyWindowHandle) {
    if handle.update(cx, |_, window, _| window.remove_window()).is_err() {
        cx.defer(move |cx| {
            if let Err(e) = handle.update(cx, |_, window, _| window.remove_window()) {
                log(&format!("failed to remove window: {e:?}"));
            }
        });
    }
}

/// Close the card and the backdrop.
pub fn close_all(cx: &mut App) {
    if !cx.has_global::<PopoverState>() {
        return;
    }
    let state = std::mem::take(cx.global_mut::<PopoverState>());
    for handle in [state.card, state.backdrop].into_iter().flatten() {
        remove(cx, handle);
    }
}

fn log(msg: &str) {
    use std::io::Write as _;
    eprintln!("[popover] {msg}");
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("wabot-popover.log"))
    {
        let _ = writeln!(f, "{msg}");
    }
}

/// The panel button does not move between toggles, so remember where the
/// pointer was on the first event of a session and open the next card there
/// immediately instead of waiting for a pointer event.
fn pointer_cache_path() -> PathBuf {
    let dir = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .unwrap_or_else(std::env::temp_dir);
    dir.join("wabot-popover-pointer")
}

fn load_pointer() -> Option<(f32, f32)> {
    let text = std::fs::read_to_string(pointer_cache_path()).ok()?;
    let mut it = text.split_whitespace();
    let x: f32 = it.next()?.parse().ok()?;
    let y: f32 = it.next()?.parse().ok()?;
    (x.is_finite() && y.is_finite()).then_some((x, y))
}

fn save_pointer(pointer: (f32, f32)) {
    let path = pointer_cache_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, format!("{} {}\n", pointer.0, pointer.1));
}

type OpenCard = Rc<dyn Fn(&mut App, Placement)>;

fn request_card(cx: &mut App, open_card: &OpenCard, pointer: Option<(f32, f32)>, screen: (f32, f32)) {
    let state = cx.global_mut::<PopoverState>();
    if state.backdrop.is_none() || state.card_requested {
        return;
    }
    state.card_requested = true;
    let waited = state.started.map(|t| t.elapsed().as_millis()).unwrap_or(0);
    log(&format!("card placement after {waited} ms: pointer={pointer:?} screen={screen:?}"));
    open_card(cx, Placement { edge: Edge::from_env(), pointer, screen });
}

/// Start a popover session: open the backdrop and, once the pointer is known
/// (or after a short timeout), ask `open_card` to create the card window.
/// `false` when the backdrop could not be created (the compositor has no
/// layer-shell), so the caller can fall back to a regular window.
pub fn open(cx: &mut App, open_card: OpenCard) -> bool {
    if !cx.has_global::<PopoverState>() {
        cx.set_global(PopoverState::default());
    }
    // Never stack a second backdrop on top of a leftover one.
    close_all(cx);
    let generation = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
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
                generation,
                started: Some(Instant::now()),
                ..Default::default()
            };
        }
        Err(e) => {
            log(&format!("backdrop failed: {e:?}"));
            return false;
        }
    }

    // Known button position: open the card right away.
    if let Some(cached) = load_pointer() {
        request_card(cx, &open_card, Some(cached), screen);
    }

    // Otherwise (first run, or no pointer event): open anyway after a short
    // wait, hugging the far end of the panel.
    let fallback = open_card.clone();
    cx.spawn(async move |cx| {
        cx.background_executor().timer(POINTER_TIMEOUT).await;
        let _ = cx.update(|cx| {
            if cx.has_global::<PopoverState>() && cx.global::<PopoverState>().generation == generation {
                request_card(cx, &fallback, None, screen);
            }
        });
    })
    .detach();

    // Safety net: if no card shows up, drop the full-screen backdrop so it
    // can never keep swallowing clicks.
    cx.spawn(async move |cx| {
        cx.background_executor().timer(CARD_DEADLINE).await;
        let _ = cx.update(|cx| {
            if cx.has_global::<PopoverState>() {
                let s = cx.global::<PopoverState>();
                if s.generation == generation && s.backdrop.is_some() && s.card.is_none() {
                    log("no card within deadline, closing backdrop");
                    close_all(cx);
                }
            }
        });
    })
    .detach();
    true
}

/// Leave popover mode for good: the compositor cannot show the card, so the
/// app behaves like a regular windowed one from here on.
pub fn disable(cx: &mut App) {
    close_all(cx);
    if cx.has_global::<PopoverMode>() {
        cx.remove_global::<PopoverMode>();
    }
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
                window.defer(cx, move |_, cx| {
                    if !cx.has_global::<PopoverState>() {
                        return;
                    }
                    let state = cx.global_mut::<PopoverState>();
                    if !state.pointer_saved {
                        // First event of the session: the pointer is on the panel button.
                        state.pointer_saved = true;
                        save_pointer(pointer);
                    }
                    request_card(cx, &open_card, Some(pointer), screen);
                });
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

/// Approximates a rounded rect with horizontal strips: the compositor blur
/// protocols only take rectangles.
fn rounded_blur_regions(width: f32, height: f32, radius: f32) -> Vec<Bounds<Pixels>> {
    if width <= 0.0 || height <= 0.0 {
        return Vec::new();
    }
    let radius = radius.max(0.0).min(width / 2.0).min(height / 2.0);
    if radius == 0.0 {
        return vec![Bounds { origin: point(px(0.), px(0.)), size: size(px(width), px(height)) }];
    }

    let segments = radius.ceil().clamp(1.0, 10.0) as usize;
    let band = radius / segments as f32;
    let mut regions = Vec::with_capacity(segments * 2 + 1);
    for segment in 0..segments {
        let offset = segment as f32 * band;
        let from_corner = radius - offset;
        let inset = radius - (radius * radius - from_corner * from_corner).max(0.0).sqrt();
        let strip_width = (width - inset * 2.0).max(0.0);
        for y in [offset, height - offset - band] {
            regions.push(Bounds { origin: point(px(inset), px(y)), size: size(px(strip_width), px(band)) });
        }
    }
    let middle = height - radius * 2.0;
    if middle > 0.0 {
        regions.push(Bounds { origin: point(px(0.), px(radius)), size: size(px(width), px(middle)) });
    }
    regions
}

/// Rounded card around the app UI. gpui cannot clip children to a rounded
/// rect, so the content is inset by a few pixels: a rectangular child corner
/// then falls inside the arc instead of poking out of it.
pub struct PopoverFrame {
    pub child: AnyView,
}

impl Render for PopoverFrame {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Blur only what the rounded card covers, not the transparent corners.
        let viewport = window.viewport_size();
        let shape = cx.shape();
        let surface = cx.surface();
        let radius = shape.radius_popover;
        window_material::set_blur_regions(window, &rounded_blur_regions(
            f32::from(viewport.width),
            f32::from(viewport.height),
            radius,
        ));
        let background = surface.popover_frame_fill;
        let border = surface.popover_frame_border;
        // The card is a fresh window on every toggle, so this plays on each
        // open: it fades in while sliding out from the panel edge it hangs
        // from (the layer surface clips the part still "behind" the panel).
        let (dx, dy) = match Edge::from_env() {
            Edge::Top => (0.0, -1.0),
            Edge::Bottom => (0.0, 1.0),
            Edge::Left => (-1.0, 0.0),
            Edge::Right => (1.0, 0.0),
        };
        div()
            .relative()
            .size_full()
            .rounded(px(radius))
            .bg(background)
            .border_1()
            .border_color(border)
            .p(px(5.0))
            .overflow_hidden()
            .child(self.child.clone())
            .with_animation(
                "popover-in",
                crate::components::motion::enter(crate::components::motion::BASE_MS),
                move |el, d| {
                    let travel = 14.0 * (1.0 - d);
                    el.opacity(d).left(px(dx * travel)).top(px(dy * travel))
                },
            )
    }
}

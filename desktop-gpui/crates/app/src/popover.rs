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

use gpui::layer_shell::{Anchor, KeyboardInteractivity, Layer, LayerShellOptions};
use gpui::{px, App, Global};

/// Popover content size in logical pixels.
pub const POPOVER_WIDTH: f32 = 880.0;
pub const POPOVER_HEIGHT: f32 = 620.0;
const EDGE_MARGIN: f32 = 8.0;

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

/// Layer-shell placement for the panel edge the button sits on
/// (`POPOVER_ANCHOR` is set by the applet; defaults to a top panel).
/// Horizontally the popover hugs the right edge, where panel applets live.
pub fn layer_options() -> LayerShellOptions {
    let edge = std::env::var("POPOVER_ANCHOR").unwrap_or_default();
    let (anchor, margin) = match edge.as_str() {
        "bottom" => (Anchor::BOTTOM | Anchor::RIGHT, (0.0, EDGE_MARGIN, EDGE_MARGIN, 0.0)),
        "left" => (Anchor::TOP | Anchor::LEFT, (EDGE_MARGIN, 0.0, 0.0, EDGE_MARGIN)),
        "right" => (Anchor::TOP | Anchor::RIGHT, (EDGE_MARGIN, EDGE_MARGIN, 0.0, 0.0)),
        _ => (Anchor::TOP | Anchor::RIGHT, (EDGE_MARGIN, EDGE_MARGIN, 0.0, 0.0)),
    };
    LayerShellOptions {
        namespace: "wabot-popover".into(),
        layer: Layer::Top,
        anchor,
        exclusive_zone: None,
        exclusive_edge: None,
        margin: Some((px(margin.0), px(margin.1), px(margin.2), px(margin.3))),
        keyboard_interactivity: KeyboardInteractivity::OnDemand,
    }
}

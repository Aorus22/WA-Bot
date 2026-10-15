//! WA Bot desktop app core library.

use std::sync::LazyLock;

pub static TOKIO_RT: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to initialize Tokio runtime")
});

pub mod backend;
pub mod backend_process;
pub mod components;
pub mod icons;
pub mod layout_pref;
pub mod localtime;
pub mod notifications;
pub mod popover;
pub mod router;
pub mod state;
pub mod text_format;
pub mod theme;
pub mod tray;
pub mod views;
pub mod voice_recorder;
pub mod window_material;


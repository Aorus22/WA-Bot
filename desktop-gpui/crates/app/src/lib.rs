//! WA Bot desktop app core library.

use std::sync::LazyLock;

pub static TOKIO_RT: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to initialize Tokio runtime")
});

pub mod backend;
pub mod components;
pub mod assets;
pub mod navigation;
pub mod notifications;
pub mod popover;
pub mod state;
pub mod theme;
pub mod tray;
pub mod updater;
pub mod utils;
pub mod views;
pub mod window;

// Keep the existing module paths stable for the rest of the app.
pub use assets::icons;
pub use backend::process as backend_process;
pub use navigation::router;
pub use utils::{localtime, text_format};
pub use views::chat::media::{video_player, voice_recorder};
pub use window::material as window_material;

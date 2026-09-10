//! WA Bot desktop app core library.

use std::sync::LazyLock;

pub static TOKIO_RT: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to initialize Tokio runtime")
});

pub mod components;
pub mod router;
pub mod state;
pub mod theme;
pub mod views;


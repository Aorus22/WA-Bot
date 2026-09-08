pub mod client;
pub mod dto;
pub mod error;
pub mod multipart;
pub mod ws;

pub use client::HttpClient;
pub use dto::*;
pub use error::{ClientError, Result};
pub use multipart::{SendMediaBuilder, SendMediaOptions};
pub use ws::{WsClient, WsEvent, WsMessage};

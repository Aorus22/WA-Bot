//! Live call media for the desktop client: microphone, speaker and camera
//! bridged to the backend over the binary media WebSocket described in
//! `docs/CALL-MEDIA-PROTOCOL.md`.
//!
//! * [`dsp`]: resampling, 960-sample framing and the playback jitter buffer.
//! * [`audio`]: cpal capture/playback threads.
//! * [`video`]: ffmpeg camera capture, JPEG splitting and decoding.
//! * [`net`]: the WebSocket loop (token, connect, retry, pump frames).
//! * [`session`]: everything for one call, started and stopped as a unit.
//! * [`controller`]: follows `CallManager` and exposes [`CallMediaState`] to the UI.

pub mod audio;
pub mod controller;
pub mod dsp;
pub mod net;
pub mod session;
pub mod ui_link;
pub mod video;

pub use controller::{init, CallMediaState};
pub use session::Session;
pub use ui_link::{Source, UiEvent, Which};

use std::sync::atomic::AtomicBool;

use parking_lot::Mutex;

use dsp::JitterBuffer;

/// A frame on its way to the backend.
#[derive(Debug)]
pub enum Outgoing {
    /// Exactly [`dsp::FRAME_SAMPLES`] mono 16 kHz samples.
    Audio(Vec<i16>),
    /// One JPEG picture.
    Video(Vec<u8>),
}

/// State shared by the threads of one session.
#[derive(Default)]
pub struct Shared {
    pub stop: AtomicBool,
    pub mic_muted: AtomicBool,
    pub speaker_muted: AtomicBool,
    /// Incoming audio waiting for the speaker.
    pub jitter: Mutex<JitterBuffer>,
}

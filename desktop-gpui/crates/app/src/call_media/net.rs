//! The media WebSocket loop of a session.
//!
//! One tokio task owns the socket: it forwards outgoing frames from the capture
//! threads, hands incoming audio to the jitter buffer and incoming pictures to
//! the decoder, and reconnects (with a fresh token) whenever the socket drops.
//! A dropped media socket never ends the call.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;
use wabot_backend_client::media::{FrameKind, MediaFrame, MediaSocket, AUDIO_FRAME_SAMPLES};

use super::dsp::pcm_s16le_to_f32;
use super::ui_link::{Source, UiLink};
use super::video::{DecodeJob, Decoder};
use super::{Outgoing, Shared};

pub type ConnectFuture = Pin<Box<dyn Future<Output = Result<MediaSocket, String>> + Send>>;
/// Opens a fresh media socket (token request + connect).
pub type Connector = Arc<dyn Fn() -> ConnectFuture + Send + Sync>;

pub struct NetConfig {
    pub connector: Connector,
    pub out_rx: flume::Receiver<Outgoing>,
    pub shared: Arc<Shared>,
    /// Decoder for the remote picture.
    pub remote: Decoder,
    pub ui: UiLink,
    pub stop: watch::Receiver<bool>,
    pub retry_delay: Duration,
}

/// Consecutive connect failures before the user is told about it. The first
/// ones are normal while a call is still becoming active.
const ERROR_AFTER_FAILURES: u32 = 3;
/// After this many failures retry less often.
const SLOW_AFTER_FAILURES: u32 = 6;

enum PumpEnd {
    Stopped,
    Closed,
}

pub async fn run(mut cfg: NetConfig) {
    let mut failures = 0u32;
    loop {
        if *cfg.stop.borrow() {
            break;
        }
        let attempt = tokio::select! {
            result = (cfg.connector)() => Some(result),
            _ = cfg.stop.changed() => None,
        };
        let Some(result) = attempt else { break };

        match result {
            Ok(mut socket) => {
                failures = 0;
                cfg.ui.ok(Source::Network);
                // Whatever piled up while offline is stale: media is real time.
                while cfg.out_rx.try_recv().is_ok() {}
                cfg.shared.jitter.lock().clear();
                let end = pump(&mut socket, &mut cfg).await;
                socket.close().await;
                if matches!(end, PumpEnd::Stopped) {
                    break;
                }
            }
            Err(message) => {
                failures += 1;
                eprintln!("[call] media connect failed ({failures}): {message}");
                if failures >= ERROR_AFTER_FAILURES {
                    cfg.ui.error(
                        Source::Network,
                        format!("Call audio and video unavailable: {message}"),
                    );
                }
            }
        }

        let delay = if failures >= SLOW_AFTER_FAILURES { cfg.retry_delay * 3 } else { cfg.retry_delay };
        tokio::select! {
            _ = tokio::time::sleep(delay) => {}
            _ = cfg.stop.changed() => break,
        }
    }
}

async fn pump(socket: &mut MediaSocket, cfg: &mut NetConfig) -> PumpEnd {
    loop {
        tokio::select! {
            _ = cfg.stop.changed() => return PumpEnd::Stopped,
            incoming = socket.next_frame() => match incoming {
                Some(Ok(frame)) => handle_incoming(&frame, &cfg.shared, &cfg.remote),
                Some(Err(error)) => {
                    eprintln!("[call] media socket error: {error}");
                    return PumpEnd::Closed;
                }
                None => return PumpEnd::Closed,
            },
            outgoing = cfg.out_rx.recv_async() => match outgoing {
                Err(_) => return PumpEnd::Stopped,
                Ok(media) => {
                    let Some(frame) = encode_outgoing(media) else { continue };
                    if let Err(error) = socket.send(&frame).await {
                        eprintln!("[call] media send failed: {error}");
                        return PumpEnd::Closed;
                    }
                }
            },
        }
    }
}

fn encode_outgoing(media: Outgoing) -> Option<MediaFrame> {
    match media {
        // The server insists on exact 960-sample frames.
        Outgoing::Audio(samples) if samples.len() == AUDIO_FRAME_SAMPLES => {
            Some(MediaFrame::outgoing_audio(&samples))
        }
        Outgoing::Audio(_) => None,
        Outgoing::Video(jpeg) => Some(MediaFrame::outgoing_video(jpeg)),
    }
}

fn handle_incoming(frame: &MediaFrame, shared: &Shared, remote: &Decoder) {
    match frame.kind {
        FrameKind::IncomingAudio => {
            shared.jitter.lock().push(&pcm_s16le_to_f32(&frame.payload));
        }
        FrameKind::IncomingVideo => {
            remote.submit(DecodeJob {
                jpeg: frame.payload.clone(),
                quarter_turns: frame.video_metadata().orientation,
            });
        }
        // Keyframe requests do not apply to JPEG mode; unknown kinds are ignored.
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_exact_audio_frames_are_sent() {
        assert!(encode_outgoing(Outgoing::Audio(vec![0; 959])).is_none());
        assert!(encode_outgoing(Outgoing::Audio(vec![0; 960])).is_some());
        let frame = encode_outgoing(Outgoing::Video(vec![0xFF, 0xD8, 0xFF, 0xD9])).unwrap();
        assert_eq!(frame.kind, FrameKind::OutgoingVideo);
    }
}

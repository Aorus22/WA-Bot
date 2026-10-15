//! Everything that belongs to one call: the network task, the microphone and
//! speaker threads, the decoders and (on demand) the camera.
//!
//! Creating a [`Session`] returns immediately; devices and the socket come up
//! in the background and report problems through the UI channel. Dropping it
//! stops all of that.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tokio::sync::watch;
use wabot_backend_client::client::HttpClient;
use wabot_backend_client::error::ClientError;
use wabot_backend_client::media::MediaSocket;

use super::net::{self, Connector, NetConfig};
use super::ui_link::{FrameSlots, UiEvent, UiLink, Which};
use super::video::{CameraCapture, CameraConfig, Decoder};
use super::{audio, Outgoing, Shared};

/// Outgoing queue: a few audio frames of slack, videos are dropped when full.
const OUTGOING_QUEUE: usize = 8;

pub struct Session {
    pub id: u64,
    pub call_id: String,
    shared: Arc<Shared>,
    ui: UiLink,
    out_tx: flume::Sender<Outgoing>,
    local_decoder: Decoder,
    camera: Mutex<Option<CameraCapture>>,
    want_video: AtomicBool,
    stop_tx: watch::Sender<bool>,
}

impl Session {
    /// Start media for `call_id` against the backend at `base_url`.
    pub fn start(
        id: u64,
        call_id: String,
        base_url: &str,
        ui_tx: flume::Sender<UiEvent>,
    ) -> Self {
        let client = HttpClient::new(base_url);
        let call = call_id.clone();
        let connector: Connector = Arc::new(move || {
            let client = client.clone();
            let call = call.clone();
            Box::pin(async move { MediaSocket::open(&client, &call).await.map_err(describe) })
        });
        Self::start_with(id, call_id, connector, ui_tx, true, Duration::from_secs(1))
    }

    /// Like [`start`](Self::start) with an injected connector. `devices: false`
    /// skips the microphone and speaker (tests, headless machines).
    pub fn start_with(
        id: u64,
        call_id: String,
        connector: Connector,
        ui_tx: flume::Sender<UiEvent>,
        devices: bool,
        retry_delay: Duration,
    ) -> Self {
        let ui = UiLink::new(id, ui_tx);
        let shared = Arc::new(Shared::default());
        let (out_tx, out_rx) = flume::bounded::<Outgoing>(OUTGOING_QUEUE);
        let (stop_tx, stop_rx) = watch::channel(false);

        let remote = Decoder::spawn("call-decode-remote", Which::Remote, false, ui.clone());
        let local_decoder = Decoder::spawn("call-decode-local", Which::Local, true, ui.clone());

        crate::TOKIO_RT.spawn(net::run(NetConfig {
            connector,
            out_rx,
            shared: Arc::clone(&shared),
            remote,
            ui: ui.clone(),
            stop: stop_rx,
            retry_delay,
        }));

        if devices {
            audio::spawn_microphone(Arc::clone(&shared), out_tx.clone(), ui.clone());
            audio::spawn_speaker(Arc::clone(&shared), ui.clone());
        }

        Self {
            id,
            call_id,
            shared,
            ui,
            out_tx,
            local_decoder,
            camera: Mutex::new(None),
            want_video: AtomicBool::new(false),
            stop_tx,
        }
    }

    /// Where this session's decoded pictures are delivered.
    pub fn slots(&self) -> Arc<FrameSlots> {
        self.ui.slots()
    }

    pub fn set_mic_muted(&self, muted: bool) {
        self.shared.mic_muted.store(muted, Ordering::Relaxed);
    }

    pub fn set_speaker_muted(&self, muted: bool) {
        self.shared.speaker_muted.store(muted, Ordering::Relaxed);
    }

    /// Switch the local camera on or off. Idempotent: a camera that failed to
    /// start is retried only after it was switched off and on again.
    pub fn set_video(&self, on: bool) {
        if self.want_video.swap(on, Ordering::SeqCst) == on {
            return;
        }
        let mut camera = self.camera.lock();
        if on {
            *camera = Some(CameraCapture::start(
                CameraConfig::from_env(),
                self.out_tx.clone(),
                self.local_decoder.clone(),
                self.ui.clone(),
            ));
        } else {
            *camera = None; // kills ffmpeg
            self.ui.ok(super::ui_link::Source::Camera);
            self.ui.clear_local();
        }
    }

    pub fn video_wanted(&self) -> bool {
        self.want_video.load(Ordering::SeqCst)
    }

    /// Test hook: a clone of the playback buffer state.
    pub fn buffered_audio_samples(&self) -> usize {
        self.shared.jitter.lock().len()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        let _ = self.stop_tx.send(true);
        *self.camera.lock() = None;
    }
}

/// A short, user-facing reason for a failed token/connect.
pub fn describe(error: ClientError) -> String {
    match error {
        ClientError::Api { status, message } => format!("{message} (HTTP {})", status.as_u16()),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::call_media::ui_link::Source;
    use std::sync::atomic::AtomicUsize;
    use std::time::Instant;

    use tokio::net::TcpListener;

    fn jpeg(width: u32, height: u32) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(width, height, image::Rgb([10, 200, 30]));
        let mut out = Vec::new();
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 80);
        img.write_with_encoder(encoder).unwrap();
        out
    }

    fn wait_for<T>(timeout: Duration, mut check: impl FnMut() -> Option<T>) -> Option<T> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Some(value) = check() {
                return Some(value);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        None
    }

    /// Echo server: audio comes back as incoming audio; every connection also
    /// gets one rotated picture. `drop_first` closes the first connection right
    /// away to exercise the reconnect path.
    fn spawn_server(drop_first: bool, picture: Vec<u8>) -> (String, Arc<AtomicUsize>) {
        use futures_util::{SinkExt, StreamExt};
        use tokio_tungstenite::tungstenite::Message;
        use wabot_backend_client::media::{FrameKind, MediaFrame};

        let accepted = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&accepted);
        let listener = crate::TOKIO_RT
            .block_on(TcpListener::bind("127.0.0.1:0"))
            .unwrap();
        let addr = listener.local_addr().unwrap();
        crate::TOKIO_RT.spawn(async move {
            loop {
                let Ok((tcp, _)) = listener.accept().await else { break };
                let n = counter.fetch_add(1, Ordering::SeqCst);
                let picture = picture.clone();
                tokio::spawn(async move {
                    let Ok(mut ws) = tokio_tungstenite::accept_async(tcp).await else { return };
                    if drop_first && n == 0 {
                        let _ = ws.close(None).await;
                        return;
                    }
                    let video = MediaFrame::new(
                        FrameKind::IncomingVideo,
                        br#"{"participant_jid":"p@s.whatsapp.net","orientation":1}"#.to_vec(),
                        picture,
                    );
                    let _ = ws.send(Message::Binary(video.encode().unwrap().into())).await;
                    while let Some(Ok(msg)) = ws.next().await {
                        if let Message::Binary(data) = msg {
                            let Ok(frame) = MediaFrame::decode(&data) else { continue };
                            if frame.kind == FrameKind::OutgoingAudio {
                                let reply = MediaFrame::new(FrameKind::IncomingAudio, vec![], frame.payload);
                                let _ = ws.send(Message::Binary(reply.encode().unwrap().into())).await;
                            }
                        }
                    }
                });
            }
        });
        (format!("ws://{addr}/ws/calls/c1/media?token=t&video=jpeg"), accepted)
    }

    fn connector_for(url: String) -> Connector {
        Arc::new(move || {
            let url = url.clone();
            Box::pin(async move { MediaSocket::connect(&url).await.map_err(describe) })
        })
    }

    #[test]
    fn audio_and_video_flow_through_a_session() {
        let (url, accepted) = spawn_server(false, jpeg(64, 32));
        let (tx, rx) = flume::unbounded();
        let session = Session::start_with(7, "c1".into(), connector_for(url), tx, false, Duration::from_millis(20));

        // Incoming picture: 64x32 rotated one quarter turn -> 32x64.
        let event = wait_for(Duration::from_secs(5), || rx.try_recv().ok().filter(|e| matches!(e, UiEvent::Frame { .. })))
            .expect("a picture event");
        assert_eq!(event, UiEvent::Frame { session: 7, which: Which::Remote });
        let image = session.slots().take(Which::Remote).expect("picture in the slot");
        assert_eq!((image.size(0).width.0, image.size(0).height.0), (32, 64));

        // Outgoing audio is echoed back into the jitter buffer.
        for _ in 0..3 {
            session.out_tx.send(Outgoing::Audio(vec![1000; 960])).unwrap();
        }
        let buffered = wait_for(Duration::from_secs(5), || {
            let n = session.buffered_audio_samples();
            (n >= 3 * 960).then_some(n)
        })
        .expect("echoed audio");
        assert_eq!(buffered, 3 * 960);
        assert_eq!(accepted.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_dropped_socket_is_reopened() {
        let (url, accepted) = spawn_server(true, jpeg(16, 16));
        let (tx, rx) = flume::unbounded();
        let session = Session::start_with(1, "c1".into(), connector_for(url), tx, false, Duration::from_millis(20));
        wait_for(Duration::from_secs(5), || (accepted.load(Ordering::SeqCst) >= 2).then_some(()))
            .expect("second connection");
        // The picture only comes from the second connection.
        wait_for(Duration::from_secs(5), || rx.try_recv().ok().filter(|e| matches!(e, UiEvent::Frame { .. })))
            .expect("picture after reconnect");
        drop(session);
    }

    #[test]
    fn repeated_connect_failures_surface_an_error_then_recover() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&attempts);
        let (url, _accepted) = spawn_server(false, jpeg(16, 16));
        let connector: Connector = Arc::new(move || {
            let n = counter.fetch_add(1, Ordering::SeqCst);
            let url = url.clone();
            Box::pin(async move {
                if n < 4 {
                    Err("call not active (HTTP 409)".to_string())
                } else {
                    MediaSocket::connect(&url).await.map_err(describe)
                }
            })
        });
        let (tx, rx) = flume::unbounded();
        let _session = Session::start_with(3, "c1".into(), connector, tx, false, Duration::from_millis(10));

        let mut saw_error = false;
        let mut saw_clear = false;
        wait_for(Duration::from_secs(5), || {
            while let Ok(event) = rx.try_recv() {
                match event {
                    UiEvent::Notice { source: Source::Network, message: Some(m), .. } => {
                        assert!(m.contains("call not active"), "{m}");
                        saw_error = true;
                    }
                    UiEvent::Notice { source: Source::Network, message: None, .. } if saw_error => {
                        saw_clear = true;
                    }
                    _ => {}
                }
            }
            saw_clear.then_some(())
        })
        .expect("error shown, then cleared after reconnecting");
    }

    #[test]
    fn dropping_a_session_stops_the_network_task() {
        let (url, accepted) = spawn_server(false, jpeg(16, 16));
        let (tx, _rx) = flume::unbounded();
        let session = Session::start_with(2, "c1".into(), connector_for(url), tx, false, Duration::from_millis(10));
        wait_for(Duration::from_secs(5), || (accepted.load(Ordering::SeqCst) >= 1).then_some(())).unwrap();
        drop(session);
        std::thread::sleep(Duration::from_millis(300));
        // No reconnect storm after the drop.
        assert_eq!(accepted.load(Ordering::SeqCst), 1);
    }

    /// Opens the real default microphone and speaker for a moment (needs audio
    /// hardware): `cargo test -p wabot call_media::session -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn real_audio_devices_open_or_report_an_error() {
        let (url, _accepted) = spawn_server(false, jpeg(16, 16));
        let (tx, rx) = flume::unbounded();
        let session = Session::start_with(4, "c1".into(), connector_for(url), tx, true, Duration::from_millis(20));
        std::thread::sleep(Duration::from_millis(1500));
        let mut mic = None;
        let mut speaker = None;
        for event in rx.try_iter() {
            if let UiEvent::Notice { source, message, .. } = event {
                match source {
                    Source::Mic => mic = Some(message),
                    Source::Speaker => speaker = Some(message),
                    _ => {}
                }
            }
        }
        eprintln!("mic: {mic:?}, speaker: {speaker:?}");
        assert!(mic.is_some() && speaker.is_some(), "both devices must report back");
        drop(session);
    }

    #[test]
    fn video_toggle_is_idempotent_and_reports_a_missing_ffmpeg() {
        std::env::set_var("WABOT_FFMPEG", "/nonexistent/ffmpeg-for-test");
        let (url, _accepted) = spawn_server(false, jpeg(16, 16));
        let (tx, rx) = flume::unbounded();
        let session = Session::start_with(5, "c1".into(), connector_for(url), tx, false, Duration::from_millis(10));
        session.set_video(true);
        session.set_video(true);
        assert!(session.video_wanted());
        let message = wait_for(Duration::from_secs(5), || {
            while let Ok(event) = rx.try_recv() {
                if let UiEvent::Notice { source: Source::Camera, message: Some(m), .. } = event {
                    return Some(m);
                }
            }
            None
        })
        .expect("camera error");
        assert!(message.contains("ffmpeg was not found"), "{message}");
        session.set_video(false);
        assert!(!session.video_wanted());
        std::env::remove_var("WABOT_FFMPEG");
    }
}

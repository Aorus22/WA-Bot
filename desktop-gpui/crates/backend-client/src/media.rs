//! Call media client: token endpoint + the binary media WebSocket.
//!
//! The wire contract lives in `docs/CALL-MEDIA-PROTOCOL.md`. In short: fetch a
//! single-use token with `POST /api/calls/{id}/media-session`, open
//! `ws(s)://host{ws_path}?token=…&video=jpeg` and exchange binary frames of
//! the shape `kind | u16 BE metadata length | JSON metadata | payload`.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

use crate::client::HttpClient;
use crate::error::{ClientError, Result};

/// Samples in one audio frame (60 ms at 16 kHz mono).
pub const AUDIO_FRAME_SAMPLES: usize = 960;
/// Audio sample rate on the wire.
pub const AUDIO_SAMPLE_RATE: u32 = 16_000;
/// The server reads at most 1 MiB per message; stay below it.
pub const MAX_FRAME_BYTES: usize = 1 << 20;

/// Response of `POST /api/calls/{id}/media-session`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MediaSession {
    pub token: String,
    /// Unix milliseconds.
    #[serde(default)]
    pub expires_at: i64,
    pub ws_path: String,
}

/// Frame kind byte (first byte of every message).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    OutgoingAudio,
    OutgoingVideo,
    IncomingAudio,
    IncomingVideo,
    KeyframeRequest,
    Unknown(u8),
}

impl FrameKind {
    pub fn to_byte(self) -> u8 {
        match self {
            FrameKind::OutgoingAudio => 0x01,
            FrameKind::OutgoingVideo => 0x02,
            FrameKind::IncomingAudio => 0x03,
            FrameKind::IncomingVideo => 0x04,
            FrameKind::KeyframeRequest => 0x05,
            FrameKind::Unknown(b) => b,
        }
    }

    pub fn from_byte(byte: u8) -> Self {
        match byte {
            0x01 => FrameKind::OutgoingAudio,
            0x02 => FrameKind::OutgoingVideo,
            0x03 => FrameKind::IncomingAudio,
            0x04 => FrameKind::IncomingVideo,
            0x05 => FrameKind::KeyframeRequest,
            other => FrameKind::Unknown(other),
        }
    }
}

/// One decoded media message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaFrame {
    pub kind: FrameKind,
    /// Raw UTF-8 JSON metadata (possibly empty).
    pub metadata: Vec<u8>,
    pub payload: Vec<u8>,
}

/// Metadata of an incoming video frame (`kind 0x04`).
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct VideoMetadata {
    #[serde(default)]
    pub participant_jid: String,
    /// Clockwise quarter turns (0..=3) to apply so the picture is upright.
    #[serde(default)]
    pub orientation: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    TooShort,
    BadMetadataLength,
    MetadataTooLong,
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrameError::TooShort => write!(f, "media frame too short"),
            FrameError::BadMetadataLength => write!(f, "media frame metadata length exceeds the frame"),
            FrameError::MetadataTooLong => write!(f, "media frame metadata longer than 65535 bytes"),
        }
    }
}

impl std::error::Error for FrameError {}

impl MediaFrame {
    pub fn new(kind: FrameKind, metadata: Vec<u8>, payload: Vec<u8>) -> Self {
        Self { kind, metadata, payload }
    }

    /// An outgoing audio frame (`0x01`) from 960 signed 16-bit samples.
    pub fn outgoing_audio(samples: &[i16]) -> Self {
        let mut payload = Vec::with_capacity(samples.len() * 2);
        for sample in samples {
            payload.extend_from_slice(&sample.to_le_bytes());
        }
        Self::new(FrameKind::OutgoingAudio, Vec::new(), payload)
    }

    /// An outgoing video frame (`0x02`) carrying one JPEG.
    pub fn outgoing_video(jpeg: Vec<u8>) -> Self {
        Self::new(FrameKind::OutgoingVideo, Vec::new(), jpeg)
    }

    /// Serialise to the wire layout.
    pub fn encode(&self) -> std::result::Result<Vec<u8>, FrameError> {
        if self.metadata.len() > u16::MAX as usize {
            return Err(FrameError::MetadataTooLong);
        }
        let mut out = Vec::with_capacity(3 + self.metadata.len() + self.payload.len());
        out.push(self.kind.to_byte());
        out.extend_from_slice(&(self.metadata.len() as u16).to_be_bytes());
        out.extend_from_slice(&self.metadata);
        out.extend_from_slice(&self.payload);
        Ok(out)
    }

    /// Parse one binary WebSocket message.
    pub fn decode(data: &[u8]) -> std::result::Result<Self, FrameError> {
        if data.len() < 3 {
            return Err(FrameError::TooShort);
        }
        let meta_len = u16::from_be_bytes([data[1], data[2]]) as usize;
        if 3 + meta_len > data.len() {
            return Err(FrameError::BadMetadataLength);
        }
        Ok(Self {
            kind: FrameKind::from_byte(data[0]),
            metadata: data[3..3 + meta_len].to_vec(),
            payload: data[3 + meta_len..].to_vec(),
        })
    }

    /// Incoming audio payload as signed 16-bit little-endian samples.
    /// A trailing odd byte is ignored.
    pub fn audio_samples(&self) -> Vec<i16> {
        self.payload
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect()
    }

    /// Video metadata; missing or malformed JSON yields the defaults.
    pub fn video_metadata(&self) -> VideoMetadata {
        if self.metadata.is_empty() {
            return VideoMetadata::default();
        }
        let mut meta: VideoMetadata = serde_json::from_slice(&self.metadata).unwrap_or_default();
        meta.orientation %= 4;
        meta
    }
}

impl HttpClient {
    /// Ask for a single-use media token for an active call. The token expires
    /// after about two minutes, so request a fresh one for every (re)connect.
    ///
    /// Like every other request of this client no `X-API-Secret` is attached;
    /// the desktop talks to its own local backend.
    pub async fn create_media_session(&self, call_id: &str) -> Result<MediaSession> {
        let resp = self
            .client()
            .post(format!(
                "{}/calls/{}/media-session",
                self.base_url(),
                path_escape(call_id)
            ))
            .timeout(Duration::from_secs(10))
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            let message = serde_json::from_str::<serde_json::Value>(&text)
                .ok()
                .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
                .unwrap_or(text);
            return Err(ClientError::Api {
                status,
                message: if message.is_empty() { status.to_string() } else { message },
            });
        }
        Ok(resp.json::<MediaSession>().await?)
    }

    /// Absolute `ws://`/`wss://` URL of the media socket for a session
    /// (JPEG video mode).
    pub fn media_ws_url(&self, session: &MediaSession) -> String {
        let root = self.base_url().trim_end_matches("/api");
        let root = if let Some(rest) = root.strip_prefix("https://") {
            format!("wss://{rest}")
        } else if let Some(rest) = root.strip_prefix("http://") {
            format!("ws://{rest}")
        } else {
            root.to_string()
        };
        let path = if session.ws_path.starts_with('/') {
            session.ws_path.clone()
        } else {
            format!("/{}", session.ws_path)
        };
        let sep = if path.contains('?') { '&' } else { '?' };
        format!("{root}{path}{sep}token={}&video=jpeg", path_escape(&session.token))
    }
}

fn path_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// An open media WebSocket.
pub struct MediaSocket {
    stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl MediaSocket {
    /// Connect to a URL built by [`HttpClient::media_ws_url`]. An HTTP-level
    /// rejection (401/403/409 …) comes back as [`ClientError::Api`].
    pub async fn connect(url: &str) -> Result<Self> {
        let attempt = tokio::time::timeout(Duration::from_secs(10), connect_async(url)).await;
        match attempt {
            Err(_) => Err(ClientError::WebSocket("connection timed out".into())),
            Ok(Ok((stream, _))) => Ok(Self { stream }),
            Ok(Err(WsError::Http(response))) => {
                let status = reqwest::StatusCode::from_u16(response.status().as_u16())
                    .unwrap_or(reqwest::StatusCode::BAD_GATEWAY);
                let body = response
                    .body()
                    .as_ref()
                    .map(|b| String::from_utf8_lossy(b).trim().to_string())
                    .unwrap_or_default();
                let message = serde_json::from_str::<serde_json::Value>(&body)
                    .ok()
                    .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
                    .unwrap_or(body);
                Err(ClientError::Api {
                    status,
                    message: if message.is_empty() { status.to_string() } else { message },
                })
            }
            Ok(Err(error)) => Err(ClientError::WebSocket(error.to_string())),
        }
    }

    /// Fetch a token and open the socket for `call_id` in one go.
    pub async fn open(client: &HttpClient, call_id: &str) -> Result<Self> {
        let session = client.create_media_session(call_id).await?;
        Self::connect(&client.media_ws_url(&session)).await
    }

    pub async fn send(&mut self, frame: &MediaFrame) -> Result<()> {
        let data = frame.encode().map_err(|e| ClientError::Custom(e.to_string()))?;
        if data.len() > MAX_FRAME_BYTES {
            return Err(ClientError::Custom("media frame larger than 1 MiB".into()));
        }
        self.stream
            .send(Message::Binary(data.into()))
            .await
            .map_err(|e| ClientError::WebSocket(e.to_string()))
    }

    /// Next media frame. Pings/pongs, text and malformed messages are skipped;
    /// `None` means the socket closed cleanly. Cancel-safe.
    pub async fn next_frame(&mut self) -> Option<Result<MediaFrame>> {
        loop {
            match self.stream.next().await? {
                Ok(Message::Binary(data)) => {
                    if let Ok(frame) = MediaFrame::decode(&data) {
                        return Some(Ok(frame));
                    }
                }
                Ok(Message::Close(_)) => return None,
                Ok(_) => {}
                Err(WsError::ConnectionClosed) | Err(WsError::AlreadyClosed) => return None,
                Err(error) => return Some(Err(ClientError::WebSocket(error.to_string()))),
            }
        }
    }

    /// Send a close frame (best effort, bounded).
    pub async fn close(&mut self) {
        let _ = tokio::time::timeout(Duration::from_secs(1), self.stream.close(None)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_async;

    #[test]
    fn frame_round_trips() {
        let frame = MediaFrame::new(
            FrameKind::IncomingVideo,
            br#"{"participant_jid":"a@s.whatsapp.net","orientation":1}"#.to_vec(),
            vec![0xFF, 0xD8, 1, 2, 3, 0xFF, 0xD9],
        );
        let bytes = frame.encode().unwrap();
        assert_eq!(bytes[0], 0x04);
        assert_eq!(u16::from_be_bytes([bytes[1], bytes[2]]) as usize, frame.metadata.len());
        let back = MediaFrame::decode(&bytes).unwrap();
        assert_eq!(back, frame);
        assert_eq!(back.video_metadata().orientation, 1);
        assert_eq!(back.video_metadata().participant_jid, "a@s.whatsapp.net");
    }

    #[test]
    fn audio_frame_is_960_samples_little_endian() {
        let samples: Vec<i16> = (0..AUDIO_FRAME_SAMPLES as i32).map(|i| (i - 480) as i16).collect();
        let bytes = MediaFrame::outgoing_audio(&samples).encode().unwrap();
        assert_eq!(bytes.len(), 3 + 1920);
        assert_eq!(&bytes[..3], &[0x01, 0, 0]);
        assert_eq!(i16::from_le_bytes([bytes[3], bytes[4]]), -480);
        let back = MediaFrame::decode(&bytes).unwrap();
        assert_eq!(back.audio_samples(), samples);
    }

    #[test]
    fn decode_rejects_malformed_frames() {
        assert_eq!(MediaFrame::decode(&[]), Err(FrameError::TooShort));
        assert_eq!(MediaFrame::decode(&[3, 0]), Err(FrameError::TooShort));
        assert_eq!(MediaFrame::decode(&[3, 0, 5, b'{']), Err(FrameError::BadMetadataLength));
        // Empty payload and metadata are fine.
        let frame = MediaFrame::decode(&[3, 0, 0]).unwrap();
        assert_eq!(frame.kind, FrameKind::IncomingAudio);
        assert!(frame.payload.is_empty());
        // Unknown kinds survive the round trip.
        assert_eq!(MediaFrame::decode(&[9, 0, 0, 7]).unwrap().kind, FrameKind::Unknown(9));
    }

    #[test]
    fn oversized_metadata_cannot_be_encoded() {
        let frame = MediaFrame::new(FrameKind::OutgoingAudio, vec![b' '; 70_000], vec![]);
        assert_eq!(frame.encode(), Err(FrameError::MetadataTooLong));
    }

    #[test]
    fn video_metadata_is_forgiving() {
        let frame = MediaFrame::new(FrameKind::IncomingVideo, Vec::new(), vec![]);
        assert_eq!(frame.video_metadata(), VideoMetadata::default());
        let frame = MediaFrame::new(FrameKind::IncomingVideo, b"not json".to_vec(), vec![]);
        assert_eq!(frame.video_metadata().orientation, 0);
        let frame = MediaFrame::new(FrameKind::IncomingVideo, br#"{"orientation":7}"#.to_vec(), vec![]);
        assert_eq!(frame.video_metadata().orientation, 3);
    }

    #[test]
    fn ws_url_follows_the_base_url() {
        let session = MediaSession {
            token: "ab+c/=".into(),
            expires_at: 0,
            ws_path: "/ws/calls/call_123/media".into(),
        };
        let client = HttpClient::new("http://127.0.0.1:3000/api");
        assert_eq!(
            client.media_ws_url(&session),
            "ws://127.0.0.1:3000/ws/calls/call_123/media?token=ab%2Bc%2F%3D&video=jpeg"
        );
        let client = HttpClient::new("https://bot.example.com");
        assert!(client.media_ws_url(&session).starts_with("wss://bot.example.com/ws/calls/"));
    }

    #[test]
    fn media_session_parses_the_documented_response() {
        let session: MediaSession = serde_json::from_str(
            r#"{ "token": "t", "expires_at": 1760000000000, "ws_path": "/ws/calls/call_123/media" }"#,
        )
        .unwrap();
        assert_eq!(session.token, "t");
        assert_eq!(session.expires_at, 1_760_000_000_000);
    }

    /// A throw-away server that echoes outgoing audio back as incoming audio
    /// (kind 0x03) and outgoing video as incoming video with metadata.
    #[tokio::test]
    async fn socket_talks_to_a_fake_server() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = accept_async(tcp).await.unwrap();
            while let Some(Ok(msg)) = ws.next().await {
                if let Message::Binary(data) = msg {
                    let frame = MediaFrame::decode(&data).unwrap();
                    let reply = match frame.kind {
                        FrameKind::OutgoingAudio => {
                            MediaFrame::new(FrameKind::IncomingAudio, vec![], frame.payload)
                        }
                        FrameKind::OutgoingVideo => MediaFrame::new(
                            FrameKind::IncomingVideo,
                            br#"{"participant_jid":"x","orientation":2}"#.to_vec(),
                            frame.payload,
                        ),
                        _ => continue,
                    };
                    ws.send(Message::Binary(reply.encode().unwrap().into())).await.unwrap();
                }
            }
        });

        let mut socket = MediaSocket::connect(&format!("ws://{addr}/ws/calls/c/media?token=t&video=jpeg"))
            .await
            .unwrap();
        let samples = vec![123i16; AUDIO_FRAME_SAMPLES];
        socket.send(&MediaFrame::outgoing_audio(&samples)).await.unwrap();
        let echoed = socket.next_frame().await.unwrap().unwrap();
        assert_eq!(echoed.kind, FrameKind::IncomingAudio);
        assert_eq!(echoed.audio_samples(), samples);

        socket.send(&MediaFrame::outgoing_video(vec![0xFF, 0xD8, 0xFF, 0xD9])).await.unwrap();
        let video = socket.next_frame().await.unwrap().unwrap();
        assert_eq!(video.kind, FrameKind::IncomingVideo);
        assert_eq!(video.video_metadata().orientation, 2);
        socket.close().await;
    }

    /// Minimal one-shot HTTP responder for the token endpoint.
    async fn serve_once(status: &str, body: &str) -> (String, tokio::task::JoinHandle<String>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let handle = tokio::spawn(async move {
            let (mut tcp, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 4096];
            let n = tcp.read(&mut buf).await.unwrap();
            tcp.write_all(response.as_bytes()).await.unwrap();
            let _ = tcp.shutdown().await;
            String::from_utf8_lossy(&buf[..n]).lines().next().unwrap_or_default().to_string()
        });
        (format!("http://{addr}/api"), handle)
    }

    #[tokio::test]
    async fn token_endpoint_is_posted_and_parsed() {
        let (base, request) = serve_once(
            "200 OK",
            r#"{"token":"tok","expires_at":1760000000000,"ws_path":"/ws/calls/call_1/media"}"#,
        )
        .await;
        let session = HttpClient::new(base).create_media_session("call_1").await.unwrap();
        assert_eq!(session.token, "tok");
        assert_eq!(request.await.unwrap(), "POST /api/calls/call_1/media-session HTTP/1.1");
    }

    #[tokio::test]
    async fn token_endpoint_errors_carry_the_server_message() {
        let (base, _request) = serve_once("409 Conflict", r#"{"error":"call not active"}"#).await;
        let error = HttpClient::new(base).create_media_session("call_1").await.unwrap_err();
        match error {
            ClientError::Api { status, message } => {
                assert_eq!(status, reqwest::StatusCode::CONFLICT);
                assert_eq!(message, "call not active");
            }
            other => panic!("unexpected error {other:?}"),
        }
    }

    #[tokio::test]
    async fn connect_failure_is_an_error_not_a_panic() {
        // Port 1 is never listening.
        assert!(MediaSocket::connect("ws://127.0.0.1:1/ws/x?token=t").await.is_err());
    }
}

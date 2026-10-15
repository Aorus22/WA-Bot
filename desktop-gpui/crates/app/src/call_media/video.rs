//! Video for calls: camera capture through an `ffmpeg` subprocess, JPEG stream
//! splitting, and decoding pictures into GPUI images.
//!
//! The camera side has no native dependency: ffmpeg opens the device
//! (`v4l2` on Linux, `dshow` on Windows, `avfoundation` on macOS) and writes an
//! MJPEG stream to stdout. [`JpegSplitter`] cuts that into single JPEGs, which
//! go to the backend as they are. The same pictures are decoded for the small
//! self-view.

use std::io::{BufRead, BufReader, Cursor, Read};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use gpui::RenderImage;
use image::imageops;
use parking_lot::Mutex;

use super::ui_link::{Source, UiLink, Which};
use super::Outgoing;

/// Largest JPEG forwarded to the backend (it reads at most 1 MiB per message).
const MAX_JPEG_BYTES: usize = 900 * 1024;
/// Bail out of a stream that never closes an image.
const MAX_PENDING_BYTES: usize = 4 * 1024 * 1024;
/// Refuse pictures bigger than this on either side.
const MAX_DIMENSION: u32 = 4096;

// --- JPEG stream splitting ---

/// Cuts a byte stream of back-to-back JPEGs (`FF D8 … FF D9`) into images.
///
/// Inside entropy-coded data a literal `FF` is always followed by `00`, so the
/// first `FF D9` after a start marker ends the picture.
#[derive(Default)]
pub struct JpegSplitter {
    buf: Vec<u8>,
    /// `buf` starts with an `FF D8` whose end has not been seen yet.
    in_image: bool,
    /// Bytes of `buf` already searched for the end marker.
    scanned: usize,
}

impl JpegSplitter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, chunk: &[u8]) -> Vec<Vec<u8>> {
        self.buf.extend_from_slice(chunk);
        let mut images = Vec::new();
        loop {
            if !self.in_image {
                match find_marker(&self.buf, 0, 0xD8) {
                    Some(start) => {
                        self.buf.drain(..start);
                        self.in_image = true;
                        self.scanned = 2;
                    }
                    None => {
                        // Keep a trailing 0xFF: its partner may arrive next.
                        let keep = usize::from(self.buf.last() == Some(&0xFF));
                        let drop_to = self.buf.len() - keep;
                        self.buf.drain(..drop_to);
                        break;
                    }
                }
            }
            match find_marker(&self.buf, self.scanned.max(2), 0xD9) {
                Some(end) => {
                    images.push(self.buf.drain(..end + 2).collect());
                    self.in_image = false;
                    self.scanned = 0;
                }
                None => {
                    // The last byte could be the 0xFF of a marker split in two.
                    self.scanned = self.buf.len().saturating_sub(1).max(2);
                    break;
                }
            }
        }
        if self.buf.len() > MAX_PENDING_BYTES {
            self.buf.clear();
            self.in_image = false;
            self.scanned = 0;
        }
        images
    }

    /// Bytes buffered for an image that is not complete yet.
    pub fn pending(&self) -> usize {
        self.buf.len()
    }
}

/// Index of the first `FF <second>` at or after `from`.
fn find_marker(buf: &[u8], from: usize, second: u8) -> Option<usize> {
    let mut i = from;
    while i + 1 < buf.len() {
        if buf[i] == 0xFF && buf[i + 1] == second {
            return Some(i);
        }
        i += 1;
    }
    None
}

// --- Camera command line ---

/// Where the camera comes from and how ffmpeg is called.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CameraConfig {
    pub ffmpeg: String,
    /// `/dev/video0`, a dshow device name, an avfoundation index …
    pub device: String,
}

impl CameraConfig {
    /// `$WABOT_FFMPEG` (default `ffmpeg`) and `$WABOT_CAMERA` (Linux default
    /// `/dev/video0`, macOS `0`; on Windows the first dshow video device).
    pub fn from_env() -> Self {
        let ffmpeg = std::env::var("WABOT_FFMPEG")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "ffmpeg".to_string());
        let device = std::env::var("WABOT_CAMERA")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_default();
        Self { ffmpeg, device }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureBackend {
    V4l2,
    Dshow,
    AvFoundation,
}

impl CaptureBackend {
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            CaptureBackend::Dshow
        } else if cfg!(target_os = "macos") {
            CaptureBackend::AvFoundation
        } else {
            CaptureBackend::V4l2
        }
    }
}

/// ffmpeg arguments for a 640x480-bounded, 15 fps MJPEG stream on stdout.
pub fn ffmpeg_args(backend: CaptureBackend, device: &str) -> Vec<String> {
    let mut args: Vec<String> = ["-hide_banner", "-loglevel", "error", "-nostdin"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    match backend {
        CaptureBackend::V4l2 => {
            args.extend(["-f", "v4l2", "-i"].map(String::from));
            args.push(device.to_string());
        }
        CaptureBackend::Dshow => {
            args.extend(["-f", "dshow", "-i"].map(String::from));
            args.push(format!("video={device}"));
        }
        CaptureBackend::AvFoundation => {
            args.extend(["-f", "avfoundation", "-framerate", "30", "-i"].map(String::from));
            args.push(device.to_string());
        }
    }
    args.extend(
        [
            "-an",
            "-vf",
            "fps=15,scale=640:480:force_original_aspect_ratio=decrease:force_divisible_by=2",
            "-pix_fmt",
            "yuvj420p",
            "-c:v",
            "mjpeg",
            "-q:v",
            "6",
            "-f",
            "mjpeg",
            "pipe:1",
        ]
        .map(String::from),
    );
    args
}

/// First video device in the output of `ffmpeg -list_devices true -f dshow -i dummy`.
pub fn parse_first_dshow_video_device(listing: &str) -> Option<String> {
    listing
        .lines()
        .filter(|line| line.contains("(video)"))
        .find_map(|line| {
            let start = line.find('"')? + 1;
            let end = start + line[start..].find('"')?;
            Some(line[start..end].to_string()).filter(|name| !name.is_empty())
        })
}

fn resolve_device(config: &CameraConfig, backend: CaptureBackend) -> Result<String, String> {
    if !config.device.is_empty() {
        return Ok(config.device.clone());
    }
    match backend {
        CaptureBackend::V4l2 => Ok("/dev/video0".to_string()),
        CaptureBackend::AvFoundation => Ok("0".to_string()),
        CaptureBackend::Dshow => {
            let output = command(&config.ffmpeg)
                .args(["-hide_banner", "-list_devices", "true", "-f", "dshow", "-i", "dummy"])
                .stdin(Stdio::null())
                .output()
                .map_err(|e| spawn_error(&config.ffmpeg, &e))?;
            // The listing goes to stderr.
            let text = String::from_utf8_lossy(&output.stderr);
            parse_first_dshow_video_device(&text)
                .ok_or_else(|| "No camera found (set WABOT_CAMERA to the device name)".to_string())
        }
    }
}

fn command(program: &str) -> Command {
    let cmd = Command::new(program);
    #[cfg(windows)]
    let cmd = {
        use std::os::windows::process::CommandExt;
        let mut cmd = cmd;
        // CREATE_NO_WINDOW
        cmd.creation_flags(0x0800_0000);
        cmd
    };
    cmd
}

fn spawn_error(ffmpeg: &str, error: &std::io::Error) -> String {
    if error.kind() == std::io::ErrorKind::NotFound {
        format!("ffmpeg was not found ({ffmpeg}). Install it, or set WABOT_FFMPEG, to send video.")
    } else {
        format!("Could not start ffmpeg: {error}")
    }
}

// --- Camera capture ---

/// A running camera. Dropping it (or [`stop`](Self::stop)) kills ffmpeg.
pub struct CameraCapture {
    stop: Arc<AtomicBool>,
    child: Arc<Mutex<Option<Child>>>,
}

impl CameraCapture {
    /// Start ffmpeg on a worker thread. Returns at once; problems (no ffmpeg,
    /// device busy …) are reported through `ui` as a camera error.
    pub fn start(
        config: CameraConfig,
        out: flume::Sender<Outgoing>,
        preview: Decoder,
        ui: UiLink,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let child = Arc::new(Mutex::new(None));
        let worker = CaptureWorker {
            config,
            out,
            preview,
            ui: ui.clone(),
            stop: Arc::clone(&stop),
            child: Arc::clone(&child),
        };
        if let Err(error) = std::thread::Builder::new()
            .name("call-camera".into())
            .spawn(move || worker.run())
        {
            ui.error(Source::Camera, format!("Could not start the camera: {error}"));
        }
        Self { stop, child }
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(child) = self.child.lock().as_mut() {
            let _ = child.kill();
        }
    }
}

impl Drop for CameraCapture {
    fn drop(&mut self) {
        self.stop();
    }
}

struct CaptureWorker {
    config: CameraConfig,
    out: flume::Sender<Outgoing>,
    preview: Decoder,
    ui: UiLink,
    stop: Arc<AtomicBool>,
    child: Arc<Mutex<Option<Child>>>,
}

impl CaptureWorker {
    fn run(self) {
        let backend = CaptureBackend::current();
        let device = match resolve_device(&self.config, backend) {
            Ok(device) => device,
            Err(message) => return self.fail(message),
        };
        if self.stop.load(Ordering::SeqCst) {
            return;
        }
        let spawned = command(&self.config.ffmpeg)
            .args(ffmpeg_args(backend, &device))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(error) => return self.fail(spawn_error(&self.config.ffmpeg, &error)),
        };
        let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
            let _ = child.kill();
            return self.fail("Could not read from ffmpeg".to_string());
        };

        // Hand the child over so stop() can kill it; if stop() already ran,
        // kill it ourselves.
        {
            let mut slot = self.child.lock();
            if self.stop.load(Ordering::SeqCst) {
                let _ = child.kill();
                let _ = child.wait();
                return;
            }
            *slot = Some(child);
        }

        let tail = Arc::new(Mutex::new(Vec::<String>::new()));
        let tail_writer = Arc::clone(&tail);
        let stderr_thread = std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(|l| l.ok()) {
                let mut tail = tail_writer.lock();
                tail.push(line);
                if tail.len() > 4 {
                    tail.remove(0);
                }
            }
        });

        let frames = self.pump(stdout);

        if let Some(mut child) = self.child.lock().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = stderr_thread.join();
        if !self.stop.load(Ordering::SeqCst) {
            let detail = tail.lock().join(" ");
            let message = if frames == 0 {
                format!("The camera could not be opened ({device}). {detail}")
            } else {
                format!("The camera stopped. {detail}")
            };
            self.ui.error(Source::Camera, message.trim().to_string());
        }
        self.ui.clear_local();
    }

    fn fail(&self, message: String) {
        eprintln!("[call] camera: {message}");
        self.ui.error(Source::Camera, message);
    }

    /// Read MJPEG from ffmpeg until it ends; returns the number of pictures.
    fn pump(&self, mut stdout: impl Read) -> u64 {
        let mut splitter = JpegSplitter::new();
        let mut chunk = vec![0u8; 64 * 1024];
        let mut frames = 0u64;
        while !self.stop.load(Ordering::SeqCst) {
            let read = match stdout.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            for jpeg in splitter.push(&chunk[..read]) {
                if frames == 0 {
                    self.ui.ok(Source::Camera);
                }
                frames += 1;
                if jpeg.len() <= MAX_JPEG_BYTES {
                    self.preview.submit(DecodeJob { jpeg: jpeg.clone(), quarter_turns: 0 });
                    let _ = self.out.try_send(Outgoing::Video(jpeg));
                }
            }
        }
        frames
    }
}

// --- Decoding ---

/// One JPEG to turn into a picture.
pub struct DecodeJob {
    pub jpeg: Vec<u8>,
    /// Clockwise quarter turns to apply after decoding.
    pub quarter_turns: u8,
}

/// Decode a JPEG into a GPUI image (BGRA), rotated by `quarter_turns` and
/// optionally mirrored (self-view).
pub fn decode_jpeg(jpeg: &[u8], quarter_turns: u8, mirror: bool) -> Result<Arc<RenderImage>, String> {
    let mut reader = image::ImageReader::with_format(Cursor::new(jpeg), image::ImageFormat::Jpeg);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let decoded = reader.decode().map_err(|e| format!("bad JPEG: {e}"))?;
    let mut rgba = decoded.into_rgba8();
    rgba = match quarter_turns % 4 {
        1 => imageops::rotate90(&rgba),
        2 => imageops::rotate180(&rgba),
        3 => imageops::rotate270(&rgba),
        _ => rgba,
    };
    if mirror {
        imageops::flip_horizontal_in_place(&mut rgba);
    }
    // GPUI wants BGRA.
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    Ok(Arc::new(RenderImage::new(vec![image::Frame::new(rgba)])))
}

/// A decoder thread with a "latest wins" inbox of two pictures: when decoding
/// is slower than the stream, the oldest waiting picture is dropped.
#[derive(Clone)]
pub struct Decoder {
    tx: flume::Sender<DecodeJob>,
    rx: flume::Receiver<DecodeJob>,
}

impl Decoder {
    pub fn spawn(name: &str, which: Which, mirror: bool, ui: UiLink) -> Self {
        let (tx, rx) = flume::bounded::<DecodeJob>(2);
        let worker_rx = rx.clone();
        let spawned = std::thread::Builder::new().name(name.to_string()).spawn(move || {
            let mut warned = false;
            // Ends when every sender (the session) is gone.
            while let Ok(job) = worker_rx.recv() {
                match decode_jpeg(&job.jpeg, job.quarter_turns, mirror) {
                    Ok(image) => ui.frame(which, image),
                    Err(error) => {
                        if !warned {
                            eprintln!("[call] {error}");
                            warned = true;
                        }
                    }
                }
            }
        });
        if let Err(error) = spawned {
            eprintln!("[call] cannot start decoder: {error}");
        }
        Self { tx, rx }
    }

    pub fn submit(&self, job: DecodeJob) {
        let mut job = job;
        loop {
            match self.tx.try_send(job) {
                Ok(()) => return,
                Err(flume::TrySendError::Full(back)) => {
                    // Make room by discarding the oldest waiting picture.
                    let _ = self.rx.try_recv();
                    job = back;
                }
                Err(flume::TrySendError::Disconnected(_)) => return,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jpeg(tag: u8, len: usize) -> Vec<u8> {
        let mut data = vec![0xFF, 0xD8];
        data.extend((0..len).map(|i| (i as u8).wrapping_add(tag) & 0x7F)); // never 0xFF
        data.extend([0xFF, 0xD9]);
        data
    }

    #[test]
    fn splitter_cuts_back_to_back_images() {
        let a = jpeg(1, 100);
        let b = jpeg(2, 50);
        let mut splitter = JpegSplitter::new();
        let mut stream = a.clone();
        stream.extend(&b);
        let out = splitter.push(&stream);
        assert_eq!(out, vec![a, b]);
        assert_eq!(splitter.pending(), 0);
    }

    #[test]
    fn splitter_handles_arbitrary_chunking() {
        let images: Vec<Vec<u8>> = (0..5).map(|i| jpeg(i, 200 + i as usize * 37)).collect();
        let stream: Vec<u8> = images.iter().flatten().copied().collect();
        for size in [1usize, 2, 3, 7, 64, 1000] {
            let mut splitter = JpegSplitter::new();
            let mut out = Vec::new();
            for chunk in stream.chunks(size) {
                out.extend(splitter.push(chunk));
            }
            assert_eq!(out, images, "chunk size {size}");
        }
    }

    #[test]
    fn splitter_skips_leading_garbage_and_stuffed_bytes() {
        let mut stream = vec![0x00, 0x12, 0xFF, 0x00, 0x34];
        // A picture whose data contains stuffed 0xFF 0x00 pairs.
        let image = vec![0xFF, 0xD8, 1, 2, 0xFF, 0x00, 3, 0xFF, 0x00, 0xFF, 0xD9];
        stream.extend(&image);
        stream.extend([0xAA, 0xBB]);
        let mut splitter = JpegSplitter::new();
        let out = splitter.push(&stream);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0], image);
    }

    #[test]
    fn splitter_waits_for_a_split_marker() {
        let mut splitter = JpegSplitter::new();
        assert!(splitter.push(&[0xFF]).is_empty());
        assert!(splitter.push(&[0xD8, 1, 2, 3, 0xFF]).is_empty());
        let out = splitter.push(&[0xD9]);
        assert_eq!(out, vec![vec![0xFF, 0xD8, 1, 2, 3, 0xFF, 0xD9]]);
    }

    #[test]
    fn splitter_discards_runaway_streams() {
        let mut splitter = JpegSplitter::new();
        splitter.push(&[0xFF, 0xD8]);
        for _ in 0..(MAX_PENDING_BYTES / 65_536 + 2) {
            splitter.push(&vec![1u8; 65_536]);
        }
        assert!(splitter.pending() < MAX_PENDING_BYTES);
        // And it recovers.
        let out = splitter.push(&jpeg(3, 10));
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn ffmpeg_command_lines() {
        let args = ffmpeg_args(CaptureBackend::V4l2, "/dev/video2");
        let joined = args.join(" ");
        assert!(joined.contains("-f v4l2 -i /dev/video2"), "{joined}");
        assert!(joined.contains("fps=15"));
        assert!(joined.contains("scale=640:480"));
        assert!(joined.ends_with("-f mjpeg pipe:1"));

        let args = ffmpeg_args(CaptureBackend::Dshow, "Integrated Camera");
        assert!(args.contains(&"video=Integrated Camera".to_string()));
        assert!(args.contains(&"dshow".to_string()));
    }

    #[test]
    fn dshow_listing_is_parsed() {
        let listing = r#"[dshow @ 000001] DirectShow video devices (some may be both video and audio devices)
[dshow @ 000001]  "Integrated Camera" (video)
[dshow @ 000001]     Alternative name "@device_pnp_\\?\usb#vid"
[dshow @ 000001]  "Microphone (Realtek)" (audio)
"#;
        assert_eq!(parse_first_dshow_video_device(listing).as_deref(), Some("Integrated Camera"));
        assert_eq!(parse_first_dshow_video_device("nothing here"), None);
    }

    fn test_jpeg(width: u32, height: u32) -> Vec<u8> {
        // A left-half red / right-half blue picture.
        let mut img = image::RgbImage::new(width, height);
        for (x, _, pixel) in img.enumerate_pixels_mut() {
            *pixel = if x < width / 2 { image::Rgb([255, 0, 0]) } else { image::Rgb([0, 0, 255]) };
        }
        let mut out = Vec::new();
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90);
        img.write_with_encoder(encoder).unwrap();
        out
    }

    fn pixel(image: &RenderImage, x: usize, y: usize) -> [u8; 4] {
        let width = image.size(0).width.0 as usize;
        let bytes = image.as_bytes(0).unwrap();
        let at = (y * width + x) * 4;
        [bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]
    }

    #[test]
    fn jpeg_decodes_to_bgra() {
        let data = test_jpeg(32, 16);
        assert!(data.starts_with(&[0xFF, 0xD8]) && data.ends_with(&[0xFF, 0xD9]));
        let image = decode_jpeg(&data, 0, false).unwrap();
        assert_eq!((image.size(0).width.0, image.size(0).height.0), (32, 16));
        let left = pixel(&image, 2, 8);
        let right = pixel(&image, 29, 8);
        // BGRA: red is [~0, ~0, 255, 255].
        assert!(left[2] > 200 && left[0] < 60, "{left:?}");
        assert!(right[0] > 200 && right[2] < 60, "{right:?}");
    }

    #[test]
    fn orientation_rotates_clockwise() {
        let data = test_jpeg(32, 16);
        // One quarter turn clockwise: 32x16 becomes 16x32, left half goes to the top.
        let image = decode_jpeg(&data, 1, false).unwrap();
        assert_eq!((image.size(0).width.0, image.size(0).height.0), (16, 32));
        let top = pixel(&image, 8, 2);
        let bottom = pixel(&image, 8, 29);
        assert!(top[2] > 200, "red on top: {top:?}");
        assert!(bottom[0] > 200, "blue at the bottom: {bottom:?}");

        let flipped = decode_jpeg(&data, 2, false).unwrap();
        assert_eq!((flipped.size(0).width.0, flipped.size(0).height.0), (32, 16));
        assert!(pixel(&flipped, 2, 8)[0] > 200, "red is now on the right side");

        let three = decode_jpeg(&data, 3, false).unwrap();
        assert!(pixel(&three, 8, 2)[0] > 200, "blue on top after three turns");
        // 4 turns is the identity.
        let four = decode_jpeg(&data, 4, false).unwrap();
        assert_eq!((four.size(0).width.0, four.size(0).height.0), (32, 16));
    }

    #[test]
    fn mirror_flips_horizontally() {
        let data = test_jpeg(32, 16);
        let image = decode_jpeg(&data, 0, true).unwrap();
        assert!(pixel(&image, 2, 8)[0] > 200, "blue is now on the left");
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        assert!(decode_jpeg(&[0xFF, 0xD8, 0xFF, 0xD9], 0, false).is_err());
        assert!(decode_jpeg(b"definitely not a jpeg", 0, false).is_err());
    }

    /// Run the real ffmpeg (skipped when it is not installed) with the
    /// production output chain, feeding it a synthetic 720p source instead of a
    /// camera, through a wrapper script standing in for the binary.
    #[cfg(target_os = "linux")]
    #[test]
    fn camera_pipeline_with_real_ffmpeg() {
        use std::os::unix::fs::PermissionsExt;
        if Command::new("ffmpeg").arg("-version").output().is_err() {
            eprintln!("ffmpeg not installed, skipping");
            return;
        }
        let marker = format!("wabot-camera-test-{}", std::process::id());
        let args = ffmpeg_args(CaptureBackend::V4l2, "/dev/video-test");
        let input_at = args.iter().position(|a| a == "-i").unwrap();
        let output_opts = args[input_at + 2..].join(" ");
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fake-ffmpeg.sh");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\nexec ffmpeg -hide_banner -loglevel error -re -f lavfi -i testsrc=size=1280x720:rate=30 -metadata title={marker} {output_opts}\n"
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

        let (ui_tx, ui_rx) = flume::unbounded();
        let ui = UiLink::new(9, ui_tx);
        let (out_tx, out_rx) = flume::bounded::<Outgoing>(64);
        let preview = Decoder::spawn("test-preview", Which::Local, true, ui.clone());
        let camera = CameraCapture::start(
            CameraConfig { ffmpeg: script.to_string_lossy().into_owned(), device: String::new() },
            out_tx,
            preview,
            ui.clone(),
        );

        let mut frames = Vec::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while frames.len() < 8 && std::time::Instant::now() < deadline {
            if let Ok(Outgoing::Video(jpeg)) = out_rx.recv_timeout(std::time::Duration::from_millis(200)) {
                frames.push(jpeg);
            }
        }
        assert!(frames.len() >= 8, "only {} frames", frames.len());
        for jpeg in &frames {
            assert!(jpeg.len() < 100 * 1024, "{} bytes", jpeg.len());
            let image = decode_jpeg(jpeg, 0, false).unwrap();
            assert_eq!((image.size(0).width.0, image.size(0).height.0), (640, 360));
        }
        // The self-view got a picture too.
        let preview_event = ui_rx.try_iter().any(|e| matches!(e, crate::call_media::UiEvent::Frame { which: Which::Local, .. }));
        assert!(preview_event);
        assert!(ui.slots().take(Which::Local).is_some());

        let running = |marker: &str| {
            std::fs::read_dir("/proc").unwrap().flatten().any(|entry| {
                std::fs::read(entry.path().join("cmdline"))
                    .map(|c| String::from_utf8_lossy(&c).contains(marker) && String::from_utf8_lossy(&c).contains("ffmpeg"))
                    .unwrap_or(false)
            })
        };
        assert!(running(&marker), "ffmpeg should be running");
        camera.stop();
        let gone = (0..50).any(|_| {
            std::thread::sleep(std::time::Duration::from_millis(100));
            !running(&marker)
        });
        assert!(gone, "ffmpeg survived stop()");
        drop(camera);
    }

    #[test]
    fn decoder_keeps_only_the_newest_pictures() {
        let (tx, _rx) = flume::unbounded();
        let ui = UiLink::new(1, tx);
        let (jobs_tx, jobs_rx) = flume::bounded::<DecodeJob>(2);
        let decoder = Decoder { tx: jobs_tx, rx: jobs_rx.clone() };
        for tag in 0..5u8 {
            decoder.submit(DecodeJob { jpeg: vec![tag], quarter_turns: 0 });
        }
        let waiting: Vec<u8> = jobs_rx.try_iter().map(|j| j.jpeg[0]).collect();
        assert_eq!(waiting, vec![3, 4]);
        drop(ui);
    }
}

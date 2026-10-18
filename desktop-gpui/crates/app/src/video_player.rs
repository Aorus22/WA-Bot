//! In-app video playback through the system's `ffmpeg`.
//!
//! gpui has no video element, so two `ffmpeg` processes do the decoding: one
//! writes raw BGRA frames at a fixed rate (the layout gpui's `RenderImage`
//! uses), the other writes interleaved `f32` samples that a `cpal` output
//! stream plays. A paused-aware wall clock that starts with the first frame
//! paces both. Seeking starts a new player at the new position.
//!
//! When `ffmpeg` is not installed the caller opens the video in the system's
//! player instead (see [`ffmpeg_available`]).

use std::collections::VecDeque;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Condvar, LazyLock, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};

/// Frames are decoded no larger than this (the viewer scales them up or down).
const MAX_FRAME_W: u32 = 1280;
const MAX_FRAME_H: u32 = 720;
/// Decoded audio kept ahead of the speakers, in seconds.
const AUDIO_BUFFER_SECONDS: f32 = 1.5;

/// Whether `ffmpeg` and `ffprobe` can be run (checked once).
pub fn ffmpeg_available() -> bool {
    static AVAILABLE: LazyLock<bool> = LazyLock::new(|| {
        ["ffmpeg", "ffprobe"].iter().all(|tool| {
            Command::new(tool)
                .arg("-version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|status| status.success())
        })
    });
    *AVAILABLE
}

/// What `ffprobe` says about a video, and the size frames are decoded at.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub duration: f32,
    pub fps: f32,
    pub has_audio: bool,
}

impl VideoInfo {
    /// Frame size for decoding: the video's own, shrunk to fit
    /// [`MAX_FRAME_W`]×[`MAX_FRAME_H`], with even sides.
    pub fn frame_size(&self) -> (u32, u32) {
        frame_size(self.width, self.height)
    }
}

fn frame_size(width: u32, height: u32) -> (u32, u32) {
    let (w, h) = (width.max(2) as f32, height.max(2) as f32);
    let scale = (MAX_FRAME_W as f32 / w).min(MAX_FRAME_H as f32 / h).min(1.0);
    let even = |v: f32| ((v * scale) as u32 / 2 * 2).max(2);
    (even(w), even(h))
}

/// Read a video's size, length, frame rate and whether it has sound. Blocking.
pub fn probe(source: &str) -> Result<VideoInfo, String> {
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-print_format", "json", "-show_streams", "-show_format", source])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("Couldn't run ffprobe: {e}"))?;
    if !out.status.success() {
        let reason = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if reason.is_empty() { "This video can't be read".into() } else { reason });
    }
    parse_probe(&String::from_utf8_lossy(&out.stdout))
}

/// Parse `ffprobe -print_format json -show_streams -show_format` output.
pub fn parse_probe(json: &str) -> Result<VideoInfo, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("Unexpected ffprobe output: {e}"))?;
    let streams = v["streams"].as_array().cloned().unwrap_or_default();
    let video = streams
        .iter()
        .find(|s| s["codec_type"] == "video")
        .ok_or("This file has no video")?;
    let has_audio = streams.iter().any(|s| s["codec_type"] == "audio");
    let (mut width, mut height) = (
        video["width"].as_u64().unwrap_or(0) as u32,
        video["height"].as_u64().unwrap_or(0) as u32,
    );
    // Phone videos are often stored sideways with a rotation tag.
    let rotation = video["side_data_list"]
        .as_array()
        .and_then(|list| list.iter().find_map(|d| d["rotation"].as_f64()))
        .or_else(|| video["tags"]["rotate"].as_str().and_then(|r| r.parse().ok()))
        .unwrap_or(0.0);
    if (rotation.abs() as i64) % 180 == 90 {
        std::mem::swap(&mut width, &mut height);
    }
    if width == 0 || height == 0 {
        return Err("This video has no picture size".into());
    }
    let rate = |key: &str| {
        video[key].as_str().and_then(|r| {
            let (n, d) = r.split_once('/')?;
            let (n, d): (f32, f32) = (n.parse().ok()?, d.parse().ok()?);
            (d > 0.0 && n > 0.0).then_some(n / d)
        })
    };
    let fps = rate("avg_frame_rate").or_else(|| rate("r_frame_rate")).unwrap_or(30.0).clamp(1.0, 60.0);
    let duration = v["format"]["duration"]
        .as_str()
        .or_else(|| video["duration"].as_str())
        .and_then(|d| d.parse::<f32>().ok())
        .unwrap_or(0.0);
    Ok(VideoInfo { width, height, duration, fps, has_audio })
}

/// Playback time: runs while playing, holds while paused.
#[derive(Default)]
struct Clock {
    elapsed: f64,
    running_since: Option<Instant>,
}

impl Clock {
    fn now(&self) -> f64 {
        self.elapsed + self.running_since.map_or(0.0, |at| at.elapsed().as_secs_f64())
    }
    fn pause(&mut self) {
        if let Some(at) = self.running_since.take() {
            self.elapsed += at.elapsed().as_secs_f64();
        }
    }
    fn resume(&mut self) {
        if self.running_since.is_none() {
            self.running_since = Some(Instant::now());
        }
    }
}

#[derive(Default)]
struct Shared {
    /// Newest frame not yet taken by the view, BGRA, and a counter that
    /// changes with every frame.
    frame: Mutex<Option<Vec<u8>>>,
    seq: AtomicU64,
    clock: Mutex<Clock>,
    /// The clock starts with the first decoded frame.
    started: AtomicBool,
    paused: AtomicBool,
    stop: AtomicBool,
    ended: AtomicBool,
    /// Volume as `f32` bits; 0 mutes.
    volume: AtomicU32,
    audio: Mutex<VecDeque<f32>>,
    audio_room: Condvar,
    error: Mutex<Option<String>>,
}

impl Shared {
    fn position(&self) -> f64 {
        self.clock.lock().unwrap_or_else(|e| e.into_inner()).now()
    }
    fn fail(&self, message: String) {
        self.error.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert(message);
    }
}

/// A video being played. Dropping it stops the decoders and the sound.
pub struct VideoPlayer {
    shared: Arc<Shared>,
    info: VideoInfo,
    /// Where in the video this player started, in seconds.
    start: f32,
    children: Arc<Mutex<Vec<Child>>>,
    threads: Vec<JoinHandle<()>>,
}

impl VideoPlayer {
    /// Start playing `source` (a URL or path) from `start` seconds. Spawns the
    /// decoders and returns at once; frames follow shortly.
    pub fn start(source: &str, info: VideoInfo, start: f32, volume: f32, paused: bool) -> Result<Self, String> {
        let start = start.clamp(0.0, (info.duration - 0.05).max(0.0));
        let shared = Arc::new(Shared::default());
        shared.volume.store(volume.to_bits(), Ordering::Relaxed);
        shared.paused.store(paused, Ordering::Relaxed);
        let children = Arc::new(Mutex::new(Vec::new()));
        let mut threads = Vec::new();

        let (fw, fh) = info.frame_size();
        let mut video = Command::new("ffmpeg")
            .args(["-nostdin", "-v", "error", "-ss", &format!("{start:.3}"), "-i", source, "-an"])
            .args(["-vf", &format!("fps={},scale={fw}:{fh}", info.fps)])
            .args(["-f", "rawvideo", "-pix_fmt", "bgra", "pipe:1"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Couldn't run ffmpeg: {e}"))?;
        let video_out = video.stdout.take().ok_or("ffmpeg gave no output")?;
        children.lock().unwrap_or_else(|e| e.into_inner()).push(video);
        {
            let shared = Arc::clone(&shared);
            let fps = info.fps;
            threads.push(
                std::thread::Builder::new()
                    .name("video-decode".into())
                    .spawn(move || decode_video(video_out, (fw, fh), fps, shared))
                    .map_err(|e| e.to_string())?,
            );
        }

        if info.has_audio {
            let (ready_tx, ready_rx) = mpsc::channel::<Result<(u32, u16), String>>();
            let output_shared = Arc::clone(&shared);
            threads.push(
                std::thread::Builder::new()
                    .name("video-sound".into())
                    .spawn(move || play_sound(output_shared, ready_tx))
                    .map_err(|e| e.to_string())?,
            );
            // Without a speaker the video still plays, silently.
            if let Ok(Ok((rate, channels))) = ready_rx.recv_timeout(Duration::from_secs(3)) {
                match Command::new("ffmpeg")
                    .args(["-nostdin", "-v", "error", "-ss", &format!("{start:.3}"), "-i", source, "-vn"])
                    .args(["-ac", &channels.to_string(), "-ar", &rate.to_string(), "-f", "f32le", "pipe:1"])
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
                {
                    Ok(mut audio) => {
                        if let Some(out) = audio.stdout.take() {
                            let shared = Arc::clone(&shared);
                            let cap = (rate as f32 * channels as f32 * AUDIO_BUFFER_SECONDS) as usize;
                            threads.push(
                                std::thread::Builder::new()
                                    .name("video-audio-decode".into())
                                    .spawn(move || decode_audio(out, cap, shared))
                                    .map_err(|e| e.to_string())?,
                            );
                        }
                        children.lock().unwrap_or_else(|e| e.into_inner()).push(audio);
                    }
                    Err(e) => eprintln!("[video] no sound: {e}"),
                }
            }
        }

        Ok(Self { shared, info, start, children, threads })
    }

    pub fn info(&self) -> &VideoInfo {
        &self.info
    }

    /// Current position in the video, in seconds.
    pub fn position(&self) -> f32 {
        (self.start as f64 + self.shared.position()).min(self.info.duration.max(0.0) as f64) as f32
    }

    /// Take the newest frame (BGRA, [`VideoInfo::frame_size`]) if one arrived
    /// since the last call. Frames nobody took in time are replaced.
    pub fn take_frame(&self) -> Option<Vec<u8>> {
        self.shared.frame.lock().unwrap_or_else(|e| e.into_inner()).take()
    }

    /// Counts the frames shown so far.
    pub fn frames_shown(&self) -> u64 {
        self.shared.seq.load(Ordering::Acquire)
    }

    pub fn is_paused(&self) -> bool {
        self.shared.paused.load(Ordering::Relaxed)
    }

    /// Whether playback reached the end of the video.
    pub fn has_ended(&self) -> bool {
        self.shared.ended.load(Ordering::Relaxed)
    }

    pub fn error(&self) -> Option<String> {
        self.shared.error.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn set_paused(&self, paused: bool) {
        self.shared.paused.store(paused, Ordering::Relaxed);
        if self.shared.started.load(Ordering::Relaxed) {
            let mut clock = self.shared.clock.lock().unwrap_or_else(|e| e.into_inner());
            if paused { clock.pause() } else { clock.resume() }
        }
    }

    pub fn set_volume(&self, volume: f32) {
        self.shared.volume.store(volume.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }
}

impl Drop for VideoPlayer {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        self.shared.audio_room.notify_all();
        for child in self.children.lock().unwrap_or_else(|e| e.into_inner()).iter_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
        // The threads end once their pipe closes or they see `stop`.
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

fn decode_video(mut out: impl Read, (w, h): (u32, u32), fps: f32, shared: Arc<Shared>) {
    let size = (w * h * 4) as usize;
    let frame_secs = 1.0 / fps as f64;
    let mut index: u64 = 0;
    loop {
        let mut buf = vec![0u8; size];
        if out.read_exact(&mut buf).is_err() {
            break;
        }
        if shared.stop.load(Ordering::Relaxed) {
            return;
        }
        if index == 0 {
            shared.started.store(true, Ordering::Relaxed);
            let mut clock = shared.clock.lock().unwrap_or_else(|e| e.into_inner());
            if !shared.paused.load(Ordering::Relaxed) {
                clock.resume();
            }
        }
        let pts = index as f64 * frame_secs;
        index += 1;
        // A frame that is already a frame late is dropped (the first one never).
        if index > 1 && shared.position() > pts + frame_secs * 1.5 {
            continue;
        }
        while shared.position() < pts {
            if shared.stop.load(Ordering::Relaxed) {
                return;
            }
            let wait = (pts - shared.position()).clamp(0.001, 0.010);
            std::thread::sleep(Duration::from_secs_f64(wait));
        }
        *shared.frame.lock().unwrap_or_else(|e| e.into_inner()) = Some(buf);
        shared.seq.fetch_add(1, Ordering::Release);
    }
    if index == 0 && !shared.stop.load(Ordering::Relaxed) {
        shared.fail("Couldn't decode this video".into());
    }
    // Let the last frame stay on screen for its time before ending.
    std::thread::sleep(Duration::from_secs_f64(frame_secs));
    shared.ended.store(true, Ordering::Relaxed);
    shared.clock.lock().unwrap_or_else(|e| e.into_inner()).pause();
}

fn decode_audio(mut out: impl Read, cap: usize, shared: Arc<Shared>) {
    let mut bytes = vec![0u8; 16 * 1024];
    loop {
        let n = match out.read(&mut bytes) {
            Ok(0) | Err(_) => return,
            Ok(n) => n - n % 4,
        };
        let samples = bytes[..n].as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b));
        let mut queue = shared.audio.lock().unwrap_or_else(|e| e.into_inner());
        while queue.len() >= cap && !shared.stop.load(Ordering::Relaxed) {
            queue = shared.audio_room.wait_timeout(queue, Duration::from_millis(50)).map(|(q, _)| q).unwrap_or_else(|e| e.into_inner().0);
        }
        if shared.stop.load(Ordering::Relaxed) {
            return;
        }
        queue.extend(samples);
    }
}

/// Open the default speaker and keep its stream alive until the player stops
/// (a `cpal` stream is not `Send`). Reports the device's rate and channels.
fn play_sound(shared: Arc<Shared>, ready: mpsc::Sender<Result<(u32, u16), String>>) {
    let opened = (|| -> Result<(cpal::Stream, u32, u16), String> {
        let device = cpal::default_host().default_output_device().ok_or("No speaker")?;
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let stream = match format {
            SampleFormat::F32 => build_output::<f32>(&device, &config, &shared),
            SampleFormat::I16 => build_output::<i16>(&device, &config, &shared),
            SampleFormat::U16 => build_output::<u16>(&device, &config, &shared),
            SampleFormat::I32 => build_output::<i32>(&device, &config, &shared),
            other => Err(format!("Unsupported speaker format {other:?}")),
        }?;
        stream.play().map_err(|e| e.to_string())?;
        Ok((stream, config.sample_rate.0, config.channels))
    })();
    match opened {
        Ok((stream, rate, channels)) => {
            let _ = ready.send(Ok((rate, channels)));
            while !shared.stop.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(50));
            }
            drop(stream);
        }
        Err(e) => {
            let _ = ready.send(Err(e));
        }
    }
}

fn build_output<T>(device: &cpal::Device, config: &cpal::StreamConfig, shared: &Arc<Shared>) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let shared = Arc::clone(shared);
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                let playing = shared.started.load(Ordering::Relaxed) && !shared.paused.load(Ordering::Relaxed);
                if !playing {
                    data.fill(T::EQUILIBRIUM);
                    return;
                }
                let volume = f32::from_bits(shared.volume.load(Ordering::Relaxed));
                let mut queue = shared.audio.lock().unwrap_or_else(|e| e.into_inner());
                for sample in data.iter_mut() {
                    *sample = T::from_sample(queue.pop_front().unwrap_or(0.0) * volume);
                }
                drop(queue);
                shared.audio_room.notify_one();
            },
            |error| eprintln!("[video] sound error: {error}"),
            None,
        )
        .map_err(|e| e.to_string())
}

/// "1:05" or "1:02:05".
pub fn format_time(seconds: f32) -> String {
    let total = seconds.max(0.0) as u64;
    let (h, m, s) = (total / 3600, total / 60 % 60, total % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROBE: &str = r#"{
      "streams": [
        {"codec_type": "video", "width": 1920, "height": 1080, "avg_frame_rate": "30000/1001"},
        {"codec_type": "audio"}
      ],
      "format": {"duration": "12.48"}
    }"#;

    #[test]
    fn probe_reads_size_rate_length_and_sound() {
        let info = parse_probe(PROBE).unwrap();
        assert_eq!((info.width, info.height), (1920, 1080));
        assert!((info.fps - 29.97).abs() < 0.01);
        assert!((info.duration - 12.48).abs() < 0.001);
        assert!(info.has_audio);
        assert_eq!(info.frame_size(), (1280, 720));
    }

    #[test]
    fn rotated_phone_video_is_portrait() {
        let json = r#"{"streams":[{"codec_type":"video","width":1280,"height":720,"avg_frame_rate":"30/1",
            "side_data_list":[{"rotation":-90}]}],"format":{"duration":"3"}}"#;
        let info = parse_probe(json).unwrap();
        assert_eq!((info.width, info.height), (720, 1280));
        assert!(!info.has_audio);
        let (w, h) = info.frame_size();
        assert!(w % 2 == 0 && h == 720 && w == 404);
    }

    #[test]
    fn audio_only_file_is_rejected() {
        assert!(parse_probe(r#"{"streams":[{"codec_type":"audio"}],"format":{}}"#).is_err());
    }

    #[test]
    fn small_videos_keep_their_size() {
        assert_eq!(frame_size(640, 360), (640, 360));
        assert_eq!(frame_size(321, 241), (320, 240));
    }

    /// Plays a generated clip through real ffmpeg: frames arrive, time runs,
    /// pausing holds it and the end is reached.
    #[test]
    fn plays_pauses_and_ends_a_real_clip() {
        if !ffmpeg_available() {
            eprintln!("ffmpeg not installed; skipped");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let clip = dir.path().join("clip.mp4");
        let made = Command::new("ffmpeg")
            .args(["-v", "error", "-f", "lavfi", "-i", "testsrc=size=320x240:rate=25:duration=1.2"])
            .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=1.2", "-shortest", "-pix_fmt", "yuv420p"])
            .arg(&clip)
            .status()
            .unwrap();
        assert!(made.success());
        let source = clip.to_str().unwrap();
        let info = probe(source).unwrap();
        assert_eq!((info.width, info.height), (320, 240));

        let player = VideoPlayer::start(source, info, 0.0, 0.0, false).unwrap();
        std::thread::sleep(Duration::from_millis(500));
        let seq = player.frames_shown();
        assert!(seq > 3, "frames should be flowing, got {seq}");
        assert_eq!(player.take_frame().unwrap().len(), 320 * 240 * 4);

        player.set_paused(true);
        let held = player.position();
        std::thread::sleep(Duration::from_millis(250));
        assert!((player.position() - held).abs() < 0.02, "paused clock moved");

        player.set_paused(false);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !player.has_ended() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(player.has_ended());
        assert!(player.error().is_none());
        let stopping = Instant::now();
        drop(player);
        assert!(stopping.elapsed() < Duration::from_secs(1), "stopping took {:?}", stopping.elapsed());
    }

    #[test]
    fn time_labels() {
        assert_eq!(format_time(65.4), "1:05");
        assert_eq!(format_time(3725.0), "1:02:05");
    }
}

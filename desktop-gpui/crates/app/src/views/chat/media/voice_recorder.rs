//! Microphone capture for voice notes.
//!
//! The default input device is read on a thread of its own (a `cpal` stream is
//! not `Send`), downmixed to mono and thinned to at most [`TARGET_RATE`] so a
//! long note stays small in memory, then wrapped in a 16-bit PCM WAV. The
//! backend transcodes that to ogg/opus and works out the waveform.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};

/// Voice needs no more than this; a faster microphone is averaged down to it.
pub const TARGET_RATE: u32 = 16_000;
/// Capture stops filling its buffer past this length.
pub const MAX_SECONDS: u32 = 30 * 60;
/// Anything shorter is treated as an accidental tap, not a note.
pub const MIN_SECONDS: f32 = 0.6;

/// What a finished recording holds.
pub struct Recording {
    /// A complete WAV file (16-bit mono PCM).
    pub wav: Vec<u8>,
    pub seconds: f32,
    /// Loudest sample seen, 0 for a microphone that delivered only silence.
    pub peak: u16,
}

#[derive(Default)]
struct Shared {
    samples: Mutex<Vec<i16>>,
    /// Samples captured so far (for the timer, without taking the lock).
    frames: AtomicUsize,
    peak: AtomicU32,
    stop: AtomicBool,
}

/// A recording in progress. Dropping it stops the capture and discards it.
pub struct VoiceRecorder {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
    rate: u32,
}

impl VoiceRecorder {
    /// Open the default microphone and start capturing. Blocks until the
    /// device answers, so call it off the UI thread. The error is meant for
    /// the user.
    pub fn start() -> Result<Self, String> {
        let shared = Arc::new(Shared::default());
        let (ready_tx, ready_rx) = mpsc::channel::<Result<u32, String>>();
        let thread_shared = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("voice-capture".into())
            .spawn(move || capture(thread_shared, ready_tx))
            .map_err(|error| format!("Tidak bisa memulai perekaman: {error}"))?;
        match ready_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(rate)) => Ok(Self { shared, thread: Some(thread), rate }),
            Ok(Err(message)) => {
                let _ = thread.join();
                Err(message)
            }
            Err(_) => {
                shared.stop.store(true, Ordering::SeqCst);
                Err("Mikrofon tidak merespons".to_string())
            }
        }
    }

    /// Length captured so far, in seconds.
    pub fn seconds(&self) -> f32 {
        self.shared.frames.load(Ordering::Relaxed) as f32 / self.rate as f32
    }

    /// Stop capturing and return what was recorded.
    pub fn finish(mut self) -> Recording {
        self.shared.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let samples = std::mem::take(&mut *self.shared.samples.lock().unwrap_or_else(|e| e.into_inner()));
        Recording {
            seconds: samples.len() as f32 / self.rate as f32,
            peak: self.shared.peak.load(Ordering::Relaxed).min(u16::MAX as u32) as u16,
            wav: encode_wav(&samples, self.rate),
        }
    }
}

impl Drop for VoiceRecorder {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
    }
}

/// The capture thread: owns the stream until told to stop.
fn capture(shared: Arc<Shared>, ready: mpsc::Sender<Result<u32, String>>) {
    match open_stream(&shared) {
        Err(message) => {
            let _ = ready.send(Err(message));
        }
        Ok((stream, rate)) => {
            let _ = ready.send(Ok(rate));
            while !shared.stop.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(40));
            }
            drop(stream);
        }
    }
}

/// What the user is told when the microphone cannot be opened; the audio
/// backend's own wording ("ALSA function ... failed") goes to the log instead.
fn unavailable(error: impl std::fmt::Display) -> String {
    eprintln!("[voice] microphone unavailable: {error}");
    "Mikrofon tidak bisa dibuka. Periksa apakah mikrofon tersambung dan diizinkan.".to_string()
}

fn open_stream(shared: &Arc<Shared>) -> Result<(cpal::Stream, u32), String> {
    let device = cpal::default_host()
        .default_input_device()
        .ok_or_else(|| "Mikrofon tidak ditemukan".to_string())?;
    let supported = device.default_input_config().map_err(unavailable)?;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let channels = config.channels.max(1) as usize;
    let in_rate = config.sample_rate.0.max(1);
    let out_rate = in_rate.min(TARGET_RATE);

    let stream = match format {
        SampleFormat::F32 => build::<f32>(&device, &config, channels, in_rate, out_rate, shared),
        SampleFormat::I16 => build::<i16>(&device, &config, channels, in_rate, out_rate, shared),
        SampleFormat::U16 => build::<u16>(&device, &config, channels, in_rate, out_rate, shared),
        SampleFormat::I32 => build::<i32>(&device, &config, channels, in_rate, out_rate, shared),
        SampleFormat::U8 => build::<u8>(&device, &config, channels, in_rate, out_rate, shared),
        other => Err(format!("Format audio mikrofon tidak didukung ({other:?})")),
    }?;
    stream.play().map_err(unavailable)?;
    Ok((stream, out_rate))
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    in_rate: u32,
    out_rate: u32,
    shared: &Arc<Shared>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let shared = Arc::clone(shared);
    let mut thinner = Thinner::new(in_rate, out_rate);
    let cap = (MAX_SECONDS * out_rate) as usize;
    let mut chunk: Vec<i16> = Vec::new();
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                chunk.clear();
                for frame in data.chunks(channels) {
                    let mono = frame.iter().map(|s| f32::from_sample(*s)).sum::<f32>() / frame.len() as f32;
                    thinner.push(mono, &mut chunk);
                }
                if chunk.is_empty() {
                    return;
                }
                let peak = chunk.iter().map(|s| s.unsigned_abs() as u32).max().unwrap_or(0);
                shared.peak.fetch_max(peak, Ordering::Relaxed);
                let mut samples = shared.samples.lock().unwrap_or_else(|e| e.into_inner());
                let room = cap.saturating_sub(samples.len());
                samples.extend_from_slice(&chunk[..chunk.len().min(room)]);
                shared.frames.store(samples.len(), Ordering::Relaxed);
            },
            |error| eprintln!("[voice] capture error: {error}"),
            None,
        )
        .map_err(unavailable)
}

/// Streaming rate reducer: averages the input samples that fall into each
/// output sample (a box filter, enough for speech), so there is no aliasing
/// from plain dropping. Passes samples through when the rates match.
struct Thinner {
    /// Output samples per input sample (<= 1).
    step: f64,
    phase: f64,
    sum: f32,
    count: u32,
}

impl Thinner {
    fn new(in_rate: u32, out_rate: u32) -> Self {
        Self { step: out_rate as f64 / in_rate as f64, phase: 0.0, sum: 0.0, count: 0 }
    }

    fn push(&mut self, sample: f32, out: &mut Vec<i16>) {
        self.sum += sample;
        self.count += 1;
        self.phase += self.step;
        if self.phase >= 1.0 - 1e-9 {
            self.phase -= 1.0;
            let mean = (self.sum / self.count as f32).clamp(-1.0, 1.0);
            out.push((mean * i16::MAX as f32) as i16);
            self.sum = 0.0;
            self.count = 0;
        }
    }
}

/// 16-bit mono PCM in a canonical 44-byte-header WAV.
pub fn encode_wav(samples: &[i16], rate: u32) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut wav = Vec::with_capacity(44 + data_len as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * 2).to_le_bytes()); // byte rate
    wav.extend_from_slice(&2u16.to_le_bytes()); // block align
    wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        wav.extend_from_slice(&sample.to_le_bytes());
    }
    wav
}

/// `m:ss` for the recording timer.
pub fn format_timer(seconds: f32) -> String {
    let whole = seconds.max(0.0) as u32;
    format!("{}:{:02}", whole / 60, whole % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs a microphone: `cargo test -p wabot voice_recorder -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn records_from_the_default_microphone() {
        let recorder = VoiceRecorder::start().expect("microphone");
        std::thread::sleep(Duration::from_millis(1200));
        let recording = recorder.finish();
        eprintln!("{} s, peak {}, {} bytes", recording.seconds, recording.peak, recording.wav.len());
        assert!(recording.seconds > 0.8);
    }

    #[test]
    fn wav_header_describes_the_samples() {
        let wav = encode_wav(&[0, 1000, -1000, i16::MAX], 16_000);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..16], b"WAVEfmt ");
        assert_eq!(u32::from_le_bytes(wav[4..8].try_into().unwrap()), 36 + 8);
        assert_eq!(u16::from_le_bytes(wav[20..22].try_into().unwrap()), 1, "PCM");
        assert_eq!(u16::from_le_bytes(wav[22..24].try_into().unwrap()), 1, "mono");
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 16_000);
        assert_eq!(u32::from_le_bytes(wav[28..32].try_into().unwrap()), 32_000);
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 8);
        assert_eq!(wav.len(), 44 + 8);
        assert_eq!(i16::from_le_bytes(wav[46..48].try_into().unwrap()), 1000);
    }

    #[test]
    fn thinner_averages_down_to_the_target_rate() {
        let mut thinner = Thinner::new(48_000, 16_000);
        let mut out = Vec::new();
        for _ in 0..4800 {
            thinner.push(0.5, &mut out);
        }
        assert_eq!(out.len(), 1600);
        assert!(out.iter().all(|s| (*s as i32 - 16_383).abs() <= 1));

        // A slower microphone passes straight through.
        let mut thinner = Thinner::new(8_000, 8_000);
        let mut out = Vec::new();
        for _ in 0..100 {
            thinner.push(0.0, &mut out);
        }
        assert_eq!(out.len(), 100);
    }

    #[test]
    fn thinner_handles_fractional_ratios() {
        let mut thinner = Thinner::new(44_100, 16_000);
        let mut out = Vec::new();
        for _ in 0..44_100 {
            thinner.push(0.1, &mut out);
        }
        assert!((out.len() as i64 - 16_000).abs() <= 1, "{}", out.len());
    }

    #[test]
    fn timer_reads_minutes_and_seconds() {
        assert_eq!(format_timer(0.0), "0:00");
        assert_eq!(format_timer(7.9), "0:07");
        assert_eq!(format_timer(125.0), "2:05");
    }
}

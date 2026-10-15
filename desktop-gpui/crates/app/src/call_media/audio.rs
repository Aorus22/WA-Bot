//! Microphone capture and speaker playback for a call (cpal).
//!
//! A cpal stream is not `Send`, so every stream lives on a thread of its own,
//! exactly like `voice_recorder`. Opening a device can take a while (or fail),
//! so both threads report through the [`UiLink`] instead of blocking the caller.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};

use super::dsp::{
    downmix, f32_to_i16, FrameChunker, PlaybackPuller, Resampler, WIRE_RATE,
};
use super::ui_link::{Source, UiLink};
use super::{Outgoing, Shared};

/// Keep a stream alive on its own thread until the session stops.
fn host_stream_thread<F>(name: &str, shared: Arc<Shared>, ui: UiLink, source: Source, open: F)
where
    F: FnOnce() -> Result<cpal::Stream, String> + Send + 'static,
{
    let spawned = std::thread::Builder::new().name(name.to_string()).spawn(move || {
        match open() {
            Err(message) => ui.error(source, message),
            Ok(stream) => {
                ui.ok(source);
                while !shared.stop.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(40));
                }
                drop(stream);
            }
        }
    });
    if let Err(error) = spawned {
        eprintln!("[call] cannot start {name}: {error}");
    }
}

/// What the user is told when a device cannot be opened; the audio backend's
/// own wording goes to the log.
fn unavailable(what: &str, error: impl std::fmt::Display) -> String {
    eprintln!("[call] {what} unavailable: {error}");
    format!("{what} could not be opened. Check that it is connected and allowed.")
}

// --- Microphone ---

/// Start capturing the default microphone: mono, 16 kHz, 960-sample frames
/// pushed into `out` whenever the mic is not muted.
pub fn spawn_microphone(shared: Arc<Shared>, out: flume::Sender<Outgoing>, ui: UiLink) {
    let thread_shared = Arc::clone(&shared);
    let thread_ui = ui.clone();
    host_stream_thread("call-mic", shared, ui, Source::Mic, move || {
        open_microphone(&thread_shared, out, &thread_ui)
    });
}

fn open_microphone(
    shared: &Arc<Shared>,
    out: flume::Sender<Outgoing>,
    ui: &UiLink,
) -> Result<cpal::Stream, String> {
    let device = cpal::default_host()
        .default_input_device()
        .ok_or_else(|| "No microphone found".to_string())?;
    let supported = device
        .default_input_config()
        .map_err(|e| unavailable("Microphone", e))?;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let stream = match format {
        SampleFormat::F32 => build_input::<f32>(&device, &config, shared, out, ui),
        SampleFormat::I16 => build_input::<i16>(&device, &config, shared, out, ui),
        SampleFormat::U16 => build_input::<u16>(&device, &config, shared, out, ui),
        SampleFormat::I32 => build_input::<i32>(&device, &config, shared, out, ui),
        SampleFormat::U8 => build_input::<u8>(&device, &config, shared, out, ui),
        other => Err(format!("Unsupported microphone format ({other:?})")),
    }?;
    stream.play().map_err(|e| unavailable("Microphone", e))?;
    Ok(stream)
}

fn build_input<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    shared: &Arc<Shared>,
    out: flume::Sender<Outgoing>,
    ui: &UiLink,
) -> Result<cpal::Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = config.channels.max(1) as usize;
    let mut resampler = Resampler::new(config.sample_rate.0.max(1), WIRE_RATE);
    let mut chunker = FrameChunker::new();
    let shared = Arc::clone(shared);
    let error_ui = ui.clone();

    // Reused between callbacks so the audio thread does not allocate.
    let mut floats: Vec<f32> = Vec::new();
    let mut mono: Vec<f32> = Vec::new();
    let mut resampled: Vec<f32> = Vec::new();
    let mut ints: Vec<i16> = Vec::new();
    let mut frames: Vec<Vec<i16>> = Vec::new();

    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                if shared.mic_muted.load(Ordering::Relaxed) {
                    // Muted: send nothing, and do not splice stale audio onto
                    // whatever comes after the unmute.
                    chunker.clear();
                    return;
                }
                floats.clear();
                floats.extend(data.iter().map(|s| f32::from_sample(*s)));
                mono.clear();
                downmix(&floats, channels, &mut mono);
                resampled.clear();
                resampler.process(&mono, &mut resampled);
                ints.clear();
                ints.extend(resampled.iter().copied().map(f32_to_i16));
                frames.clear();
                chunker.push(&ints, &mut frames);
                for frame in frames.drain(..) {
                    // A full queue means the network is behind: drop, never wait.
                    let _ = out.try_send(Outgoing::Audio(frame));
                }
            },
            move |error| {
                eprintln!("[call] microphone error: {error}");
                error_ui.error(Source::Mic, "The microphone stopped working");
            },
            None,
        )
        .map_err(|e| unavailable("Microphone", e))
}

// --- Speaker ---

/// Start playing the jitter buffer on the default output device.
pub fn spawn_speaker(shared: Arc<Shared>, ui: UiLink) {
    let thread_shared = Arc::clone(&shared);
    let thread_ui = ui.clone();
    host_stream_thread("call-speaker", shared, ui, Source::Speaker, move || {
        open_speaker(&thread_shared, &thread_ui)
    });
}

fn open_speaker(shared: &Arc<Shared>, ui: &UiLink) -> Result<cpal::Stream, String> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or_else(|| "No speaker found".to_string())?;
    let supported = device
        .default_output_config()
        .map_err(|e| unavailable("Speaker", e))?;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let stream = match format {
        SampleFormat::F32 => build_output::<f32>(&device, &config, shared, ui),
        SampleFormat::I16 => build_output::<i16>(&device, &config, shared, ui),
        SampleFormat::U16 => build_output::<u16>(&device, &config, shared, ui),
        SampleFormat::I32 => build_output::<i32>(&device, &config, shared, ui),
        SampleFormat::U8 => build_output::<u8>(&device, &config, shared, ui),
        other => Err(format!("Unsupported speaker format ({other:?})")),
    }?;
    stream.play().map_err(|e| unavailable("Speaker", e))?;
    Ok(stream)
}

fn build_output<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    shared: &Arc<Shared>,
    ui: &UiLink,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels.max(1) as usize;
    let mut puller = PlaybackPuller::new(config.sample_rate.0.max(1), channels);
    let mut scratch: Vec<f32> = Vec::new();
    let shared = Arc::clone(shared);
    let error_ui = ui.clone();

    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                scratch.clear();
                scratch.resize(data.len(), 0.0);
                let muted = shared.speaker_muted.load(Ordering::Relaxed);
                puller.fill(&mut shared.jitter.lock(), &mut scratch, muted);
                for (slot, sample) in data.iter_mut().zip(&scratch) {
                    *slot = T::from_sample(*sample);
                }
            },
            move |error| {
                eprintln!("[call] speaker error: {error}");
                error_ui.error(Source::Speaker, "The speaker stopped working");
            },
            None,
        )
        .map_err(|e| unavailable("Speaker", e))
}

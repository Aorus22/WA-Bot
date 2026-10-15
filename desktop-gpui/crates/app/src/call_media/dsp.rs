//! Pure audio helpers for call media: no devices, no I/O, fully unit-tested.

use std::collections::VecDeque;
use std::f64::consts::PI;

/// Sample rate on the media WebSocket.
pub const WIRE_RATE: u32 = 16_000;
/// Samples per wire frame (60 ms).
pub const FRAME_SAMPLES: usize = 960;
/// Playback starts once this many frames are buffered.
pub const START_FRAMES: usize = 2;
/// Playback never holds more than ~200 ms; the oldest audio is dropped.
pub const MAX_BUFFER_SAMPLES: usize = (WIRE_RATE as usize) / 5;

/// Sinc zero crossings on each side of the kernel (before scaling by the
/// cut-off). Eight is plenty for speech.
const ZEROS: f64 = 8.0;

/// Streaming windowed-sinc resampler for mono f32 audio.
///
/// Going down (e.g. 48 kHz microphone to 16 kHz) the kernel is stretched so it
/// low-passes below the new Nyquist frequency before decimating, which is what
/// keeps the high end from folding back into the speech band. Going up
/// (16 kHz to a 48 kHz speaker) it is a band-limited interpolator. Any rate pair
/// works; equal rates pass through untouched.
pub struct Resampler {
    passthrough: bool,
    /// Input samples advanced per output sample.
    step: f64,
    /// Kernel cut-off relative to the input Nyquist frequency.
    cutoff: f64,
    /// Kernel half width, in input samples.
    half: isize,
    buf: Vec<f32>,
    /// Position of the next output sample inside `buf` (input-sample units).
    pos: f64,
}

impl Resampler {
    pub fn new(in_rate: u32, out_rate: u32) -> Self {
        let in_rate = in_rate.max(1);
        let out_rate = out_rate.max(1);
        let step = in_rate as f64 / out_rate as f64;
        let cutoff = (1.0 / step).min(1.0) * 0.95;
        Self {
            passthrough: in_rate == out_rate,
            step,
            cutoff,
            half: (ZEROS / cutoff).ceil() as isize,
            buf: Vec::new(),
            pos: 0.0,
        }
    }

    /// Input samples consumed per output sample.
    pub fn step(&self) -> f64 {
        self.step
    }

    /// Feed `input`, appending every output sample that is now computable.
    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        if self.passthrough {
            out.extend_from_slice(input);
            return;
        }
        self.buf.extend_from_slice(input);
        let len = self.buf.len() as isize;
        let half = self.half;
        loop {
            let center = self.pos.floor() as isize;
            // Taps span center-half+1 ..= center+half.
            if center + half >= len {
                break;
            }
            let mut acc = 0.0f64;
            let mut weights = 0.0f64;
            for i in (center - half + 1)..=(center + half) {
                let t = i as f64 - self.pos;
                let weight = self.kernel(t);
                weights += weight;
                if i >= 0 {
                    acc += weight * self.buf[i as usize] as f64;
                }
            }
            out.push(if weights.abs() > 1e-12 { (acc / weights) as f32 } else { 0.0 });
            self.pos += self.step;
        }
        // Forget what no later output can reach.
        let keep_from = (self.pos.floor() as isize - half + 1).max(0) as usize;
        if keep_from > 0 {
            self.buf.drain(..keep_from);
            self.pos -= keep_from as f64;
        }
    }

    fn kernel(&self, t: f64) -> f64 {
        let half = self.half as f64;
        if t.abs() >= half {
            return 0.0;
        }
        let x = PI * self.cutoff * t;
        let sinc = if x.abs() < 1e-9 { 1.0 } else { x.sin() / x };
        let window = 0.5 * (1.0 + (PI * t / half).cos());
        sinc * window
    }
}

/// Cuts a sample stream into exact [`FRAME_SAMPLES`]-sample frames.
#[derive(Default)]
pub struct FrameChunker {
    buf: Vec<i16>,
}

impl FrameChunker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append samples and push every complete frame onto `frames`.
    pub fn push(&mut self, samples: &[i16], frames: &mut Vec<Vec<i16>>) {
        self.buf.extend_from_slice(samples);
        let whole = self.buf.len() / FRAME_SAMPLES * FRAME_SAMPLES;
        if whole == 0 {
            return;
        }
        for frame in self.buf[..whole].chunks_exact(FRAME_SAMPLES) {
            frames.push(frame.to_vec());
        }
        self.buf.drain(..whole);
    }

    /// Forget a partial frame (after muting, for instance).
    pub fn clear(&mut self) {
        self.buf.clear();
    }

    pub fn pending(&self) -> usize {
        self.buf.len()
    }
}

pub fn f32_to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16
}

pub fn i16_to_f32(sample: i16) -> f32 {
    sample as f32 / 32768.0
}

/// Average interleaved frames of `channels` samples into mono.
pub fn downmix(interleaved: &[f32], channels: usize, mono: &mut Vec<f32>) {
    let channels = channels.max(1);
    if channels == 1 {
        mono.extend_from_slice(interleaved);
        return;
    }
    mono.extend(
        interleaved
            .chunks_exact(channels)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32),
    );
}

/// Playback jitter buffer at the wire rate.
///
/// Silence until [`START_FRAMES`] frames are queued, then a steady drain;
/// running dry goes back to buffering. Anything beyond ~200 ms is dropped from
/// the front so latency cannot grow without bound.
#[derive(Default)]
pub struct JitterBuffer {
    samples: VecDeque<f32>,
    started: bool,
    dropped: u64,
}

impl JitterBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, frame: &[f32]) {
        self.samples.extend(frame.iter().copied());
        if self.samples.len() > MAX_BUFFER_SAMPLES {
            let excess = self.samples.len() - MAX_BUFFER_SAMPLES;
            self.samples.drain(..excess);
            self.dropped += excess as u64;
        }
    }

    /// Fill `out`; whatever is not available is silence.
    pub fn pull(&mut self, out: &mut [f32]) {
        if !self.started {
            if self.samples.len() >= START_FRAMES * FRAME_SAMPLES {
                self.started = true;
            } else {
                out.fill(0.0);
                return;
            }
        }
        let available = self.samples.len().min(out.len());
        for (slot, sample) in out.iter_mut().zip(self.samples.drain(..available)) {
            *slot = sample;
        }
        if available < out.len() {
            out[available..].fill(0.0);
            // Ran dry: rebuffer before playing again.
            self.started = false;
        }
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn started(&self) -> bool {
        self.started
    }

    /// Samples discarded because the buffer overflowed.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    pub fn clear(&mut self) {
        self.samples.clear();
        self.started = false;
    }
}

/// Pull-based adapter between the wire-rate [`JitterBuffer`] and a device
/// callback that wants `rate` Hz, `channels` interleaved.
pub struct PlaybackPuller {
    resampler: Resampler,
    channels: usize,
    pending: VecDeque<f32>,
    wire: Vec<f32>,
    resampled: Vec<f32>,
}

impl PlaybackPuller {
    pub fn new(device_rate: u32, channels: usize) -> Self {
        Self {
            resampler: Resampler::new(WIRE_RATE, device_rate),
            channels: channels.max(1),
            pending: VecDeque::new(),
            wire: Vec::new(),
            resampled: Vec::new(),
        }
    }

    /// Fill an interleaved device buffer. With `muted` the audio is consumed
    /// (so latency does not build up) but the output is silent.
    pub fn fill(&mut self, jitter: &mut JitterBuffer, out: &mut [f32], muted: bool) {
        let frames = out.len() / self.channels;
        while self.pending.len() < frames {
            let missing = frames - self.pending.len();
            let want = (missing as f64 * self.resampler.step()).ceil() as usize + 1;
            self.wire.clear();
            self.wire.resize(want, 0.0);
            jitter.pull(&mut self.wire);
            self.resampled.clear();
            self.resampler.process(&self.wire, &mut self.resampled);
            self.pending.extend(self.resampled.iter().copied());
        }
        for frame in out.chunks_exact_mut(self.channels) {
            let sample = self.pending.pop_front().unwrap_or(0.0);
            let sample = if muted { 0.0 } else { sample };
            // Mono source: left and right, nothing on surround channels.
            for (index, slot) in frame.iter_mut().enumerate() {
                *slot = if index < 2 { sample } else { 0.0 };
            }
        }
        // A trailing partial frame (never expected) stays silent.
        let used = frames * self.channels;
        out[used..].fill(0.0);
    }
}

/// `s16le` bytes to normalised f32.
pub fn pcm_s16le_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(2)
        .map(|b| i16_to_f32(i16::from_le_bytes([b[0], b[1]])))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f64, rate: u32, seconds: f64) -> Vec<f32> {
        (0..(rate as f64 * seconds) as usize)
            .map(|n| (2.0 * PI * freq * n as f64 / rate as f64).sin() as f32 * 0.5)
            .collect()
    }

    fn rms(samples: &[f32]) -> f64 {
        (samples.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / samples.len() as f64).sqrt()
    }

    fn resample_all(input: &[f32], from: u32, to: u32) -> Vec<f32> {
        let mut resampler = Resampler::new(from, to);
        let mut out = Vec::new();
        resampler.process(input, &mut out);
        out
    }

    #[test]
    fn equal_rates_pass_through() {
        let input = sine(440.0, 16_000, 0.1);
        assert_eq!(resample_all(&input, 16_000, 16_000), input);
    }

    #[test]
    fn output_length_follows_the_ratio() {
        for (from, to) in [(48_000u32, 16_000u32), (44_100, 16_000), (32_000, 16_000), (8_000, 16_000), (16_000, 48_000), (16_000, 44_100)] {
            let input = vec![0.1f32; from as usize]; // one second
            let out = resample_all(&input, from, to);
            let expected = to as i64;
            // Only the kernel look-ahead is missing at the tail.
            assert!(
                (out.len() as i64 - expected).abs() <= 40,
                "{from}->{to}: got {} want ~{expected}",
                out.len()
            );
        }
    }

    #[test]
    fn dc_level_is_preserved() {
        let out = resample_all(&vec![0.5f32; 48_000], 48_000, 16_000);
        for sample in &out[100..out.len() - 100] {
            assert!((sample - 0.5).abs() < 1e-3, "{sample}");
        }
    }

    #[test]
    fn speech_band_survives_downsampling() {
        for rate in [48_000u32, 44_100, 32_000] {
            let out = resample_all(&sine(1_000.0, rate, 1.0), rate, 16_000);
            let body = &out[200..out.len() - 200];
            let ratio = rms(body) / (0.5 / 2f64.sqrt());
            assert!((0.9..1.1).contains(&ratio), "{rate}: amplitude ratio {ratio}");
        }
    }

    #[test]
    fn content_above_the_new_nyquist_is_filtered_not_aliased() {
        // 12 kHz at 48 kHz would fold to 4 kHz if it were simply decimated.
        let out = resample_all(&sine(12_000.0, 48_000, 1.0), 48_000, 16_000);
        let body = &out[200..out.len() - 200];
        assert!(rms(body) < 0.01, "residual {}", rms(body));

        // The same holds for a non-integer ratio (44.1 kHz).
        let out = resample_all(&sine(15_000.0, 44_100, 1.0), 44_100, 16_000);
        let body = &out[200..out.len() - 200];
        assert!(rms(body) < 0.02, "residual {}", rms(body));
    }

    #[test]
    fn upsampling_keeps_the_tone() {
        let out = resample_all(&sine(500.0, 16_000, 1.0), 16_000, 48_000);
        let body = &out[300..out.len() - 300];
        let ratio = rms(body) / (0.5 / 2f64.sqrt());
        assert!((0.95..1.05).contains(&ratio), "{ratio}");
    }

    #[test]
    fn chunked_input_matches_one_shot() {
        let input = sine(700.0, 44_100, 0.5);
        let whole = resample_all(&input, 44_100, 16_000);
        let mut resampler = Resampler::new(44_100, 16_000);
        let mut chunked = Vec::new();
        let mut cursor = 0;
        for size in [1usize, 17, 441, 3, 1024, 7].iter().cycle() {
            if cursor >= input.len() {
                break;
            }
            let end = (cursor + size).min(input.len());
            resampler.process(&input[cursor..end], &mut chunked);
            cursor = end;
        }
        assert_eq!(whole.len(), chunked.len());
        for (a, b) in whole.iter().zip(&chunked) {
            assert!((a - b).abs() < 1e-5);
        }
    }

    #[test]
    fn chunker_emits_exact_960_sample_frames() {
        let mut chunker = FrameChunker::new();
        let mut frames = Vec::new();
        chunker.push(&vec![1; 500], &mut frames);
        assert!(frames.is_empty());
        assert_eq!(chunker.pending(), 500);
        chunker.push(&vec![2; 2000], &mut frames);
        assert_eq!(frames.len(), 2);
        assert!(frames.iter().all(|f| f.len() == FRAME_SAMPLES));
        assert_eq!(frames[0][499], 1);
        assert_eq!(frames[0][500], 2);
        assert_eq!(chunker.pending(), 2500 - 1920);
        chunker.clear();
        assert_eq!(chunker.pending(), 0);
    }

    #[test]
    fn a_microphone_second_becomes_about_sixteen_frames() {
        // 48 kHz stereo-ish callback sizes -> 16 kHz -> frames.
        let mut resampler = Resampler::new(48_000, 16_000);
        let mut chunker = FrameChunker::new();
        let mut frames = Vec::new();
        let input = sine(300.0, 48_000, 1.0);
        for block in input.chunks(480) {
            let mut mono = Vec::new();
            downmix(block, 1, &mut mono);
            let mut out = Vec::new();
            resampler.process(&mono, &mut out);
            let ints: Vec<i16> = out.into_iter().map(f32_to_i16).collect();
            chunker.push(&ints, &mut frames);
        }
        assert!((15..=16).contains(&frames.len()), "{}", frames.len());
    }

    #[test]
    fn downmix_averages_channels() {
        let mut mono = Vec::new();
        downmix(&[1.0, 0.0, 0.5, 0.5, -1.0, 1.0], 2, &mut mono);
        assert_eq!(mono, vec![0.5, 0.5, 0.0]);
        let mut mono = Vec::new();
        downmix(&[0.1, 0.2], 1, &mut mono);
        assert_eq!(mono, vec![0.1, 0.2]);
    }

    #[test]
    fn sample_conversion_clamps() {
        assert_eq!(f32_to_i16(2.0), i16::MAX);
        assert_eq!(f32_to_i16(-2.0), -i16::MAX);
        assert_eq!(f32_to_i16(0.0), 0);
        assert_eq!(pcm_s16le_to_f32(&[0x00, 0x40, 0xFF]), vec![0.5]);
    }

    #[test]
    fn jitter_buffer_waits_for_two_frames() {
        let mut jitter = JitterBuffer::new();
        let mut out = vec![9.0f32; 100];
        jitter.push(&vec![0.25; FRAME_SAMPLES]);
        jitter.pull(&mut out);
        assert!(out.iter().all(|s| *s == 0.0), "still buffering");
        assert!(!jitter.started());

        jitter.push(&vec![0.25; FRAME_SAMPLES]);
        jitter.pull(&mut out);
        assert!(out.iter().all(|s| *s == 0.25));
        assert!(jitter.started());
        assert_eq!(jitter.len(), 2 * FRAME_SAMPLES - 100);
    }

    #[test]
    fn jitter_buffer_rebuffers_after_running_dry() {
        let mut jitter = JitterBuffer::new();
        jitter.push(&vec![1.0; 2 * FRAME_SAMPLES]);
        let mut out = vec![0.0f32; 2 * FRAME_SAMPLES + 10];
        jitter.pull(&mut out);
        assert_eq!(out[2 * FRAME_SAMPLES - 1], 1.0);
        assert_eq!(out[2 * FRAME_SAMPLES], 0.0);
        assert!(!jitter.started());
        // One new frame is not enough to resume.
        jitter.push(&vec![1.0; FRAME_SAMPLES]);
        let mut out = vec![5.0f32; 10];
        jitter.pull(&mut out);
        assert!(out.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn jitter_buffer_drops_the_oldest_audio_past_200ms() {
        let mut jitter = JitterBuffer::new();
        let mut frame = vec![0.0f32; FRAME_SAMPLES];
        for index in 0..10 {
            frame.fill(index as f32);
            jitter.push(&frame);
        }
        assert_eq!(jitter.len(), MAX_BUFFER_SAMPLES);
        assert_eq!(jitter.dropped() as usize, 10 * FRAME_SAMPLES - MAX_BUFFER_SAMPLES);
        let mut out = vec![0.0f32; 1];
        jitter.pull(&mut out);
        // The newest 3200 samples survive: frame 9 fills the end, frame 6 starts it.
        assert_eq!(out[0], 6.0);
    }

    #[test]
    fn puller_upsamples_and_duplicates_channels() {
        let mut jitter = JitterBuffer::new();
        jitter.push(&vec![0.5; 3 * FRAME_SAMPLES]);
        let mut puller = PlaybackPuller::new(48_000, 2);
        let mut out = vec![0.0f32; 2 * 4800]; // 100 ms stereo
        puller.fill(&mut jitter, &mut out, false);
        // After the kernel warm-up the signal is a steady 0.5 on both channels.
        for frame in out.chunks_exact(2).skip(200) {
            assert!((frame[0] - 0.5).abs() < 0.01 && frame[0] == frame[1], "{frame:?}");
        }
        // ~100 ms of wire audio was consumed (1600 samples) plus a little look-ahead.
        let consumed = 3 * FRAME_SAMPLES - jitter.len();
        assert!((1500..1800).contains(&consumed), "{consumed}");
    }

    #[test]
    fn puller_muted_is_silent_but_drains() {
        let mut jitter = JitterBuffer::new();
        jitter.push(&vec![0.5; 3 * FRAME_SAMPLES]);
        let mut puller = PlaybackPuller::new(16_000, 1);
        let mut out = vec![1.0f32; 800];
        puller.fill(&mut jitter, &mut out, true);
        assert!(out.iter().all(|s| *s == 0.0));
        assert!(jitter.len() < 3 * FRAME_SAMPLES);
    }

    #[test]
    fn puller_only_feeds_the_first_two_channels() {
        let mut jitter = JitterBuffer::new();
        jitter.push(&vec![0.5; 3 * FRAME_SAMPLES]);
        let mut puller = PlaybackPuller::new(16_000, 6);
        let mut out = vec![1.0f32; 6 * 50];
        puller.fill(&mut jitter, &mut out, false);
        for frame in out.chunks_exact(6) {
            assert_eq!(frame[0], 0.5);
            assert_eq!(frame[1], 0.5);
            assert!(frame[2..].iter().all(|s| *s == 0.0));
        }
    }
}

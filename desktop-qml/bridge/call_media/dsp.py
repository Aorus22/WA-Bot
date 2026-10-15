"""Pure audio helpers for call media (no Qt, no devices): a port of `call_media/dsp.rs`.

* `Resampler`    streaming windowed-sinc resampler for mono float audio (only used when the
                 sound card refuses the 16 kHz mono wire format; Qt normally converts for us).
* `FrameChunker` cuts a PCM stream into exact 960-sample (60 ms) wire frames.
* `JitterBuffer` playback buffer: silence until 2 frames are queued, then a steady drain; running
                 dry re-buffers; anything beyond ~200 ms is dropped from the front.
* `decode_pcm` / `encode_pcm`  device sample formats <-> mono float.
"""
from __future__ import annotations

import math
import sys
from array import array
from operator import mul

from .frame import AUDIO_FRAME_BYTES, AUDIO_FRAME_SAMPLES, AUDIO_RATE

WIRE_RATE = AUDIO_RATE
FRAME_SAMPLES = AUDIO_FRAME_SAMPLES
START_FRAMES = 2
MAX_BUFFER_SAMPLES = WIRE_RATE // 5   # ~200 ms
ZEROS = 8.0                           # sinc zero crossings per side (before cut-off scaling)
PHASES = 64                           # fractional-position quantisation of the kernel table


class Resampler:
    """Streaming mono resampler. Downsampling stretches the kernel so it low-passes under the new
    Nyquist frequency before decimating; upsampling is a band-limited interpolator. Equal rates
    pass through untouched. Kernels are pre-computed per fractional phase (64 phases: <0.01 sample
    of timing error, inaudible for speech) so `process` is a handful of C-level dot products."""

    def __init__(self, in_rate: int, out_rate: int):
        in_rate, out_rate = max(1, int(in_rate)), max(1, int(out_rate))
        self.passthrough = in_rate == out_rate
        self.step = in_rate / out_rate
        self.cutoff = min(1.0 / self.step, 1.0) * 0.95
        self.half = int(math.ceil(ZEROS / self.cutoff))
        self._buf: list[float] = [0.0] * (self.half - 1)   # stands in for the missing past
        self._pos = float(self.half - 1)
        self._taps: list[list[float]] | None = None

    def _kernel(self, t: float) -> float:
        half = float(self.half)
        if abs(t) >= half:
            return 0.0
        x = math.pi * self.cutoff * t
        sinc = 1.0 if abs(x) < 1e-9 else math.sin(x) / x
        return sinc * 0.5 * (1.0 + math.cos(math.pi * t / half))

    def _table(self) -> list[list[float]]:
        if self._taps is None:
            table = []
            for p in range(PHASES + 1):
                frac = p / PHASES
                w = [self._kernel(k - self.half + 1 - frac) for k in range(2 * self.half)]
                s = sum(w)
                table.append([x / s for x in w] if abs(s) > 1e-12 else [0.0] * len(w))
            self._taps = table
        return self._taps

    def process(self, samples: list[float]) -> list[float]:
        if self.passthrough:
            return list(samples)
        buf, half = self._buf, self.half
        buf.extend(samples)
        table, out, pos, step = self._table(), [], self._pos, self.step
        n = len(buf)
        while True:
            c = int(pos)
            if c + half >= n:
                break
            taps = table[int(round((pos - c) * PHASES))]
            lo = c - half + 1
            out.append(sum(map(mul, taps, buf[lo:lo + 2 * half])))
            pos += step
        keep = max(0, int(pos) - half + 1)
        if keep:
            del buf[:keep]
            pos -= keep
        self._pos = pos
        return out


class FrameChunker:
    """Cuts a PCM s16le byte stream into exact 1920-byte (960-sample) frames."""

    def __init__(self) -> None:
        self._buf = bytearray()

    def push(self, pcm: bytes) -> list[bytes]:
        self._buf += pcm
        whole = len(self._buf) // AUDIO_FRAME_BYTES * AUDIO_FRAME_BYTES
        if not whole:
            return []
        frames = [bytes(self._buf[i:i + AUDIO_FRAME_BYTES]) for i in range(0, whole, AUDIO_FRAME_BYTES)]
        del self._buf[:whole]
        return frames

    def clear(self) -> None:
        self._buf.clear()

    @property
    def pending(self) -> int:
        """Buffered samples that do not make a frame yet."""
        return len(self._buf) // 2


class JitterBuffer:
    """Playback buffer at the wire rate holding s16le bytes."""

    def __init__(self) -> None:
        self._buf = bytearray()
        self.started = False
        self.dropped = 0   # samples discarded on overflow

    def __len__(self) -> int:
        """Buffered samples."""
        return len(self._buf) // 2

    def push(self, pcm: bytes) -> None:
        self._buf += pcm[: len(pcm) & ~1]
        excess = len(self._buf) // 2 - MAX_BUFFER_SAMPLES
        if excess > 0:
            del self._buf[: excess * 2]
            self.dropped += excess

    def pull(self, nsamples: int) -> bytes:
        """`nsamples` samples (zero padded). Silence until START_FRAMES frames are queued;
        running dry goes back to buffering."""
        want = nsamples * 2
        if not self.started:
            if len(self) >= START_FRAMES * FRAME_SAMPLES:
                self.started = True
            else:
                return bytes(want)
        take = min(len(self._buf), want)
        out = bytes(self._buf[:take])
        del self._buf[:take]
        if take < want:
            out += bytes(want - take)
            self.started = False
        return out

    def clear(self) -> None:
        self._buf.clear()
        self.started = False


# --- sample formats ---------------------------------------------------------------------
def decode_pcm(data: bytes, fmt: str, channels: int) -> list[float]:
    """Interleaved device samples ("i16" | "i32" | "f32" | "u8") -> mono float in [-1, 1]."""
    channels = max(1, channels)
    if fmt == "i16":
        a = array("h"); a.frombytes(data[: len(data) // 2 * 2]); scale = 32768.0
    elif fmt == "i32":
        a = array("i"); a.frombytes(data[: len(data) // 4 * 4]); scale = 2147483648.0
    elif fmt == "f32":
        a = array("f"); a.frombytes(data[: len(data) // 4 * 4]); scale = 1.0
    elif fmt == "u8":
        a = array("h", (b - 128 for b in data)); scale = 128.0
    else:
        raise ValueError(f"unsupported sample format {fmt}")
    if sys.byteorder == "big" and fmt in ("i16", "i32", "f32"):
        a.byteswap()
    if channels == 1:
        return [s / scale for s in a]
    n = len(a) // channels
    return [sum(a[i * channels:(i + 1) * channels]) / (channels * scale) for i in range(n)]


def encode_pcm(mono: list[float], fmt: str, channels: int) -> bytes:
    """Mono float -> interleaved device samples (the mono signal on every channel)."""
    channels = max(1, channels)
    clipped = [max(-1.0, min(1.0, s)) for s in mono]
    if fmt == "i16":
        a = array("h", (int(round(s * 32767.0)) for s in clipped))
    elif fmt == "i32":
        a = array("i", (int(round(s * 2147483647.0)) for s in clipped))
    elif fmt == "f32":
        a = array("f", clipped)
    elif fmt == "u8":
        return bytes(int(round(s * 127.0)) + 128 for s in clipped for _ in range(channels))
    else:
        raise ValueError(f"unsupported sample format {fmt}")
    if channels > 1:
        a = type(a)(a.typecode, (v for v in a for _ in range(channels)))
    if sys.byteorder == "big":
        a.byteswap()
    return a.tobytes()


def f32_to_s16(samples: list[float]) -> bytes:
    a = array("h", (int(round(max(-1.0, min(1.0, s)) * 32767.0)) for s in samples))
    if sys.byteorder == "big":
        a.byteswap()
    return a.tobytes()

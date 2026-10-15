"""Voice-note capture for the composer. Mirrors GPUI `voice_recorder.rs`: the default input
device is read as raw PCM, downmixed to mono and thinned to at most 16 kHz (a box filter),
then wrapped in a 16-bit mono WAV. The backend transcodes that WAV to ogg/opus (`ptt=true`)
and derives the waveform, exactly as it does for the GPUI client.

`PcmAccumulator` is pure Python (fed bytes, unit-tested without audio hardware).
`VoiceRecorder` wraps `QAudioSource` (QtMultimedia) and feeds it.
"""
from __future__ import annotations

import struct
import sys
from array import array
from collections import deque

from PySide6.QtCore import QObject, QTimer, Signal

TARGET_RATE = 16_000
MAX_SECONDS = 30 * 60
MIN_SECONDS = 0.6
LEVEL_BARS = 36


class Thinner:
    """Streaming rate reducer: averages the input samples that fall into each output sample."""

    def __init__(self, in_rate: int, out_rate: int):
        self.step = out_rate / in_rate
        self.phase = 0.0
        self.total = 0.0
        self.count = 0

    def push(self, sample: float, out: array) -> None:
        self.total += sample
        self.count += 1
        self.phase += self.step
        if self.phase >= 1.0 - 1e-9:
            self.phase -= 1.0
            mean = max(-1.0, min(1.0, self.total / self.count))
            out.append(int(mean * 32767))
            self.total = 0.0
            self.count = 0


def encode_wav(samples, rate: int) -> bytes:
    """16-bit mono PCM in a canonical 44-byte-header WAV."""
    data = array("h", samples)
    if sys.byteorder == "big":
        data.byteswap()
    raw = data.tobytes()
    n = len(raw)
    return (b"RIFF" + struct.pack("<I", 36 + n) + b"WAVEfmt " + struct.pack("<IHHIIHH", 16, 1, 1, rate, rate * 2, 2, 16)
            + b"data" + struct.pack("<I", n) + raw)


def format_timer(seconds: float) -> str:
    whole = int(max(0.0, seconds))
    return f"{whole // 60}:{whole % 60:02d}"


# sample kinds understood by the accumulator -> (array typecode / struct char, bytes, scale to [-1, 1])
_KINDS = {
    "int16": ("h", 2, 1 / 32768),
    "int32": ("i", 4, 1 / 2147483648),
    "float": ("f", 4, 1.0),
    "uint8": ("B", 1, None),
}


class PcmAccumulator:
    """Collects mono 16-bit samples at min(in_rate, 16 kHz) from interleaved PCM chunks."""

    def __init__(self, in_rate: int, channels: int = 1, kind: str = "int16"):
        if kind not in _KINDS:
            raise ValueError(f"unsupported sample kind {kind!r}")
        self.in_rate = max(1, in_rate)
        self.out_rate = min(self.in_rate, TARGET_RATE)
        self.channels = max(1, channels)
        self.kind = kind
        self.samples = array("h")
        self.peak = 0
        self.levels: deque[float] = deque([0.0] * LEVEL_BARS, maxlen=LEVEL_BARS)
        self._thin = None if self.out_rate == self.in_rate else Thinner(self.in_rate, self.out_rate)
        self._cap = MAX_SECONDS * self.out_rate
        self._carry = b""  # incomplete frame left over from the previous chunk

    @property
    def seconds(self) -> float:
        return len(self.samples) / self.out_rate

    def feed(self, data: bytes) -> None:
        code, width, scale = _KINDS[self.kind]
        frame = width * self.channels
        data = self._carry + bytes(data)
        usable = len(data) - len(data) % frame
        self._carry = data[usable:]
        if not usable:
            return
        raw = array(code)
        raw.frombytes(data[:usable])
        if sys.byteorder == "big" and width > 1:
            raw.byteswap()
        ch = self.channels
        if ch == 1:
            mono = raw
        else:
            cols = [raw[i::ch] for i in range(ch)]
            mono = [sum(v) / ch for v in zip(*cols)]
        if scale is None:  # unsigned 8-bit, centred on 128
            norm = ((v - 128) / 128 for v in mono)
        elif scale == 1.0:
            norm = iter(mono)
        else:
            norm = (v * scale for v in mono)
        out = array("h")
        if self._thin is None:
            for v in norm:
                out.append(int(max(-1.0, min(1.0, v)) * 32767))
        else:
            for v in norm:
                self._thin.push(v, out)
        if not out:
            return
        chunk_peak = max(max(out), -min(out))
        self.peak = max(self.peak, chunk_peak)
        self.levels.append(min(1.0, chunk_peak / 32767))
        room = max(0, self._cap - len(self.samples))
        self.samples.extend(out[:room])

    def finish(self) -> dict:
        return {"wav": encode_wav(self.samples, self.out_rate), "seconds": self.seconds, "peak": self.peak}


def _format_kind(fmt) -> str | None:
    from PySide6.QtMultimedia import QAudioFormat
    sf = QAudioFormat.SampleFormat
    return {sf.Int16: "int16", sf.Int32: "int32", sf.Float: "float", sf.UInt8: "uint8"}.get(fmt.sampleFormat())


class VoiceRecorder(QObject):
    """Opens the default microphone and accumulates PCM. `start()` returns an error string
    meant for the user, or "" on success. Dropping/`stop()`ing discards the capture."""
    levelsChanged = Signal()

    def __init__(self, parent: QObject | None = None):
        super().__init__(parent)
        self._src = None
        self._io = None
        self._acc: PcmAccumulator | None = None
        self._tick = QTimer(self)
        self._tick.setInterval(100)
        self._tick.timeout.connect(self.levelsChanged)

    @property
    def active(self) -> bool:
        return self._src is not None

    def seconds(self) -> float:
        return self._acc.seconds if self._acc else 0.0

    def levels(self) -> list[float]:
        return list(self._acc.levels) if self._acc else []

    def start(self) -> str:
        try:
            from PySide6.QtMultimedia import QAudioFormat, QAudioSource, QMediaDevices
        except ImportError as e:  # pragma: no cover - QtMultimedia missing
            print(f"[voice] QtMultimedia unavailable: {e}", file=sys.stderr)
            return "Mikrofon tidak bisa dibuka. Periksa apakah mikrofon tersambung dan diizinkan."
        device = QMediaDevices.defaultAudioInput()
        if device.isNull():
            return "Mikrofon tidak ditemukan"
        want = QAudioFormat()
        want.setSampleRate(TARGET_RATE)
        want.setChannelCount(1)
        want.setSampleFormat(QAudioFormat.SampleFormat.Int16)
        fmt = want if device.isFormatSupported(want) else device.preferredFormat()
        kind = _format_kind(fmt)
        if kind is None:
            return f"Format audio mikrofon tidak didukung ({fmt.sampleFormat().name})"
        self._acc = PcmAccumulator(fmt.sampleRate(), fmt.channelCount(), kind)
        src = QAudioSource(device, fmt, self)
        io = src.start()
        if io is None or src.error() != QAudioSource.Error.NoError:
            print(f"[voice] microphone unavailable: {src.error()}", file=sys.stderr)
            src.deleteLater()
            self._acc = None
            return "Mikrofon tidak bisa dibuka. Periksa apakah mikrofon tersambung dan diizinkan."
        self._src, self._io = src, io
        io.readyRead.connect(self._drain)
        self._tick.start()
        return ""

    def _drain(self) -> None:
        if self._io is not None and self._acc is not None:
            data = bytes(self._io.readAll())
            if data:
                self._acc.feed(data)

    def finish(self) -> dict:
        """Stop capturing and return {wav, seconds, peak}."""
        self._drain()
        acc = self._acc
        self.stop()
        return acc.finish() if acc else {"wav": encode_wav([], TARGET_RATE), "seconds": 0.0, "peak": 0}

    def stop(self) -> None:
        self._tick.stop()
        if self._src is not None:
            try:
                self._src.stop()
            finally:
                self._src.deleteLater()
        self._src = self._io = None
        self._acc = None

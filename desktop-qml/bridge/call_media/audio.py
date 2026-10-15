"""Microphone capture and speaker playback (QtMultimedia): port of `call_media/audio.rs`.

Both objects live on the session's worker thread. The wire format is mono 16 kHz signed 16-bit;
Qt's audio stack converts for us on PulseAudio/PipeWire/WASAPI/CoreAudio. When a device refuses
that format the device's preferred format is used and `dsp.Resampler` converts in Python.

Devices are chosen by `QAudioDevice.id()` (empty = system default). Following the default device
when it changes, and device hot-plug, are handled by restarting the stream.
"""
from __future__ import annotations

import sys
from typing import Callable

from PySide6.QtCore import QObject, QTimer, Signal, Slot
from PySide6.QtMultimedia import QAudioDevice, QAudioFormat, QAudioSink, QAudioSource, QMediaDevices

from . import dsp, permissions
from .frame import AUDIO_FRAME_BYTES, AUDIO_RATE

SF = QAudioFormat.SampleFormat
_FMT_NAMES = {SF.UInt8: "u8", SF.Int16: "i16", SF.Int32: "i32", SF.Float: "f32"}
PUMP_MS = 10
# How much audio is kept queued inside the sound card: low = low latency, high = fewer underruns.
SPEAKER_QUEUE_MS = 50


def list_inputs() -> list[dict]:
    default = QMediaDevices.defaultAudioInput().id()
    return [{"id": bytes(d.id()).decode(), "name": d.description(), "isDefault": d.id() == default}
            for d in QMediaDevices.audioInputs()]


def list_outputs() -> list[dict]:
    default = QMediaDevices.defaultAudioOutput().id()
    return [{"id": bytes(d.id()).decode(), "name": d.description(), "isDefault": d.id() == default}
            for d in QMediaDevices.audioOutputs()]


def _pick(devices: list[QAudioDevice], wanted: str, default: QAudioDevice) -> QAudioDevice:
    if wanted:
        for d in devices:
            if bytes(d.id()).decode() == wanted:
                return d
    return default


def wire_format() -> QAudioFormat:
    f = QAudioFormat()
    f.setSampleRate(AUDIO_RATE)
    f.setChannelCount(1)
    f.setSampleFormat(SF.Int16)
    return f


def _negotiate(dev: QAudioDevice) -> tuple[QAudioFormat, bool]:
    """(format to open the device with, needs Python conversion)."""
    wf = wire_format()
    if dev.isFormatSupported(wf):
        return wf, False
    return dev.preferredFormat(), True


def unavailable(what: str, error: object) -> str:
    print(f"[call] {what} unavailable: {error}", file=sys.stderr)
    return f"{what} could not be opened. Check that it is connected and allowed."


class Microphone(QObject):
    """Captures the chosen input and emits 960-sample wire frames while not muted."""
    frame = Signal(bytes)
    notice = Signal(str, str)   # source, message ("" clears)

    def __init__(self, device_id: str = "", parent: QObject | None = None):
        super().__init__(parent)
        self._device_id = device_id
        self._muted = False
        self._running = False
        self._src: QAudioSource | None = None
        self._io = None
        self._conv: _CaptureConverter | None = None
        self._chunker = dsp.FrameChunker()
        self._devices = QMediaDevices(self)
        self._devices.audioInputsChanged.connect(self._on_devices)
        self._current_id = ""
        self.frames_emitted = 0

    @Slot()
    def start(self) -> None:
        self._running = True
        permissions.ensure("microphone", self, self._begin)

    def _begin(self, granted: bool) -> None:
        if not self._running:
            return
        if not granted:
            self.notice.emit("mic", permissions.DENIED["microphone"])
            return
        self._open()

    def _open(self) -> None:
        self._close()
        devices = QMediaDevices.audioInputs()
        default = QMediaDevices.defaultAudioInput()
        if not devices or default.isNull():
            self.notice.emit("mic", "No microphone found")
            return
        dev = _pick(devices, self._device_id, default)
        fmt, convert = _negotiate(dev)
        name = _FMT_NAMES.get(fmt.sampleFormat())
        if name is None:
            self.notice.emit("mic", f"Unsupported microphone format ({fmt.sampleFormat().name})")
            return
        src = QAudioSource(dev, fmt, self)
        src.setBufferSize(max(fmt.bytesForDuration(120_000), 4096))
        io = src.start()
        if io is None or src.error() != QAudioSource.Error.NoError:
            self.notice.emit("mic", unavailable("Microphone", src.error()))
            src.deleteLater()
            return
        io.readyRead.connect(self._read)
        src.stateChanged.connect(self._state)
        self._src, self._io, self._current_id = src, io, bytes(dev.id()).decode()
        self._conv = _CaptureConverter(fmt, name) if convert else None
        self._chunker.clear()
        self.notice.emit("mic", "")

    def _close(self) -> None:
        src, self._src, self._io = self._src, None, None
        if src is not None:
            try:
                src.stateChanged.disconnect()
            except (RuntimeError, TypeError):
                pass
            src.stop()
            src.deleteLater()

    def _state(self, _state) -> None:
        if self._src is not None and self._src.error() not in (QAudioSource.Error.NoError,):
            self.notice.emit("mic", "The microphone stopped working")

    def _read(self) -> None:
        io = self._io
        if io is None:
            return
        data = bytes(io.readAll())
        if not data:
            return
        if self._muted:
            self._chunker.clear()   # nothing sent, and no stale audio spliced onto the unmute
            return
        pcm = self._conv.to_wire(data) if self._conv else data
        for f in self._chunker.push(pcm):
            self.frames_emitted += 1
            self.frame.emit(f)

    def _on_devices(self) -> None:
        # unplugged (selected device vanished) or the system default moved: reopen
        if not self._running:
            return
        ids = {bytes(d.id()).decode() for d in QMediaDevices.audioInputs()}
        want = self._device_id or bytes(QMediaDevices.defaultAudioInput().id()).decode()
        if self._current_id not in ids or (not self._device_id and want != self._current_id):
            self._open()

    @Slot(bool)
    def set_muted(self, muted: bool) -> None:
        self._muted = muted
        if muted:
            self._chunker.clear()

    @Slot(str)
    def set_device(self, device_id: str) -> None:
        if device_id != self._device_id:
            self._device_id = device_id
            if self._running:
                self._open()

    @Slot()
    def stop(self) -> None:
        self._running = False
        self._close()


class _CaptureConverter:
    def __init__(self, fmt: QAudioFormat, name: str):
        self.name = name
        self.channels = max(1, fmt.channelCount())
        self.resampler = dsp.Resampler(fmt.sampleRate(), AUDIO_RATE)
        self._rem = b""
        self._width = {"u8": 1, "i16": 2, "i32": 4, "f32": 4}[name] * self.channels

    def to_wire(self, data: bytes) -> bytes:
        data = self._rem + data
        whole = len(data) // self._width * self._width
        self._rem = data[whole:]
        mono = dsp.decode_pcm(data[:whole], self.name, self.channels)
        return dsp.f32_to_s16(self.resampler.process(mono))


class _PlaybackConverter:
    """Pull-based adapter between the wire-rate jitter buffer and a device format."""

    def __init__(self, fmt: QAudioFormat, name: str):
        self.name, self.channels = name, max(1, fmt.channelCount())
        self.resampler = dsp.Resampler(AUDIO_RATE, fmt.sampleRate())
        self._pending: list[float] = []
        self.bytes_per_frame = {"u8": 1, "i16": 2, "i32": 4, "f32": 4}[name] * self.channels

    def fill(self, jitter: dsp.JitterBuffer, frames: int, muted: bool) -> bytes:
        while len(self._pending) < frames:
            want = int((frames - len(self._pending)) * self.resampler.step) + 2
            pcm = jitter.pull(want)
            self._pending.extend(self.resampler.process(dsp.decode_pcm(pcm, "i16", 1)))
        mono, self._pending = self._pending[:frames], self._pending[frames:]
        if muted:
            mono = [0.0] * frames
        return dsp.encode_pcm(mono, self.name, self.channels)


class Speaker(QObject):
    """Plays the jitter buffer on the chosen output. Keeps ~50 ms queued in the sound card."""
    notice = Signal(str, str)

    def __init__(self, jitter: dsp.JitterBuffer, device_id: str = "", parent: QObject | None = None):
        super().__init__(parent)
        self.jitter = jitter
        self._device_id = device_id
        self._muted = False
        self._running = False
        self._sink: QAudioSink | None = None
        self._io = None
        self._conv: _PlaybackConverter | None = None
        self._queue_bytes = 0
        self._current_id = ""
        self._timer = QTimer(self)
        self._timer.setInterval(PUMP_MS)
        self._timer.timeout.connect(self._pump)
        self._devices = QMediaDevices(self)
        self._devices.audioOutputsChanged.connect(self._on_devices)
        self.bytes_written = 0

    @Slot()
    def start(self) -> None:
        self._running = True
        self._open()

    def _open(self) -> None:
        self._close()
        devices = QMediaDevices.audioOutputs()
        default = QMediaDevices.defaultAudioOutput()
        if not devices or default.isNull():
            self.notice.emit("speaker", "No speaker found")
            return
        dev = _pick(devices, self._device_id, default)
        fmt, convert = _negotiate(dev)
        name = _FMT_NAMES.get(fmt.sampleFormat())
        if name is None:
            self.notice.emit("speaker", f"Unsupported speaker format ({fmt.sampleFormat().name})")
            return
        sink = QAudioSink(dev, fmt, self)
        sink.setBufferSize(max(fmt.bytesForDuration(250_000), 4096))
        io = sink.start()
        if io is None or sink.error() != QAudioSink.Error.NoError:
            self.notice.emit("speaker", unavailable("Speaker", sink.error()))
            sink.deleteLater()
            return
        sink.stateChanged.connect(self._state)
        self._sink, self._io, self._current_id = sink, io, bytes(dev.id()).decode()
        self._conv = _PlaybackConverter(fmt, name) if convert else None
        self._queue_bytes = fmt.bytesForDuration(SPEAKER_QUEUE_MS * 1000)
        self._bpf = fmt.bytesPerFrame()
        self._timer.start()
        self.notice.emit("speaker", "")

    def _close(self) -> None:
        self._timer.stop()
        sink, self._sink, self._io = self._sink, None, None
        if sink is not None:
            try:
                sink.stateChanged.disconnect()
            except (RuntimeError, TypeError):
                pass
            sink.stop()
            sink.deleteLater()

    def _state(self, _state) -> None:
        if self._sink is not None and self._sink.error() in (
                QAudioSink.Error.OpenError, QAudioSink.Error.IOError, QAudioSink.Error.FatalError):
            self.notice.emit("speaker", "The speaker stopped working")

    def _pump(self) -> None:
        sink, io = self._sink, self._io
        if sink is None or io is None:
            return
        queued = sink.bufferSize() - sink.bytesFree()
        need = self._queue_bytes - queued
        frames = need // max(1, self._bpf)
        if frames <= 0:
            return
        if self._conv is not None:
            data = self._conv.fill(self.jitter, frames, self._muted)
        else:
            data = self.jitter.pull(frames)   # wire format: one 16-bit sample per frame
            if self._muted:
                data = bytes(len(data))
        if data:
            self.bytes_written += io.write(data)

    def _on_devices(self) -> None:
        if not self._running:
            return
        ids = {bytes(d.id()).decode() for d in QMediaDevices.audioOutputs()}
        want = self._device_id or bytes(QMediaDevices.defaultAudioOutput().id()).decode()
        if self._current_id not in ids or (not self._device_id and want != self._current_id):
            self._open()

    @Slot(bool)
    def set_muted(self, muted: bool) -> None:
        self._muted = muted

    @Slot(str)
    def set_device(self, device_id: str) -> None:
        if device_id != self._device_id:
            self._device_id = device_id
            if self._running:
                self._open()

    @Slot()
    def stop(self) -> None:
        self._running = False
        self._close()

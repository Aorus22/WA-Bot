"""Everything that belongs to one call's media: network, audio and H.264 video on one worker QThread.

`MediaSession` is created on the UI thread, moved to a QThread by `SessionHandle`, and driven by
queued signals. Pictures cross threads through "latest value" slots (like GPUI's `FrameSlots`) so a
stalled UI never accumulates decoded frames and a slow encoder never queues camera frames.
"""
from __future__ import annotations

import threading
from typing import Callable

from PySide6.QtCore import QMetaObject, QObject, Qt, QThread, Signal, Slot
from PySide6.QtGui import QImage

from . import dsp, video
from .audio import Microphone, Speaker
from .net import MediaNet


class MediaSession(QObject):
    notice = Signal(str, str)            # source (network|mic|speaker|camera), message ("" clears)
    remoteFrameReady = Signal()          # a picture is waiting in take_remote_frame()
    networkConnected = Signal(bool)
    stopped = Signal()

    # control (queued from the UI thread)
    setMicMuted = Signal(bool)
    setSpeakerMuted = Signal(bool)
    setMicDevice = Signal(str)
    setSpeakerDevice = Signal(str)
    requestStop = Signal()

    def __init__(self, call_id: str, base_url: Callable[[], str], *, devices: bool = True,
                 retry_delay_ms: int = 1000, mic_id: str = "", speaker_id: str = ""):
        super().__init__()
        self.call_id = call_id
        self._base_url = base_url
        self._devices = devices
        self._retry_ms = retry_delay_ms
        self._mic_id, self._spk_id = mic_id, speaker_id
        self.jitter = dsp.JitterBuffer()
        self.net: MediaNet | None = None
        self.mic: Microphone | None = None
        self.speaker: Speaker | None = None
        self._encoder: video.H264Encoder | None = None
        self._decoder: video.H264Decoder | None = None
        self._keyframe_needed = True
        self._encoder_failed = False
        self._decoder_failed = False
        self._lock = threading.Lock()
        self._remote: QImage | None = None
        self._remote_pending = False
        self._out: QImage | None = None
        self._out_pending = False
        self._muted_mic = False
        self._muted_spk = False
        self.stats = {"encoded": 0, "decoded": 0, "bad_video": 0}
        self.requestStop.connect(self.stop)

    # -- lifecycle (worker thread) ---------------------------------------------------------
    @Slot()
    def start(self) -> None:
        net = self.net = MediaNet(self.call_id, self._base_url, self._retry_ms, self)
        net.notice.connect(self.notice)
        net.connectedChanged.connect(self._on_net_connected)
        net.audioReceived.connect(self._on_audio)
        net.videoReceived.connect(self._on_video)
        net.keyframeRequested.connect(self._request_keyframe)
        if self._devices:
            mic = self.mic = Microphone(self._mic_id, self)
            mic.set_muted(self._muted_mic)
            mic.frame.connect(net.send_audio)
            mic.notice.connect(self.notice)
            self.setMicMuted.connect(mic.set_muted)
            self.setMicDevice.connect(mic.set_device)
            spk = self.speaker = Speaker(self.jitter, self._spk_id, self)
            spk.set_muted(self._muted_spk)
            spk.notice.connect(self.notice)
            self.setSpeakerMuted.connect(spk.set_muted)
            self.setSpeakerDevice.connect(spk.set_device)
            mic.start()
            spk.start()
        net.start()

    @Slot()
    def stop(self) -> None:
        for part in (self.net, self.mic, self.speaker):
            if part is not None:
                part.stop()
        if self._encoder is not None:
            self._encoder.close()
            self._encoder = None
        if self._decoder is not None:
            self._decoder.close()
            self._decoder = None
        self.stopped.emit()

    @Slot(bool)
    def _on_net_connected(self, up: bool) -> None:
        if up:
            self.jitter.clear()   # whatever piled up while offline is stale: media is real time
        self.networkConnected.emit(up)

    # -- incoming -----------------------------------------------------------------------------
    @Slot(bytes)
    def _on_audio(self, pcm: bytes) -> None:
        self.jitter.push(pcm)

    @Slot(bytes)
    def _on_video(self, access_unit: bytes) -> None:
        if self._decoder_failed:
            return
        if self._decoder is None:
            try:
                self._decoder = video.H264Decoder()
            except Exception as exc:
                self._decoder_failed = True
                self.notice.emit("network", f"H.264 decoder unavailable: {exc}")
                self.stats["bad_video"] += 1
                return
        try:
            img = self._decoder.decode(access_unit)
        except Exception as exc:
            self._decoder_failed = True
            self.notice.emit("network", f"H.264 decoding failed: {exc}")
            self.stats["bad_video"] += 1
            return
        if img is None:
            self.stats["bad_video"] += 1
            return
        self.stats["decoded"] += 1
        with self._lock:
            self._remote = img
            wake = not self._remote_pending
            self._remote_pending = True
        if wake:
            self.remoteFrameReady.emit()

    def take_remote_frame(self) -> QImage | None:
        """UI side: the newest decoded picture, re-arming the wake-up."""
        with self._lock:
            img, self._remote, self._remote_pending = self._remote, None, False
        return img

    # -- outgoing video ---------------------------------------------------------------------------
    def offer_video(self, image: QImage) -> None:
        """Any thread: queue only the newest image for H.264 encoding; older frames are dropped."""
        with self._lock:
            self._out = image
            wake = not self._out_pending
            self._out_pending = True
        if wake:
            QMetaObject.invokeMethod(self, "_encode_pending", Qt.ConnectionType.QueuedConnection)

    @Slot()
    def _encode_pending(self) -> None:
        with self._lock:
            img, self._out, self._out_pending = self._out, None, False
        if img is None or self.net is None:
            return
        if self._encoder_failed:
            return
        if self._encoder is None:
            try:
                self._encoder = video.H264Encoder()
                if self._keyframe_needed:
                    self._encoder.request_keyframe()
                    self._keyframe_needed = False
            except Exception as exc:
                self._encoder_failed = True
                self.notice.emit("camera", f"H.264 encoder unavailable: {exc}")
                return
        try:
            access_unit = self._encoder.encode(img)
        except Exception as exc:
            self._encoder_failed = True
            self.notice.emit("camera", f"H.264 encoding failed: {exc}")
            self.stats["bad_video"] += 1
            return
        if access_unit is not None:
            self.stats["encoded"] += 1
            self.net.send_video(access_unit)

    @Slot()
    def _request_keyframe(self) -> None:
        if self._encoder is None:
            self._keyframe_needed = True
        else:
            self._encoder.request_keyframe()

    # -- test hooks ------------------------------------------------------------------------------
    @property
    def buffered_audio_samples(self) -> int:
        return len(self.jitter)


class SessionHandle(QObject):
    """Owns the worker QThread of a MediaSession. `close()` is asynchronous; `join()` blocks."""
    finished = Signal()

    def __init__(self, session: MediaSession, parent: QObject | None = None):
        super().__init__(parent)
        self.session = session
        self.thread = QThread()
        self.thread.setObjectName(f"call-media-{session.call_id}")
        session.moveToThread(self.thread)
        self.thread.started.connect(session.start)
        session.stopped.connect(self.thread.quit)
        self.thread.finished.connect(self._done)
        self._closing = False
        self.thread.start()

    def close(self) -> None:
        if not self._closing:
            self._closing = True
            self.session.requestStop.emit()

    @Slot()
    def _done(self) -> None:
        self.finished.emit()

    def join(self, timeout_ms: int = 3000) -> bool:
        self.close()
        ok = self.thread.wait(timeout_ms)
        if not ok:
            self.thread.terminate()
            self.thread.wait(500)
        return ok

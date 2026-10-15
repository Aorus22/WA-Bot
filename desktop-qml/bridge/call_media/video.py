"""Camera capture and JPEG helpers for call video (protocol: baseline JPEG per message).

* `encode_jpeg`  scale to <= 640x480 (aspect kept) and compress (quality ~0.6, well under 100 KB).
* `decode_jpeg`  parse one incoming picture and rotate it by `orientation` quarter turns.
* `CameraCapture` QCamera -> throttled (15 fps) QImages for the encoder plus the raw QVideoFrame
                  for the local self-view. Lives on the UI thread (QtMultimedia requirement); the
                  expensive scaling/JPEG work is done by the session thread.
"""
from __future__ import annotations

import sys
import time

from PySide6.QtCore import QBuffer, QIODevice, QObject, Qt, QTransform, Signal, Slot
from PySide6.QtGui import QImage
from PySide6.QtMultimedia import QCamera, QCameraDevice, QMediaCaptureSession, QMediaDevices, QVideoFrame, QVideoSink

from . import permissions

MAX_W, MAX_H = 640, 480
FPS = 15
JPEG_QUALITY = 60
MAX_JPEG_BYTES = 900 * 1024      # the server reads at most 1 MiB per message
MAX_DIMENSION = 4096             # refuse pictures bigger than this on either side


def encode_jpeg(image: QImage, quality: int = JPEG_QUALITY) -> bytes | None:
    """Scaled baseline JPEG, or None when the image is unusable/too large even at low quality."""
    if image.isNull():
        return None
    if image.width() > MAX_W or image.height() > MAX_H:
        image = image.scaled(MAX_W, MAX_H, Qt.AspectRatioMode.KeepAspectRatio, Qt.TransformationMode.SmoothTransformation)
    if image.format() not in (QImage.Format.Format_RGB32, QImage.Format.Format_RGB888):
        image = image.convertToFormat(QImage.Format.Format_RGB32)
    for q in (quality, 45, 30):
        buf = QBuffer()
        buf.open(QIODevice.OpenModeFlag.WriteOnly)
        if not image.save(buf, "JPEG", q):
            return None
        data = bytes(buf.data())
        if len(data) <= MAX_JPEG_BYTES:
            return data
    return None


def decode_jpeg(data: bytes, quarter_turns: int = 0) -> QImage | None:
    """One incoming picture, rotated clockwise by `quarter_turns` (0..3) so it shows upright."""
    img = QImage.fromData(data)
    if img.isNull() or img.width() > MAX_DIMENSION or img.height() > MAX_DIMENSION:
        return None
    turns = quarter_turns % 4
    if turns:
        img = img.transformed(QTransform().rotate(90 * turns))
    return img.convertToFormat(QImage.Format.Format_RGB32) if img.format() != QImage.Format.Format_RGB32 else img


def list_cameras() -> list[dict]:
    default = QMediaDevices.defaultVideoInput().id()
    return [{"id": bytes(d.id()).decode(), "name": d.description(), "isDefault": d.id() == default}
            for d in QMediaDevices.videoInputs()]


class CameraCapture(QObject):
    """UI-thread camera. `start()`/`stop()` are idempotent."""
    frame = Signal(QImage)            # throttled, for encoding/sending
    preview = Signal(QVideoFrame)     # every frame, for the self-view sink
    notice = Signal(str, str)         # source "camera", message ("" clears)
    started = Signal()                # first frame arrived

    def __init__(self, parent: QObject | None = None):
        super().__init__(parent)
        self._device_id = ""
        self._camera: QCamera | None = None
        self._session: QMediaCaptureSession | None = None
        self._sink: QVideoSink | None = None
        self._last = 0.0
        self._active = False
        self._got_frame = False

    @property
    def active(self) -> bool:
        return self._active

    def set_device(self, device_id: str) -> None:
        if device_id != self._device_id:
            self._device_id = device_id
            if self._active:
                self.stop()
                self.start()

    def start(self) -> None:
        if self._active:
            return
        self._active = True
        self._got_frame = False
        permissions.ensure("camera", self, self._begin)

    def _begin(self, granted: bool) -> None:
        if not self._active:
            return
        if not granted:
            self.notice.emit("camera", permissions.DENIED["camera"])
            return
        devices = QMediaDevices.videoInputs()
        if not devices:
            self.notice.emit("camera", "No camera found")
            return
        dev = next((d for d in devices if bytes(d.id()).decode() == self._device_id), None) or QMediaDevices.defaultVideoInput()
        if dev.isNull():
            dev = devices[0]
        cam = QCamera(dev, self)
        cam.errorOccurred.connect(self._on_error)
        session = QMediaCaptureSession(self)
        sink = QVideoSink(self)
        sink.videoFrameChanged.connect(self._on_frame)
        session.setCamera(cam)
        session.setVideoSink(sink)
        self._camera, self._session, self._sink = cam, session, sink
        cam.start()

    def _on_error(self, _err, message: str) -> None:
        print(f"[call] camera error: {message}", file=sys.stderr)
        self.notice.emit("camera", "Camera could not be opened. Check that it is connected and allowed.")

    def _on_frame(self, vf: QVideoFrame) -> None:
        if not vf.isValid() or not self._active:
            return
        if not self._got_frame:
            self._got_frame = True
            self.notice.emit("camera", "")
            self.started.emit()
        self.preview.emit(vf)
        now = time.monotonic()
        if now - self._last < 1.0 / FPS:
            return
        self._last = now
        img = vf.toImage()
        if not img.isNull():
            self.frame.emit(img)

    def stop(self) -> None:
        self._active = False
        cam, session, sink = self._camera, self._session, self._sink
        self._camera = self._session = self._sink = None
        if cam is not None:
            cam.stop()
        if session is not None:
            session.setCamera(None)
            session.setVideoSink(None)
            session.deleteLater()
        if sink is not None:
            sink.deleteLater()
        if cam is not None:
            cam.deleteLater()
        self.notice.emit("camera", "")

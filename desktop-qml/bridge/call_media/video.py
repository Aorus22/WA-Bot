"""Camera capture and H.264 Annex-B access-unit codecs for call video.

Encoding and decoding run on the media session's worker thread. Camera capture
stays on the UI thread as required by Qt Multimedia and only offers a bounded,
latest-frame slot to that worker.
"""
from __future__ import annotations

import math
import sys
import time

from PySide6.QtCore import QObject, Qt, Signal, Slot
from PySide6.QtGui import QImage
from PySide6.QtMultimedia import QCamera, QCameraDevice, QMediaCaptureSession, QMediaDevices, QVideoFrame, QVideoSink

from . import permissions
from . import frame as wire

MAX_W, MAX_H = 640, 480
FPS = 15
BIT_RATE = 600_000
MAX_ACCESS_UNIT_BYTES = wire.MAX_FRAME_BYTES - 3
MAX_DIMENSION = 4096             # refuse pictures bigger than this on either side


def _fit_even(image: QImage) -> QImage | None:
    if image.isNull() or image.width() > MAX_DIMENSION or image.height() > MAX_DIMENSION:
        return None
    scale = min(1.0, MAX_W / image.width(), MAX_H / image.height())
    width = max(2, math.floor(image.width() * scale) & ~1)
    height = max(2, math.floor(image.height() * scale) & ~1)
    if width == image.width() and height == image.height():
        return image
    return image.scaled(width, height, Qt.AspectRatioMode.IgnoreAspectRatio,
                        Qt.TransformationMode.SmoothTransformation)


class H264Encoder:
    """GStreamer/OpenH264 encoder producing one baseline Annex-B access unit per frame."""

    def __init__(self):
        self._Gst, self._GstVideo = _gstreamer()
        self._pipeline = None
        self._source = None
        self._sink = None
        self._size: tuple[int, int] | None = None
        self._pts = 0
        self._force_keyframe = True
        self._keyframe_count = 0

    def request_keyframe(self) -> None:
        self._force_keyframe = True

    def _open(self, width: int, height: int) -> None:
        self.close()
        Gst = self._Gst
        pipeline = Gst.parse_launch(
            "appsrc name=source is-live=true format=time block=true "
            "! videoconvert ! video/x-raw,format=I420 "
            f"! openh264enc bitrate={BIT_RATE} gop-size={FPS * 2} "
            "complexity=low rate-control=bitrate multi-thread=1 enable-frame-skip=false "
            "! video/x-h264,stream-format=byte-stream,alignment=au,profile=baseline "
            "! appsink name=sink sync=false max-buffers=1 drop=true"
        )
        source, sink = pipeline.get_by_name("source"), pipeline.get_by_name("sink")
        source.set_property("caps", Gst.Caps.from_string(
            f"video/x-raw,format=RGB,width={width},height={height},framerate={FPS}/1"))
        source.set_property("max-bytes", width * height * 3)
        pipeline.set_state(Gst.State.PLAYING)
        self._pipeline, self._source, self._sink = pipeline, source, sink
        self._size = (width, height)
        self._pts = 0
        self._force_keyframe = True

    def encode(self, image: QImage) -> bytes | None:
        fitted = _fit_even(image)
        if fitted is None:
            return None
        width, height = fitted.width(), fitted.height()
        if self._pipeline is None or self._size != (width, height):
            self._open(width, height)

        rgb = fitted.convertToFormat(QImage.Format.Format_RGB888)
        rgb_bytes = bytes(rgb.constBits()[:rgb.sizeInBytes()])
        row_bytes, source_stride = width * 3, rgb.bytesPerLine()
        packed = bytearray(row_bytes * height)
        for row in range(height):
            source = row * source_stride
            target = row * row_bytes
            packed[target:target + row_bytes] = rgb_bytes[source:source + row_bytes]
        if self._force_keyframe:
            event = self._GstVideo.video_event_new_upstream_force_key_unit(
                self._Gst.CLOCK_TIME_NONE, True, self._keyframe_count)
            self._sink.send_event(event)
            self._keyframe_count += 1

        buffer = self._Gst.Buffer.new_allocate(None, len(packed), None)
        buffer.fill(0, packed)
        buffer.pts = self._pts * self._Gst.SECOND // FPS
        buffer.dts = buffer.pts
        buffer.duration = self._Gst.SECOND // FPS
        self._pts += 1
        flow = self._source.emit("push-buffer", buffer)
        if flow != self._Gst.FlowReturn.OK:
            raise RuntimeError(f"GStreamer rejected camera frame: {flow.value_nick}")

        sample = self._sink.emit("try-pull-sample", self._Gst.SECOND // 10)
        if sample is None:
            error = _pipeline_error(self._pipeline, self._Gst)
            if error:
                raise RuntimeError(error)
            return None
        access_unit = _sample_bytes(sample, self._Gst)
        self._force_keyframe = False
        if len(access_unit) > MAX_ACCESS_UNIT_BYTES or not wire.is_h264_annexb(access_unit):
            return None
        return access_unit

    def close(self) -> None:
        if self._pipeline is not None:
            self._pipeline.set_state(self._Gst.State.NULL)
        self._pipeline = self._source = self._sink = None
        self._size = None


class H264Decoder:
    """Stateful OpenH264 decoder for the server's per-message Annex-B access units."""

    def __init__(self):
        self._Gst, self._GstVideo = _gstreamer()
        Gst = self._Gst
        self._pipeline = Gst.parse_launch(
            "appsrc name=source is-live=true format=time block=true "
            "caps=\"video/x-h264,stream-format=byte-stream,alignment=au\" "
            "! h264parse ! openh264dec ! videoconvert ! video/x-raw,format=RGB "
            "! appsink name=sink sync=false max-buffers=1 drop=true"
        )
        self._source = self._pipeline.get_by_name("source")
        self._sink = self._pipeline.get_by_name("sink")
        self._pipeline.set_state(Gst.State.PLAYING)
        self._pts = 0

    def decode(self, access_unit: bytes) -> QImage | None:
        if (not wire.is_h264_annexb(access_unit)
                or len(access_unit) > MAX_ACCESS_UNIT_BYTES):
            return None
        buffer = self._Gst.Buffer.new_allocate(None, len(access_unit), None)
        buffer.fill(0, access_unit)
        buffer.pts = self._pts * self._Gst.SECOND // FPS
        buffer.dts = buffer.pts
        buffer.duration = self._Gst.SECOND // FPS
        self._pts += 1
        flow = self._source.emit("push-buffer", buffer)
        if flow != self._Gst.FlowReturn.OK:
            raise RuntimeError(f"GStreamer rejected remote video frame: {flow.value_nick}")
        sample = self._sink.emit("try-pull-sample", self._Gst.SECOND // 10)
        if sample is None:
            error = _pipeline_error(self._pipeline, self._Gst)
            if error:
                raise RuntimeError(error)
            return None
        caps = sample.get_caps()
        info = self._GstVideo.VideoInfo.new_from_caps(caps)
        if info.width > MAX_DIMENSION or info.height > MAX_DIMENSION:
            return None
        data = _sample_bytes(sample, self._Gst)
        image = QImage(data, info.width, info.height, info.stride[0],
                       QImage.Format.Format_RGB888).copy()
        return image if not image.isNull() else None

    def close(self) -> None:
        if self._pipeline is not None:
            self._pipeline.set_state(self._Gst.State.NULL)
        self._pipeline = self._source = self._sink = None


def _gstreamer():
    """Load the Linux GStreamer runtime and require its baseline OpenH264 elements."""
    try:
        import gi

        gi.require_version("Gst", "1.0")
        gi.require_version("GstVideo", "1.0")
        from gi.repository import Gst, GstVideo
    except (ImportError, ValueError) as exc:
        raise RuntimeError("GStreamer with OpenH264 support is required for call video") from exc
    if not Gst.is_initialized():
        Gst.init(None)
    missing = [name for name in ("openh264enc", "openh264dec", "h264parse")
               if Gst.ElementFactory.find(name) is None]
    if missing:
        raise RuntimeError("GStreamer plugins missing: " + ", ".join(missing))
    return Gst, GstVideo


def _sample_bytes(sample, Gst) -> bytes:
    buffer = sample.get_buffer()
    mapped_ok, mapped = buffer.map(Gst.MapFlags.READ)
    if not mapped_ok:
        raise RuntimeError("GStreamer could not map a media buffer")
    try:
        return bytes(mapped.data)
    finally:
        buffer.unmap(mapped)


def _pipeline_error(pipeline, Gst) -> str:
    if pipeline is None:
        return ""
    message = pipeline.get_bus().pop_filtered(Gst.MessageType.ERROR)
    if message is None:
        return ""
    error, debug = message.parse_error()
    return str(error) + (f" ({debug})" if debug else "")


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

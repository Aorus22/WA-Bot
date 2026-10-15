"""Synthetic frame/audio sources for tests, screenshots and headless machines (no hardware).
`WABOT_CALL_SYNTH=1` makes the call controller use them instead of QCamera, which is also how the
offscreen screenshots show live video tiles."""
from __future__ import annotations

import math

from PySide6.QtCore import QObject, QPointF, QRectF, Qt, QTimer, Signal
from PySide6.QtGui import QColor, QFont, QImage, QLinearGradient, QPainter

from .video import FPS


def make_frame(n: int, w: int = 640, h: int = 480, label: str = "", hue: int = 200) -> QImage:
    """A moving gradient with a bouncing disc; `n` is the frame counter."""
    img = QImage(w, h, QImage.Format.Format_RGB32)
    p = QPainter(img)
    g = QLinearGradient(QPointF(0, 0), QPointF(w, h))
    g.setColorAt(0, QColor.fromHsv((hue + n) % 360, 160, 90))
    g.setColorAt(1, QColor.fromHsv((hue + 60 + n) % 360, 200, 200))
    p.fillRect(QRectF(0, 0, w, h), g)
    p.setRenderHint(QPainter.RenderHint.Antialiasing)
    cx = w / 2 + math.sin(n / 12) * w / 3
    cy = h / 2 + math.cos(n / 9) * h / 4
    p.setBrush(QColor(255, 255, 255, 200))
    p.setPen(Qt.PenStyle.NoPen)
    p.drawEllipse(QPointF(cx, cy), h / 8, h / 8)
    if label:
        f = QFont()
        f.setPixelSize(max(12, h // 14))
        p.setFont(f)
        p.setPen(QColor("white"))
        p.drawText(QRectF(12, 8, w - 24, h / 6), Qt.AlignmentFlag.AlignLeft, f"{label} #{n}")
    p.end()
    return img


class SyntheticSource(QObject):
    """Emits frames at 15 fps; usable wherever a camera/remote picture source is expected."""
    frame = Signal(QImage)

    def __init__(self, w: int = 640, h: int = 480, label: str = "", hue: int = 200, parent=None):
        super().__init__(parent)
        self._n, self._w, self._h, self._label, self._hue = 0, w, h, label, hue
        self._timer = QTimer(self)
        self._timer.setInterval(int(1000 / FPS))
        self._timer.timeout.connect(self._tick)

    def start(self) -> None:
        self._timer.start()

    def stop(self) -> None:
        self._timer.stop()

    def _tick(self) -> None:
        self._n += 1
        self.frame.emit(make_frame(self._n, self._w, self._h, self._label, self._hue))

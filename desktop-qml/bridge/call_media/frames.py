"""Thread-safe latest-frame provider for local and remote call video."""
from __future__ import annotations

from threading import Lock

from PySide6.QtCore import QSize, Qt
from PySide6.QtGui import QImage
from PySide6.QtQuick import QQuickImageProvider


class CallFrameProvider(QQuickImageProvider):
    """QML URLs use `image://callframes/remote?rev=N` and `.../local?rev=N`."""

    def __init__(self):
        super().__init__(QQuickImageProvider.ImageType.Image)
        self._lock = Lock()
        self._images: dict[str, QImage] = {}

    def set_frame(self, key: str, image: QImage | None) -> None:
        with self._lock:
            if image is None or image.isNull():
                self._images.pop(key, None)
            else:
                self._images[key] = image.copy()

    def requestImage(self, id: str, size: QSize, requestedSize: QSize):  # noqa: N802
        key = id.partition("?")[0]
        with self._lock:
            image = self._images.get(key)
            image = image.copy() if image is not None else QImage()
        if image.isNull():
            image = QImage(1, 1, QImage.Format.Format_ARGB32_Premultiplied)
            image.fill(Qt.GlobalColor.transparent)
        if requestedSize.isValid() and requestedSize.width() > 0 and requestedSize.height() > 0:
            image = image.scaled(requestedSize, Qt.AspectRatioMode.KeepAspectRatio, Qt.TransformationMode.SmoothTransformation)
        size.setWidth(image.width())
        size.setHeight(image.height())
        return image

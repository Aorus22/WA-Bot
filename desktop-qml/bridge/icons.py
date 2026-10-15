"""`image://icons/<name>?c=<#rrggbb>` — Lucide SVGs recoloured at load time
(SVGs use `currentColor`). Crisp at any size because the SVG is rasterised at the
requested size. Add an icon by dropping `<name>.svg` into qml/assets/icons."""
from __future__ import annotations

from pathlib import Path
from urllib.parse import parse_qs

from PySide6.QtCore import QByteArray, QSize, Qt
from PySide6.QtGui import QColor, QImage, QPainter
from PySide6.QtQuick import QQuickImageProvider
from PySide6.QtSvg import QSvgRenderer

ICON_DIR = Path(__file__).resolve().parent.parent / "qml" / "assets" / "icons"


class IconProvider(QQuickImageProvider):
    def __init__(self):
        super().__init__(QQuickImageProvider.ImageType.Image)
        self._cache: dict[str, str] = {}

    def _svg(self, name: str) -> str | None:
        if name not in self._cache:
            p = ICON_DIR / f"{name}.svg"
            self._cache[name] = p.read_text() if p.is_file() and "/" not in name else ""
        return self._cache[name] or None

    def requestImage(self, id: str, size: QSize, requestedSize: QSize):  # noqa: N802
        name, _, query = id.partition("?")
        color = parse_qs(query).get("c", ["#ffffff"])[0]
        svg = self._svg(name)
        side = max(requestedSize.width(), requestedSize.height(), 0) or 24
        img = QImage(side, side, QImage.Format.Format_ARGB32_Premultiplied)
        img.fill(Qt.GlobalColor.transparent)
        if svg:
            r = QSvgRenderer(QByteArray(svg.replace("currentColor", QColor(color).name(QColor.NameFormat.HexRgb)).encode()))
            p = QPainter(img)
            p.setRenderHint(QPainter.RenderHint.Antialiasing)
            r.render(p)
            p.end()
        size.setWidth(side)
        size.setHeight(side)
        return img

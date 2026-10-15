"""WA Bot desktop (Qt Quick/QML). Run: `python main.py [--url URL | --port N]`.
QML edits need no build step: just restart (or use `scripts/dev.sh`)."""
from __future__ import annotations

import sys
from pathlib import Path

from PySide6.QtCore import QUrl
from PySide6.QtGui import QGuiApplication
from PySide6.QtQml import QQmlApplicationEngine

from bridge.api import Api
from bridge.backend import BackendController
from bridge.events import EventHub

ROOT = Path(__file__).resolve().parent


def main(argv: list[str]) -> int:
    app = QGuiApplication(argv)
    app.setApplicationName("wa-bot")
    app.setDesktopFileName("wa-bot")

    backend = BackendController(argv[1:])
    api = Api(backend)
    hub = EventHub(backend)
    app.aboutToQuit.connect(hub.stop)
    app.aboutToQuit.connect(backend.shutdown)  # owner-only: attached backends stay up

    engine = QQmlApplicationEngine()
    ctx = engine.rootContext()
    ctx.setContextProperty("backend", backend)
    ctx.setContextProperty("api", api)
    ctx.setContextProperty("events", hub)
    engine.load(QUrl.fromLocalFile(str(ROOT / "qml" / "Main.qml")))
    if not engine.rootObjects():
        return 1
    backend.start()
    hub.start()
    return app.exec()


if __name__ == "__main__":
    sys.exit(main(sys.argv))

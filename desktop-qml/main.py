"""WA Bot desktop (Qt Quick/QML). Run: `python main.py [--url URL | --port N]`.
QML edits need no build step: just restart (or use `scripts/dev.sh`)."""
from __future__ import annotations

import os
import sys
from pathlib import Path

from PySide6.QtCore import QUrl
from PySide6.QtGui import QFontDatabase, QGuiApplication
from PySide6.QtQml import QQmlApplicationEngine
from PySide6.QtQuickControls2 import QQuickStyle

from bridge.api import Api
from bridge.appstate import AppState
from bridge.backend import BackendController
from bridge.events import EventHub
from bridge.icons import IconProvider
from bridge.registry import Services, register_all
from bridge.settings import Settings
from bridge.theme import ThemeManager
from bridge.toast import ToastBus

ROOT = Path(__file__).resolve().parent
QML = ROOT / "qml"


def build(argv: list[str], qt_app: QGuiApplication, settings: Settings | None = None):
    """Create services and the QML engine (not yet loaded). Used by main() and tests."""
    backend = BackendController(argv[1:])
    api = Api(backend)
    hub = EventHub(backend)
    settings = settings or Settings()
    services = Services(
        backend=backend, api=api, hub=hub, settings=settings,
        theme=ThemeManager(settings), app=AppState(hub), qt_app=qt_app, toast=ToastBus(),
    )
    qt_app.aboutToQuit.connect(hub.stop)
    qt_app.aboutToQuit.connect(backend.shutdown)  # owner-only: attached backends stay up

    engine = QQmlApplicationEngine()
    services.engine = engine
    engine.warnings.connect(lambda ws: [print(f"[qml] {w.toString()}", file=sys.stderr) for w in ws])
    engine.addImportPath(str(QML))
    engine.addImageProvider("icons", IconProvider())
    failed = register_all(services)
    if failed:
        print(f"[app] feature modules failed to load: {', '.join(failed)}", file=sys.stderr)
    return services, engine


def main(argv: list[str]) -> int:
    QQuickStyle.setStyle("Basic")  # unstyled controls: every look comes from our theme
    app = QGuiApplication(argv)
    app.setApplicationName("wa-bot")
    app.setDesktopFileName("wa-bot")
    for font in (QML / "assets" / "fonts").glob("*.ttf"):
        QFontDatabase.addApplicationFont(str(font))

    services, engine = build(argv, app)
    engine.load(QUrl.fromLocalFile(str(QML / "Main.qml")))
    if not engine.rootObjects():
        return 1
    services.backend.start()
    services.hub.start()
    return app.exec()


if __name__ == "__main__":
    sys.exit(main(sys.argv))

#!/usr/bin/env python3
"""Run QML as a registry backend owner until a separate smoke test asks it to quit.

The caller supplies stop/ready marker files and an isolated WABOT_HOME. This
helper exists only for process lifecycle verification; it never scans or uses
the account QR.
"""
from __future__ import annotations

import argparse
import os
import sys
import time
from pathlib import Path

from PySide6.QtCore import QTimer, QUrl
from PySide6.QtGui import QFontDatabase
from PySide6.QtQuickControls2 import QQuickStyle
from PySide6.QtWidgets import QApplication

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from bridge.platform import LocalInstanceServer  # noqa: E402
from bridge.settings import Settings  # noqa: E402
from main import QML, build  # noqa: E402


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--ready-file", required=True)
    parser.add_argument("--stop-file", required=True)
    parser.add_argument("--timeout", type=float, default=45.0)
    args = parser.parse_args(argv)

    QQuickStyle.setStyle("Basic")
    app = QApplication(["qml-owner-smoke"])
    app.setApplicationName("wa-bot-owner-smoke")
    for font in (QML / "assets" / "fonts").glob("*.ttf"):
        QFontDatabase.addApplicationFont(str(font))

    instance = LocalInstanceServer([], name=f"wa-bot-owner-smoke-{os.getpid()}")
    services, engine = build(["qml-owner-smoke"], app, Settings(), single_instance=instance)
    engine.load(QUrl.fromLocalFile(str(QML / "Main.qml")))
    if not engine.rootObjects():
        return 1
    services.exposed["Platform"].attachWindow()
    services.backend.start()
    services.hub.start()

    ready_file, stop_file = Path(args.ready_file), Path(args.stop_file)
    deadline = time.monotonic() + args.timeout
    exit_code = 0
    timer = QTimer()
    timer.setInterval(50)

    def poll() -> None:
        nonlocal exit_code
        if services.backend.state == "ready" and services.hub.connected and not ready_file.exists():
            ready_file.write_text(services.backend.apiUrl)
        if stop_file.exists():
            services.exposed["Platform"].quit()
        elif time.monotonic() >= deadline:
            exit_code = 2
            services.exposed["Platform"].quit()

    timer.timeout.connect(poll)
    timer.start()
    result = app.exec()
    timer.stop()
    services.backend.shutdown()
    return exit_code or result


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

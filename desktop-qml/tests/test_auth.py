"""Login and QR state flow against an unlinked local mock backend."""
from __future__ import annotations

import sys
import json
from pathlib import Path

from PySide6.QtCore import QCoreApplication, QEventLoop, QTimer

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from mock_backend import MockBackend  # noqa: E402
import mock_auth  # noqa: E402
from bridge.api import Api  # noqa: E402
from bridge.appstate import AppState  # noqa: E402
from bridge.auth import Auth, QR_FETCH_ERROR  # noqa: E402
from bridge.backend import BackendController  # noqa: E402
from bridge.events import EventHub  # noqa: E402
from bridge.registry import Services  # noqa: E402
from bridge.toast import ToastBus  # noqa: E402

_RIGS = []


def spin(cond, ms=5000):
    loop = QEventLoop()
    timer = QTimer(loop)
    timer.setInterval(20)
    timer.timeout.connect(lambda: loop.quit() if cond() else None)
    timeout = QTimer(loop)
    timeout.setSingleShot(True)
    timeout.timeout.connect(loop.quit)
    timeout.start(ms)
    timer.start()
    loop.exec()
    timer.stop()
    timeout.stop()
    return cond()


def settle(ms=200):
    spin(lambda: False, ms)


class Rig:
    def __init__(self):
        self.app = QCoreApplication.instance() or QCoreApplication([])
        self.mock = MockBackend(areas=["auth"]).start()
        self.backend = BackendController(["--url", self.mock.url])
        self.hub, self.api = EventHub(self.backend), Api(self.backend)
        self.state, self.toast = AppState(self.hub), ToastBus()
        self.toasts: list[tuple[str, str]] = []
        self.toast.shown.connect(lambda _id, message, kind: self.toasts.append((kind, message)))
        self.services = Services(backend=self.backend, api=self.api, hub=self.hub, settings=None, theme=None,
                                 app=self.state, qt_app=self.app, toast=self.toast)
        self.auth = Auth(self.services, self.state)
        _RIGS.append(self)
        self.backend.start()

    def close(self):
        settle()
        self.hub.stop()
        self.backend.shutdown()
        self.mock.stop()


def test_unlinked_status_renders_qr_and_ws_auth_clears_it():
    r = Rig()
    try:
        assert spin(lambda: r.backend.state == "ready" and r.auth.status == "unauthenticated" and r.auth.qr == mock_auth.QR_A)
        assert r.auth.qr == mock_auth.QR_A
        assert r.auth.qrSource.startswith("image://qr/") and not r.auth.qrInvalid
        r.auth.setTab("phone")
        assert r.auth.activeTab == "phone"
        r.auth.handle_event("qr_code", json.dumps({"code": "bad"}))
        assert r.auth.qr == "bad" and r.auth.qrSource.startswith("image://qr/")
        r.auth.handle_event("auth_success", json.dumps({"phone": "628123456789"}))
        assert r.auth.status == "authenticated" and r.auth.phone == "628123456789"
        assert r.auth.qr == "" and r.state.auth == "authenticated"
    finally:
        r.close()


def test_qr_refresh_error_and_logout_failure_are_visible():
    r = Rig()
    try:
        assert spin(lambda: r.backend.state == "ready" and r.auth.status == "unauthenticated" and r.auth.qr == mock_auth.QR_A)
        r.mock.state["qr"] = ""
        r.auth.refreshQr()
        assert spin(lambda: r.auth.qrError == QR_FETCH_ERROR)
        r.mock.state["logout_fail"] = True
        failed = []
        r.auth.loggedOut.connect(failed.append)
        r.auth.logout()
        assert spin(lambda: failed)
        assert failed == [False] and r.auth.status == "unauthenticated"
        assert any(kind == "error" and "Logout failed" in msg for kind, msg in r.toasts)
    finally:
        r.close()

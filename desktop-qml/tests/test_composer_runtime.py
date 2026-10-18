"""Composer dialogue and text actions use the existing REST contract against the mock server."""
from __future__ import annotations

import sys
from pathlib import Path

from PySide6.QtCore import QCoreApplication, QEventLoop, QTimer

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from mock_backend import MockBackend  # noqa: E402
from mock_common import BUDI  # noqa: E402
from bridge.api import Api  # noqa: E402
from bridge.appstate import AppState  # noqa: E402
from bridge.backend import BackendController  # noqa: E402
from bridge.composer import Composer  # noqa: E402
from bridge.events import EventHub  # noqa: E402
from bridge.registry import Services  # noqa: E402
from bridge.settings import Settings  # noqa: E402
from bridge.toast import ToastBus  # noqa: E402

_RIGS = []

def spin(cond, ms=6000):
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


def settle(ms=250):
    spin(lambda: False, ms)


def test_text_poll_location_contact_send_shapes(tmp_path):
    app = QCoreApplication.instance() or QCoreApplication([])
    mock = MockBackend(areas=["chats", "composer"]).start()
    backend = BackendController(["--url", mock.url])
    hub = EventHub(backend)
    api = Api(backend)
    state = AppState(hub)
    state.openChat(BUDI)
    toast = ToastBus()
    services = Services(backend=backend, api=api, hub=hub, settings=Settings(tmp_path / "settings.json"),
                        theme=None, app=state, qt_app=app, toast=toast)
    composer = Composer(services)
    _RIGS.append((composer, services, backend, hub, api, state))
    backend.start()
    try:
        assert spin(lambda: backend.state == "ready")
        assert composer.sendPoll("When?", ["Morning", "Afternoon"], False) == ""
        assert spin(lambda: not composer.busy and any(c["name"] == "poll" for c in mock.state["composer"]["calls"]))
        assert composer.sendLocation("-6.2", "106.8", "Jakarta", "Indonesia", False) == ""
        assert spin(lambda: not composer.busy and any(c["name"] == "location" for c in mock.state["composer"]["calls"]))
        assert composer.sendContact("Sari", "+6282222222222") == ""
        assert spin(lambda: not composer.busy and any(c["name"] == "contact" for c in mock.state["composer"]["calls"]))
        composer.setText("Hello from QML")
        composer.send()
        assert spin(lambda: len(mock.state["composer"]["calls"]) >= 4)
        calls = mock.state["composer"]["calls"]
        by_name = {call["name"]: call for call in calls}
        assert by_name["poll"]["body"] == {"question": "When?", "options": ["Morning", "Afternoon"], "multiSelect": False}
        assert by_name["location"]["body"]["latitude"] == -6.2
        assert by_name["contact"]["body"]["displayName"] == "Sari"
        assert by_name["send-message"]["body"]["message"] == "Hello from QML"
    finally:
        settle()
        hub.stop()
        backend.shutdown()
        mock.stop()

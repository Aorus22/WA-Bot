"""Call history and signalling through the QML bridge against a local mock."""
from __future__ import annotations

import sys
from pathlib import Path

from PySide6.QtCore import QCoreApplication, QEventLoop, QTimer

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from mock_backend import MockBackend  # noqa: E402
from bridge.api import Api  # noqa: E402
from bridge.appstate import AppState  # noqa: E402
from bridge.backend import BackendController  # noqa: E402
from bridge.calls import Calls  # noqa: E402
from bridge.events import EventHub  # noqa: E402
from bridge.registry import Services  # noqa: E402
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


class FrameStore:
    def set_frame(self, _key, _image):
        pass


class Rig:
    def __init__(self):
        self.app = QCoreApplication.instance() or QCoreApplication([])
        self.mock = MockBackend(areas=["calls"]).start()
        self.backend = BackendController(["--url", self.mock.url])
        self.api = Api(self.backend)
        self.hub = EventHub(self.backend)
        self.app_state = AppState(self.hub)
        self.toast = ToastBus()
        self.services = Services(backend=self.backend, api=self.api, hub=self.hub, settings=None,
                                 theme=None, app=self.app_state, qt_app=self.app, toast=self.toast,
                                 call_frames=FrameStore())
        self.calls = Calls(self.services)
        _RIGS.append(self)
        self.backend.start()
        assert spin(lambda: self.backend.state == "ready")
        self.hub.start()
        assert spin(lambda: self.hub.connected)
        assert self.mock.wait_for_client()
        assert spin(lambda: not self.calls.loading)

    def close(self):
        spin(lambda: False, 250)  # drain the REST callbacks scheduled by call actions
        self.hub.stop()
        self.backend.shutdown()
        self.mock.stop()


def test_history_filter_search_and_contact_display():
    r = Rig()
    try:
        rows = r.calls.history.rows()
        assert len(rows) == 3
        assert rows[0]["id"] == "c1" and rows[0]["missed"] is True
        assert rows[0]["displayName"] == "6281111111111"
        assert rows[0]["typeLabel"] == "Audio"
        r.calls.setFilter("missed")
        assert [row["id"] for row in r.calls.history.rows()] == ["c1", "c3"]
        r.calls.setQuery("628222")
        assert r.calls.history.count == 0
        r.calls.setFilter("all")
        assert [row["id"] for row in r.calls.history.rows()] == ["c2"]
    finally:
        r.close()


def test_history_name_matches_gpui_even_if_chat_contact_is_resolved():
    r = Rig()
    try:
        class Contacts:
            @staticmethod
            def get(_target):
                return {"name": "Budi Santoso"}

        r.calls._chats = Contacts()
        row = r.calls._row(r.mock.state["calls_history"][0])
        assert row["displayName"] == "6281111111111"
    finally:
        r.close()


def test_callback_reuses_call_type_and_call_actions_refresh_state():
    r = Rig()
    try:
        r.calls.callBack("c2")
        assert spin(lambda: r.calls.activeCall.get("id") == "mock-call-1"), r.mock.requests
        assert r.calls.hasActiveCall
        method, path, body = next(x for x in r.mock.requests if x[0] == "POST" and x[1] == "/api/calls")
        assert method == "POST" and path == "/api/calls"
        assert b'"type": "video"' in body
        r.calls.hangupActive()
        assert spin(lambda: not r.calls.hasActiveCall)
        assert ("hangup", "mock-call-1") in r.mock.state["call_actions"]
    finally:
        r.close()


def test_incoming_call_answer_and_video_controls_use_backend_contract():
    r = Rig()
    try:
        incoming = {
            "id": "in-1", "status": "ringing", "type": "audio", "direction": "incoming",
            "source": "incoming", "media_mode": "tts", "target": "6281111111111@s.whatsapp.net",
            "group_jid": None, "participants": None, "started_at": 1_800_000_000,
            "answered_at": None, "video_enabled": False, "remote_video_enabled": False,
        }
        r.mock.state["active_call"] = incoming
        import json
        r.hub.event.emit("call.incoming", json.dumps(incoming))
        assert spin(lambda: r.calls.incomingCall.get("id") == "in-1")
        r.calls.answer("in-1")
        assert spin(lambda: r.calls.activeCall.get("status") == "connected")
        r.calls.startVideo("in-1")
        assert spin(lambda: r.mock.state["active_call"].get("video_enabled") is True)
        assert ("answer", "in-1") in r.mock.state["call_actions"]
        assert ("video_start", "in-1") in r.mock.state["call_actions"]
    finally:
        r.close()


def test_group_call_and_participant_add_follow_rest_shapes():
    r = Rig()
    try:
        r.calls.startGroupCall("120363000000000001@g.us", ["6281111111111@s.whatsapp.net"], "group_audio")
        assert spin(lambda: r.calls.activeCall.get("id") == "mock-group-call-1")
        r.calls.addParticipants("mock-group-call-1", ["6282222222222@s.whatsapp.net", "6282222222222@s.whatsapp.net"])
        assert spin(lambda: len(r.mock.state["active_call"].get("participants", [])) == 2)
        assert ("participants", "mock-group-call-1", ["6282222222222@s.whatsapp.net"]) in r.mock.state["call_actions"]
    finally:
        r.close()

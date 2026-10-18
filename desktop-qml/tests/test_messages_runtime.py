"""Message bridge startup/reconnect behavior against the local mock backend."""
from __future__ import annotations

import json
import sys
from pathlib import Path

from PySide6.QtCore import QCoreApplication, QEventLoop, QTimer

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from mock_backend import MockBackend  # noqa: E402
from mock_chats import BUDI  # noqa: E402
from bridge.api import Api  # noqa: E402
from bridge.appstate import AppState  # noqa: E402
from bridge.backend import BackendController  # noqa: E402
from bridge.events import EventHub  # noqa: E402
from bridge.messages import Messages  # noqa: E402
from bridge.registry import Services  # noqa: E402
from bridge.settings import Settings  # noqa: E402
from bridge.toast import ToastBus  # noqa: E402


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


def _message_history(count: int) -> list[dict]:
    start = 1_700_000_000_000
    return [{
        "id": f"m{i:03}", "chatId": BUDI, "from": BUDI, "to": "me", "content": f"message {i}",
        "timestamp": start + i * 1000, "status": "received", "type": "text",
    } for i in range(count)]


def _message_rig(tmp_path, rows: list[dict]):
    app = QCoreApplication.instance() or QCoreApplication([])
    mock = MockBackend(areas=["chats"]).start()
    mock.state["messages"][BUDI] = rows
    backend = BackendController(["--url", mock.url])
    hub, api = EventHub(backend), Api(backend)
    app_state = AppState(hub)
    services = Services(backend=backend, api=api, hub=hub,
                        settings=Settings(tmp_path / "settings.json"), theme=None,
                        app=app_state, qt_app=app, toast=ToastBus())
    messages = Messages(services)
    backend.start()
    assert spin(lambda: backend.state == "ready")
    hub.start()
    assert spin(lambda: hub.connected)
    app_state.openChat(BUDI)
    assert spin(lambda: not messages.loading and messages.model.count > 0)
    return messages, mock, backend, hub


def test_preselected_chat_loads_after_backend_discovery(tmp_path):
    app = QCoreApplication.instance() or QCoreApplication([])
    mock = MockBackend(areas=["chats"]).start()
    backend = BackendController(["--url", mock.url])
    hub = EventHub(backend)
    api = Api(backend)
    app_state = AppState(hub)
    app_state.openChat(BUDI)  # restore/deep-link selection before URL discovery completes
    services = Services(backend=backend, api=api, hub=hub,
                        settings=Settings(tmp_path / "settings.json"), theme=None,
                        app=app_state, qt_app=app)
    messages = Messages(services)

    # Let the initial selection attempt run against the unresolved controller first.
    QTimer.singleShot(80, backend.start)
    try:
        assert spin(lambda: backend.state == "ready" and messages.model.count == 7 and not messages.loading)
        assert messages.chatId == BUDI
        assert any(method == "GET" and f"/chats/{BUDI.replace('@', '%40')}/messages" in path
                   for method, path, _body in mock.requests)
    finally:
        hub.stop()
        backend.shutdown()
        mock.stop()


def test_api_without_resolved_backend_completes_callback_and_releases_token():
    app = QCoreApplication.instance() or QCoreApplication([])
    backend = BackendController(["--url", "http://127.0.0.1:1/api"])
    api = Api(backend)
    result = []
    try:
        api.fetch("GET", "/chats", None, lambda status, body: result.append((status, body)))
        assert result == [(0, {"error": "backend not ready"})]
        assert api._py == {}
    finally:
        backend.shutdown()


def test_forward_send_posts_each_message_in_order(tmp_path):
    app = QCoreApplication.instance() or QCoreApplication([])
    mock = MockBackend(areas=["chats"]).start()
    backend = BackendController(["--url", mock.url])
    hub, api = EventHub(backend), Api(backend)
    app_state = AppState(hub)
    app_state.openChat(BUDI)
    services = Services(backend=backend, api=api, hub=hub,
                        settings=Settings(tmp_path / "settings.json"), theme=None,
                        app=app_state, qt_app=app, toast=ToastBus())
    messages = Messages(services)
    backend.start()
    finished = []
    messages.forwardFinished.connect(finished.append)
    targets = ["6282222222222@s.whatsapp.net"]
    try:
        assert spin(lambda: backend.state == "ready" and messages.model.count == 7 and not messages.loading)
        messages.forwardSend(BUDI, ["bm1", "bm2"], targets)
        assert spin(lambda: finished)
        assert finished == [True]
        assert mock.state["forwards"] == [(BUDI, "bm1", targets), (BUDI, "bm2", targets)]
    finally:
        hub.stop()
        backend.shutdown()
        mock.stop()


def test_message_pagination_and_duplicate_out_of_order_events(tmp_path):
    rows = _message_history(130)
    messages, mock, backend, hub = _message_rig(tmp_path, rows)
    try:
        assert messages.model.count == 100
        assert messages.model.rows()[0]["id"] == "m129"
        assert messages.model.rows()[-1]["id"] == "m030"
        assert messages.hasMore

        changed_latest = {**rows[129], "content": "authoritative update"}
        messages._on_event("new_message", json.dumps(changed_latest))
        messages._on_event("new_message", json.dumps(changed_latest))
        assert messages.model.count == 100
        assert messages.model.rows()[0]["content"] == "authoritative update"

        interleaved = {**rows[80], "id": "out-of-order", "timestamp": rows[80]["timestamp"] + 500}
        messages._on_event("new_message", json.dumps(interleaved))
        ordered = messages.model.rows()
        assert ordered[0]["id"] == "m129"
        assert ordered[[row["id"] for row in ordered].index("m081") + 1]["id"] == "out-of-order"
        assert [row["id"] for row in ordered].count("out-of-order") == 1

        pending = {"id": "temp-local-1", "chatId": BUDI, "content": "optimistic send", "timestamp": rows[129]["timestamp"] + 1000}
        messages.addPending(pending)
        echo = {**pending, "id": "server-echo-1", "from": "me", "to": BUDI, "status": "sent", "type": "text"}
        messages._on_event("new_message", json.dumps(echo))
        ids = [row["id"] for row in messages.model.rows()]
        assert ids.count("server-echo-1") == 1 and "temp-local-1" not in ids

        pending_ack = {**pending, "id": "temp-ack-1", "content": "ack without echo"}
        messages.addPending(pending_ack)
        messages.ackPending("temp-ack-1", "server-ack-1")
        acked = messages.model.get_row("server-ack-1")
        assert acked and not acked["pending"] and acked["status"] == "sent"
        messages.failPending("temp-ack-1", "late failure")  # old local id is already gone
        assert messages.model.contains("server-ack-1")

        messages.addPending({**pending, "id": "temp-fail-1", "content": "failed send"})
        messages.failPending("temp-fail-1", "offline")
        failed = messages.model.get_row("temp-fail-1")
        assert failed and failed["failed"] and failed["errorText"] == "offline"
        messages.discardPending("temp-fail-1")
        assert not messages.model.contains("temp-fail-1")

        # A later server page confirms the REST-acknowledged optimistic row.
        server_ack = {**pending_ack, "id": "server-ack-1", "from": "me", "to": BUDI, "status": "sent", "type": "text"}
        messages._set_messages(rows + [server_ack], has_more=False, has_next=False)
        assert "server-ack-1" not in messages._optimistic

        # A context window has newer history on its forward edge; fetch those pages in order.
        messages._set_messages(rows[30:71], has_more=True, has_next=True)
        messages.loadNewer()
        assert messages.loadingNewer
        assert spin(lambda: not messages._loading_newer and messages.model.count == 81)
        assert not messages.loadingNewer
        assert messages.hasMoreNext
        assert messages.model.rows()[0]["id"] == "m110"
        messages.loadNewer()
        assert spin(lambda: not messages._loading_newer and messages.model.count == 100)
        assert not messages.hasMoreNext
        assert messages.model.rows()[0]["id"] == "m129"
        assert messages.model.rows()[-1]["id"] == "m030"

        messages.loadOlder()
        assert spin(lambda: not messages.loadingMore and messages.model.count == 130)
        assert not messages.hasMore
        assert [row["id"] for row in messages.model.rows()] == [f"m{i:03}" for i in reversed(range(130))]

        messages._set_messages(rows[30:71], has_more=True, has_next=True)
        scrolled = []
        messages.scrollToBottomRequested.connect(lambda: scrolled.append(True))
        messages.jumpToLatest()
        assert spin(lambda: not messages.loading and messages.model.rows()[0]["id"] == "m129")
        assert not messages.hasMoreNext and scrolled == [True]
        assert any(method == "GET" and "/messages?limit=100" in path for method, path, _body in mock.requests)
        message_requests = [path for method, path, _body in mock.requests
                            if method == "GET" and "/messages?" in path]
        assert any("limit=40&after=" in path for path in message_requests)
        assert any("limit=40&before=" in path for path in message_requests)
    finally:
        hub.stop()
        backend.shutdown()
        mock.stop()


def test_message_refresh_preserves_pending_view_once_dto_metadata(tmp_path):
    rows = _message_history(7)
    messages, mock, backend, hub = _message_rig(tmp_path, rows)
    try:
        messages.addPending({
            "id": "temp-view-once-1", "chatId": BUDI, "from": "me", "to": BUDI,
            "content": "[Image]", "timestamp": rows[-1]["timestamp"] + 1000,
            "type": "image", "status": "pending",
            "extra": {"viewOnce": {"mediaType": "photo", "viewed": False}},
        })

        # A delayed backend URL change must refresh the same chat without clearing
        # the local upload that was added while discovery completed.
        messages._on_backend()
        assert spin(lambda: not messages.loading)

        # Model a chat-history response that began before the upload was staged.
        messages._set_messages(rows, has_more=False, has_next=False)

        pending = messages.model.get_row("temp-view-once-1")
        assert pending is not None
        assert pending["pending"]
        assert pending["viewOnce"] == {"viewed": False, "label": "photo"}

        # The REST ack can rekey the local row before an already-running refresh
        # returns an older snapshot. The DTO metadata survives the ID rekey.
        messages.ackPending("temp-view-once-1", "server-view-once-1")
        messages._set_messages(rows, has_more=False, has_next=False)
        acknowledged = messages.model.get_row("server-view-once-1")
        assert acknowledged is not None
        assert not acknowledged["pending"]
        assert acknowledged["viewOnce"] == {"viewed": False, "label": "photo"}

        authoritative = {
            "id": "server-view-once-1", "chatId": BUDI, "from": "me", "to": BUDI,
            "content": "[Image]", "timestamp": rows[-1]["timestamp"] + 1000,
            "type": "image", "status": "sent",
            "extra": {"viewOnce": {"mediaType": "photo", "viewed": False}},
        }
        messages._set_messages(rows + [authoritative], has_more=False, has_next=False)
        assert sum(row["id"] == "server-view-once-1" for row in messages.model.rows()) == 1
        assert "server-view-once-1" not in messages._optimistic
    finally:
        hub.stop()
        backend.shutdown()
        mock.stop()

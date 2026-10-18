"""Bridge classes (REST + WebSocket + reconnect) against one backend and three simultaneous clients."""
import json
import sys
from pathlib import Path

import pytest
from PySide6.QtCore import QCoreApplication, QEventLoop, QTimer

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from bridge import discovery as d  # noqa: E402
from bridge.api import Api  # noqa: E402
from bridge.backend import BackendController  # noqa: E402
from bridge.events import EventHub  # noqa: E402

pytestmark = pytest.mark.skipif(d.backend_binary() is None, reason="wa-bot-backend not found")


@pytest.fixture(scope="module")
def app():
    return QCoreApplication.instance() or QCoreApplication([])


def wait_for(cond, ms=10000):
    loop = QEventLoop()
    poll = QTimer(loop)
    poll.setInterval(50)
    poll.timeout.connect(lambda: loop.quit() if cond() else None)
    timeout = QTimer(loop)
    timeout.setSingleShot(True)
    timeout.timeout.connect(loop.quit)
    timeout.start(ms)
    poll.start()
    loop.exec()
    poll.stop()
    timeout.stop()
    return cond()


def test_two_clients_one_backend(app, tmp_path, monkeypatch):
    monkeypatch.setenv("WABOT_HOME", str(tmp_path))
    a = BackendController([])
    a.start()
    assert wait_for(lambda: a.state == "ready")
    assert a.owner

    b = BackendController([])  # second client: must attach, not spawn
    b.start()
    assert wait_for(lambda: b.state == "ready")
    assert not b.owner and b.apiUrl == a.apiUrl

    c = BackendController([])  # a third frontend must attach to that same owner too
    c.start()
    assert wait_for(lambda: c.state == "ready")
    assert not c.owner and c.apiUrl == a.apiUrl

    hubs = [EventHub(a), EventHub(b), EventHub(c)]
    for h in hubs:
        h.start()
    assert wait_for(lambda: all(h.connected for h in hubs))

    api = Api(c)
    got = []
    api.finished.connect(lambda tok, st, body: got.append((tok, st, json.loads(body))))
    api.request("GET", "/status", "", "s")
    assert wait_for(lambda: got)
    assert got[0][:2] == ("s", 200) and "isLoggedIn" in got[0][2]

    # attached client shutting down leaves the owner's backend alone
    b.shutdown()
    c.shutdown()
    assert d.is_wabot_backend(d.loopback_port(a.apiUrl))

    for h in hubs:
        h.stop()
    a.shutdown()
    assert not d.core_file().exists()


def test_socket_follows_backend_restart(app, tmp_path, monkeypatch):
    monkeypatch.setenv("WABOT_HOME", str(tmp_path))
    a = BackendController([])
    a.start()
    assert wait_for(lambda: a.state == "ready")
    hub = EventHub(a)
    hub.start()
    assert wait_for(lambda: hub.connected)
    first = a.apiUrl

    a._owned.proc.kill()  # crash the owned backend
    assert wait_for(lambda: not hub.connected, 5000)
    assert wait_for(lambda: hub.connected and a.owner, 20000)  # respawned, socket re-established
    assert a.apiUrl.startswith("http://127.0.0.1:")
    hub.stop()
    a.shutdown()
    assert first  # url may legitimately differ (ephemeral port)


def test_attached_client_takes_ownership_after_owner_exits(app, tmp_path, monkeypatch):
    monkeypatch.setenv("WABOT_HOME", str(tmp_path))
    owner = BackendController([])
    owner.start()
    assert wait_for(lambda: owner.state == "ready")
    assert owner.owner

    attached = BackendController([])
    attached.start()
    assert wait_for(lambda: attached.state == "ready")
    assert not attached.owner and attached.apiUrl == owner.apiUrl

    hub = EventHub(attached)
    hub.start()
    assert wait_for(lambda: hub.connected)
    try:
        owner.shutdown()
        # The disconnected socket triggers re-resolution; the survivor starts
        # a replacement backend and owns it from then on.
        assert wait_for(lambda: hub.connected and attached.owner, 20000)
    finally:
        hub.stop()
        attached.shutdown()
        owner.shutdown()
    assert not d.core_file().exists()

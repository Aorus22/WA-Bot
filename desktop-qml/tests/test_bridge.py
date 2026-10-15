"""Bridge classes (REST + WebSocket + reconnect) against a real spawned backend,
including two simultaneous clients on one backend."""
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
    loop, t = QEventLoop(), QTimer()
    t.setInterval(50)
    t.timeout.connect(lambda: loop.quit() if cond() else None)
    QTimer.singleShot(ms, loop.quit)
    t.start()
    loop.exec()
    t.stop()
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

    hubs = [EventHub(a), EventHub(b)]
    for h in hubs:
        h.start()
    assert wait_for(lambda: all(h.connected for h in hubs))

    api = Api(b)
    got = []
    api.finished.connect(lambda tok, st, body: got.append((tok, st, json.loads(body))))
    api.request("GET", "/status", "", "s")
    assert wait_for(lambda: got)
    assert got[0][:2] == ("s", 200) and "isLoggedIn" in got[0][2]

    # attached client shutting down leaves the owner's backend alone
    b.shutdown()
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

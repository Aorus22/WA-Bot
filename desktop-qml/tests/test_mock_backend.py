import json
import urllib.request

from PySide6.QtCore import QCoreApplication

from mock_backend import MockBackend
from test_bridge import wait_for
from bridge.api import Api
from bridge.backend import BackendController
from bridge.events import EventHub


def test_mock_serves_rest_and_ws():
    app = QCoreApplication.instance() or QCoreApplication([])
    mb = MockBackend().start()
    try:
        with urllib.request.urlopen(mb.url + "/status") as r:
            assert json.load(r)["isLoggedIn"] is True
        be = BackendController(["--url", mb.url])
        be.start()
        assert wait_for(lambda: be.state == "ready")
        hub = EventHub(be)
        got = []
        hub.event.connect(lambda t, p: got.append((t, json.loads(p))))
        hub.start()
        assert wait_for(lambda: hub.connected)
        mb.broadcast("qr_code", {"code": "abc"})
        assert wait_for(lambda: got)
        assert got[0] == ("qr_code", {"code": "abc"})
        api = Api(be)
        res = []
        api.fetch("GET", "/status", None, lambda st, body: res.append((st, body)))
        assert wait_for(lambda: res) and res[0][0] == 200
        hub.stop()
    finally:
        mb.stop()

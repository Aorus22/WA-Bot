"""Headless check of the bridge against a real or mock backend: resolves the backend
the way the app does, asks /status and waits for the WebSocket. Read-only.
    python scripts/smoke.py [--port N | --url URL]"""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from PySide6.QtCore import QCoreApplication, QTimer  # noqa: E402

from bridge.api import Api  # noqa: E402
from bridge.backend import BackendController  # noqa: E402
from bridge.events import EventHub  # noqa: E402


def run(argv: list[str], timeout_ms: int = 8000) -> dict:
    app = QCoreApplication.instance() or QCoreApplication(["smoke"])
    backend = BackendController(argv)
    api, hub = Api(backend), EventHub(backend)
    result = {"url": "", "status": None, "ws": False, "events": []}

    def check() -> None:
        if result["status"] is not None and hub.connected:
            result["ws"] = True
            app.quit()

    backend.apiUrlChanged.connect(lambda: api.request("GET", "/status", "", "s"))
    api.finished.connect(lambda t, st, body: (result.update(status=(st, json.loads(body or "{}"))), check()))
    hub.event.connect(lambda t, p: result["events"].append(t))
    hub.connectedChanged.connect(check)
    QTimer.singleShot(timeout_ms, app.quit)
    backend.start()
    hub.start()
    app.exec()
    result["url"] = backend.apiUrl
    result["owner"] = backend.owner
    hub.stop()
    return result


if __name__ == "__main__":
    r = run(sys.argv[1:])
    print(json.dumps(r, indent=2))
    sys.exit(0 if r["ws"] and r["status"] and r["status"][0] == 200 else 1)

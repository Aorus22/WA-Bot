"""The message gallery viewer state follows the open chat and uses mock media URLs."""
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
from bridge.events import EventHub  # noqa: E402
from bridge.messages import Messages  # noqa: E402
from bridge.registry import Services  # noqa: E402
from bridge.settings import Settings  # noqa: E402

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


def test_open_close_viewer_uses_loaded_gallery(tmp_path):
    app = QCoreApplication.instance() or QCoreApplication([])
    mock = MockBackend(areas=["chats"]).start()
    backend = BackendController(["--url", mock.url])
    hub = EventHub(backend)
    api = Api(backend)
    state = AppState(hub)
    state.openChat(BUDI)
    services = Services(backend=backend, api=api, hub=hub, settings=Settings(tmp_path / "settings.json"),
                        theme=None, app=state, qt_app=app)
    messages = Messages(services)
    _RIGS.append((messages, services, backend, hub, api, state))
    backend.start()
    try:
        assert spin(lambda: backend.state == "ready" and messages.model.contains("bm1") and not messages.loading)
        gallery = messages.gallery()
        assert len(gallery) == 1 and gallery[0]["id"] == "bm1"
        messages.openViewer("bm1")
        assert messages.viewerOpen and messages.viewerCount == 1
        assert messages.viewerItem["src"].endswith("/media/b-photo-1.png")
        messages.moveViewer(1)  # navigation stays within gallery bounds
        assert messages.viewerIndex == 0

        # The Info drawer can expose historical media older than the loaded
        # conversation page; it still opens, navigates and saves through viewer state.
        historical = {
            "id": "historical-image", "chatId": BUDI, "from": BUDI, "to": "me",
            "content": "Historical photo", "timestamp": 1_600_000_000_000,
            "status": "received", "type": "image", "mediaUrl": "/media/b-photo-1.png",
        }
        assert not messages.model.contains("historical-image")
        messages.openViewerFromList(BUDI, [historical], "historical-image")
        assert messages.viewerOpen and messages.viewerItem["id"] == "historical-image"
        assert messages.viewerItem["src"].endswith("/media/b-photo-1.png")
        assert not messages.model.contains("historical-image")
        saved = tmp_path / "historical.png"
        messages.saveMedia("historical-image", str(saved))
        assert spin(saved.is_file)

        historical_video = {
            **historical, "id": "historical-video", "type": "video",
            "mediaUrl": "/media/old-video.mp4",
            "extra": {"viewOnce": {"mediaType": "video", "viewed": False}},
        }
        messages.openViewerFromList(BUDI, [historical_video], "historical-video")
        assert not messages.viewerOpen

        state.openChat("")
        assert not messages.viewerOpen
    finally:
        settle()
        hub.stop()
        backend.shutdown()
        mock.stop()

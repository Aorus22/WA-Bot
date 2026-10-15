"""JSON WebSocket (`/ws`) with reconnect. Mirrors GPUI `ws.rs`: no handshake is
needed (GPUI passes no user id); the URL is re-resolved on every reconnect and the
retry backoff is 3s. Emits raw `(type, payload-json)`; typed handling lives in
stores so QML never parses envelopes."""
from __future__ import annotations

import json

from PySide6.QtCore import Property, QObject, QTimer, QUrl, Signal, Slot
from PySide6.QtWebSockets import QWebSocket

from .backend import BackendController

RECONNECT_MS = 3000
NORMAL_CLOSE = 1000


class EventHub(QObject):
    connectedChanged = Signal()
    event = Signal(str, str)  # type, payload JSON

    def __init__(self, backend: BackendController, parent: QObject | None = None):
        super().__init__(parent)
        self._backend = backend
        self._running = False
        self._connected = False
        self._sock: QWebSocket | None = None
        self._retry = QTimer(self)
        self._retry.setSingleShot(True)
        self._retry.setInterval(RECONNECT_MS)
        self._retry.timeout.connect(self._connect)
        backend.apiUrlChanged.connect(self._url_changed)

    @Property(bool, notify=connectedChanged)
    def connected(self) -> bool:
        return self._connected

    @Slot()
    def start(self) -> None:
        self._running = True
        self._connect()

    @Slot()
    def stop(self) -> None:
        self._running = False
        self._retry.stop()
        if self._sock:
            self._sock.close()

    def _url_changed(self) -> None:
        # Backend moved: drop the old socket; the close handler reconnects.
        if self._running and self._sock and self._connected:
            self._sock.close()

    def _connect(self) -> None:
        if not self._running:
            return
        url = self._backend.ws_url()
        if not url:
            self._retry.start(500)
            return
        if self._sock:
            self._sock.deleteLater()
        sock = self._sock = QWebSocket()
        sock.connected.connect(lambda: self._set_connected(True))
        sock.disconnected.connect(self._on_disconnected)
        sock.textMessageReceived.connect(self._on_text)
        sock.open(QUrl(url))

    def _set_connected(self, value: bool) -> None:
        if value != self._connected:
            self._connected = value
            self.connectedChanged.emit()

    def _on_disconnected(self) -> None:
        self._set_connected(False)
        if self._running:
            self._backend.recheck()
            self._retry.start()

    def _on_text(self, text: str) -> None:
        try:
            msg = json.loads(text)
        except ValueError:
            return
        if isinstance(msg, dict) and isinstance(msg.get("type"), str):
            self.event.emit(msg["type"], json.dumps(msg.get("payload", {})))

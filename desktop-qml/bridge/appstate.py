"""App-wide navigation + selection state shared by QML and Python stores.

Routes mirror GPUI `router.rs`: chat, chat/<id>, status, channels, calls, triggers,
triggers/new, triggers/<id>, cron(+new/<id>), webhooks(+new/<id>/logs), documentation, settings.
"""
from __future__ import annotations

from PySide6.QtCore import Property, QObject, Signal, Slot

NEEDS_WHATSAPP = {"chat", "status", "channels", "calls"}


class AppState(QObject):
    pathChanged = Signal()
    activeChatChanged = Signal()
    authChanged = Signal()
    connectionChanged = Signal()
    windowVisibleChanged = Signal()

    def __init__(self, hub=None, parent: QObject | None = None):
        super().__init__(parent)
        self._path = "chat"
        self._active_chat = ""
        self._auth = "checking"  # checking | unauthenticated | authenticated
        self._phone = ""
        self._window_visible = True
        self._attempt = 0
        self._hub = hub
        if hub is not None:
            hub.connectedChanged.connect(self._on_conn)

    # -- navigation --------------------------------------------------------
    @Property(str, notify=pathChanged)
    def path(self) -> str:
        return self._path

    @Property(str, notify=pathChanged)
    def section(self) -> str:
        return self._path.split("/", 1)[0]

    @Property(str, notify=pathChanged)
    def param(self) -> str:
        parts = self._path.split("/", 1)
        return parts[1] if len(parts) > 1 else ""

    @Slot(str)
    def navigate(self, path: str) -> None:
        path = path.strip("/") or "chat"
        if path != self._path:
            self._path = path
            self.pathChanged.emit()
        sec, _, param = path.partition("/")
        if sec == "chat" and param and param != self._active_chat:
            self.setActiveChat(param)

    @Slot(str)
    def openChat(self, chat_id: str) -> None:
        self.setActiveChat(chat_id)
        self.navigate(f"chat/{chat_id}" if chat_id else "chat")

    # -- selected chat --------------------------------------------------------
    @Property(str, notify=activeChatChanged)
    def activeChatId(self) -> str:
        return self._active_chat

    @Slot(str)
    def setActiveChat(self, chat_id: str) -> None:
        if chat_id != self._active_chat:
            self._active_chat = chat_id
            self.activeChatChanged.emit()

    # -- account --------------------------------------------------------------
    @Property(str, notify=authChanged)
    def auth(self) -> str:
        return self._auth

    @Property(str, notify=authChanged)
    def phone(self) -> str:
        return self._phone

    def set_auth(self, status: str, phone: str | None = None) -> None:
        changed = status != self._auth or (phone is not None and phone != self._phone)
        self._auth = status
        if phone is not None:
            self._phone = phone
        if changed:
            self.authChanged.emit()

    @Property(bool, notify=authChanged)
    def unlinked(self) -> bool:
        return self._auth == "unauthenticated"

    # -- connection (WebSocket) ----------------------------------------------
    @Property(str, notify=connectionChanged)
    def connection(self) -> str:
        """connected | connecting | reconnecting"""
        if self._hub is not None and self._hub.connected:
            return "connected"
        return "reconnecting" if self._attempt else "connecting"

    @Property(int, notify=connectionChanged)
    def attempt(self) -> int:
        return self._attempt

    def _on_conn(self) -> None:
        if self._hub.connected:
            self._attempt = 0
        else:
            self._attempt += 1
        self.connectionChanged.emit()

    # -- window --------------------------------------------------------------
    @Property(bool, notify=windowVisibleChanged)
    def windowVisible(self) -> bool:
        """True while the window is shown and focused — a conversation only counts as read then."""
        return self._window_visible

    @Slot(bool)
    def setWindowVisible(self, v: bool) -> None:
        if v != self._window_visible:
            self._window_visible = v
            self.windowVisibleChanged.emit()

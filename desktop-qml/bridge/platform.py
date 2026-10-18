"""Desktop notifications, tray controls, and resident-window actions.

The bridge uses the existing GPUI notification policy in ``platform_core``. Tray support is
optional and only created when a QApplication and a desktop tray host are available; QML-only
offscreen runs stay headless.
"""
from __future__ import annotations

import json
import math
import os
import sys
import threading
import asyncio
from pathlib import Path
from typing import Any

from PySide6.QtCore import Property, QObject, QStandardPaths, QTimer, Qt, Signal, Slot
from PySide6.QtGui import QIcon
from PySide6.QtNetwork import QLocalServer, QLocalSocket
from PySide6.QtWidgets import QApplication, QMenu, QSystemTrayIcon

from . import platform_core as core


class LocalInstanceServer(QObject):
    """Per-user local socket. A second launch sends ``open`` or ``toggle`` to the resident app."""
    MAX_COMMAND_BYTES = 4096
    commandReceived = Signal(str)

    def __init__(self, argv: list[str], *, name: str | None = None,
                 popover_supported: bool = True, parent: QObject | None = None):
        super().__init__(parent)
        self.flags = core.parse_flags(argv)
        self.signal = self.flags.signal if (not self.flags.popover or popover_supported) else "open"
        self._buffers: dict[int, bytearray] = {}
        user_key = str(os.getuid()) if hasattr(os, "getuid") else os.environ.get("USERNAME", "user")
        self.name = name or f"wa-bot-desktop-{user_key}"
        if self.flags.standalone:
            self.server = None
            self._client = None
            self.is_primary = True
            return
        # Probe the resident process before creating a listener. On some Qt/Linux builds a
        # second QLocalServer.listen(name) can replace the socket path of a live server.
        client = QLocalSocket(self)
        self._client = client
        client.connectToServer(self.name)
        if client.waitForConnected(400):
            data = json.dumps({"command": self.signal}).encode("utf-8") + b"\n"
            client.write(data)
            client.flush()
            client.waitForBytesWritten(250)
            client.disconnectFromServer()
            client.deleteLater()
            self._client = None
            self.server = None
            self.is_primary = False
            return
        client.deleteLater()
        self._client = None
        self.server: QLocalServer | None = QLocalServer(self)
        self.server.setSocketOptions(QLocalServer.SocketOption.UserAccessOption)
        if self.server.listen(self.name):
            self.is_primary = True
            self.server.newConnection.connect(self._accept)
            return
        # Another process may have won the listen race; deliver the command if so.
        self.server.deleteLater()
        self.server = None
        retry = QLocalSocket(self)
        self._client = retry
        retry.connectToServer(self.name)
        if retry.waitForConnected(500):
            retry.write(json.dumps({"command": self.signal}).encode("utf-8") + b"\n")
            retry.flush()
            retry.waitForBytesWritten(250)
            retry.disconnectFromServer()
            retry.deleteLater()
            self._client = None
            self.is_primary = False
        else:
            self.is_primary = False
            self.last_connect_error = retry.errorString()
            retry.deleteLater()
            self._client = None

    @Slot()
    def _accept(self) -> None:
        if self.server is None:
            return
        while self.server.hasPendingConnections():
            socket = self.server.nextPendingConnection()
            socket.readyRead.connect(lambda s=socket: self._read(s))
            socket.disconnected.connect(lambda s=socket: self._buffers.pop(id(s), None))
            socket.disconnected.connect(socket.deleteLater)

    def _read(self, socket: QLocalSocket) -> None:
        key = id(socket)
        buffer = self._buffers.setdefault(key, bytearray())
        buffer.extend(bytes(socket.readAll()))
        if len(buffer) > self.MAX_COMMAND_BYTES:
            self._buffers.pop(key, None)
            socket.abort()
            socket.deleteLater()
            return
        if b"\n" not in buffer:
            return
        line, _, _remainder = buffer.partition(b"\n")
        self._buffers.pop(key, None)
        try:
            body = json.loads(line.decode("utf-8", "strict"))
        except ValueError:
            body = None
        command = body.get("command") if isinstance(body, dict) else None
        if command in ("open", "toggle"):
            self.commandReceived.emit(command)
        socket.abort()
        socket.deleteLater()


class Platform(QObject):
    trayChanged = Signal()
    popoverChanged = Signal()
    showWindowRequested = Signal()
    toggleWindowRequested = Signal()
    quitRequested = Signal()
    _notificationFallback = Signal(str, str)

    def __init__(self, services, argv: list[str] | None = None, parent: QObject | None = None):
        super().__init__(parent)
        self._services = services
        self._flags = core.parse_flags(argv or [])
        self._popover_mode = self._flags.popover and bool(getattr(services, "popover_supported", False))
        self._popover_open = False
        self._popover_backdrop_visible = False
        self._popover_card_visible = False
        self._popover_pointer_saved = False
        self._popover_generation = 0
        screen = services.qt_app.primaryScreen()
        self._popover_screen = (screen.geometry().width(), screen.geometry().height()) if screen else (1920, 1080)
        self._popover_edge = core.edge_from_env()
        self._popover_margins = core.card_margins(core.Placement(
            edge=self._popover_edge, screen=self._popover_screen))[1]
        self._start_visible = not self._flags.popover or (not self._popover_mode and self._flags.toggle)
        self._window = None
        self._tray: QSystemTrayIcon | None = None
        self._menu: QMenu | None = None
        self._quitting = False
        self._notifier = getattr(services, "notifier", None)
        self._instance = getattr(services, "single_instance", None)
        self._notificationFallback.connect(self._show_tray_notice)
        injected_cache = getattr(services, "platform_cache_dir", None)
        cache_root = QStandardPaths.writableLocation(QStandardPaths.StandardLocation.CacheLocation)
        self._cache_dir = Path(injected_cache) if injected_cache else Path(cache_root or Path.home() / ".cache" / "wa-bot") / "notification-claims"
        self._hub.event.connect(self._on_event)
        if self._instance is not None:
            self._instance.commandReceived.connect(self._on_instance_command)
            if self._instance.server is not None:
                self._services.qt_app.aboutToQuit.connect(self._instance.server.close)
        self._init_tray()

    @Property(bool, notify=trayChanged)
    def trayAvailable(self) -> bool:
        return self._tray is not None and QSystemTrayIcon.isSystemTrayAvailable()

    @Property(bool, notify=trayChanged)
    def trayVisible(self) -> bool:
        return self._tray is not None and self._tray.isVisible()

    @Property(bool, constant=True)
    def popoverMode(self) -> bool:
        return self._popover_mode

    @Property(bool, constant=True)
    def popoverRequested(self) -> bool:
        return self._flags.popover

    @Property(bool, constant=True)
    def startVisible(self) -> bool:
        return self._start_visible

    @Property(bool, notify=popoverChanged)
    def popoverBackdropVisible(self) -> bool:
        return self._popover_backdrop_visible

    @Property(bool, notify=popoverChanged)
    def popoverCardVisible(self) -> bool:
        return self._popover_card_visible

    @Property(int, notify=popoverChanged)
    def popoverScreenWidth(self) -> int:
        return int(self._popover_screen[0])

    @Property(int, notify=popoverChanged)
    def popoverScreenHeight(self) -> int:
        return int(self._popover_screen[1])

    @Property(str, constant=True)
    def popoverEdge(self) -> str:
        return self._popover_edge

    @Property(int, notify=popoverChanged)
    def popoverTopMargin(self) -> int:
        return self._popover_margins[0]

    @Property(int, notify=popoverChanged)
    def popoverRightMargin(self) -> int:
        return self._popover_margins[1]

    @Property(int, notify=popoverChanged)
    def popoverBottomMargin(self) -> int:
        return self._popover_margins[2]

    @Property(int, notify=popoverChanged)
    def popoverLeftMargin(self) -> int:
        return self._popover_margins[3]

    @Property(bool, notify=trayChanged)
    def quitting(self) -> bool:
        return self._quitting

    @property
    def _hub(self):
        return self._services.hub

    @property
    def _app(self):
        return self._services.app

    def _init_tray(self) -> None:
        app = self._services.qt_app
        if not isinstance(app, QApplication) or not QSystemTrayIcon.isSystemTrayAvailable():
            self.trayChanged.emit()
            return
        icon_path = Path(__file__).resolve().parent.parent / "qml" / "assets" / "icons" / "inbox.svg"
        tray = self._tray = QSystemTrayIcon(QIcon(str(icon_path)), self)
        tray.setToolTip("WA Bot Desktop")
        menu = self._menu = QMenu()
        open_action = menu.addAction("Open WA Bot")
        open_action.triggered.connect(self.showWindowRequested)
        menu.addSeparator()
        quit_action = menu.addAction("Quit")
        quit_action.triggered.connect(self.quit)
        tray.setContextMenu(menu)
        tray.activated.connect(self._tray_activated)
        tray.messageClicked.connect(self.showWindowRequested)
        tray.show()
        self.trayChanged.emit()

    @Slot()
    def attachWindow(self) -> None:
        roots = self._services.engine.rootObjects() if self._services.engine else []
        self._window = roots[0] if roots else None
        if self._window is not None and self._popover_mode and self._flags.toggle:
            self.openPopover()

    def _set_popover_margins(self, pointer: tuple[float, float] | None) -> None:
        placement = core.Placement(edge=self._popover_edge, pointer=pointer, screen=self._popover_screen)
        self._popover_margins = core.card_margins(placement)[1]
        self.popoverChanged.emit()

    def _show_popover_fallback(self, generation: int) -> None:
        if generation != self._popover_generation or not self._popover_open or self._popover_card_visible:
            return
        self._set_popover_margins(None)
        self._popover_card_visible = True
        self.popoverChanged.emit()

    @Slot()
    def openPopover(self) -> None:
        if not self._popover_mode:
            self.showWindowRequested.emit()
            return
        if self._popover_open:
            return
        screen = self._services.qt_app.primaryScreen()
        if screen is not None:
            geometry = screen.geometry()
            self._popover_screen = (geometry.width(), geometry.height())
        self._popover_generation += 1
        generation = self._popover_generation
        self._popover_open = True
        self._popover_backdrop_visible = True
        self._popover_pointer_saved = False
        cached_pointer = core.load_popover_pointer()
        if cached_pointer is not None:
            self._set_popover_margins(cached_pointer)
            self._popover_card_visible = True
            self.popoverChanged.emit()
        else:
            self._popover_card_visible = False
            self._set_popover_margins(None)
            QTimer.singleShot(120, lambda: self._show_popover_fallback(generation))
        self.popoverChanged.emit()

    @Slot()
    def showPopover(self) -> None:
        if not self._popover_open:
            self.openPopover()

    @Slot()
    def togglePopover(self) -> None:
        if self._popover_open:
            self.dismissPopover()
        else:
            self.openPopover()

    @Slot(float, float)
    def capturePopoverPointer(self, x: float, y: float) -> None:
        if not self._popover_mode or not self._popover_open or self._popover_pointer_saved:
            return
        pointer = (float(x), float(y))
        if not all(map(math.isfinite, pointer)):
            return
        core.save_popover_pointer(pointer)
        self._popover_pointer_saved = True
        if not self._popover_card_visible:
            self._set_popover_margins(pointer)
            self._popover_card_visible = True
            self.popoverChanged.emit()

    @Slot()
    def dismissPopover(self) -> None:
        if not self._popover_open:
            return
        self._popover_generation += 1
        self._popover_open = False
        self._popover_backdrop_visible = False
        self._popover_card_visible = False
        self._popover_pointer_saved = False
        self.popoverChanged.emit()

    @Slot(str)
    def _on_instance_command(self, command: str) -> None:
        if self._popover_mode:
            self.togglePopover() if command == "toggle" else self.showPopover()
        else:
            self.showWindowRequested.emit()

    @Slot()
    def showWindow(self) -> None:
        self.showWindowRequested.emit()

    @Slot()
    def toggleWindow(self) -> None:
        self.toggleWindowRequested.emit()

    @Slot()
    def quit(self) -> None:
        self._quitting = True
        if self._tray is not None:
            self._tray.hide()
        self.trayChanged.emit()
        self.quitRequested.emit()
        self._services.qt_app.quit()

    @Slot()
    def hideToTray(self) -> None:
        if self._window is not None and self.trayAvailable:
            self._window.hide()

    @Slot()
    def _tray_activated(self, reason) -> None:
        if reason in (QSystemTrayIcon.ActivationReason.Trigger,
                      QSystemTrayIcon.ActivationReason.DoubleClick):
            self.showWindowRequested.emit()

    def _on_event(self, typ: str, payload_json: str) -> None:
        if typ != "new_message" or not self._services.settings.get_py("notifications_enabled", True):
            return
        try:
            msg = json.loads(payload_json) if payload_json else {}
        except ValueError:
            return
        if not isinstance(msg, dict):
            return
        chat_id = str(msg.get("chatId") or "")
        msg_id = str(msg.get("id") or "")
        if not chat_id or not msg_id or core.reading_chat(chat_id, self._app.windowVisible, self._app.activeChatId):
            return
        chats = self._services.exposed.get("Chats")
        chat = chats.get(chat_id) if chats is not None else None
        notice = core.plan(msg, chat, core.now_ms())
        if notice is None:
            return
        key = core.notice_key(str(self._services.backend.root_url() or ""), chat_id, msg_id)
        try:
            claimed = core.claim_notice(self._cache_dir, key)
        except OSError:
            claimed = Path()  # deliver even if the cache is unwritable; only dedupe is lost
        if claimed is None:
            return
        self._show_notice(notice.title, notice.body)

    def _show_notice(self, title: str, body: str) -> None:
        if self._notifier is not None:
            self._notifier.show(title, body)
            return
        if not self._dbus_notify(title, body) and self._tray is not None:
            self._show_tray_notice(title, body)

    def _dbus_notify(self, title: str, body: str) -> bool:
        if not os.environ.get("DBUS_SESSION_BUS_ADDRESS"):
            return False
        try:
            from dbus_next import Message, Variant
            from dbus_next.aio import MessageBus
        except ImportError:
            return False

        def work() -> None:
            async def send() -> None:
                bus = await MessageBus().connect()
                try:
                    message = Message(
                        destination="org.freedesktop.Notifications",
                        path="/org/freedesktop/Notifications",
                        interface="org.freedesktop.Notifications",
                        member="Notify",
                        signature="susssasa{sv}i",
                        body=["WA Bot Desktop", 0, "", title, body, [], {
                            "desktop-entry": Variant("s", "wa-bot-qml"),
                        }, 5000],
                    )
                    reply = await bus.call(message)
                    if reply.message_type.name == "ERROR":
                        raise RuntimeError(reply.body[0] if reply.body else "notification service failed")
                finally:
                    bus.disconnect()
            try:
                asyncio.run(send())
            except Exception as exc:  # desktop services may be absent or reject notifications
                print(f"[platform] notification failed: {exc}", file=sys.stderr)
                self._notificationFallback.emit(title, body)

        threading.Thread(target=work, name="wabot-notification", daemon=True).start()
        return True

    @Slot(str, str)
    def _show_tray_notice(self, title: str, body: str) -> None:
        if self._tray is not None:
            self._tray.showMessage(title, body, QSystemTrayIcon.MessageIcon.Information, 5000)


def register(services) -> None:
    platform = Platform(services, argv=getattr(services, "argv", None))
    services.platform = platform
    services.expose("Platform", platform)

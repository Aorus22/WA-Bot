"""Desktop notification policy and local single-instance IPC stay inside test-owned state."""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import uuid
from pathlib import Path
from types import SimpleNamespace

from PySide6.QtCore import QCoreApplication, QEventLoop, QObject, QTimer, Signal
import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from bridge.platform import LocalInstanceServer, Platform  # noqa: E402
from mock_backend import MockBackend  # noqa: E402


def spin(cond, ms=3000):
    app = QCoreApplication.instance()
    end = __import__("time").monotonic() + ms / 1000
    while __import__("time").monotonic() < end:
        app.processEvents()
        if cond():
            return True
        __import__("time").sleep(0.01)
    return cond()


class Hub(QObject):
    event = Signal(str, str)


class Notifier:
    def __init__(self):
        self.items = []

    def show(self, title, body):
        self.items.append((title, body))


def test_notifications_respect_read_mute_and_cross_client_receipts(tmp_path):
    app = QCoreApplication.instance() or QCoreApplication([])
    hub, notifier, second_notifier = Hub(), Notifier(), Notifier()
    state = SimpleNamespace(windowVisible=False, activeChatId="")
    chat = {"name": "Budi", "isGroup": False, "archived": False, "muteMode": "off"}
    notifications_enabled = [True]
    chats = SimpleNamespace(get=lambda _jid: chat)
    settings = SimpleNamespace(get_py=lambda key, default=None:
                                notifications_enabled[0] if key == "notifications_enabled" else default)
    cache_dir = tmp_path / "claims"
    services = SimpleNamespace(
        hub=hub, app=state, toast=None, api=None, settings=settings,
        backend=SimpleNamespace(root_url=lambda: "http://127.0.0.1:3000"),
        exposed={"Chats": chats}, qt_app=app, notifier=notifier, platform_cache_dir=cache_dir,
    )
    second_services = SimpleNamespace(**{
        **vars(services), "notifier": second_notifier,
    })
    platform = Platform(services, argv=[])
    second_platform = Platform(second_services, argv=[])
    msg = {"id": "m1", "chatId": "6281111111111@s.whatsapp.net", "from": "6281111111111@s.whatsapp.net",
           "content": "Hello there", "timestamp": __import__("time").time_ns() // 1_000_000, "status": "received", "type": "text"}
    hub.event.emit("new_message", json.dumps(msg))
    assert notifier.items == [("Budi", "Hello there")]
    assert second_notifier.items == []
    hub.event.emit("new_message", json.dumps(msg))
    assert len(notifier.items) == 1
    assert second_notifier.items == []

    state.windowVisible = True
    state.activeChatId = msg["chatId"]
    hub.event.emit("new_message", json.dumps({**msg, "id": "m2"}))
    assert len(notifier.items) == 1
    state.windowVisible = False

    chat["muteMode"] = "forever"
    hub.event.emit("new_message", json.dumps({**msg, "id": "m3"}))
    chat.update(muteMode="until", mutedUntil=__import__("time").time_ns() // 1_000_000 + 60_000)
    hub.event.emit("new_message", json.dumps({**msg, "id": "m4"}))
    assert len(notifier.items) == 1

    chat.update(muteMode="until", mutedUntil=__import__("time").time_ns() // 1_000_000 - 1000)
    hub.event.emit("new_message", json.dumps({**msg, "id": "m5"}))
    assert len(notifier.items) == 2

    chat["archived"] = True
    hub.event.emit("new_message", json.dumps({**msg, "id": "m6"}))
    chat["archived"] = False
    notifications_enabled[0] = False
    hub.event.emit("new_message", json.dumps({**msg, "id": "m7"}))
    notifications_enabled[0] = True
    hub.event.emit("new_message", json.dumps({**msg, "id": "m8", "timestamp": msg["timestamp"] - 6 * 60_000}))
    assert len(notifier.items) == 2
    platform.deleteLater()
    second_platform.deleteLater()


def test_local_second_instance_routes_show_and_toggle_to_platform():
    app = QCoreApplication.instance() or QCoreApplication([])
    name = "wa-bot-test-" + uuid.uuid4().hex
    primary = LocalInstanceServer([], name=name)
    assert primary.is_primary and primary.server is not None
    services = SimpleNamespace(
        hub=Hub(),
        app=SimpleNamespace(windowVisible=False, activeChatId=""),
        toast=None,
        api=None,
        settings=SimpleNamespace(get_py=lambda _key, default=None: default),
        backend=SimpleNamespace(root_url=lambda: "http://127.0.0.1:3000"),
        exposed={},
        qt_app=app,
        notifier=Notifier(),
        platform_cache_dir="/tmp/wabot-single-instance-test",
        single_instance=primary,
    )
    platform = Platform(services, argv=[])
    opened, toggled = [], []
    platform.showWindowRequested.connect(lambda: opened.append(True))
    platform.toggleWindowRequested.connect(lambda: toggled.append(True))
    secondary = LocalInstanceServer(["--toggle"], name=name)
    assert not secondary.is_primary
    assert spin(lambda: toggled == [True])

    opener = LocalInstanceServer([], name=name)
    assert not opener.is_primary
    assert spin(lambda: opened == [True])

    primary.server.close()
    QCoreApplication.processEvents()
    platform.deleteLater()


def test_local_instance_reader_accumulates_fragmented_command():
    app = QCoreApplication.instance() or QCoreApplication([])
    server = LocalInstanceServer([], name="unused-" + uuid.uuid4().hex)
    received = []
    server.commandReceived.connect(received.append)

    class FragmentedSocket:
        def __init__(self):
            self.chunks = [b'{"command":', b'"toggle"}\n']
            self.aborted = False

        def readAll(self):
            return self.chunks.pop(0) if self.chunks else b""

        def abort(self):
            self.aborted = True

        def deleteLater(self):
            pass

    sock = FragmentedSocket()
    server._read(sock)
    assert received == []
    assert not sock.aborted
    server._read(sock)
    assert received == ["toggle"]
    assert sock.aborted
    assert not server._buffers
    if server.server is not None:
        server.server.close()


def test_dbus_notification_uses_private_test_bus():
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    from platform_fakes.harness import FakeSession, PrivateBus, available
    if not available():
        pytest.skip("dbus-daemon or dbus-next is not installed")

    bus = PrivateBus()
    fake = FakeSession(bus.address)
    child_code = r'''from types import SimpleNamespace
from PySide6.QtCore import QCoreApplication, QObject, Signal, QTimer
from bridge.platform import Platform
class Hub(QObject):
    event = Signal(str, str)
app = QCoreApplication([])
services = SimpleNamespace(hub=Hub(), app=SimpleNamespace(windowVisible=False, activeChatId=""), toast=None,
    api=None, settings=SimpleNamespace(get_py=lambda _k, default=None: default),
    backend=SimpleNamespace(root_url=lambda: "http://127.0.0.1:3000"), exposed={}, qt_app=app,
    platform_cache_dir="/tmp/wabot-private-dbus-test")
platform = Platform(services, argv=[])
if not platform._dbus_notify("WA Bot", "Private bus test"):
    raise SystemExit(2)
QTimer.singleShot(350, app.quit)
app.exec()
'''
    env = dict(os.environ, DBUS_SESSION_BUS_ADDRESS=bus.address)
    project = Path(__file__).resolve().parent.parent
    try:
        child = subprocess.run([sys.executable, "-c", child_code], cwd=project, env=env,
                               capture_output=True, text=True, timeout=6)
        assert child.returncode == 0, child.stderr or child.stdout
        event = fake.wait(lambda e: e.get("ev") == "notify" and e.get("summary") == "WA Bot",
                          timeout=4)
        assert event and event["body"] == "Private bus test"
        assert event["app"] == "WA Bot Desktop"
        assert event["hints"]["desktop-entry"] == {"sig": "s", "value": "wa-bot-qml"}
    finally:
        fake.stop()
        bus.stop()


def test_tray_registers_with_private_status_notifier_host_under_xvfb(tmp_path):
    xvfb_run = shutil.which("xvfb-run")
    if not xvfb_run:
        pytest.skip("xvfb-run is unavailable")
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    from platform_fakes.harness import FakeSession, PrivateBus, available
    if not available():
        pytest.skip("dbus-daemon or dbus-next is not installed")

    bus = PrivateBus()
    fake = FakeSession(bus.address, "--no-notifications")
    child_code = r'''from types import SimpleNamespace
from PySide6.QtCore import QTimer
from PySide6.QtWidgets import QApplication, QSystemTrayIcon
from bridge.platform import Platform
app = QApplication([])
services = SimpleNamespace(
    hub=SimpleNamespace(event=type("Event", (), {"connect": lambda *_: None})()),
    app=SimpleNamespace(windowVisible=True, activeChatId=""), toast=None, api=None,
    settings=SimpleNamespace(get_py=lambda _key, default=None: default),
    backend=SimpleNamespace(root_url=lambda: "http://127.0.0.1:3000"),
    exposed={}, qt_app=app, notifier=None, platform_cache_dir="CACHE_DIR",
)
platform = Platform(services, argv=[])
print("TRAY_AVAILABLE=" + str(platform.trayAvailable), flush=True)
if not platform.trayAvailable:
    raise SystemExit(3)
shown = []
platform.showWindowRequested.connect(lambda: shown.append(True))
platform._tray_activated(QSystemTrayIcon.ActivationReason.Trigger)
if shown != [True]:
    raise SystemExit(4)
print("TRAY_ACTIVATION=show", flush=True)
platform.quitRequested.connect(app.quit)
QTimer.singleShot(300, platform.quit)
app.exec()
'''
    child_code = child_code.replace("CACHE_DIR", str(tmp_path / "cache"))
    env = dict(os.environ, DBUS_SESSION_BUS_ADDRESS=bus.address, QT_QPA_PLATFORM="xcb",
               QT_QPA_PLATFORMTHEME="generic", GTK_USE_PORTAL="0")
    project = Path(__file__).resolve().parent.parent
    try:
        child = subprocess.run([xvfb_run, "-a", sys.executable, "-c", child_code], cwd=project,
                               env=env, capture_output=True, text=True, timeout=20)
        assert child.returncode == 0, child.stdout + child.stderr
        assert "TRAY_AVAILABLE=True" in child.stdout
        assert "TRAY_ACTIVATION=show" in child.stdout
        registration = fake.wait(lambda event: event.get("ev") == "register", timeout=4)
        assert registration is not None
    finally:
        fake.stop()
        bus.stop()


def test_qml_close_hides_to_tray_and_platform_show_restores_window(tmp_path):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    env["PYTHONUNBUFFERED"] = "1"
    output = tmp_path / "tray-close-cycle.png"
    try:
        result = subprocess.run(
            [
                sys.executable,
                str(Path(__file__).resolve().parents[1] / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "chat",
                "--tray-close-cycle",
                "--preset", "default-dark",
                "--theme-mode", "dark",
                "--size", "1100x720",
                "--wait", "500",
            ],
            cwd=Path(__file__).resolve().parents[1],
            env=env,
            capture_output=True,
            text=True,
            timeout=25,
        )
        assert result.returncode == 0, result.stdout + result.stderr
        assert "[qml]" not in result.stdout + result.stderr
        assert "close-to-tray QML close was intercepted and show/toggle restored the window" in result.stdout
        assert output.is_file()
    finally:
        backend.stop()

"""WA Bot desktop (Qt Quick/QML). Run: `python main.py [--url URL | --port N]`.
QML edits need no build step: just restart (or use `scripts/dev.sh`)."""
from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path

from PySide6.QtCore import QLibraryInfo, QUrl
from PySide6.QtGui import QFontDatabase, QGuiApplication, QImage
from PySide6.QtQml import QQmlApplicationEngine
from PySide6.QtQuickControls2 import QQuickStyle
from PySide6.QtWidgets import QApplication

from bridge.api import Api
from bridge.appstate import AppState
from bridge.backend import BackendController
from bridge.call_media.frames import CallFrameProvider
from bridge.events import EventHub
from bridge.icons import IconProvider
from bridge.registry import Services, register_all
from bridge.settings import Settings
from bridge.theme import ThemeManager
from bridge.toast import ToastBus

ROOT = Path(__file__).resolve().parent
QML = ROOT / "qml"


def _layer_shell_runtime() -> tuple[bool, str | None]:
    """Find the optional KDE layer-shell QPA/QML plugins before QApplication starts."""
    if sys.platform != "linux":
        return False, None
    qpa = os.environ.get("QT_QPA_PLATFORM", "").split(";", 1)[0]
    wayland_session = qpa.startswith("wayland") or (
        not qpa and bool(os.environ.get("WAYLAND_DISPLAY"))
        and os.environ.get("XDG_SESSION_TYPE", "").lower() in ("", "wayland")
    )
    if not wayland_session:
        return False, None
    wayland_info = shutil.which("wayland-info")
    if not wayland_info:
        return False, None
    try:
        registry = subprocess.run([wayland_info], capture_output=True, text=True, timeout=2, check=False)
    except (OSError, subprocess.TimeoutExpired):
        return False, None
    if registry.returncode != 0 or "zwlr_layer_shell_v1" not in registry.stdout:
        return False, None
    integration = os.environ.get("QT_WAYLAND_SHELL_INTEGRATION", "")
    if integration and integration != "layer-shell":
        return False, None

    plugin_roots = [Path(QLibraryInfo.path(QLibraryInfo.LibraryPath.PluginsPath))]
    plugin_roots.extend(Path(p) for p in os.environ.get("QT_PLUGIN_PATH", "").split(os.pathsep) if p)
    qml_roots = [Path(QLibraryInfo.path(QLibraryInfo.LibraryPath.QmlImportsPath))]
    qml_roots.extend(Path(p) for p in os.environ.get("QML2_IMPORT_PATH", "").split(os.pathsep) if p)
    qml_roots.extend((Path("/usr/lib/qt6/qml"), Path("/usr/lib64/qt6/qml")))
    qml_root = next((p for p in qml_roots if (p / "org/kde/layershell/qmldir").is_file()), None)
    qml_plugin = qml_root / "org/kde/layershell/libLayerShellQtQml.so" if qml_root else None
    layer_plugin = next((p / "wayland-shell-integration/liblayer-shell.so"
                         for p in plugin_roots
                         if (p / "wayland-shell-integration/liblayer-shell.so").is_file()), None)
    wayland_qpa = any(any((p / "platforms").glob("libqwayland*.so")) for p in plugin_roots)
    if qml_root is None or qml_plugin is None or not qml_plugin.is_file() or layer_plugin is None or not wayland_qpa:
        return False, None

    if not qpa:
        os.environ["QT_QPA_PLATFORM"] = "wayland"
    os.environ["QT_WAYLAND_SHELL_INTEGRATION"] = "layer-shell"
    return True, str(qml_root)


def build(argv: list[str], qt_app: QGuiApplication, settings: Settings | None = None,
          single_instance=None, popover_supported: bool = False,
          layer_shell_import_path: str | None = None):
    """Create services and the QML engine (not yet loaded). Used by main() and tests."""
    backend = BackendController(argv[1:])
    api = Api(backend)
    hub = EventHub(backend)
    settings = settings or Settings()
    services = Services(
        backend=backend, api=api, hub=hub, settings=settings,
        theme=ThemeManager(settings), app=AppState(hub), qt_app=qt_app, toast=ToastBus(),
    )
    services.call_frames = CallFrameProvider()
    services.argv = argv[1:]
    services.single_instance = single_instance
    services.popover_supported = popover_supported
    qt_app.aboutToQuit.connect(hub.stop)
    qt_app.aboutToQuit.connect(backend.shutdown)  # owner-only: attached backends stay up

    engine = QQmlApplicationEngine()
    services.engine = engine
    engine.warnings.connect(lambda ws: [print(f"[qml] {w.toString()}", file=sys.stderr) for w in ws])
    engine.addImportPath(str(QML))
    if layer_shell_import_path:
        engine.addImportPath(layer_shell_import_path)
    engine.addImageProvider("icons", IconProvider())
    engine.addImageProvider("callframes", services.call_frames)
    failed = register_all(services)
    if failed:
        print(f"[app] feature modules failed to load: {', '.join(failed)}", file=sys.stderr)
    return services, engine


def main(argv: list[str]) -> int:
    if len(argv) > 1 and argv[1] == "--self-test-h264":
        from bridge.call_media.video import H264Decoder, H264Encoder
        image = QImage(320, 240, QImage.Format.Format_RGB888)
        image.fill(0xFF5078A0)
        encoder, decoder = H264Encoder(), H264Decoder()
        try:
            access_unit = encoder.encode(image)
            decoded = decoder.decode(access_unit or b"")
            if decoded is None or (decoded.width(), decoded.height()) != (320, 240):
                print("[codec] bundled H.264 encode/decode failed", file=sys.stderr)
                return 1
            print("[codec] bundled H.264 Annex-B encode/decode passed")
            return 0
        except Exception as exc:
            print(f"[codec] bundled H.264 encode/decode failed: {exc}", file=sys.stderr)
            return 1
        finally:
            decoder.close()
            encoder.close()

    popover_supported, layer_shell_import_path = _layer_shell_runtime()
    QQuickStyle.setStyle("Basic")  # unstyled controls: every look comes from our theme
    app = QApplication(argv)
    app.setApplicationName("wa-bot")
    app.setDesktopFileName("wa-bot")
    for font in (QML / "assets" / "fonts").glob("*.ttf"):
        QFontDatabase.addApplicationFont(str(font))

    from bridge.platform import LocalInstanceServer
    instance = LocalInstanceServer(argv[1:], popover_supported=popover_supported)
    if not instance.is_primary:
        app.quit()
        return 0

    services, engine = build(argv, app, single_instance=instance,
                             popover_supported=popover_supported,
                             layer_shell_import_path=layer_shell_import_path)
    main_qml = "PopoverMain.qml" if services.platform.popoverMode else "Main.qml"
    engine.load(QUrl.fromLocalFile(str(QML / main_qml)))
    if not engine.rootObjects():
        return 1
    services.platform.attachWindow()
    services.backend.start()
    services.hub.start()
    return app.exec()


if __name__ == "__main__":
    sys.exit(main(sys.argv))

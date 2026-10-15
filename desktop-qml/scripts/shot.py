"""Render the app offscreen and save a PNG.
    QT_QPA_PLATFORM=offscreen python scripts/shot.py out.png [--route chat] [--preset whatsapp-dark]
        [--size 1100x720] [--scale 1.0] [--wait 2500] [--url http://127.0.0.1:PORT/api] [--home DIR]
Everything after the known options is passed to the app (e.g. --port N). Uses the same startup as main.py."""
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PySide6.QtCore import QTimer, QUrl  # noqa: E402
from PySide6.QtGui import QFontDatabase, QGuiApplication  # noqa: E402
from PySide6.QtQuickControls2 import QQuickStyle  # noqa: E402

import main as app_main  # noqa: E402


def opt(args, name, default=None, flag=False):
    if name in args:
        i = args.index(name)
        if flag:
            args.pop(i)
            return True
        v = args[i + 1]
        del args[i:i + 2]
        return v
    return False if flag else default


def run(argv: list[str]) -> int:
    args = argv[1:]
    out = args.pop(0)
    route = opt(args, "--route", "chat")
    preset = opt(args, "--preset")
    size = opt(args, "--size", "1100x720")
    wait = int(opt(args, "--wait", "2500"))
    scale = opt(args, "--scale")
    QQuickStyle.setStyle("Basic")
    app = QGuiApplication(["shot", *args])
    for f in (app_main.QML / "assets" / "fonts").glob("*.ttf"):
        QFontDatabase.addApplicationFont(str(f))
    from bridge.settings import Settings
    import tempfile
    settings = Settings(Path(tempfile.mkdtemp()) / "settings.json")  # never touch the user's settings
    if preset:
        settings.set_transient("theme_preset", preset)
    if scale:
        settings.set_transient("ui_font_scale", int(float(scale) * 100))
    settings.set_transient("window_transparency", 0)
    services, engine = app_main.build(["shot", *args], app, settings)
    engine.load(QUrl.fromLocalFile(str(app_main.QML / "Main.qml")))
    if not engine.rootObjects():
        return 1
    w, h = (int(x) for x in size.split("x"))
    win = engine.rootObjects()[0]
    win.setWidth(w)
    win.setHeight(h)
    services.app.navigate(route)
    services.backend.start()
    services.hub.start()

    def grab():
        img = win.grabWindow()
        img.save(out)
        print("saved", out, img.width(), img.height())
        app.quit()

    QTimer.singleShot(wait, grab)
    code = app.exec()
    services.backend.shutdown()
    return code


if __name__ == "__main__":
    sys.exit(run(sys.argv))

"""Exercise chat-header pointer paths in the real QML scene with local data only."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests"))

from mock_backend import MockBackend  # noqa: E402

BUDI = "6281111111111@s.whatsapp.net"


def test_sidebar_and_profile_mouse_actions(tmp_path):
    backend = MockBackend(areas=["auth", "chats", "rich", "calls", "composer", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    try:
        for flag, output, expected in (
            ("--click-sidebar-toggle", "sidebar.png", "sidebar toggle mouse click: False -> True"),
            ("--click-chat-profile", "profile.png", "chat profile mouse click opened info: '' -> 'info'"),
        ):
            result = subprocess.run(
                [
                    sys.executable,
                    str(ROOT / "scripts" / "shot.py"),
                    str(tmp_path / output),
                    "--url", backend.url,
                    "--route", "chat",
                    "--preset", "default-dark",
                    "--theme-mode", "dark",
                    "--size", "1100x720",
                    "--wait", "500",
                    "--select-chat", BUDI,
                    flag,
                ],
                cwd=ROOT,
                env=env,
                capture_output=True,
                text=True,
                timeout=25,
            )
            assert result.returncode == 0, result.stdout + result.stderr
            assert expected in result.stdout
            assert (tmp_path / output).is_file()
    finally:
        backend.stop()

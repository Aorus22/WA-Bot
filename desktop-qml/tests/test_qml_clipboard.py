"""Exercise composer clipboard paste through Qt Quick's Ctrl+V shortcut."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests"))

from mock_backend import MockBackend  # noqa: E402
from mock_common import BUDI  # noqa: E402


@pytest.mark.parametrize("kind", ["image", "text"])
def test_clipboard_paste_path_uses_qml_composer_shortcut(tmp_path, kind):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    args = ["--clipboard-paste", kind]
    output = tmp_path / f"clipboard-{kind}.png"
    try:
        command = [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "chat",
                "--select-chat", BUDI,
                *args,
                "--preset", "default-dark",
                "--theme-mode", "dark",
                "--size", "1100x720",
                "--wait", "500",
            ]
        result = subprocess.run(
            command,
            cwd=ROOT,
            env=env,
            capture_output=True,
            text=True,
            timeout=25,
        )
        assert result.returncode == 0, result.stdout + result.stderr
        assert "[qml]" not in result.stdout + result.stderr
        assert f"clipboard {kind} paste applied" in result.stdout
        if kind == "image":
            assert "clipboard image temporary file cleaned" in result.stdout
        assert output.is_file()
        assert not any(call["name"] in {"send-media", "send-message"}
                       for call in backend.state["composer"]["calls"])
    finally:
        backend.stop()

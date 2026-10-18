"""Exercise the composer Enter contract through actual QML key events."""
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


@pytest.mark.parametrize(
    ("key", "markdown", "should_send"),
    [
        ("Enter", False, True),
        ("ShiftEnter", False, False),
        ("CtrlEnter", False, True),
        ("Enter", True, False),
        ("ShiftEnter", True, False),
        ("CtrlEnter", True, True),
    ],
)
def test_composer_enter_keys_match_markdown_mode(tmp_path, key, markdown, should_send):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    try:
        output = tmp_path / f"composer-{key.lower()}-{markdown}.png"
        command = [
            sys.executable,
            str(ROOT / "scripts" / "shot.py"),
            str(output),
            "--url", backend.url,
            "--route", "chat",
            "--select-chat", BUDI,
            "--composer-text", "keyboard sample",
            "--composer-key", key,
            "--preset", "default-dark",
            "--theme-mode", "dark",
            "--size", "1100x720",
            "--wait", "500",
        ]
        if markdown:
            command.append("--composer-markdown")
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
        assert f"composer {key} (markdown={markdown}):" in result.stdout
        sends = [call for call in backend.state["composer"]["calls"] if call["name"] == "send-message"]
        assert bool(sends) is should_send
        if should_send:
            expected = "{{md:a2V5Ym9hcmQgc2FtcGxl}}" if markdown else "keyboard sample"
            assert sends[-1]["body"]["message"] == expected
        assert output.is_file()
    finally:
        backend.stop()

"""Verify the chat timer menu and options are reachable by keyboard."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests"))

from mock_backend import MockBackend  # noqa: E402
from mock_common import BUDI, TEAM  # noqa: E402


def test_chat_disappearing_duration_can_be_set_from_keyboard_menu(tmp_path):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    env["PYTHONUNBUFFERED"] = "1"
    output = tmp_path / "keyboard-disappearing-messages.png"
    try:
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "chat",
                "--select-chat", BUDI,
                "--disappearing-seconds", "86400",
                "--preset", "default-dark",
                "--theme-mode", "dark",
                "--size", "1100x720",
                "--wait", "500",
            ],
            cwd=ROOT,
            env=env,
            capture_output=True,
            text=True,
            timeout=30,
        )
        assert result.returncode == 0, result.stdout + result.stderr
        assert "[qml]" not in result.stdout + result.stderr
        assert "menu item accessible name='Disappearing messages'; ArrowDown/Space opened options; Space set 24 hours" in result.stdout
        chat = next(row for row in backend.state["chats"] if row["id"] == BUDI)
        assert chat["disappearingSecs"] == 86400
        assert output.is_file()
    finally:
        backend.stop()


def test_disappearing_message_note_has_accessible_expiry_text(tmp_path):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    row = next(message for message in backend.state["messages"][TEAM] if message["id"] == "t2")
    row["extra"] = {"disappearing": {"seconds": 86400}}
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    env["PYTHONUNBUFFERED"] = "1"
    output = tmp_path / "disappearing-message-note.png"
    try:
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "chat",
                "--select-chat", TEAM,
                "--disappearing-message-key", "t2",
                "--preset", "default-dark",
                "--theme-mode", "dark",
                "--size", "1100x720",
                "--wait", "500",
            ],
            cwd=ROOT,
            env=env,
            capture_output=True,
            text=True,
            timeout=30,
        )
        assert result.returncode == 0, result.stdout + result.stderr
        assert "[qml]" not in result.stdout + result.stderr
        assert "disappearing message accessible name='Disappears in 24 hours'" in result.stdout
        assert output.is_file()
    finally:
        backend.stop()

"""Verify message forwarding by searching/selecting chats and using keyboard focus."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests"))

from mock_backend import MockBackend  # noqa: E402
from mock_common import BUDI, TEAM  # noqa: E402


def test_forward_dialog_search_selection_and_submit_are_keyboard_accessible(tmp_path):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    env["PYTHONUNBUFFERED"] = "1"
    output = tmp_path / "keyboard-message-forward.png"
    try:
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "chat",
                "--select-chat", BUDI,
                "--forward-message", "b1",
                "--forward-key", TEAM,
                "--preset", "default-dark",
                "--theme-mode", "dark",
                "--size", "1100x720",
                "--wait", "500",
            ],
            cwd=ROOT,
            env=env,
            capture_output=True,
            text=True,
            timeout=35,
        )
        assert result.returncode == 0, result.stdout + result.stderr
        assert "[qml]" not in result.stdout + result.stderr
        assert "forward target accessible name='Forward to Tim Proyek'; Tab/Space forwarded b1 to Tim Proyek" in result.stdout
        assert backend.state["forwards"] == [(BUDI, "b1", [TEAM])]
        assert output.is_file()
    finally:
        backend.stop()


def test_multiple_selected_messages_forward_in_chronological_order(tmp_path):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    env["PYTHONUNBUFFERED"] = "1"
    output = tmp_path / "keyboard-multi-message-forward.png"
    try:
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "chat",
                "--select-chat", TEAM,
                "--multi-forward-key", f"t2,t3:{BUDI}",
                "--preset", "default-dark",
                "--theme-mode", "dark",
                "--size", "1100x720",
                "--wait", "500",
            ],
            cwd=ROOT,
            env=env,
            capture_output=True,
            text=True,
            timeout=40,
        )
        assert result.returncode == 0, result.stdout + result.stderr
        assert "[qml]" not in result.stdout + result.stderr
        assert "forward target accessible name='Forward to Budi Santoso'; Tab/Space forwarded t2,t3 to Budi Santoso" in result.stdout
        assert backend.state["forwards"] == [(TEAM, "t2", [BUDI]), (TEAM, "t3", [BUDI])]
        assert output.is_file()
    finally:
        backend.stop()

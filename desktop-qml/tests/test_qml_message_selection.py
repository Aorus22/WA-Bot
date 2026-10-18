"""Verify the selected-message toolbar copies text through its QML control."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests"))

from mock_backend import MockBackend  # noqa: E402
from mock_common import BUDI  # noqa: E402


def test_selection_toolbar_copy_has_accessible_space_action(tmp_path):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    env["PYTHONUNBUFFERED"] = "1"
    output = tmp_path / "selected-message-copy.png"
    try:
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "chat",
                "--select-chat", BUDI,
                "--select-message", "b1",
                "--selection-copy-key",
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
        assert "selection Copy accessible name='Copy'; Space copied the selected message and exited selection mode" in result.stdout
        assert not [call for call in backend.state["composer"]["calls"] if call["name"] == "send-message"]
        assert output.is_file()
    finally:
        backend.stop()


def test_selection_toolbar_delete_requires_second_space_confirmation(tmp_path):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    env["PYTHONUNBUFFERED"] = "1"
    output = tmp_path / "selected-message-delete.png"
    try:
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "chat",
                "--select-chat", BUDI,
                "--select-message", "b1",
                "--selection-delete-key",
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
        assert "selection Delete accessible name='Delete for everyone (own messages only)'; second Space confirmed 'the same Delete action' and exited selection mode" in result.stdout
        assert backend.state["message_deletions"] == [(BUDI, "b1")]
        deleted = next(row for row in backend.state["messages"][BUDI] if row["id"] == "b1")
        assert deleted["revokedAt"] > 0
        assert output.is_file()
    finally:
        backend.stop()

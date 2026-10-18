"""Verify existing reaction chips can be keyboard focused and toggled."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests"))

from mock_backend import MockBackend  # noqa: E402
from mock_common import BUDI, TEAM  # noqa: E402


def test_message_reaction_chip_has_accessible_name_and_space_action(tmp_path):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    target = next(row for row in backend.state["messages"][TEAM] if row["id"] == "t2")
    target["reactions"] = [{"emoji": "👍", "senders": [BUDI]}]
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    env["PYTHONUNBUFFERED"] = "1"
    output = tmp_path / "message-reaction-chip.png"
    try:
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "chat",
                "--select-chat", TEAM,
                "--react-chip-key", "t2:👍",
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
        assert "reaction chip accessible name='React with 👍'; Space applied 👍 to t2" in result.stdout
        assert backend.state["message_reactions"] == [(TEAM, "t2", {"emoji": "👍"})]
        assert output.is_file()
    finally:
        backend.stop()

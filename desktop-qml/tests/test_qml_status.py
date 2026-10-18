"""Drive a status story row in a local QML scene, never a live account."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests"))

from mock_backend import MockBackend  # noqa: E402
from mock_common import BUDI  # noqa: E402


def test_status_story_row_mouse_click_opens_viewer(tmp_path):
    backend = MockBackend(areas=["status"]).start()
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    try:
        output = tmp_path / "status-viewer.png"
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "status",
                "--click-status", BUDI,
                "--preset", "default-dark",
                "--theme-mode", "dark",
                "--size", "1100x720",
                "--wait", "500",
            ],
            cwd=ROOT,
            env=env,
            capture_output=True,
            text=True,
            timeout=25,
        )
        assert result.returncode == 0, result.stdout + result.stderr
        assert "[qml]" not in result.stdout + result.stderr
        assert f"status story mouse click: {BUDI} → viewer True" in result.stdout
        assert ("viewed", "b1") in backend.state["status_calls"]
        assert output.is_file()
    finally:
        backend.stop()


def test_status_composer_posts_text_after_qml_mouse_clicks(tmp_path):
    backend = MockBackend(areas=["status"]).start()
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    text = "Status posted from the QML composer"
    try:
        output = tmp_path / "status-posted.png"
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "status",
                "--status-post-text", text,
                "--preset", "default-dark",
                "--theme-mode", "dark",
                "--size", "1100x720",
                "--wait", "500",
            ],
            cwd=ROOT,
            env=env,
            capture_output=True,
            text=True,
            timeout=25,
        )
        assert result.returncode == 0, result.stdout + result.stderr
        assert "[qml]" not in result.stdout + result.stderr
        assert "status text posted through QML mouse controls" in result.stdout
        assert backend.state["status_calls"][-1] == ("text", {"text": text, "background": 0xFF075E54})
        assert len(backend.state["statuses"][0]["statuses"]) == 3
        assert output.is_file()
    finally:
        backend.stop()


def test_status_viewer_reply_posts_from_qml(tmp_path):
    backend = MockBackend(areas=["status"]).start()
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    reply = "Reply sent from QML status viewer"
    try:
        output = tmp_path / "status-reply.png"
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "status",
                "--status-reply-sender", BUDI,
                "--status-reply-text", reply,
                "--preset", "default-dark",
                "--theme-mode", "dark",
                "--size", "1100x720",
                "--wait", "500",
            ],
            cwd=ROOT,
            env=env,
            capture_output=True,
            text=True,
            timeout=25,
        )
        assert result.returncode == 0, result.stdout + result.stderr
        assert "[qml]" not in result.stdout + result.stderr
        assert f"status reply acknowledged through QML: {BUDI}" in result.stdout
        assert ("reply", "b1", reply) in backend.state["status_calls"]
        assert output.is_file()
    finally:
        backend.stop()

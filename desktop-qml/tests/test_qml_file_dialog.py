"""A selected QML FileDialog document reaches the multipart upload contract."""
from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests"))

from mock_backend import MockBackend  # noqa: E402
from mock_common import BUDI  # noqa: E402


def test_file_dialog_selection_uploads_document(tmp_path):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    backend.state["composer"]["delay"]["send-media"] = 0.3
    source = tmp_path / "selected-from-dialog.txt"
    source.write_text("chosen through the QML FileDialog", encoding="utf-8")
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    env["PYTHONUNBUFFERED"] = "1"
    output = tmp_path / "file-dialog.png"
    try:
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "chat",
                "--select-chat", BUDI,
                "--file-dialog-file", str(source),
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
        assert f"file dialog accepted selection: {source.name}" in result.stdout
        assert "file dialog upload completed" in result.stdout
        uploads = [call for call in backend.state["composer"]["calls"] if call["name"] == "send-media"]
        assert len(uploads) == 1
        upload = uploads[0]["body"]
        assert upload["fields"]["target"] == BUDI
        assert upload["fields"]["type"] == "document"
        assert upload["file"]["name"] == source.name
        assert upload["file"]["data"] == source.read_bytes()
        assert output.is_file()
    finally:
        backend.stop()

"""Verify GPUI conversation scrolling shortcuts on the QML message list."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests"))

from mock_backend import MockBackend  # noqa: E402


@pytest.mark.parametrize(
    ("key", "focus_composer", "focus_search", "expect_no_scroll", "draft", "caret_start"),
    [
        ("PageUp", True, False, False, None, False),
        ("PageDown", True, False, False, None, False),
        ("Home", True, False, False, None, False),
        ("End", True, False, False, None, False),
        ("PageUp", False, False, False, None, False),
        ("PageUp", False, True, True, None, False),
        ("Home", True, False, True, "draft text", False),
        ("End", True, False, True, "draft text", True),
    ],
)
def test_chat_scroll_shortcuts_respect_focus_and_caret(tmp_path, key, focus_composer, focus_search, expect_no_scroll, draft, caret_start):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    try:
        focus_name = "composer" if focus_composer else "chat-search" if focus_search else "message-list"
        output = tmp_path / f"chat-{key.lower()}-{focus_name}-{expect_no_scroll}.png"
        command = [
            sys.executable,
            str(ROOT / "scripts" / "shot.py"),
            str(output),
            "--url", backend.url,
            "--route", "chat",
            "--message-context",
            "--scroll-key", key,
            "--preset", "default-dark",
            "--theme-mode", "dark",
            "--size", "1100x720",
            "--wait", "500",
        ]
        if draft is not None:
            command.extend(["--composer-text", draft])
        if focus_composer:
            command.append("--focus-composer")
        if focus_search:
            command.append("--focus-chat-search")
        if expect_no_scroll:
            command.append("--expect-no-scroll")
        if caret_start:
            command.append("--caret-start")
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
        assert f"chat scroll key {key} (focus={focus_name}):" in result.stdout
        assert output.is_file()
    finally:
        backend.stop()

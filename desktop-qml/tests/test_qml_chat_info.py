"""QML chat-info drawer actions follow the current chat and group REST contracts."""
from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests"))

from mock_backend import MockBackend  # noqa: E402
from mock_chats import BUDI, RINA  # noqa: E402


def _run_info_action(tmp_path, backend, option, value, chat_id=BUDI):
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    output = tmp_path / f"chat-info-{option.lstrip('-')}-{value}.png"
    result = subprocess.run(
        [
            sys.executable,
            str(ROOT / "scripts" / "shot.py"),
            str(output),
            "--url", backend.url,
            "--route", "chat",
            "--select-chat", chat_id,
            option, str(value),
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
    assert output.is_file()
    return result.stdout


def _run_info_navigation(tmp_path, backend, page):
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    output = tmp_path / f"chat-info-subpage-{page}.png"
    result = subprocess.run(
        [
            sys.executable,
            str(ROOT / "scripts" / "shot.py"),
            str(output),
            "--url", backend.url,
            "--route", "chat",
            "--select-chat", BUDI,
            "--chat-info-page", page,
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
    assert f"chat info {page} subpage opened and returned through its header action" in result.stdout
    assert output.is_file()
    return result.stdout


@pytest.mark.parametrize("mode,expected", [
    ("off", "off"),
    ("8h", "until"),
    ("1w", "until"),
    ("forever", "forever"),
])
def test_chat_info_notification_mute_choices_post_exact_mode(tmp_path, mode, expected):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    try:
        output = _run_info_action(tmp_path, backend, "--info-mute-mode", mode)
        assert f"chat info setting mouse action applied: mute:{mode}" in output
        chat = next(row for row in backend.state["chats"] if row["id"] == BUDI)
        assert chat["muteMode"] == expected
        request = next((body for method, path, body in reversed(backend.requests)
                        if method == "POST" and path.endswith("/mute")), None)
        assert request is not None and json.loads(request) == {"mode": mode}
    finally:
        backend.stop()


@pytest.mark.parametrize("seconds", [0, 86_400, 604_800, 7_776_000])
def test_chat_info_disappearing_choices_post_exact_duration(tmp_path, seconds):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    try:
        chat_id = RINA if seconds == 0 else BUDI
        output = _run_info_action(tmp_path, backend, "--info-disappearing", seconds, chat_id)
        assert f"chat info setting mouse action applied: disappearing:{seconds}" in output
        chat = next(row for row in backend.state["chats"] if row["id"] == chat_id)
        assert chat["disappearingSecs"] == seconds
        request = next((body for method, path, body in reversed(backend.requests)
                        if method == "POST" and path.endswith("/disappearing")), None)
        assert request is not None and json.loads(request) == {"seconds": seconds}
    finally:
        backend.stop()


@pytest.mark.parametrize("page,endpoint", [
    ("media", "/media?limit=30"),
    ("docs", "/docs?limit=30"),
    ("links", "/links?limit=30"),
])
def test_chat_info_media_summary_opens_each_subpage_and_header_returns(tmp_path, page, endpoint):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    try:
        output = _run_info_navigation(tmp_path, backend, page)
        assert f"chat info {page} subpage opened and returned through its header action" in output
        assert "chat info starred control visible=True" in output
        assert any(method == "GET" and path.endswith(endpoint) for method, path, _body in backend.requests)
    finally:
        backend.stop()

"""Accessible icon controls can be named and activated with the keyboard."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests"))

from mock_backend import MockBackend  # noqa: E402
from mock_common import BUDI, TEAM  # noqa: E402


def test_icon_button_has_accessible_name_and_space_activation(tmp_path):
    backend = MockBackend(areas=["status"]).start()
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    output = tmp_path / "accessible-status-composer.png"
    try:
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "status",
                "--status-new-key",
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
        assert "accessible name='New status'; Space opened status composer" in result.stdout
        assert not backend.state["status_calls"]
        assert output.is_file()
    finally:
        backend.stop()


def test_text_button_has_accessible_name_and_space_activation(tmp_path):
    backend = MockBackend(areas=["status"]).start()
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    output = tmp_path / "accessible-status-post.png"
    text = "Posted by pressing an accessible QML button"
    try:
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "status",
                "--status-post-key", text,
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
        assert "Btn accessible name='Post status'; Space posted status text" in result.stdout
        assert backend.state["status_calls"][-1] == ("text", {"text": text, "background": 0xFF075E54})
        assert output.is_file()
    finally:
        backend.stop()


def test_chat_row_has_accessible_name_and_space_opens_chat(tmp_path):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    env["PYTHONUNBUFFERED"] = "1"
    output = tmp_path / "accessible-chat-row.png"
    try:
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "chat",
                "--keyboard-chat", BUDI,
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
        assert "chat row accessible name='Open chat Budi Santoso'; Space opened chat" in result.stdout
        assert any(method == "GET" and f"/chats/{BUDI.replace('@', '%40')}/messages" in path
                   for method, path, _ in backend.requests)
        assert output.is_file()
    finally:
        backend.stop()


def test_chat_rows_move_keyboard_focus_with_arrow_down(tmp_path):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    env["PYTHONUNBUFFERED"] = "1"
    output = tmp_path / "chat-row-arrow-focus.png"
    try:
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "chat",
                "--chat-row-arrow", f"{TEAM}:Down",
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
        assert "chat row Down moved focus to accessible row 'Open chat Budi Santoso'" in result.stdout
        assert output.is_file()
    finally:
        backend.stop()


def test_chat_rows_move_keyboard_focus_with_arrow_up(tmp_path):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    env["PYTHONUNBUFFERED"] = "1"
    output = tmp_path / "chat-row-arrow-up-focus.png"
    try:
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "chat",
                "--chat-row-arrow", f"{BUDI}:Up",
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
        assert "chat row Up moved focus to accessible row 'Open chat Tim Proyek'" in result.stdout
        assert output.is_file()
    finally:
        backend.stop()


def test_archived_chats_button_has_accessible_name_and_space_activation(tmp_path):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    env["PYTHONUNBUFFERED"] = "1"
    output = tmp_path / "accessible-archived-chats.png"
    try:
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "chat",
                "--keyboard-archive",
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
        assert "Archived button accessible name='Open archived chats'; Space opened archive" in result.stdout
        assert output.is_file()
    finally:
        backend.stop()


def test_chat_sidebar_tab_order_reaches_archive_and_first_chat(tmp_path):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    env["PYTHONUNBUFFERED"] = "1"
    output = tmp_path / "chat-sidebar-tab-order.png"
    try:
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "chat",
                "--tab-chat-list",
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
        assert "Tab order: search → 'Open archived chats' → 'Open chat Tim Proyek'" in result.stdout
        assert output.is_file()
    finally:
        backend.stop()

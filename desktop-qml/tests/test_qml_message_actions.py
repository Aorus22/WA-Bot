"""Verify context-menu Reply can be reached and triggered from the keyboard."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests"))

from mock_backend import MockBackend  # noqa: E402
from mock_common import BUDI, SARI, TEAM  # noqa: E402


@pytest.mark.parametrize(
    ("flag", "action", "chat_id", "argument", "expected_output"),
    [
        ("--reply-message-key", "Reply", BUDI, "b4", "message context action accessible name='Reply'; Enter started reply for b4"),
        ("--edit-message-key", "Edit", TEAM, "t2", "message context action accessible name='Edit'; Enter started edit for t2"),
        ("--copy-message-key", "Copy", TEAM, "t2", "message context action accessible name='Copy'; Enter copied t2"),
        ("--react-message-key", "Reaction", TEAM, "t2:👍", "message context quick reaction accessible name='👍'; mouse applied 👍 to t2"),
        ("--pin-message-key", "Pin", TEAM, "t2", "message context action accessible name='Pin'; Enter pinned t2"),
        ("--star-message-key", "Star", TEAM, "t2", "message context action accessible name='Star'; Enter starred t2"),
        ("--message-info-key", "Info", TEAM, "t2", "message context action accessible name='Message info'; Enter opened receipts: read=1 delivered=1 remaining=1"),
        ("--context-forward-key", "ContextForward", TEAM, "t2:" + BUDI,
         "forward target accessible name='Forward to Budi Santoso'; Tab/Space forwarded t2 to Budi Santoso"),
        ("--delete-message-key", "Delete", TEAM, "t2", "message context action accessible name='Delete'; Enter deleted t2"),
    ],
)
def test_message_context_actions_work_with_enter(tmp_path, flag, action, chat_id, argument, expected_output):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    if action == "Info":
        backend.state["message_receipts"][(TEAM, "t2")] = {
            "recipients": [BUDI, SARI, "6283333333333@s.whatsapp.net"],
            "receipts": [
                {"jid": BUDI, "deliveredAt": 1_700_000_000_000, "readAt": 1_700_000_000_500},
                {"jid": SARI, "deliveredAt": 1_700_000_001_000, "readAt": 0},
            ],
        }
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    env["PYTHONUNBUFFERED"] = "1"
    output = tmp_path / f"message-context-{action.lower()}.png"
    try:
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "chat",
                "--select-chat", chat_id,
                flag, argument,
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
        assert expected_output in result.stdout
        if action == "Reaction":
            assert backend.state["message_reactions"] == [(TEAM, "t2", {"emoji": "👍"})]
        if action == "ContextForward":
            assert backend.state["forwards"] == [(TEAM, "t2", [BUDI])]
        if action == "Star":
            starred = next(row for row in backend.state["messages"][TEAM] if row["id"] == "t2")
            assert starred["starred"] is True
        if action == "Pin":
            assert backend.state["message_pin_calls"] == [(TEAM, "t2", {"pinned": True, "duration": 604800})]
            assert [pin["messageId"] for pin in backend.state["message_pins"][TEAM]] == ["t2"]
        if action == "Delete":
            assert backend.state["message_deletions"] == [(TEAM, "t2")]
            deleted = next(row for row in backend.state["messages"][TEAM] if row["id"] == "t2")
            assert deleted["revokedAt"] > 0
        if action == "Info":
            assert any(method == "GET" and path.endswith("/chats/120363000000000001%40g.us/messages/t2/receipts")
                       for method, path, _body in backend.requests)
        assert output.is_file()
    finally:
        backend.stop()


def test_direct_message_info_shows_read_and_delivery_stages(tmp_path):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    backend.state["message_receipts"][(BUDI, "b1")] = {
        "recipients": [BUDI],
        "receipts": [{"jid": BUDI, "deliveredAt": 1_700_000_000_000, "readAt": 1_700_000_000_500}],
    }
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    output = tmp_path / "message-info-direct.png"
    try:
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "chat",
                "--select-chat", BUDI,
                "--message-info-key", "b1",
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
        assert "message context action accessible name='Message info'; Enter opened receipts: read=1 delivered=0 remaining=0" in result.stdout
        assert any(method == "GET" and path.endswith("/chats/6281111111111%40s.whatsapp.net/messages/b1/receipts")
                   for method, path, _body in backend.requests)
        assert output.is_file()
    finally:
        backend.stop()

"""Exercise rich message cards through QML controls and their mock contracts."""
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


def test_poll_option_mouse_click_votes_through_rest(tmp_path):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    env["PYTHONUNBUFFERED"] = "1"
    output = tmp_path / "poll-vote.png"
    try:
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "chat",
                "--select-chat", BUDI,
                "--vote-poll", "rich-poll:Sore",
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
        assert "poll option mouse click voted: Sore" in result.stdout
        assert backend.state["poll_votes"] == [(BUDI, "rich-poll", ["Sore"])]
        vote_requests = [(path, json.loads(body)) for method, path, body in backend.requests
                         if method == "POST" and path.endswith("/messages/rich-poll/vote")]
        assert vote_requests and vote_requests[-1][1] == {"options": ["Sore"]}
        assert output.is_file()
    finally:
        backend.stop()


def test_poll_option_has_accessible_name_and_space_activation(tmp_path):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    env["PYTHONUNBUFFERED"] = "1"
    output = tmp_path / "accessible-poll-vote.png"
    try:
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "chat",
                "--select-chat", BUDI,
                "--vote-poll-key", "rich-poll:Sore",
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
        assert "poll accessible name='Vote Sore'; Space voted Sore" in result.stdout
        assert backend.state["poll_votes"] == [(BUDI, "rich-poll", ["Sore"])]
        assert output.is_file()
    finally:
        backend.stop()


def test_contact_card_mouse_click_opens_recipient_chat(tmp_path):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    env["PYTHONUNBUFFERED"] = "1"
    output = tmp_path / "contact-open.png"
    contact_jid = "6282222222222@s.whatsapp.net"
    try:
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "chat",
                "--select-chat", BUDI,
                "--open-contact", f"rich-contact:{contact_jid}",
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
        assert f"contact card opened chat: {contact_jid}" in result.stdout
        assert any(method == "GET" and f"/chats/{contact_jid.replace('@', '%40')}/messages" in path
                   for method, path, _ in backend.requests)
        assert output.is_file()
    finally:
        backend.stop()


def test_location_card_mouse_click_opens_google_maps_url(tmp_path):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    rows = backend.state["messages"][BUDI]
    location_row = next(row for row in rows if row["id"] == "rich-location")
    location_row["timestamp"] = max(row["timestamp"] for row in rows) + 1000
    rows.sort(key=lambda row: row["timestamp"])
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    env["PYTHONUNBUFFERED"] = "1"
    output = tmp_path / "location-map.png"
    expected_url = "https://maps.google.com/?q=-6.2,106.816666"
    try:
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "chat",
                "--select-chat", BUDI,
                "--open-location", "rich-location",
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
        assert f"location card opened map URL: {expected_url}" in result.stdout
        assert output.is_file()
    finally:
        backend.stop()


def test_link_preview_has_accessible_name_and_space_opens_url(tmp_path):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    rows = backend.state["messages"][BUDI]
    link_row = next(row for row in rows if row["id"] == "rich-link")
    link_row["timestamp"] = max(row["timestamp"] for row in rows) + 1000
    rows.sort(key=lambda row: row["timestamp"])
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    env["PYTHONUNBUFFERED"] = "1"
    output = tmp_path / "accessible-link-preview.png"
    expected_url = "https://example.com/article"
    try:
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "chat",
                "--select-chat", BUDI,
                "--open-link-key", "rich-link",
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
        assert f"link preview accessible name='Open link preview example.com'; Space opened {expected_url}" in result.stdout
        assert output.is_file()
    finally:
        backend.stop()

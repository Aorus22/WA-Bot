"""Drive the Calls history filter chip in a local QML scene, never a live account."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests"))

from mock_backend import MockBackend  # noqa: E402


def test_calls_filter_chip_mouse_click(tmp_path):
    backend = MockBackend(areas=["calls"]).start()
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    env["PYTHONUNBUFFERED"] = "1"
    try:
        output = tmp_path / "calls-incoming.png"
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "calls",
                "--calls-filter", "incoming",
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
        assert "calls filter mouse click: incoming → incoming" in result.stdout
        assert output.is_file()
    finally:
        backend.stop()


@pytest.mark.parametrize("action", ["answer", "reject"])
def test_incoming_call_action_button_uses_shared_rest_and_ws_state(tmp_path, action):
    backend = MockBackend(areas=["calls"]).start()
    backend.state["active_call"] = {
        "id": "shot-incoming", "status": "ringing", "type": "audio", "direction": "incoming",
        "source": "incoming", "media_mode": "tts", "target": "6281111111111@s.whatsapp.net",
        "group_jid": None, "participants": None, "started_at": 1, "answered_at": None,
        "video_enabled": False, "remote_video_enabled": False,
    }
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    try:
        output = tmp_path / f"calls-{action}.png"
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "calls",
                "--incoming-call",
                "--call-action", action,
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
        assert f"incoming call mouse action applied: {action}" in result.stdout
        assert backend.state["call_actions"] == [(action, "shot-incoming")]
        if action == "answer":
            assert backend.state["active_call"]["status"] == "connected"
        else:
            assert backend.state["active_call"] is None
        assert output.is_file()
    finally:
        backend.stop()


def test_active_call_end_button_hangs_up_via_rest_and_ws(tmp_path):
    backend = MockBackend(areas=["calls"]).start()
    backend.state["active_call"] = {
        "id": "shot-active", "status": "connected", "type": "audio", "direction": "outgoing",
        "source": "ui", "media_mode": "tts", "target": "6281111111111@s.whatsapp.net",
        "group_jid": None, "participants": None, "started_at": 1, "answered_at": 1,
        "video_enabled": False, "remote_video_enabled": False,
    }
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    try:
        output = tmp_path / "calls-hangup.png"
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "calls",
                "--active-call",
                "--call-action", "hangup",
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
        assert "active call mouse action applied: hangup" in result.stdout
        assert backend.state["call_actions"] == [("hangup", "shot-active")]
        assert backend.state["active_call"] is None
        assert output.is_file()
    finally:
        backend.stop()


def test_group_call_add_participant_button_uses_shared_contracts(tmp_path):
    backend = MockBackend(areas=["calls"]).start()
    backend.state["active_call"] = {
        "id": "shot-group-call", "status": "connected", "type": "group_audio", "direction": "outgoing",
        "source": "ui", "media_mode": "tts", "target": "120363000000000001@g.us",
        "group_jid": "120363000000000001@g.us", "participants": ["6281111111111@s.whatsapp.net"],
        "started_at": 1, "answered_at": 1, "video_enabled": False, "remote_video_enabled": False,
    }
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    participant = "6285555555555@s.whatsapp.net"
    try:
        output = tmp_path / "calls-group-participant.png"
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "calls",
                "--group-call",
                "--call-action", "add-participant",
                "--call-participant-jid", participant,
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
        assert "group-call participant added through QML controls" in result.stdout
        assert backend.state["call_actions"] == [("participants", "shot-group-call", [participant])]
        assert participant in backend.state["active_call"]["participants"]
        assert output.is_file()
    finally:
        backend.stop()


@pytest.mark.parametrize("action", ["video-start", "video-stop"])
def test_active_call_video_toggle_uses_shared_contracts(tmp_path, action):
    backend = MockBackend(areas=["calls"]).start()
    initial_video = action == "video-stop"
    backend.state["active_call"] = {
        "id": "shot-active", "status": "connected", "type": "video" if initial_video else "audio",
        "direction": "outgoing", "source": "ui", "media_mode": "tts",
        "target": "6281111111111@s.whatsapp.net", "group_jid": None, "participants": None,
        "started_at": 1, "answered_at": 1, "video_enabled": initial_video,
        "remote_video_enabled": False,
    }
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    try:
        output = tmp_path / f"calls-{action}.png"
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "calls",
                "--active-call",
                "--call-action", action,
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
        assert f"active call mouse action applied: {action}" in result.stdout
        action_name = "video_start" if action == "video-start" else "video_stop"
        assert backend.state["call_actions"] == [(action_name, "shot-active")]
        assert backend.state["active_call"]["video_enabled"] is (action == "video-start")
        assert output.is_file()
    finally:
        backend.stop()


@pytest.mark.parametrize("action", ["mic-toggle", "speaker-toggle"])
def test_active_call_audio_controls_toggle_without_opening_devices(tmp_path, action):
    backend = MockBackend(areas=["calls"]).start()
    backend.state["active_call"] = {
        "id": "shot-active", "status": "connected", "type": "audio", "direction": "outgoing",
        "source": "ui", "media_mode": "tts", "target": "6281111111111@s.whatsapp.net",
        "group_jid": None, "participants": None, "started_at": 1, "answered_at": 1,
        "video_enabled": False, "remote_video_enabled": False,
    }
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    try:
        output = tmp_path / f"calls-{action}.png"
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "calls",
                "--active-call",
                "--call-action", action,
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
        assert f"active call mouse action applied: {action}" in result.stdout
        assert backend.state["call_actions"] == []
        assert backend.state["media_session_count"] == 0
        assert output.is_file()
    finally:
        backend.stop()
